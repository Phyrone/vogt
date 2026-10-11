//! Keeps the per-repository caches in sync.
//!
//! Every registered repository gets a watcher on its git directory and a
//! worker that walks whatever git has that the cache does not, remembers it,
//! and lays the graph out again when the reachable commits or refs changed.
//! The walk is resumable: what it has enqueued but not expanded lives in the
//! cache's `pending` table, so an interrupted index continues where it left
//! off instead of starting over. The frontend is told about both progress and
//! finished rounds with `repo-changed` events.

use std::{
    collections::{BinaryHeap, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use git2::Oid;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Notify;

use crate::{
    cache::{self, Cache, Fact},
    history::{self, Refs},
};

/// How many commits one turn of the walk reads at once.
const BATCH: usize = 2_000;
/// How long the worker lets file changes settle before syncing.
const QUIET: Duration = Duration::from_millis(400);

/// A registered repository whose cache is kept up to date.
pub struct Repo {
    pub id: String,
    pub path: PathBuf,
    pub cache: Cache,
    /// Set by the watcher; the worker syncs once the changes go quiet.
    dirty: AtomicBool,
    /// Set when the entry is dropped; the worker leaves.
    stop: AtomicBool,
    /// Wakes the worker from its wait.
    wake: Notify,
    /// The worker is gone.
    stopped: Notify,
    /// Dropped last, so the watch lives as long as the entry.
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
}

impl Repo {
    fn new(id: String, path: PathBuf, cache: Cache) -> Self {
        Self {
            id,
            path,
            cache,
            dirty: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            wake: Notify::new(),
            stopped: Notify::new(),
            watcher: Mutex::new(None),
        }
    }
}

/// The repositories the app keeps in sync, by registry id.
#[derive(Default)]
pub struct Repos(Mutex<HashMap<String, Arc<Repo>>>);

impl Repos {
    pub fn get(&self, repo: &str) -> Option<Arc<Repo>> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(repo)
            .cloned()
    }
}

/// One registered repository, as the frontend knows it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registered {
    id: String,
    workspace: String,
    path: PathBuf,
}

/// What the frontend hears about a repository.
#[derive(Clone, Debug, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Changed {
    repo: String,
    /// Goes up with every laid-out round.
    version: i64,
    indexed: i64,
    laid: i64,
    complete: bool,
}

/// Opens (or reuses) the cache of a repository and makes sure it is watched
/// and indexed.
pub async fn ensure<R: tauri::Runtime>(
    app: &AppHandle<R>,
    repos: &Repos,
    workspace: &str,
    repo: &str,
    path: &Path,
) -> Result<Arc<Repo>, String> {
    if let Some(existing) = repos.get(repo) {
        return Ok(existing);
    }
    let cache = cache::open(app, workspace, repo).await?;
    let entry = Arc::new(Repo::new(repo.to_owned(), path.to_owned(), cache));
    {
        let mut map = repos.0.lock().unwrap_or_else(|e| e.into_inner());
        // another command may have opened it while this one awaited
        if let Some(existing) = map.get(repo) {
            return Ok(existing.clone());
        }
        *entry.watcher.lock().unwrap_or_else(|e| e.into_inner()) = Some(watch(&entry)?);
        map.insert(repo.to_owned(), entry.clone());
    }
    tauri::async_runtime::spawn(worker(app.clone(), entry.clone()));
    entry.wake.notify_one();
    Ok(entry)
}

/// Watches the git directory of a repository: the refs, HEAD, packed refs,
/// FETCH_HEAD and the linked worktrees' HEADs. Objects are not watched; a
/// fetch or commit always moves a ref too.
fn watch(repo: &Arc<Repo>) -> Result<notify::RecommendedWatcher, String> {
    let (gitdir, commondir) = history::git_dirs(&repo.path).map_err(|e| e.to_string())?;
    let repo = repo.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.is_ok() {
            repo.dirty.store(true, Ordering::SeqCst);
            repo.wake.notify_one();
        }
    })
    .map_err(|e| e.to_string())?;
    let mut watch = |dir: PathBuf, recursive: bool| -> Result<(), String> {
        if dir.exists() {
            watcher
                .watch(
                    &dir,
                    if recursive {
                        RecursiveMode::Recursive
                    } else {
                        RecursiveMode::NonRecursive
                    },
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    // a linked worktree keeps its own HEAD; the rest lives in the common dir
    watch(gitdir.clone(), false)?;
    watch(commondir.join("refs"), true)?;
    watch(commondir.join("worktrees"), true)?;
    if commondir != gitdir {
        watch(commondir, false)?;
    }
    Ok(watcher)
}

/// The worker of one repository: waits for changes to settle, then syncs.
async fn worker<R: tauri::Runtime>(app: AppHandle<R>, repo: Arc<Repo>) {
    loop {
        repo.wake.notified().await;
        if repo.stop.load(Ordering::SeqCst) {
            break;
        }
        // let a burst of file events settle before reading anything
        loop {
            tokio::time::sleep(QUIET).await;
            if !repo.dirty.swap(false, Ordering::SeqCst) || repo.stop.load(Ordering::SeqCst) {
                break;
            }
        }
        if repo.stop.load(Ordering::SeqCst) {
            break;
        }
        if let Err(e) = sync(&app, &repo).await {
            eprintln!("vogt: syncing {} failed: {e}", repo.id);
        }
    }
    repo.stopped.notify_one();
}

/// Stops the worker of an entry and waits for it to be gone.
async fn stop(repo: &Repo) {
    repo.stop.store(true, Ordering::SeqCst);
    repo.wake.notify_one();
    repo.stopped.notified().await;
}

/// Keeps one repository's cache current: reads its refs and walks it.
async fn sync<R: tauri::Runtime>(app: &AppHandle<R>, repo: &Repo) -> Result<(), String> {
    let pool = &repo.cache.pool;
    let path = repo.path.clone();
    let mut refs = tauri::async_runtime::spawn_blocking(move || history::refs(&path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    refs.heads = read_heads(pool).await?;
    let id = repo.id.clone();
    let report = move |version, indexed, laid, complete| {
        let _ = app.emit(
            "repo-changed",
            Changed {
                repo: id.clone(),
                version,
                indexed,
                laid,
                complete,
            },
        );
    };
    walk(pool, &repo.path, &refs, BATCH, &report, &|| {
        repo.stop.load(Ordering::SeqCst)
    })
    .await
}

/// Reads the whole repository into the cache, in batches of `batch`, and lays
/// the graph out again whenever the reachable commits changed. `report` hears
/// about progress and finished rounds; `stop` is asked between batches so a
/// dropped entry can leave early.
pub async fn walk(
    pool: &Pool<Sqlite>,
    path: &Path,
    refs: &Refs,
    batch: usize,
    report: &(dyn Fn(i64, i64, i64, bool) + Sync),
    stop: &(dyn Fn() -> bool + Sync),
) -> Result<(), String> {
    let progress = cache::progress(pool).await.map_err(|e| e.to_string())?;
    let fingerprint = refs.fingerprint();
    let previous = cache::meta(pool, "fingerprint")
        .await
        .map_err(|e| e.to_string())?;
    let unchanged = previous.as_deref() == Some(&fingerprint);
    if unchanged && progress.complete && progress.indexed == progress.laid {
        return Ok(());
    }

    let mut indexed = progress.indexed;
    let mut version = cache::meta(pool, "version")
        .await
        .map_err(|e| e.to_string())?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut laid = progress.laid;

    // the tips themselves, then everything the work list still holds
    let tips: Vec<Oid> = refs.tips();
    let hashes: Vec<String> = tips.iter().map(Oid::to_string).collect();
    let known = cache::known(pool, &hashes)
        .await
        .map_err(|e| e.to_string())?;
    let fresh: Vec<Oid> = tips
        .into_iter()
        .filter(|tip| !known.contains(&tip.to_string()))
        .collect();
    // the branch tip and HEAD are the same commit often enough
    let mut seen = HashSet::new();
    let fresh: Vec<Oid> = fresh.into_iter().filter(|tip| seen.insert(*tip)).collect();
    if !fresh.is_empty() {
        let facts = read_facts(path, fresh).await?;
        cache::enqueue(pool, &facts)
            .await
            .map_err(|e| e.to_string())?;
        indexed += facts.len() as i64;
        cache::set_meta(pool, &[("indexed", indexed.to_string())])
            .await
            .map_err(|e| e.to_string())?;
    }

    let mut heap: BinaryHeap<(i64, String)> = cache::pending(pool)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(hash, time)| (time, hash))
        .collect();
    let mut next_round = (laid.max(1) * 2).max(batch as i64);
    let mut processed = 0usize;

    loop {
        if stop() {
            return Ok(());
        }
        let mut popped = Vec::with_capacity(batch);
        while popped.len() < batch {
            match heap.pop() {
                Some((_, hash)) => popped.push(hash),
                None => break,
            }
        }
        if popped.is_empty() {
            break;
        }

        // the parents of the popped commits are what this turn reads
        let ids: Vec<Oid> = popped.iter().filter_map(|hash| hash.parse().ok()).collect();
        let read = read_facts(path, ids).await?;
        let mut wanted: HashSet<String> = HashSet::new();
        for fact in &read {
            for parent in &fact.parents {
                wanted.insert(parent.clone());
            }
        }
        let wanted: Vec<String> = wanted.into_iter().collect();
        let known = cache::known(pool, &wanted)
            .await
            .map_err(|e| e.to_string())?;
        let unknown: Vec<Oid> = wanted
            .iter()
            .filter(|hash| !known.contains(*hash))
            .filter_map(|hash| hash.parse().ok())
            .collect();
        let parents = if unknown.is_empty() {
            Vec::new()
        } else {
            read_facts(path, unknown).await?
        };
        cache::expand(pool, &popped, &parents)
            .await
            .map_err(|e| e.to_string())?;
        for fact in &parents {
            heap.push((fact.time, fact.hash.clone()));
        }
        // the count is what the commit table holds, so only new parents count
        indexed += parents.len() as i64;
        processed += popped.len();
        cache::set_meta(pool, &[("indexed", indexed.to_string())])
            .await
            .map_err(|e| e.to_string())?;

        if last_emit.elapsed() > Duration::from_millis(250) {
            last_emit = Instant::now();
            report(version, indexed, laid, false);
        }

        if indexed >= next_round {
            laid = history::materialize(pool, refs, version + 1)
                .await
                .map_err(|e| e.to_string())?
                .total as i64;
            version += 1;
            next_round = (laid * 2).max(laid + batch as i64);
            cache::set_meta(pool, &[("version", version.to_string())])
                .await
                .map_err(|e| e.to_string())?;
            report(version, indexed, laid, false);
        }
    }

    // the walk is through: mark it and lay out whatever changed since
    cache::set_meta(
        pool,
        &[("complete", "1".into()), ("fingerprint", fingerprint)],
    )
    .await
    .map_err(|e| e.to_string())?;
    if processed > 0
        || !unchanged
        || laid != cache::progress(pool).await.map_err(|e| e.to_string())?.laid
    {
        laid = history::materialize(pool, refs, version + 1)
            .await
            .map_err(|e| e.to_string())?
            .total as i64;
        version += 1;
        cache::set_meta(pool, &[("version", version.to_string())])
            .await
            .map_err(|e| e.to_string())?;
    }
    report(version, indexed, laid, true);
    Ok(())
}

/// The HEADs of the worktrees the app has seen, as tips the walk must cover.
async fn read_heads(pool: &sqlx::Pool<sqlx::Sqlite>) -> Result<Vec<Oid>, String> {
    let heads: Vec<String> = cache::meta(pool, "heads")
        .await
        .map_err(|e| e.to_string())?
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    Ok(heads.iter().filter_map(|head| head.parse().ok()).collect())
}

/// Remembers the HEADs the app has opened a worktree at, and wakes the worker
/// when one of them is new.
pub async fn note_heads(repo: &Repo, heads: &[Oid]) -> Result<(), String> {
    let pool = &repo.cache.pool;
    let mut known: Vec<String> = cache::meta(pool, "heads")
        .await
        .map_err(|e| e.to_string())?
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    let mut added = false;
    for head in heads {
        let head = head.to_string();
        if !known.contains(&head) {
            known.push(head);
            added = true;
        }
    }
    if added {
        cache::set_meta(
            pool,
            &[(
                "heads",
                serde_json::to_string(&known).map_err(|e| e.to_string())?,
            )],
        )
        .await
        .map_err(|e| e.to_string())?;
        repo.wake.notify_one();
    }
    Ok(())
}

/// Waits for the first laid-out round, so that opening a repository can show
/// rows instead of a spinner whenever it can.
pub async fn wait_for_first(repo: &Repo) {
    for _ in 0..100 {
        match cache::progress(&repo.cache.pool).await {
            Ok(progress) if progress.laid > 0 || progress.complete => return,
            Ok(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            Err(_) => return,
        }
    }
}

/// Reads the facts of `ids` from git, on a blocking thread.
async fn read_facts(path: &Path, ids: Vec<Oid>) -> Result<Vec<Fact>, String> {
    let path = path.to_owned();
    tauri::async_runtime::spawn_blocking(move || history::read_facts(&path, &ids))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Watches every registered repository; one that is no longer in the list is
/// stopped, but its cache file is kept for a later re-add.
#[tauri::command]
pub async fn sync_repositories<R: tauri::Runtime>(
    app: AppHandle<R>,
    repos: State<'_, Repos>,
    list: Vec<Registered>,
) -> Result<(), String> {
    let wanted: HashSet<String> = list
        .iter()
        .map(|registered| registered.id.clone())
        .collect();
    let gone: Vec<Arc<Repo>> = {
        let mut map = repos.0.lock().unwrap_or_else(|e| e.into_inner());
        let gone = map
            .values()
            .filter(|repo| !wanted.contains(&repo.id))
            .cloned()
            .collect();
        map.retain(|id, _| wanted.contains(id));
        gone
    };
    for repo in gone {
        stop(&repo).await;
    }
    for registered in &list {
        ensure(
            &app,
            &repos,
            &registered.workspace,
            &registered.id,
            &registered.path,
        )
        .await?;
    }
    Ok(())
}

/// Drops a repository's cache and stops keeping it in sync; answers with the
/// cache file it deleted, for the frontend to close.
#[tauri::command]
pub async fn forget_repo<R: tauri::Runtime>(
    app: AppHandle<R>,
    repos: State<'_, Repos>,
    workspace: String,
    repo: String,
) -> Result<String, String> {
    let entry = repos
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&repo);
    if let Some(entry) = entry {
        stop(&entry).await;
    }
    cache::remove(&app, &workspace, &repo)
        .await
        .map(|file| file.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cache, history};
    use std::sync::atomic::AtomicUsize;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vogt-walk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn init(dir: &Path) -> git2::Repository {
        git2::Repository::init_opts(dir, git2::RepositoryInitOptions::new().initial_head("main"))
            .unwrap()
    }

    /// An empty commit on `main`, parented on `parents`.
    fn commit(repo: &git2::Repository, message: &str, parents: &[Oid]) -> Oid {
        let signature = git2::Signature::now("Vogt", "vogt@example.com").unwrap();
        let tree = repo
            .find_tree(repo.treebuilder(None).unwrap().write().unwrap())
            .unwrap();
        let parents: Vec<git2::Commit> = parents
            .iter()
            .map(|id| repo.find_commit(*id).unwrap())
            .collect();
        repo.commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            message,
            &tree,
            &parents.iter().collect::<Vec<_>>(),
        )
        .unwrap()
    }

    /// A repository on `main` with `count` commits, and its refs.
    fn repository(name: &str, count: usize) -> (PathBuf, git2::Repository, Refs) {
        let dir = scratch(name);
        let repo = init(&dir);
        let mut parent = Vec::new();
        for i in 0..count {
            parent = vec![commit(&repo, &format!("commit {i}"), &parent)];
        }
        let refs = history::refs(&dir).unwrap();
        (dir, repo, refs)
    }

    async fn pool() -> Pool<Sqlite> {
        let pool = Pool::<Sqlite>::connect("sqlite::memory:").await.unwrap();
        cache::prepare(&pool).await.unwrap();
        pool
    }

    #[test]
    fn the_walk_reads_every_commit_and_lays_it_out() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, refs) = repository("full", 5);
            let pool = pool().await;
            let reports = AtomicUsize::new(0);

            walk(
                &pool,
                &dir,
                &refs,
                2,
                &|_, _, _, _| {
                    reports.fetch_add(1, Ordering::SeqCst);
                },
                &|| false,
            )
            .await
            .unwrap();

            let progress = cache::progress(&pool).await.unwrap();
            assert_eq!(
                (progress.indexed, progress.laid, progress.complete),
                (5, 5, true)
            );
            assert!(reports.load(Ordering::SeqCst) >= 2);
            let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM node WHERE view_id = 'all'")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(rows, 5);
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn an_interrupted_walk_resumes_where_it_stopped() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, refs) = repository("resume", 6);
            let pool = pool().await;
            let calls = AtomicUsize::new(0);

            walk(
                &pool,
                &dir,
                &refs,
                2,
                &|_, _, _, _| {
                    calls.fetch_add(1, Ordering::SeqCst);
                },
                &|| calls.load(Ordering::SeqCst) > 0,
            )
            .await
            .unwrap();

            let stopped = cache::progress(&pool).await.unwrap();
            assert!(!stopped.complete);
            assert!(stopped.indexed > 0 && stopped.indexed < 6);
            assert!(!cache::pending(&pool).await.unwrap().is_empty());

            // starting again picks the work list up where it was left
            walk(&pool, &dir, &refs, 2, &|_, _, _, _| {}, &|| false)
                .await
                .unwrap();

            let done = cache::progress(&pool).await.unwrap();
            assert_eq!((done.indexed, done.laid, done.complete), (6, 6, true));
            assert!(cache::pending(&pool).await.unwrap().is_empty());
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn a_second_walk_only_reads_what_is_new() {
        tauri::async_runtime::block_on(async {
            let (dir, repo, refs) = repository("again", 4);
            let pool = pool().await;
            walk(&pool, &dir, &refs, 2, &|_, _, _, _| {}, &|| false)
                .await
                .unwrap();
            let version = cache::meta(&pool, "version").await.unwrap();

            // nothing changed: the walk does not even lay out again
            walk(&pool, &dir, &refs, 2, &|_, _, _, _| {}, &|| false)
                .await
                .unwrap();
            assert_eq!(cache::meta(&pool, "version").await.unwrap(), version);

            // one more commit: it is read, and the view grows by one
            let tip = refs.head.unwrap();
            commit(&repo, "later", &[tip]);
            let refs = history::refs(&dir).unwrap();
            walk(&pool, &dir, &refs, 2, &|_, _, _, _| {}, &|| false)
                .await
                .unwrap();

            let progress = cache::progress(&pool).await.unwrap();
            assert_eq!(
                (progress.indexed, progress.laid, progress.complete),
                (5, 5, true)
            );
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    #[ignore = "reads a real repository; set VOGT_REPO and VOGT_CACHE to run"]
    fn measure_a_real_repository() {
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
        let path = std::env::var("VOGT_REPO").unwrap();
        let file = std::env::var("VOGT_CACHE").unwrap();
        let _ = std::fs::remove_file(&file);
        tauri::async_runtime::block_on(async {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(4)
                .connect_with(
                    SqliteConnectOptions::new()
                        .filename(&file)
                        .create_if_missing(true)
                        .journal_mode(SqliteJournalMode::Wal)
                        .synchronous(SqliteSynchronous::Normal),
                )
                .await
                .unwrap();
            cache::prepare(&pool).await.unwrap();
            let path = Path::new(&path);
            let refs = history::refs(path).unwrap();
            let start = Instant::now();
            walk(
                &pool,
                path,
                &refs,
                BATCH,
                &|version, indexed, laid, complete| {
                    eprintln!(
                        "round v{version}: indexed {indexed}, laid {laid}, complete {complete} ({:.1}s)",
                        start.elapsed().as_secs_f32()
                    );
                },
                &|| false,
            )
            .await
            .unwrap();
            eprintln!("walk done in {:.1}s", start.elapsed().as_secs_f32());
            let progress = cache::progress(&pool).await.unwrap();
            eprintln!(
                "progress: indexed {} laid {} complete {}",
                progress.indexed, progress.laid, progress.complete
            );
        });
    }
}
