//! The per-repository SQLite cache: one file per registered repository under
//! the app's cache directory.
//!
//! The app's own database holds the registry; everything a repository's
//! history needs lives here instead, so a huge history can be indexed and
//! paged without touching the registry and a cache can be thrown away without
//! losing anything that was not derived from git.
//!
//! `commits` holds what git knows about a commit — hash, time, parents,
//! subject, author, body — read once and never changed. `pending` is where an
//! interrupted walk resumes from. `views`/`node`/`edge` hold a laid-out view
//! of those commits: rows with a lane and a line, and the edges between them.
//! `meta` holds progress and the current line list. The frontend reads the
//! view tables through tauri-plugin-sql; the Rust side owns every write.
//!
//! The schema is defined in `src/lib/db/repo-schema.ts`; drizzle-kit generates
//! the migrations into `drizzle-repo/`, and [`prepare`] applies them. The
//! queries are checked against a database built from those migrations (see
//! `pnpm db:repo:prepare`), so a schema change that this module misses does not
//! compile.

use std::{collections::HashMap, collections::HashSet, path::PathBuf, time::Duration};

use serde::Serialize;
use sqlx::{
    Pool, Sqlite, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use crate::maintenance;

/// The cache file of one registered repository.
pub struct Cache {
    pub file: PathBuf,
    pub pool: Pool<Sqlite>,
}

/// Where the cache of the repository `repo` in the workspace `workspace` lives.
///
/// The ids come from the webview and end up in a path that `remove` deletes,
/// so anything but the UUIDs the registry hands out is refused.
pub fn file<R: tauri::Runtime>(
    app: &AppHandle<R>,
    workspace: &str,
    repo: &str,
) -> Result<PathBuf, String> {
    for id in [workspace, repo] {
        if !is_uuid(id) {
            return Err(format!("{id:?} is not a registry id."));
        }
    }
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("workspace")
        .join(workspace)
        .join("repo");
    Ok(dir.join(format!("{repo}.db")))
}

/// Whether `id` has the shape of a UUID, `8-4-4-4-12` hex digits: the ids the
/// registry generates with `crypto.randomUUID()`.
fn is_uuid(id: &str) -> bool {
    id.len() == 36
        && id.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// The freedesktop cache directory tag: readers that see it know everything
/// under the directory can be skipped, deleted or ignored.
const TAG: &str = "CACHEDIR.TAG";
const TAG_CONTENT: &str = "Signature: 8a477f597d28d172789f06886806bc55\n\
                           # This file is a cache directory tag created by VOGT.\n\
                           # For information about cache directory tags, see https://bford.info/cachedir/\n";

/// Marks the app's cache directory with [`TAG`], so backup and indexing tools
/// leave the repository caches alone.
pub fn tag<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    write_tag(&dir)
}

/// Writes the tag into `dir` unless it is there already.
fn write_tag(dir: &std::path::Path) -> Result<(), String> {
    let file = dir.join(TAG);
    if !file.exists() {
        std::fs::write(&file, TAG_CONTENT).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Opens the cache file of a repository, creating it and its schema when it is
/// not there yet.
pub async fn open<R: tauri::Runtime>(
    app: &AppHandle<R>,
    workspace: &str,
    repo: &str,
) -> Result<Cache, String> {
    let file = file(app, workspace, repo)?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let new = !file.exists();
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&file)
                .create_if_missing(true)
                // the frontend reads the same file through tauri-plugin-sql
                .journal_mode(SqliteJournalMode::Wal)
                .synchronous(SqliteSynchronous::Normal)
                .busy_timeout(Duration::from_secs(10)),
        )
        .await
        .map_err(|e| e.to_string())?;

    if new {
        // costs nothing while the file is empty; an older cache is rebuilt by
        // its first maintenance pass instead of while it is being opened
        maintenance::incremental(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }
    prepare(&pool).await.map_err(|e| e.to_string())?;

    Ok(Cache { file, pool })
}

/// Brings `pool` up to the schema the migrations describe, applying the ones
/// it has not seen. A file that predates the migrations is emptied first: the
/// cache holds nothing that cannot be read out of git again.
pub async fn prepare(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    // `user_version` was the hand-written schema's marker; the migrations do
    // not use it, so a non-zero value means the file is from before them
    let legacy: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await?;
    if legacy != 0 {
        reset(pool).await?;
        sqlx::raw_sql("PRAGMA user_version = 0")
            .execute(pool)
            .await?;
    }

    if let Err(failure) = migrate(pool).await {
        // the migrations run one statement at a time, so an interruption can
        // leave half a schema behind; the cache is derived, so start over once
        eprintln!("vogt: rebuilding a repository cache after a failed migration: {failure}");
        reset(pool).await?;
        migrate(pool).await?;
    }
    Ok(())
}

/// Empties every table of a cache file; the frontend's connection may be
/// open, so the tables are dropped rather than the file deleted.
pub async fn reset(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS commits;
         DROP TABLE IF EXISTS pending;
         DROP TABLE IF EXISTS views;
         DROP TABLE IF EXISTS node;
         DROP TABLE IF EXISTS edge;
         DROP TABLE IF EXISTS meta;
         DROP TABLE IF EXISTS migration;",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// The table [`migrate`] records the applied migrations in. It is created
/// here rather than by a migration, so it exists before the first one is
/// recorded in it; `scripts/repo-schema.mjs` creates the same table for the
/// query macros to check against, and the two must match.
const MIGRATION_TABLE: &str = "CREATE TABLE IF NOT EXISTS migration (
    version     INTEGER PRIMARY KEY,
    description TEXT NOT NULL
)";

/// Applies every migration the file has not seen yet.
///
/// One statement per call, deliberately without a transaction: a `&mut`
/// executor borrow is what makes an async future unprovable for the commands
/// that await this, and a cache file can be built again when this is
/// interrupted.
async fn migrate(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    let applied: i64 = match sqlx::query_scalar!("SELECT MAX(version) FROM migration")
        .fetch_one(pool)
        .await
    {
        Ok(applied) => applied.unwrap_or(0),
        // the first migration creates the bookkeeping table
        Err(sqlx::Error::Database(e)) if e.message().contains("no such table") => 0,
        Err(e) => return Err(e),
    };
    for migration in crate::repo_migrations() {
        if migration.version <= applied {
            continue;
        }
        sqlx::raw_sql(migration.sql).execute(pool).await?;
        if migration.version == 1 {
            sqlx::raw_sql(MIGRATION_TABLE).execute(pool).await?;
        }
        sqlx::query!(
            "INSERT INTO migration (version, description) VALUES (?, ?)",
            migration.version,
            migration.description
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Deletes the cache file of a repository, with its write-ahead log, and
/// answers with the file it removed.
pub async fn remove<R: tauri::Runtime>(
    app: &AppHandle<R>,
    workspace: &str,
    repo: &str,
) -> Result<PathBuf, String> {
    let file = file(app, workspace, repo)?;
    for path in [
        file.clone(),
        file.with_extension("db-wal"),
        file.with_extension("db-shm"),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Could not delete {}: {e}", path.display())),
        }
    }
    Ok(file)
}

/// A commit as it is stored: the facts git knows, and nothing about how it is
/// drawn.
#[derive(Clone, Debug, Serialize)]
pub struct Fact {
    pub hash: String,
    pub time: i64,
    pub parents: Vec<String>,
    pub subject: String,
    pub author: String,
    pub body: Option<String>,
}

/// Remembers `facts` and adds them to the work list, in one transaction.
pub async fn enqueue(pool: &Pool<Sqlite>, facts: &[Fact]) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    remember(&mut transaction, facts).await?;
    transaction.commit().await
}

/// Inserts `facts` into `commits` and `pending`, leaving the ones already
/// there alone. Each chunk goes in as one JSON array, so the statements stay
/// fixed and checked however many rows a chunk holds; `parents` is stored as
/// the JSON array it arrives as.
async fn remember(connection: &mut SqliteConnection, facts: &[Fact]) -> Result<(), sqlx::Error> {
    for chunk in facts.chunks(2_000) {
        let json = serde_json::to_string(chunk).unwrap_or_default();
        sqlx::query!(
            "INSERT OR IGNORE INTO commits
             (hash, time, parents, subject, author, body, expanded)
             SELECT value ->> 'hash', value ->> 'time', value -> 'parents',
                    value ->> 'subject', value ->> 'author', value ->> 'body', 0
             FROM json_each(?)",
            json
        )
        .execute(&mut *connection)
        .await?;
        sqlx::query!(
            "INSERT OR IGNORE INTO pending (hash, time)
             SELECT value ->> 'hash', value ->> 'time' FROM json_each(?)",
            json
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// The commits whose parents have not been read yet, newest first.
pub async fn pending(pool: &Pool<Sqlite>) -> Result<Vec<(String, i64)>, sqlx::Error> {
    let rows = sqlx::query!("SELECT hash, time FROM pending ORDER BY time DESC, hash")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|row| (row.hash, row.time)).collect())
}

/// The subset of `hashes` that is already remembered.
pub async fn known(pool: &Pool<Sqlite>, hashes: &[String]) -> Result<HashSet<String>, sqlx::Error> {
    let mut known = HashSet::new();
    for chunk in hashes.chunks(5_000) {
        let json = serde_json::to_string(chunk).unwrap_or_default();
        let rows = sqlx::query_scalar!(
            "SELECT hash FROM commits WHERE hash IN (SELECT value FROM json_each(?))",
            json
        )
        .fetch_all(pool)
        .await?;
        known.extend(rows);
    }
    Ok(known)
}

/// The parents of `hashes`, as they were stored when the commits were read.
pub async fn parents(
    pool: &Pool<Sqlite>,
    hashes: &[String],
) -> Result<HashMap<String, Vec<String>>, sqlx::Error> {
    let mut parents = HashMap::new();
    for chunk in hashes.chunks(5_000) {
        let json = serde_json::to_string(chunk).unwrap_or_default();
        let rows = sqlx::query!(
            "SELECT hash, parents FROM commits WHERE hash IN (SELECT value FROM json_each(?))",
            json
        )
        .fetch_all(pool)
        .await?;
        parents.extend(rows.into_iter().map(|row| {
            (
                row.hash,
                serde_json::from_str(&row.parents).unwrap_or_default(),
            )
        }));
    }
    Ok(parents)
}

/// Marks `popped` as expanded — their parents have been read — and remembers
/// the `parents` discovered on the way. One transaction, so an interruption
/// resumes before it.
pub async fn expand(
    pool: &Pool<Sqlite>,
    popped: &[String],
    parents: &[Fact],
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let popped = serde_json::to_string(popped).unwrap_or_default();
    sqlx::query!(
        "UPDATE commits SET expanded = 1 WHERE hash IN (SELECT value FROM json_each(?))",
        popped
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query!(
        "DELETE FROM pending WHERE hash IN (SELECT value FROM json_each(?))",
        popped
    )
    .execute(&mut *transaction)
    .await?;
    remember(&mut transaction, parents).await?;
    transaction.commit().await
}

/// One value from the `meta` table.
pub async fn meta(pool: &Pool<Sqlite>, key: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar!("SELECT value FROM meta WHERE key = ?", key)
        .fetch_optional(pool)
        .await
}

/// Writes `entries` into the `meta` table.
pub async fn set_meta(pool: &Pool<Sqlite>, entries: &[(&str, String)]) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    write_meta(&mut transaction, entries).await?;
    transaction.commit().await
}

async fn write_meta(
    connection: &mut SqliteConnection,
    entries: &[(&str, String)],
) -> Result<(), sqlx::Error> {
    for (key, value) in entries {
        sqlx::query!(
            "INSERT INTO meta (key, value) VALUES (?, ?)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            key,
            value
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// One commit as a row of a view: where it sits in the list and in the graph.
#[derive(Serialize)]
pub struct NodeRow {
    pub row: i64,
    pub hash: String,
    pub lane: i64,
    pub line: String,
    pub pushed: bool,
    pub refs: String,
}

/// A stretch of a line between two commits.
#[derive(Serialize)]
pub struct EdgeRow {
    pub from: i64,
    pub to: i64,
    pub line: String,
    pub points: String,
}

/// Replaces the laid-out rows and edges of a view, records the view's size
/// and writes the `meta` entries that describe it, such as its line list, in
/// one transaction; readers see the old view until it commits, and never new
/// rows with the old lines.
///
/// shortcut: the whole view is written again on every round. Most of it is
/// unchanged, but a new commit can move lanes and lines anywhere below it, so
/// diffing would need a full read anyway; revisit once a large repository
/// updates often enough for the rewrite to be felt.
#[allow(clippy::too_many_arguments)]
pub async fn save_view(
    pool: &Pool<Sqlite>,
    id: &str,
    only: &[String],
    total: usize,
    lanes: usize,
    version: i64,
    nodes: &[NodeRow],
    edges: &[EdgeRow],
    meta: &[(&str, String)],
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::query!("DELETE FROM node WHERE view_id = ?", id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query!("DELETE FROM edge WHERE view_id = ?", id)
        .execute(&mut *transaction)
        .await?;
    // a chunk goes in as one JSON array, as in `remember`; `pushed` arrives as
    // a JSON boolean and `->>` turns it into 0 or 1
    for chunk in nodes.chunks(1_500) {
        let json = serde_json::to_string(chunk).unwrap_or_default();
        sqlx::query!(
            "INSERT INTO node (view_id, row, hash, lane, line, pushed, refs)
             SELECT ?, value ->> 'row', value ->> 'hash', value ->> 'lane',
                    value ->> 'line', value ->> 'pushed', value ->> 'refs'
             FROM json_each(?)",
            id,
            json
        )
        .execute(&mut *transaction)
        .await?;
    }
    for chunk in edges.chunks(1_500) {
        let json = serde_json::to_string(chunk).unwrap_or_default();
        sqlx::query!(
            "INSERT OR REPLACE INTO edge (view_id, \"from\", \"to\", line, points)
             SELECT ?, value ->> 'from', value ->> 'to', value ->> 'line', value ->> 'points'
             FROM json_each(?)",
            id,
            json
        )
        .execute(&mut *transaction)
        .await?;
    }
    let only = serde_json::to_string(only).unwrap_or_default();
    let total = total as i64;
    let lanes = lanes as i64;
    sqlx::query!(
        "INSERT INTO views (id, only_lines, total, lanes, version) VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             only_lines = excluded.only_lines,
             total = excluded.total,
             lanes = excluded.lanes,
             version = excluded.version",
        id,
        only,
        total,
        lanes,
        version
    )
    .execute(&mut *transaction)
    .await?;
    // a mask is derived from this view and rebuilt when it is asked for again,
    // so the unmasked layout drops every other view with it
    if id == ViewInfo::ALL {
        sqlx::query!("DELETE FROM node WHERE view_id <> ?", id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query!("DELETE FROM edge WHERE view_id <> ?", id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query!("DELETE FROM views WHERE id <> ?", id)
            .execute(&mut *transaction)
            .await?;
    }
    write_meta(&mut transaction, meta).await?;
    transaction.commit().await
}

/// A laid-out view as the tables remember it.
#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ViewInfo {
    pub id: String,
    pub only: Vec<String>,
    pub total: usize,
    pub lanes: usize,
    pub version: i64,
}

impl ViewInfo {
    /// The view that holds every commit, whose id it is.
    pub const ALL: &'static str = "all";
}

/// The laid-out view `id`, if it has one.
pub async fn view(pool: &Pool<Sqlite>, id: &str) -> Result<Option<ViewInfo>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT id, only_lines, total, lanes, version FROM views WHERE id = ?",
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| ViewInfo {
        id: row.id,
        only: serde_json::from_str(&row.only_lines).unwrap_or_default(),
        total: row.total as usize,
        lanes: row.lanes as usize,
        version: row.version,
    }))
}

/// How far the index is.
#[derive(Clone, Copy, Debug, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Progress {
    /// Commits whose facts are remembered.
    pub indexed: i64,
    /// Rows the laid-out view has.
    pub laid: i64,
    /// The whole reachable history has been read.
    pub complete: bool,
}

/// The progress the meta table and the laid-out view remember.
pub async fn progress(pool: &Pool<Sqlite>) -> Result<Progress, sqlx::Error> {
    let indexed: i64 = meta(pool, "indexed")
        .await?
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let complete = meta(pool, "complete").await?.as_deref() == Some("1");
    let laid = view(pool, ViewInfo::ALL)
        .await?
        .map(|view| view.total as i64)
        .unwrap_or(0);
    Ok(Progress {
        indexed,
        laid,
        complete,
    })
}

/// The line the commit `hash` sits on, if it is laid out yet.
///
/// Kept out of the commands that call it: inlined, the macro trips the
/// higher-ranked bounds `#[tauri::command]` puts on its future.
pub async fn line_of(pool: &Pool<Sqlite>, hash: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT line FROM node WHERE view_id = ? AND hash = ?",
        ViewInfo::ALL,
        hash
    )
    .fetch_optional(pool)
    .await
}

/// Commits on `line` that no remote has.
pub async fn ahead(pool: &Pool<Sqlite>, line: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT COUNT(*) FROM node WHERE view_id = ? AND line = ? AND pushed = 0",
        ViewInfo::ALL,
        line
    )
    .fetch_one(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Oid;
    use sqlx::Row;

    fn fact(hash: u8, time: i64, parents: &[u8]) -> Fact {
        Fact {
            hash: Oid::from_bytes(&[hash; 20]).unwrap().to_string(),
            time,
            parents: parents
                .iter()
                .map(|parent| Oid::from_bytes(&[*parent; 20]).unwrap().to_string())
                .collect(),
            subject: format!("subject {hash}"),
            author: "Vogt".into(),
            body: None,
        }
    }

    async fn pool() -> Pool<Sqlite> {
        let pool = Pool::<Sqlite>::connect("sqlite::memory:").await.unwrap();
        prepare(&pool).await.unwrap();
        pool
    }

    #[test]
    fn facts_come_back_as_they_were_stored() {
        tauri::async_runtime::block_on(async {
            let pool = pool().await;
            let stored = vec![fact(3, 30, &[2, 1]), fact(1, -10, &[])];
            enqueue(&pool, &stored).await.unwrap();
            // enqueuing again changes nothing
            enqueue(&pool, &stored).await.unwrap();

            let read = sqlx::query!(
                "SELECT hash, time, parents, subject FROM commits ORDER BY time DESC, hash"
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(read.len(), 2);
            assert_eq!(read[0].hash, stored[0].hash);
            assert_eq!(read[0].time, 30);
            let parents: Vec<String> = serde_json::from_str(&read[0].parents).unwrap();
            assert_eq!(parents, stored[0].parents);
            assert_eq!(read[0].subject, "subject 3");
            assert_eq!(pending(&pool).await.unwrap().len(), 2);
            // the parents column keeps its compact JSON form, a missing body
            // stays NULL and a written one comes back as it was
            let row = sqlx::query!("SELECT parents, body, expanded FROM commits WHERE time = 30")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(
                row.parents,
                serde_json::to_string(&stored[0].parents).unwrap()
            );
            assert_eq!((row.body, row.expanded), (None, 0));
            let mut with_body = fact(4, 40, &[]);
            with_body.body = Some("why \"it\" matters\n".into());
            enqueue(&pool, std::slice::from_ref(&with_body))
                .await
                .unwrap();
            let body = sqlx::query_scalar!("SELECT body FROM commits WHERE time = 40")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(body, with_body.body);

            let known = known(&pool, &[stored[0].hash.clone(), "nope".into()])
                .await
                .unwrap();
            assert!(known.contains(&stored[0].hash));
            assert_eq!(known.len(), 1);
        });
    }

    #[test]
    fn expanding_marks_commits_and_remembers_their_parents() {
        tauri::async_runtime::block_on(async {
            let pool = pool().await;
            let child = fact(2, 20, &[1]);
            enqueue(&pool, std::slice::from_ref(&child)).await.unwrap();

            let parent = fact(1, 10, &[]);
            expand(
                &pool,
                std::slice::from_ref(&child.hash),
                std::slice::from_ref(&parent),
            )
            .await
            .unwrap();

            // the popped commit is done; its parent is now the work list
            assert_eq!(
                pending(&pool).await.unwrap(),
                [(parent.hash.clone(), parent.time)]
            );
            let count = sqlx::query_scalar!("SELECT COUNT(*) FROM commits")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(count, 2);
            let expanded = sqlx::query!("SELECT hash FROM commits WHERE expanded = 1")
                .fetch_all(&pool)
                .await
                .unwrap();
            assert_eq!(expanded.len(), 1);
            assert_eq!(expanded[0].hash, child.hash);
        });
    }

    #[test]
    fn a_view_is_saved_with_its_meta() {
        tauri::async_runtime::block_on(async {
            let pool = pool().await;
            let node = |row: i64, pushed| NodeRow {
                row,
                hash: format!("hash {row}"),
                lane: 0,
                line: "main".into(),
                pushed,
                refs: "[]".into(),
            };
            let edge = EdgeRow {
                from: 0,
                to: 1,
                line: "main".into(),
                points: "[[0,0]]".into(),
            };
            save_view(
                &pool,
                ViewInfo::ALL,
                &[],
                2,
                1,
                7,
                &[node(0, false), node(1, true)],
                std::slice::from_ref(&edge),
                &[("lines", "[\"main\"]".into())],
            )
            .await
            .unwrap();

            let pushed = sqlx::query_scalar!("SELECT pushed FROM node ORDER BY row")
                .fetch_all(&pool)
                .await
                .unwrap();
            assert_eq!(pushed, [0, 1]);
            let saved = sqlx::query!(r#"SELECT "from", "to", line, points FROM edge"#)
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(
                (saved.from, saved.to, saved.line, saved.points),
                (0, 1, edge.line, edge.points)
            );
            assert_eq!(
                view(&pool, ViewInfo::ALL).await.unwrap().unwrap().version,
                7
            );
            assert_eq!(
                meta(&pool, "lines").await.unwrap().as_deref(),
                Some("[\"main\"]")
            );
        });
    }

    #[test]
    fn the_line_queries_use_the_line_index() {
        tauri::async_runtime::block_on(async {
            let pool = pool().await;
            for query in [
                "EXPLAIN QUERY PLAN
                 SELECT COUNT(*) FROM node WHERE view_id = 'all' AND line = ?",
                "EXPLAIN QUERY PLAN
                 SELECT row, hash, line, pushed, refs FROM node
                 WHERE view_id = 'all' AND line IN (SELECT value FROM json_each(?))",
            ] {
                let rows = sqlx::query(query).fetch_all(&pool).await.unwrap();
                let plan = rows
                    .iter()
                    .map(|row| row.get::<String, _>(3))
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(plan.contains("node_line"), "{plan}");
            }
        });
    }

    #[test]
    fn only_a_uuid_names_a_cache_file() {
        assert!(is_uuid("0f8fad5b-d9cb-469f-a165-70867728950e"));
        assert!(is_uuid("0F8FAD5B-D9CB-469F-A165-70867728950E"));
        for id in [
            "",
            "..",
            "../..",
            "a/b",
            "repo",
            "0f8fad5b-d9cb-469f-a165-70867728950",
            "0f8fad5b-d9cb-469f-a165-70867728950e0",
            "0f8fad5b/d9cb-469f-a165-70867728950e",
            "../fad5b-d9cb-469f-a165-70867728950e",
        ] {
            assert!(!is_uuid(id), "{id:?}");
        }

        use tauri::utils::config::{AppDirectoriesOverride, AppDirectoryOverrides};
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().app.app_directories_override =
            Some(AppDirectoriesOverride::Directories(AppDirectoryOverrides {
                cache: Some(std::env::temp_dir().join("vogt-ids")),
                ..Default::default()
            }));
        let app = tauri::test::mock_builder().build(context).unwrap();
        let workspace = "0f8fad5b-d9cb-469f-a165-70867728950e";
        assert!(file(app.handle(), workspace, "../../escape").is_err());
        assert!(file(app.handle(), "..", workspace).is_err());
        assert!(file(app.handle(), workspace, workspace).is_ok());
    }

    #[test]
    fn the_cache_directory_is_tagged_once() {
        let dir = std::env::temp_dir().join(format!("vogt-tag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        write_tag(&dir).unwrap();
        let tag = dir.join(TAG);
        let content = std::fs::read_to_string(&tag).unwrap();
        assert!(content.starts_with("Signature: 8a477f597d28d172789f06886806bc55"));

        // an existing tag is left as it is
        let kept = "Signature: 8a477f597d28d172789f06886806bc55\n# kept\n";
        std::fs::write(&tag, kept).unwrap();
        write_tag(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&tag).unwrap(), kept);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
