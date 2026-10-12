//! Keeps the per-repository caches in sync.
//!
//! Every registered repository gets a watcher on its git directory and a
//! worker that walks whatever git has that the cache does not, remembers it,
//! and lays the graph out again when the reachable commits or refs changed.
//! The walk is resumable: what it has enqueued but not expanded lives in the
//! cache's `pending` table, so an interrupted index continues where it left
//! off instead of starting over. The frontend is told about both progress and
//! finished rounds with `repo-changed` events.
//!
//! The worker is also what tends its cache file (see `maintenance.rs`), and
//! [`shutdown`] closes every cache when the app exits.

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
    maintenance::{self, Pass},
};

/// How many commits one turn of the walk reads at once.
const BATCH: usize = 2_000;
/// How long the worker lets file changes settle before syncing.
const QUIET: Duration = Duration::from_millis(400);
/// How long the exit waits for a worker to stop.
const SHUTDOWN: Duration = Duration::from_secs(5);

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
    let (gitdir, commondir) = history::git_dirs(&repo.path).map_err(history::open_error)?;
    let repo = repo.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        if !relevant(&event.paths) {
            return;
        }
        repo.dirty.store(true, Ordering::SeqCst);
        repo.wake.notify_one();
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

/// Whether a change to `paths` can move a ref or HEAD. An editor rewrites
/// `.git/index` and `.git/index.lock` on every `git status`, which is not
/// worth a sync; a ref update arrives as the rename onto its final name. An
/// event without paths is a rescan signal and is always relevant.
fn relevant(paths: &[PathBuf]) -> bool {
    paths.is_empty()
        || paths.iter().any(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            name != "index" && !name.ends_with(".lock")
        })
}

/// The worker of one repository: waits for changes to settle, then syncs, and
/// tends the cache file whenever a pass is due.
async fn worker<R: tauri::Runtime>(app: AppHandle<R>, repo: Arc<Repo>) {
    // notifying on drop covers the panic case too, so `stop` cannot wait for
    // a worker that is already gone
    let _gone = Gone(&repo.stopped);
    // when the cache was last tended; the first pass follows the first sync
    let mut tended: Option<Instant> = None;
    loop {
        // without a change to sync, the worker still comes back for the cache,
        // when its next pass is due: a wake on the way must not restart the wait
        let due = maintenance::due_in(tended.map(|at| at.elapsed()));
        let changed = tokio::time::timeout(due, repo.wake.notified())
            .await
            .is_ok();
        if repo.stop.load(Ordering::SeqCst) {
            break;
        }
        if changed {
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
            if repo.stop.load(Ordering::SeqCst) {
                break;
            }
        }
        // here rather than on a timer of its own: the worker is what writes
        // the cache, so a pass between two syncs never competes with one
        if tended.is_none_or(|at| maintenance::due_in(Some(at.elapsed())).is_zero()) {
            if let Err(e) = maintenance::tend(&repo.cache.pool, Pass::Running).await {
                eprintln!("vogt: tending the cache of {} failed: {e}", repo.id);
            }
            tended = Some(Instant::now());
        }
    }
}

/// Notifies `stopped` when it is dropped, wherever the worker ends.
struct Gone<'a>(&'a Notify);

impl Drop for Gone<'_> {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

/// Stops the worker of an entry and drops its watcher; the cache stays open.
async fn halt(repo: &Repo) {
    repo.stop.store(true, Ordering::SeqCst);
    repo.wake.notify_one();
    repo.stopped.notified().await;
    // the watcher's callback owns the repo; dropping it breaks the cycle
    repo.watcher
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
}

/// Stops the worker of an entry, drops its watcher and closes its pool, so
/// the cache file can be deleted and nothing the entry owns is left behind.
async fn stop(repo: &Repo) {
    halt(repo).await;
    maintenance::close(&repo.cache.pool).await;
}

/// Stops the workers and gives each cache whose worker stopped its exit pass,
/// then closes it, for the app to exit: SQLite folds a write-ahead log into
/// its file and removes it when the last connection closes, which a process
/// that just ends never does.
///
/// This is no guarantee for every cache. A worker in the middle of a batch is
/// waited for [`SHUTDOWN`] at most; after that its cache gets neither the pass
/// nor the close and keeps its log, left to SQLite's recovery and the walk's
/// own resuming on the next start.
///
/// The exit pass itself takes as long as it takes; rebuilding a cache of
/// gigabytes is the one thing that makes the exit wait noticeably, once per
/// file.
pub async fn shutdown(repos: &Repos) {
    let open: Vec<Arc<Repo>> = repos
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain()
        .map(|(_, repo)| repo)
        .collect();
    // the workers stop side by side, so one in a long batch does not use up
    // the time the others are given
    let halts: Vec<_> = open
        .into_iter()
        .map(|repo| {
            tauri::async_runtime::spawn(async move {
                let halted = tokio::time::timeout(SHUTDOWN, halt(&repo)).await.is_ok();
                (repo, halted)
            })
        })
        .collect();
    // the passes run one after the other: a rebuild needs its file's size
    // again in temporary space and once more in the log, and several large
    // caches rebuilt at once would need all of that at the same time
    for halt in halts {
        let Ok((repo, halted)) = halt.await else {
            continue;
        };
        if !halted {
            eprintln!("vogt: exiting while {} is still being indexed", repo.id);
            continue;
        }
        if let Err(e) = maintenance::tend(&repo.cache.pool, Pass::Exit).await {
            eprintln!("vogt: tending the cache of {} failed: {e}", repo.id);
        }
        maintenance::close(&repo.cache.pool).await;
    }
}

/// Keeps one repository's cache current: reads its refs and walks it.
async fn sync<R: tauri::Runtime>(app: &AppHandle<R>, repo: &Repo) -> Result<(), String> {
    let pool = &repo.cache.pool;
    let path = repo.path.clone();
    let refs = tauri::async_runtime::spawn_blocking(move || history::refs(&path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
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
    if unchanged && progress.complete {
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

    // the tips themselves, then everything the work list still holds; remote
    // tips are walked too, so a fetched commit marks what it reaches pushed
    let mut tips: Vec<Oid> = refs.tips();
    tips.extend(refs.remote_tips.iter().copied());
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

        // the parents of the popped commits are what this turn reads; they
        // were stored when the commits were enqueued, so git is only asked
        // for the parents the cache does not hold yet
        let stored = cache::parents(pool, &popped)
            .await
            .map_err(|e| e.to_string())?;
        let mut wanted: HashSet<String> = HashSet::new();
        for parents in stored.values() {
            wanted.extend(parents.iter().cloned());
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

    // the walk is through: lay out whatever changed since, then mark the walk
    // done. Storing the fingerprint only after the layout means a quit in
    // between leaves it unchanged, and the next start lays the view out again.
    if processed > 0 || !unchanged {
        laid = history::materialize(pool, refs, version + 1)
            .await
            .map_err(|e| e.to_string())?
            .total as i64;
        version += 1;
        cache::set_meta(pool, &[("version", version.to_string())])
            .await
            .map_err(|e| e.to_string())?;
    }
    cache::set_meta(
        pool,
        &[("complete", "1".into()), ("fingerprint", fingerprint)],
    )
    .await
    .map_err(|e| e.to_string())?;
    report(version, indexed, laid, true);
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
        // one broken entry must not keep the others out of the app; its own
        // page reports the failure through `open_history`
        if let Err(failure) = ensure(
            &app,
            &repos,
            &registered.workspace,
            &registered.id,
            &registered.path,
        )
        .await
        {
            eprintln!("vogt: watching {} failed: {failure}", registered.id);
        }
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

/// Drops every repository cache and lets the indexer build them again.
/// Not implemented yet; the settings page already offers the action.
#[tauri::command]
pub async fn rebuild_caches() -> Result<(), String> {
    Err("Clearing and rebuilding caches is not implemented yet".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cache, history};
    use std::sync::atomic::AtomicUsize;
    use tauri::Manager;

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

    /// A commit no ref points at, parented on `parents`.
    fn detached_commit(repo: &git2::Repository, message: &str, parents: &[Oid]) -> Oid {
        let signature = git2::Signature::now("Vogt", "vogt@example.com").unwrap();
        let tree = repo
            .find_tree(repo.treebuilder(None).unwrap().write().unwrap())
            .unwrap();
        let parents: Vec<git2::Commit> = parents
            .iter()
            .map(|id| repo.find_commit(*id).unwrap())
            .collect();
        repo.commit(
            None,
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

    /// A mock app whose caches live under `root`, so a test never writes into
    /// the user's real cache directory.
    fn app_with_cache(root: &Path) -> tauri::App<tauri::test::MockRuntime> {
        use tauri::utils::config::{AppDirectoriesOverride, AppDirectoryOverrides};
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().app.app_directories_override =
            Some(AppDirectoriesOverride::Directories(AppDirectoryOverrides {
                cache: Some(root.to_owned()),
                ..Default::default()
            }));
        tauri::test::mock_builder()
            .manage(Repos::default())
            .build(context)
            .unwrap()
    }

    #[test]
    fn the_commands_sync_a_broken_path_and_forget_a_repository() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, _refs) = repository("commands", 3);
            let root = scratch("commands-cache");
            std::fs::create_dir_all(&root).unwrap();
            let missing = root.join("not-a-repository");
            let app = app_with_cache(&root);

            let list = vec![
                Registered {
                    id: "broken".into(),
                    workspace: "ws".into(),
                    path: missing.clone(),
                },
                Registered {
                    id: "real".into(),
                    workspace: "ws".into(),
                    path: dir.clone(),
                },
            ];
            // a broken entry must not stop the ones after it
            sync_repositories(app.handle().clone(), app.state::<Repos>(), list)
                .await
                .unwrap();

            // opening the broken one reports the folder, not what libgit2 said
            let failure = crate::history::open_history(
                app.handle().clone(),
                app.state::<Repos>(),
                "ws".into(),
                "broken".into(),
                missing,
            )
            .await
            .unwrap_err();
            assert_eq!(failure, "This folder is not a git repository.");

            // the real one is watched, indexed and laid out
            let repos = app.state::<Repos>();
            let real = repos.get("real").expect("the real repository is watched");
            let mut progress = cache::progress(&real.cache.pool).await.unwrap();
            for _ in 0..200 {
                // `complete` alone is set before the last layout runs
                if (progress.indexed, progress.laid, progress.complete) == (3, 3, true) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
                progress = cache::progress(&real.cache.pool).await.unwrap();
            }
            assert_eq!(
                (progress.indexed, progress.laid, progress.complete),
                (3, 3, true)
            );

            // forgetting it takes its cache file and write-ahead siblings along
            let file = real.cache.file.clone();
            assert!(file.exists());
            let removed = forget_repo(
                app.handle().clone(),
                app.state::<Repos>(),
                "ws".into(),
                "real".into(),
            )
            .await
            .unwrap();
            assert_eq!(removed, file.to_string_lossy());
            assert!(!file.exists());
            assert!(!file.with_extension("db-wal").exists());
            assert!(!file.with_extension("db-shm").exists());

            std::fs::remove_dir_all(dir).unwrap();
            std::fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn a_synced_cache_is_tended_and_shutting_down_tends_and_closes_it() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, _refs) = repository("shutdown", 3);
            let root = scratch("shutdown-cache");
            std::fs::create_dir_all(&root).unwrap();
            let app = app_with_cache(&root);
            let list = vec![Registered {
                id: "real".into(),
                workspace: "ws".into(),
                path: dir.clone(),
            }];
            sync_repositories(app.handle().clone(), app.state::<Repos>(), list)
                .await
                .unwrap();
            let repos = app.state::<Repos>();
            let real = repos.get("real").expect("the repository is watched");
            let pool = &real.cache.pool;

            // a new cache file starts out in incremental auto-vacuum mode
            let mode: i64 = sqlx::query_scalar("PRAGMA auto_vacuum")
                .persistent(false)
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(mode, 2);

            // the worker tends it once the first sync is through, which is
            // when the planner's statistics appear
            let mut analyzed = false;
            for _ in 0..200 {
                analyzed = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM sqlite_stat1 WHERE tbl = 'commits'",
                )
                .fetch_one(pool)
                .await
                .is_ok_and(|rows| rows > 0);
                if analyzed {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            assert!(analyzed);

            // a few free pages, which no running pass would bother with
            sqlx::raw_sql(
                "CREATE TABLE junk AS SELECT zeroblob(8000) AS data;
                 DROP TABLE junk;",
            )
            .execute(pool)
            .await
            .unwrap();
            let free: i64 = sqlx::query_scalar("PRAGMA freelist_count")
                .fetch_one(pool)
                .await
                .unwrap();
            assert!(free > 0);
            let file = real.cache.file.clone();
            assert!(file.with_extension("db-wal").exists());

            shutdown(&repos).await;

            // the last connection folded the log into the file and removed it
            assert!(real.cache.pool.is_closed());
            assert!(repos.get("real").is_none());
            assert!(file.exists());
            assert!(!file.with_extension("db-wal").exists());
            assert!(!file.with_extension("db-shm").exists());

            // and the exit pass handed the free pages back first
            let closed = sqlx::sqlite::SqlitePoolOptions::new()
                .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&file))
                .await
                .unwrap();
            let free: i64 = sqlx::query_scalar("PRAGMA freelist_count")
                .fetch_one(&closed)
                .await
                .unwrap();
            assert_eq!(free, 0);
            maintenance::close(&closed).await;

            std::fs::remove_dir_all(dir).unwrap();
            std::fs::remove_dir_all(root).unwrap();
        });
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
    fn an_amended_tip_leaves_the_old_one_out_of_the_view() {
        tauri::async_runtime::block_on(async {
            let (dir, repo, _refs) = repository("amend", 3);
            let root = scratch("amend-cache");
            let app = app_with_cache(&root);
            let cache = cache::open(app.handle(), "ws", "repo").await.unwrap();
            let entry = Repo::new("repo".into(), dir.clone(), cache);

            sync(app.handle(), &entry).await.unwrap();
            let old = repo.head().unwrap().target().unwrap();
            // a head an earlier open had noted, as the cache's `heads` key did
            cache::set_meta(
                &entry.cache.pool,
                &[("heads", serde_json::to_string(&[old.to_string()]).unwrap())],
            )
            .await
            .unwrap();

            // the tip is replaced by a sibling on the same parent
            let parent = repo.find_commit(old).unwrap().parent(0).unwrap().id();
            let new = detached_commit(&repo, "rewritten", &[parent]);
            repo.reference("refs/heads/main", new, true, "rewritten")
                .unwrap();
            assert_ne!(old, new);

            sync(app.handle(), &entry).await.unwrap();

            let hashes: Vec<String> =
                sqlx::query_scalar!("SELECT hash FROM node WHERE view_id = 'all'")
                    .fetch_all(&entry.cache.pool)
                    .await
                    .unwrap();
            assert!(hashes.contains(&new.to_string()));
            assert!(!hashes.contains(&old.to_string()));

            maintenance::close(&entry.cache.pool).await;
            std::fs::remove_dir_all(dir).unwrap();
            std::fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn the_detached_head_of_a_linked_worktree_is_walked() {
        tauri::async_runtime::block_on(async {
            let (dir, repo, _refs) = repository("worktree-head", 1);
            let linked = dir.join("feature");
            repo.worktree("feature", &linked, None).unwrap();

            // a commit only the linked worktree's HEAD points at
            let head = repo.head().unwrap().target().unwrap();
            let orphan = detached_commit(&repo, "detached work", &[head]);
            git2::Repository::open(&linked)
                .unwrap()
                .set_head_detached(orphan)
                .unwrap();

            let refs = history::refs(&dir).unwrap();
            assert!(refs.heads.contains(&orphan));

            let pool = pool().await;
            walk(&pool, &dir, &refs, 2, &|_, _, _, _| {}, &|| false)
                .await
                .unwrap();
            let hashes: Vec<String> =
                sqlx::query_scalar!("SELECT hash FROM node WHERE view_id = 'all'")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            assert!(hashes.contains(&orphan.to_string()));
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn a_deleted_branch_leaves_the_view_and_an_unchanged_walk_stays_quiet() {
        tauri::async_runtime::block_on(async {
            let (dir, repo, _refs) = repository("relayout", 1);
            let a = repo.head().unwrap().target().unwrap();
            let b = detached_commit(&repo, "gone work", &[a]);
            repo.branch("gone", &repo.find_commit(b).unwrap(), true)
                .unwrap();

            let pool = pool().await;
            walk(
                &pool,
                &dir,
                &history::refs(&dir).unwrap(),
                2,
                &|_, _, _, _| {},
                &|| false,
            )
            .await
            .unwrap();
            assert_eq!(cache::progress(&pool).await.unwrap().laid, 2);

            // the branch is gone, so its commit leaves the view
            repo.find_branch("gone", git2::BranchType::Local)
                .unwrap()
                .delete()
                .unwrap();
            let refs = history::refs(&dir).unwrap();
            walk(&pool, &dir, &refs, 2, &|_, _, _, _| {}, &|| false)
                .await
                .unwrap();
            let progress = cache::progress(&pool).await.unwrap();
            assert_eq!(
                (progress.indexed, progress.laid, progress.complete),
                (2, 1, true)
            );
            assert_eq!(
                cache::meta(&pool, "fingerprint").await.unwrap().as_deref(),
                Some(refs.fingerprint().as_str())
            );

            // nothing changed: no report, although the cache holds one more
            // commit than the view shows
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
            assert_eq!(reports.load(Ordering::SeqCst), 0);

            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn a_fetched_remote_tip_marks_its_commits_pushed_without_drawing_them() {
        tauri::async_runtime::block_on(async {
            let (dir, repo, _refs) = repository("remote", 1);
            let a = repo.head().unwrap().target().unwrap();
            // the remote fetched a child of A that main does not have yet
            let b = detached_commit(&repo, "on the remote", &[a]);
            repo.reference("refs/remotes/origin/main", b, true, "fetch")
                .unwrap();

            let refs = history::refs(&dir).unwrap();
            assert_eq!(refs.remote_tips, [b]);
            let pool = pool().await;
            walk(&pool, &dir, &refs, 2, &|_, _, _, _| {}, &|| false)
                .await
                .unwrap();

            let a_hash = a.to_string();
            let pushed: i64 = sqlx::query_scalar!(
                "SELECT pushed FROM node WHERE view_id = 'all' AND hash = ?",
                a_hash
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(pushed, 1);
            let b_hash = b.to_string();
            let remote_only: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM node WHERE view_id = 'all' AND hash = ?",
                b_hash
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(remote_only, 0);
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn the_watcher_ignores_index_churn() {
        let git = Path::new("/repo/.git");
        assert!(!relevant(&[git.join("index")]));
        assert!(!relevant(&[git.join("index.lock")]));
        assert!(!relevant(&[git.join("index"), git.join("index.lock")]));
        assert!(relevant(&[git.join("HEAD")]));
        assert!(relevant(&[git.join("packed-refs")]));
        assert!(relevant(&[git.join("refs/heads/main")]));
        // one relevant path is enough
        assert!(relevant(&[git.join("index"), git.join("refs/heads/main")]));
        // an event without paths is a rescan signal, never ignorable
        assert!(relevant(&[]));
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
            // the mode `cache::open` gives a new file, which every write pays for
            maintenance::incremental(&pool).await.unwrap();
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
