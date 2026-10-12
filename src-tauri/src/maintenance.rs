//! Housekeeping for the SQLite files: the app's own database and every
//! repository cache.
//!
//! One pass, [`tend`], does what a long-lived SQLite file needs: it refreshes
//! the planner's statistics, hands free pages back to the file system and
//! cuts the write-ahead log. Every file is kept in incremental auto-vacuum
//! mode so the second part stays cheap; a file that is not in that mode yet is
//! rebuilt once with `VACUUM`.
//!
//! While the app runs a pass holds back, see [`Pass::Running`]; the pass every
//! file gets when the app exits does all of it, see [`Pass::Exit`].
//!
//! A repository cache is tended by its own worker in `index.rs`, between two
//! syncs, and by `index::shutdown` on the way out. The app's database belongs
//! to tauri-plugin-sql and the frontend, so [`start`] borrows the plugin's
//! pool for it and [`on_exit`] opens the file once the plugin has let go.

use std::time::Duration;

use sqlx::{
    Pool, Row, Sqlite,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_sql::{DbInstances, DbPool};

/// How long a file goes between two passes. SQLite's advice for connections
/// that stay open is to optimize every few hours.
pub const EVERY: Duration = Duration::from_secs(4 * 60 * 60);
/// How long until the next pass over a file whose last one was `since` ago,
/// or that has not had one yet.
pub fn due_in(since: Option<Duration>) -> Duration {
    since.map_or(EVERY, |since| EVERY.saturating_sub(since))
}

/// How long [`start`] waits for the frontend to open the app's database.
const RETRY: Duration = Duration::from_secs(30);
/// What `PRAGMA auto_vacuum` answers for a file in incremental mode.
const INCREMENTAL: i64 = 2;
/// Free pages are handed back once they are one in this many of the file. Below
/// that the next write takes them again, and truncating would only be churn.
const SLACK: i64 = 4;
/// The largest file, in pages, that is rebuilt while the app runs: 256 MB of
/// the usual 4 KiB pages, which the tests cannot afford. A rebuild holds the
/// file's write lock from start to end, about ten seconds for a gigabyte, and
/// other writers give up after ten.
const REBUILD: i64 = if cfg!(test) { 64 } else { 65_536 };

/// When a pass runs, which decides how much it may hold the file up.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    /// While the app is used. Nothing that keeps a write waiting for long: a
    /// file above [`REBUILD`] is not rebuilt, and free pages stay until they
    /// are worth truncating the file for.
    Running,
    /// While the app exits. Nobody waits for the file any more, so whatever
    /// the running passes left is done: the rebuild of a file of any size,
    /// and every free page handed back.
    Exit,
}

/// One maintenance pass over the file behind `pool`.
pub async fn tend(pool: &Pool<Sqlite>, pass: Pass) -> Result<(), sqlx::Error> {
    // `optimize` is ANALYZE for the tables that need it: the ones without
    // statistics or whose size changed tenfold since. 0x10000 checks every
    // table instead of the ones this connection happened to query, which in a
    // pool says nothing; the limit keeps a huge table from being read in full.
    sqlx::raw_sql("PRAGMA analysis_limit = 1000; PRAGMA optimize = 0x10002;")
        .execute(pool)
        .await?;

    let Space { pages, free, mode } = space(pool).await?;
    let exiting = pass == Pass::Exit;
    if mode != INCREMENTAL {
        // the rebuild leaves no free page behind either
        if exiting || pages <= REBUILD {
            incremental(pool).await?;
        }
    } else if free > 0 && (exiting || free * SLACK >= pages) {
        sqlx::raw_sql("PRAGMA incremental_vacuum")
            .execute(pool)
            .await?;
    }

    // a write-ahead log keeps the size of the largest transaction it ever
    // held, a rebuild or a whole laid-out view, until it is folded into the
    // file and cut. A reader in the way, or a file without such a log, makes
    // this answer that it did nothing rather than fail.
    sqlx::raw_sql("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await?;
    Ok(())
}

/// How the file behind a pool uses its pages.
struct Space {
    /// Pages the file has.
    pages: i64,
    /// Pages of those that hold nothing.
    free: i64,
    /// What `PRAGMA auto_vacuum` answers.
    mode: i64,
}

/// Reads the [`Space`] of the file behind `pool`.
///
/// One batch on one connection, in this order, and never a cached statement:
/// SQLite answers the mode from what the connection last saw of the file and
/// compiles that answer into the statement. The two counts read the file
/// first, so the mode after them is the current one even when another
/// connection has rebuilt the file since.
async fn space(pool: &Pool<Sqlite>) -> Result<Space, sqlx::Error> {
    let rows = sqlx::raw_sql("PRAGMA page_count; PRAGMA freelist_count; PRAGMA auto_vacuum;")
        .fetch_all(pool)
        .await?;
    let value = |row: usize| -> Result<i64, sqlx::Error> {
        rows.get(row).ok_or(sqlx::Error::RowNotFound)?.try_get(0)
    };
    Ok(Space {
        pages: value(0)?,
        free: value(1)?,
        mode: value(2)?,
    })
}

/// Rebuilds the file behind `pool` in incremental auto-vacuum mode.
///
/// The mode is fixed when a file gets its first page — which switching to the
/// write-ahead log already writes, so the connect options cannot set it — and
/// only a `VACUUM` changes it afterwards. Both statements go through one
/// connection: the pragma only tells that connection what the next `VACUUM`
/// should build.
pub async fn incremental(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::raw_sql("PRAGMA auto_vacuum = INCREMENTAL; VACUUM;")
        .execute(pool)
        .await?;
    Ok(())
}

/// Closes `pool` so that the file's write-ahead log goes with it.
///
/// SQLite folds the log into the file and removes it when a closing
/// connection finds itself the last one, so two that close at the same time
/// each see the other and both leave it. That is what closing a pool right
/// after a query does: the query's connection is handed back by a task of its
/// own, and a pool that is closed meanwhile closes it beside the idle ones.
/// Waiting until every connection is back makes the pool close them in turn.
pub async fn close(pool: &Pool<Sqlite>) {
    while pool.num_idle() < pool.size() as usize {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    pool.close().await;
}

/// Tends the app's own database, `name` to tauri-plugin-sql, for as long as
/// the app runs: first once the frontend has opened it, then every [`EVERY`].
pub fn start<R: Runtime>(app: &AppHandle<R>, name: &'static str) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let wait = match plugin_pool(&app, name).await {
                Some(pool) => {
                    if let Err(failure) = tend(&pool, Pass::Running).await {
                        eprintln!("vogt: tending the app database failed: {failure}");
                    }
                    EVERY
                }
                None => RETRY,
            };
            tokio::time::sleep(wait).await;
        }
    });
}

/// The exit pass over the app's own database, `name` to tauri-plugin-sql.
///
/// The plugin closes its pools before the app hears of the exit, so the file
/// is opened once more, from where the plugin keeps it.
pub async fn on_exit<R: Runtime>(app: &AppHandle<R>, name: &str) -> Result<(), String> {
    let file = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join(name.strip_prefix("sqlite:").unwrap_or(name));
    if !file.exists() {
        // the frontend never got as far as opening it
        return Ok(());
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(file))
        .await
        .map_err(|e| e.to_string())?;
    let tended = tend(&pool, Pass::Exit).await;
    close(&pool).await;
    tended.map_err(|e| e.to_string())
}

/// The pool tauri-plugin-sql holds for the database the frontend loaded as
/// `name`, if it has been loaded.
async fn plugin_pool<R: Runtime>(app: &AppHandle<R>, name: &str) -> Option<Pool<Sqlite>> {
    let instances = app.try_state::<DbInstances>()?;
    let pools = instances.0.read().await;
    let DbPool::Sqlite(pool) = pools.get(name)?;
    // a clone is a handle on the same pool, and lets go of the plugin's lock
    Some(pool.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqliteJournalMode;
    use std::path::PathBuf;

    /// A database file of its own in the write-ahead log, as both kinds of
    /// file are opened, and not in incremental mode yet.
    async fn file(name: &str) -> (PathBuf, Pool<Sqlite>) {
        let dir = std::env::temp_dir().join(format!("vogt-tend-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pool = SqlitePoolOptions::new()
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(dir.join("test.db"))
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Wal),
            )
            .await
            .unwrap();
        (dir, pool)
    }

    /// Fills `blobs` with `rows` rows of two pages each.
    async fn fill(pool: &Pool<Sqlite>, rows: i64) {
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS blobs (id INTEGER PRIMARY KEY, data BLOB)")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?)
             INSERT INTO blobs (data) SELECT zeroblob(8000) FROM n",
        )
        .bind(rows)
        .execute(pool)
        .await
        .unwrap();
    }

    #[test]
    fn a_file_in_another_mode_is_rebuilt_as_incremental() {
        tauri::async_runtime::block_on(async {
            let (dir, pool) = file("convert").await;
            fill(&pool, 10).await;
            assert_eq!(space(&pool).await.unwrap().mode, 0);

            tend(&pool, Pass::Running).await.unwrap();

            assert_eq!(space(&pool).await.unwrap().mode, INCREMENTAL);
            let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blobs")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(kept, 10);

            close(&pool).await;
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn the_next_pass_is_due_four_hours_after_the_last() {
        let minute = Duration::from_secs(60);
        assert_eq!(due_in(None), EVERY);
        assert_eq!(due_in(Some(Duration::ZERO)), EVERY);
        // a wake late in the wait leaves the rest of it, not a whole new one
        assert_eq!(due_in(Some(EVERY - minute)), minute);
        assert_eq!(due_in(Some(EVERY + minute)), Duration::ZERO);
    }

    #[test]
    fn closing_right_after_a_query_leaves_no_log_behind() {
        tauri::async_runtime::block_on(async {
            // the connection of the last query is still on its way back to the
            // pool here, which is the moment a plain close gets wrong
            for round in 0..20 {
                let (dir, pool) = file(&format!("close-{round}")).await;
                fill(&pool, 10).await;
                tend(&pool, Pass::Exit).await.unwrap();

                close(&pool).await;

                assert_eq!(pool.size(), 0);
                let left: Vec<_> = std::fs::read_dir(&dir)
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name())
                    .collect();
                assert_eq!(left, ["test.db"], "round {round}");
                std::fs::remove_dir_all(dir).unwrap();
            }
        });
    }

    #[test]
    fn a_large_file_waits_for_the_exit_to_be_rebuilt() {
        tauri::async_runtime::block_on(async {
            let (dir, pool) = file("large").await;
            fill(&pool, 100).await;
            let before = space(&pool).await.unwrap();
            assert!(before.pages > REBUILD);

            // a rebuild would keep every other writer out for its whole length
            tend(&pool, Pass::Running).await.unwrap();
            assert_eq!(space(&pool).await.unwrap().mode, 0);

            tend(&pool, Pass::Exit).await.unwrap();
            assert_eq!(space(&pool).await.unwrap().mode, INCREMENTAL);
            let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blobs")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(kept, 100);

            close(&pool).await;
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn the_app_database_gets_its_exit_pass_where_the_plugin_keeps_it() {
        use tauri::utils::config::{AppDirectoriesOverride, AppDirectoryOverrides};
        tauri::async_runtime::block_on(async {
            let (dir, pool) = file("app").await;
            fill(&pool, 100).await;
            close(&pool).await;
            let mut context = tauri::test::mock_context(tauri::test::noop_assets());
            context.config_mut().app.app_directories_override =
                Some(AppDirectoriesOverride::Directories(AppDirectoryOverrides {
                    config: Some(dir.clone()),
                    ..Default::default()
                }));
            let app = tauri::test::mock_builder().build(context).unwrap();

            on_exit(app.handle(), "sqlite:test.db").await.unwrap();
            // a database the frontend never opened is nothing to fail over
            on_exit(app.handle(), "sqlite:missing.db").await.unwrap();
            assert!(!dir.join("missing.db").exists());

            let pool = SqlitePoolOptions::new()
                .connect_with(SqliteConnectOptions::new().filename(dir.join("test.db")))
                .await
                .unwrap();
            assert_eq!(space(&pool).await.unwrap().mode, INCREMENTAL);

            close(&pool).await;
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn a_rebuild_through_another_connection_is_seen() {
        tauri::async_runtime::block_on(async {
            let (dir, pool) = file("seen").await;
            fill(&pool, 10).await;
            // a second pool on the file, as the frontend has on a cache; it
            // has seen the file in its old mode
            let other = SqlitePoolOptions::new()
                .connect_with(SqliteConnectOptions::new().filename(dir.join("test.db")))
                .await
                .unwrap();
            assert_eq!(space(&other).await.unwrap().mode, 0);

            incremental(&pool).await.unwrap();

            assert_eq!(space(&other).await.unwrap().mode, INCREMENTAL);

            close(&other).await;
            close(&pool).await;
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn free_pages_are_handed_back_once_they_are_worth_it() {
        tauri::async_runtime::block_on(async {
            let (dir, pool) = file("release").await;
            incremental(&pool).await.unwrap();
            fill(&pool, 200).await;
            // the statistics take their own pages; have them before counting
            tend(&pool, Pass::Running).await.unwrap();
            let full = space(&pool).await.unwrap();
            assert_eq!(full.free, 0);

            // a few free pages stay where the next write finds them
            sqlx::raw_sql("DELETE FROM blobs WHERE id <= 10")
                .execute(&pool)
                .await
                .unwrap();
            let few = space(&pool).await.unwrap().free;
            assert!(few > 0);
            tend(&pool, Pass::Running).await.unwrap();
            let kept = space(&pool).await.unwrap();
            assert_eq!((kept.pages, kept.free), (full.pages, few));

            // most of the file being free is worth truncating it
            sqlx::raw_sql("DELETE FROM blobs WHERE id <= 150")
                .execute(&pool)
                .await
                .unwrap();
            tend(&pool, Pass::Running).await.unwrap();
            let released = space(&pool).await.unwrap();
            assert_eq!(released.free, 0);
            assert!(released.pages < full.pages / 2);

            // on the way out even a few are handed back
            sqlx::raw_sql("DELETE FROM blobs WHERE id <= 155")
                .execute(&pool)
                .await
                .unwrap();
            let few = space(&pool).await.unwrap();
            assert!(few.free > 0 && few.free * SLACK < few.pages);
            tend(&pool, Pass::Exit).await.unwrap();
            let left = space(&pool).await.unwrap();
            assert_eq!(left.free, 0);
            assert!(left.pages < few.pages);

            close(&pool).await;
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn the_write_ahead_log_is_cut() {
        tauri::async_runtime::block_on(async {
            let (dir, pool) = file("log").await;
            incremental(&pool).await.unwrap();
            fill(&pool, 50).await;
            let log = dir.join("test.db-wal");
            assert!(std::fs::metadata(&log).unwrap().len() > 0);

            tend(&pool, Pass::Running).await.unwrap();

            assert_eq!(std::fs::metadata(&log).unwrap().len(), 0);
            let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blobs")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(kept, 50);

            close(&pool).await;
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn the_planner_gets_statistics() {
        tauri::async_runtime::block_on(async {
            let (dir, pool) = file("analyze").await;
            sqlx::raw_sql(
                "CREATE TABLE lines (id INTEGER PRIMARY KEY, line TEXT NOT NULL);
                 CREATE INDEX lines_line ON lines (line);
                 WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 500)
                 INSERT INTO lines (line) SELECT 'line ' || (i % 7) FROM n;",
            )
            .execute(&pool)
            .await
            .unwrap();

            tend(&pool, Pass::Running).await.unwrap();

            let stat: String =
                sqlx::query_scalar("SELECT stat FROM sqlite_stat1 WHERE idx = 'lines_line'")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            // 500 rows, about 71 for each of the seven lines
            assert!(stat.starts_with("500 "), "{stat}");

            close(&pool).await;
            std::fs::remove_dir_all(dir).unwrap();
        });
    }
}
