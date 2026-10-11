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

use std::{collections::HashSet, path::PathBuf, time::Duration};

use serde::Serialize;
use sqlx::{
    Pool, QueryBuilder, Sqlite,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use tauri::{AppHandle, Manager};
use ts_rs::TS;

/// The cache file of one registered repository.
pub struct Cache {
    pub file: PathBuf,
    pub pool: Pool<Sqlite>,
}

/// Where the cache of the repository `repo` in the workspace `workspace` lives.
pub fn file<R: tauri::Runtime>(
    app: &AppHandle<R>,
    workspace: &str,
    repo: &str,
) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("workspace")
        .join(workspace)
        .join("repo");
    Ok(dir.join(format!("{repo}.db")))
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
async fn reset(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
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

/// Applies every migration the file has not seen yet.
///
/// One statement per call, deliberately without a transaction: a `&mut`
/// executor borrow is what makes an async future unprovable for the commands
/// that await this, and a cache file can be built again when this is
/// interrupted.
async fn migrate(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    let applied: i64 = match sqlx::query_scalar::<_, i64>("SELECT MAX(version) FROM migration")
        .fetch_one(pool)
        .await
    {
        Ok(applied) => applied,
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
            // created here rather than by a migration, so it exists before the
            // first one is recorded in it
            sqlx::raw_sql(
                "CREATE TABLE IF NOT EXISTS migration (
                     version     INTEGER PRIMARY KEY,
                     description TEXT NOT NULL
                 )",
            )
            .execute(pool)
            .await?;
        }
        sqlx::query("INSERT INTO migration (version, description) VALUES (?, ?)")
            .bind(migration.version)
            .bind(migration.description)
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
#[derive(Clone, Debug)]
pub struct Fact {
    pub hash: String,
    pub time: i64,
    pub parents: Vec<String>,
    pub subject: String,
    pub author: String,
    pub body: Option<String>,
}

/// Remembers `facts` and adds them to the work list, in one transaction.
///
/// The inserts are built rather than checked at compile time: they take as
/// many values as a chunk holds.
pub async fn enqueue(pool: &Pool<Sqlite>, facts: &[Fact]) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    for chunk in facts.chunks(2_000) {
        QueryBuilder::new(
            "INSERT OR IGNORE INTO commits
             (hash, time, parents, subject, author, body, expanded) ",
        )
        .push_values(chunk, |mut row, fact| {
            row.push_bind(&fact.hash)
                .push_bind(fact.time)
                .push_bind(serde_json::to_string(&fact.parents).unwrap_or_default())
                .push_bind(&fact.subject)
                .push_bind(&fact.author)
                .push_bind(fact.body.as_deref())
                .push_bind(0);
        })
        .build()
        .execute(&mut *transaction)
        .await?;
        QueryBuilder::new("INSERT OR IGNORE INTO pending (hash, time) ")
            .push_values(chunk, |mut row, fact| {
                row.push_bind(&fact.hash).push_bind(fact.time);
            })
            .build()
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await
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
    for chunk in parents.chunks(2_000) {
        QueryBuilder::new(
            "INSERT OR IGNORE INTO commits
             (hash, time, parents, subject, author, body, expanded) ",
        )
        .push_values(chunk, |mut row, fact| {
            row.push_bind(&fact.hash)
                .push_bind(fact.time)
                .push_bind(serde_json::to_string(&fact.parents).unwrap_or_default())
                .push_bind(&fact.subject)
                .push_bind(&fact.author)
                .push_bind(fact.body.as_deref())
                .push_bind(0);
        })
        .build()
        .execute(&mut *transaction)
        .await?;
        QueryBuilder::new("INSERT OR IGNORE INTO pending (hash, time) ")
            .push_values(chunk, |mut row, fact| {
                row.push_bind(&fact.hash).push_bind(fact.time);
            })
            .build()
            .execute(&mut *transaction)
            .await?;
    }
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
    for (key, value) in entries {
        sqlx::query!(
            "INSERT INTO meta (key, value) VALUES (?, ?)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            key,
            value
        )
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await
}

/// One commit as a row of a view: where it sits in the list and in the graph.
pub struct NodeRow {
    pub row: i64,
    pub hash: String,
    pub lane: i64,
    pub line: String,
    pub pushed: bool,
    pub refs: String,
}

/// A stretch of a line between two commits.
pub struct EdgeRow {
    pub from: i64,
    pub to: i64,
    pub line: String,
    pub points: String,
}

/// Replaces the laid-out rows and edges of a view, and records the view's
/// size, in one transaction; readers see the old view until it commits.
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
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::query!("DELETE FROM node WHERE view_id = ?", id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query!("DELETE FROM edge WHERE view_id = ?", id)
        .execute(&mut *transaction)
        .await?;
    for chunk in nodes.chunks(1_500) {
        QueryBuilder::new("INSERT INTO node (view_id, row, hash, lane, line, pushed, refs) ")
            .push_values(chunk, |mut row, node| {
                row.push_bind(id)
                    .push_bind(node.row)
                    .push_bind(&node.hash)
                    .push_bind(node.lane)
                    .push_bind(&node.line)
                    .push_bind(node.pushed)
                    .push_bind(&node.refs);
            })
            .build()
            .execute(&mut *transaction)
            .await?;
    }
    for chunk in edges.chunks(1_500) {
        QueryBuilder::new("INSERT OR REPLACE INTO edge (view_id, \"from\", \"to\", line, points) ")
            .push_values(chunk, |mut row, edge| {
                row.push_bind(id)
                    .push_bind(edge.from)
                    .push_bind(edge.to)
                    .push_bind(&edge.line)
                    .push_bind(&edge.points);
            })
            .build()
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
            enqueue(&pool, &[child.clone()]).await.unwrap();

            let parent = fact(1, 10, &[]);
            expand(&pool, &[child.hash.clone()], &[parent.clone()])
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
