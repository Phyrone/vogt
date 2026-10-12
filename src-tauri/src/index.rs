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
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Notify, watch};

use crate::{
    cache::{self, Cache, Fact},
    history::{self, Refs},
    maintenance::{self, Pass},
};

/// How many commits one turn of the walk reads at once.
const BATCH: usize = 2_000;
/// How long the worker lets file changes settle before syncing.
const QUIET: Duration = Duration::from_millis(400);
/// How long the exit or a rebuild waits for a worker to stop.
const SHUTDOWN: Duration = Duration::from_secs(if cfg!(test) { 1 } else { 5 });

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
    /// Turns true once the worker is gone; a state rather than a signal, so
    /// everyone who waits for it hears it.
    stopped: watch::Sender<bool>,
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
            stopped: watch::Sender::new(false),
            watcher: Mutex::new(None),
        }
    }
}

/// The repositories the app keeps in sync, by registry id.
#[derive(Default)]
pub struct Repos {
    open: Mutex<HashMap<String, Arc<Repo>>>,
    /// Held while an entry is opened, so two commands that both miss one do
    /// not open its file side by side: on a new file, a migration that fails
    /// in one resets the tables under the other. A rebuild holds it too, while
    /// it puts new entries in the place of the old ones.
    opening: tokio::sync::Mutex<()>,
    /// Set by [`shutdown`] while it holds `opening`; nothing is opened after.
    closed: AtomicBool,
}

impl Repos {
    pub fn get(&self, repo: &str) -> Option<Arc<Repo>> {
        self.open
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
    let _opening = repos.opening.lock().await;
    // the app is exiting: an entry opened now would never be closed
    if repos.closed.load(Ordering::SeqCst) {
        return Err("The app is shutting down.".into());
    }
    // another command may have opened it while this one waited
    if let Some(existing) = repos.get(repo) {
        return Ok(existing);
    }
    let cache = cache::open(app, workspace, repo).await?;
    start(
        app,
        repos,
        Repo::new(repo.to_owned(), path.to_owned(), cache),
    )
    .await
}

/// Watches a freshly opened entry, puts it into `repos`, in the place of any
/// entry with its id, and starts its worker. The caller holds `repos.opening`.
async fn start<R: tauri::Runtime>(
    app: &AppHandle<R>,
    repos: &Repos,
    repo: Repo,
) -> Result<Arc<Repo>, String> {
    let entry = Arc::new(repo);
    match watch(&entry) {
        Ok(watcher) => *entry.watcher.lock().unwrap_or_else(|e| e.into_inner()) = Some(watcher),
        Err(failure) => {
            // nothing else holds the entry, so its pool would only be dropped,
            // leaving the file's write-ahead log behind
            maintenance::close(&entry.cache.pool).await;
            return Err(failure);
        }
    }
    repos
        .open
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(entry.id.clone(), entry.clone());
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
    // FSEvents reports the real path of what it watches
    let reported = if cfg!(target_os = "macos") {
        commondir
            .canonicalize()
            .unwrap_or_else(|_| commondir.clone())
    } else {
        commondir.clone()
    };
    let mut watcher = notify::RecommendedWatcher::new(
        move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            if !relevant(&event, &reported) {
                return;
            }
            repo.dirty.store(true, Ordering::SeqCst);
            repo.wake.notify_one();
        },
        // a ref is a file, never a link to a directory; following one would
        // let a `.git` that links `refs/x` to the home directory have a watch
        // added for every directory under it
        notify::Config::default().with_follow_symlinks(false),
    )
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

/// Whether `event` in the repository whose common git directory is
/// `commondir` can move a ref or HEAD. Reading a file is no change: the
/// inotify backend reports every open, and a sync reads the refs, so letting
/// those through would have each sync wake the next. An editor rewrites the
/// index and `index.lock` of a worktree on every `git status`, which is not
/// worth a sync; a ref update arrives as the rename onto its final name. A
/// ref may be named `index` too, so only the index files themselves are left
/// out: the ones in `commondir` and in a linked worktree's directory under
/// it. A rescan signal, or an event without paths, is always relevant, and so
/// is a kind the backend could not tell.
fn relevant(event: &notify::Event, commondir: &Path) -> bool {
    if matches!(event.kind, notify::EventKind::Access(_)) {
        return false;
    }
    let worktrees = commondir.join("worktrees");
    event.need_rescan()
        || event.paths.is_empty()
        || event.paths.iter().any(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let parent = path.parent();
            let index = name == "index"
                && (parent == Some(commondir) || parent.and_then(Path::parent) == Some(&worktrees));
            !index && !name.ends_with(".lock")
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

/// Marks the worker stopped when it is dropped, wherever the worker ends.
struct Gone<'a>(&'a watch::Sender<bool>);

impl Drop for Gone<'_> {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}

/// Stops the worker of an entry and drops its watcher; the cache stays open.
async fn halt(repo: &Repo) {
    repo.stop.store(true, Ordering::SeqCst);
    repo.wake.notify_one();
    // the sender lives in the entry, so the wait ends only with a `true`
    let _ = repo.stopped.subscribe().wait_for(|gone| *gone).await;
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
    // an entry still being opened, or renewed by a rebuild, is in the map
    // once the lock is free, and is drained with the rest; a command still
    // waiting for the lock finds `closed` and opens nothing
    let _opening = repos.opening.lock().await;
    repos.closed.store(true, Ordering::SeqCst);
    let open: Vec<Arc<Repo>> = repos
        .open
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
    if !unchanged && progress.complete {
        // the refs moved, so the history is not whole again until this walk
        // is through; a repository opened meanwhile shows that it is indexing
        cache::set_meta(pool, &[("complete", "0".into())])
            .await
            .map_err(|e| e.to_string())?;
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
        // an entry still being opened is in the map once the lock is free, so
        // it can be told to go
        let _opening = repos.opening.lock().await;
        let mut map = repos.open.lock().unwrap_or_else(|e| e.into_inner());
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
///
/// A worker still in a batch after [`SHUTDOWN`] is not waited for, as in
/// [`rebuild_caches`]: its cache is closed in the background once the worker
/// is through, and deleted unless the app exits or the repository is opened
/// again first.
#[tauri::command]
pub async fn forget_repo<R: tauri::Runtime>(
    app: AppHandle<R>,
    repos: State<'_, Repos>,
    workspace: String,
    repo: String,
) -> Result<String, String> {
    // held until the file is gone, so no command opens it again meanwhile
    let _opening = repos.opening.lock().await;
    let entry = repos
        .open
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&repo);
    if let Some(entry) = entry {
        // every command that opens or drops a cache waits for the lock, so it
        // is not held for a whole batch
        if tokio::time::timeout(SHUTDOWN, halt(&entry)).await.is_err() {
            eprintln!(
                "vogt: {} is still being indexed; its cache is deleted after this batch",
                entry.id
            );
            let file = cache::file(&app, &workspace, &repo)?;
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                // the worker leaves after its batch, as it was told to
                halt(&entry).await;
                maintenance::close(&entry.cache.pool).await;
                let repos = app.state::<Repos>();
                let _opening = repos.opening.lock().await;
                // the exit has come, or a command opened the file again meanwhile
                if repos.closed.load(Ordering::SeqCst) || repos.get(&repo).is_some() {
                    return;
                }
                if let Err(failure) = cache::remove(&app, &workspace, &repo).await {
                    eprintln!("vogt: deleting the cache of {repo} failed: {failure}");
                }
            });
            return Ok(file.to_string_lossy().into_owned());
        }
        maintenance::close(&entry.cache.pool).await;
    }
    cache::remove(&app, &workspace, &repo)
        .await
        .map(|file| file.to_string_lossy().into_owned())
}

/// Empties the cache of every watched repository and lets the indexer build
/// it again, which the frontend hears about with `repo-changed` as usual. The
/// files stay: the frontend may hold connections to them, so their tables are
/// dropped and made anew instead.
///
/// A worker still in a batch after [`SHUTDOWN`] is not waited for: its cache
/// is rebuilt in the background once the worker is through, unless the app
/// exits or the repository leaves the registry first.
#[tauri::command]
pub async fn rebuild_caches<R: tauri::Runtime>(
    app: AppHandle<R>,
    repos: State<'_, Repos>,
) -> Result<(), String> {
    // nothing opens or removes an entry until every one is renewed; the old
    // entries stay in the map meanwhile, so the commands still find a pool
    let _opening = repos.opening.lock().await;
    let open: Vec<Arc<Repo>> = repos
        .open
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .cloned()
        .collect();
    // the workers stop side by side, as in `shutdown`, so the lock waits
    // `SHUTDOWN` for them at most, however many there are
    let halts: Vec<_> = open
        .into_iter()
        .map(|repo| {
            tauri::async_runtime::spawn(async move {
                let halted = tokio::time::timeout(SHUTDOWN, halt(&repo)).await.is_ok();
                (repo, halted)
            })
        })
        .collect();
    let mut failures = Vec::new();
    for halting in halts {
        let Ok((repo, halted)) = halting.await else {
            continue;
        };
        if halted {
            if let Err(failure) = renew(&app, &repos, &repo).await {
                failures.push(format!("{}: {failure}", repo.id));
            }
            continue;
        }
        eprintln!(
            "vogt: {} is still being indexed; its cache is rebuilt after this batch",
            repo.id
        );
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // the worker leaves after its batch, as it was told to
            halt(&repo).await;
            let repos = app.state::<Repos>();
            let _opening = repos.opening.lock().await;
            // the exit or the registry has dealt with the entry meanwhile
            let current = repos.get(&repo.id);
            if repos.closed.load(Ordering::SeqCst)
                || !current.is_some_and(|current| Arc::ptr_eq(&current, &repo))
            {
                return;
            }
            if let Err(failure) = renew(&app, &repos, &repo).await {
                eprintln!(
                    "vogt: rebuilding the cache of {} failed: {failure}",
                    repo.id
                );
            }
        });
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Some caches could not be rebuilt: {}",
            failures.join("; ")
        ))
    }
}

/// Empties the cache of a halted entry and puts a new entry on the same pool
/// in its place, with a worker that walks from scratch. The caller holds
/// `repos.opening`.
async fn renew<R: tauri::Runtime>(
    app: &AppHandle<R>,
    repos: &Repos,
    repo: &Repo,
) -> Result<(), String> {
    let emptied = empty(&repo.cache.pool).await.map_err(|e| e.to_string());
    let cache = Cache {
        file: repo.cache.file.clone(),
        pool: repo.cache.pool.clone(),
    };
    let fresh = Repo::new(repo.id.clone(), repo.path.clone(), cache);
    if let Err(failure) = start(app, repos, fresh).await {
        // `start` closed the pool the halted entry shares; without the entry,
        // the next command to want the repository opens it again
        repos
            .open
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&repo.id);
        return Err(failure);
    }
    emptied
}

/// Drops every table of a cache and makes them again, empty. The version is
/// kept, so the rounds of the next walk still count up from where the
/// frontend is.
async fn empty(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    let version = cache::meta(pool, "version").await?;
    cache::reset(pool).await?;
    cache::prepare(pool).await?;
    if let Some(version) = version {
        cache::set_meta(pool, &[("version", version)]).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cache, history};
    use std::sync::atomic::AtomicUsize;
    use tauri::Manager;

    /// Registry ids are UUIDs, and the cache paths are built from them.
    const WORKSPACE: &str = "00000000-0000-4000-8000-000000000001";
    const REAL: &str = "00000000-0000-4000-8000-000000000002";
    const BROKEN: &str = "00000000-0000-4000-8000-000000000003";

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
                    id: BROKEN.into(),
                    workspace: WORKSPACE.into(),
                    path: missing.clone(),
                },
                Registered {
                    id: REAL.into(),
                    workspace: WORKSPACE.into(),
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
                WORKSPACE.into(),
                BROKEN.into(),
                missing,
            )
            .await
            .unwrap_err();
            assert_eq!(failure, "This folder is not a git repository.");

            // the real one is watched, indexed and laid out
            let repos = app.state::<Repos>();
            let real = repos.get(REAL).expect("the real repository is watched");
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
                WORKSPACE.into(),
                REAL.into(),
            )
            .await
            .unwrap();
            assert_eq!(removed, file.to_string_lossy());
            assert!(!file.exists());
            assert!(!file.with_extension("db-wal").exists());
            assert!(!file.with_extension("db-shm").exists());

            std::fs::remove_dir_all(dir).unwrap();
            let _ = std::fs::remove_dir_all(root);
        });
    }

    #[test]
    fn two_commands_opening_a_new_cache_at_once_share_one_entry() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, _refs) = repository("together", 3);
            let root = scratch("together-cache");
            let app = app_with_cache(&root);
            let open = || {
                let app = app.handle().clone();
                let dir = dir.clone();
                tauri::async_runtime::spawn(async move {
                    ensure(&app, &app.state::<Repos>(), WORKSPACE, REAL, &dir)
                        .await
                        .unwrap()
                })
            };
            let (first, second) = (open(), open());
            let (first, second) = (first.await.unwrap(), second.await.unwrap());
            assert!(Arc::ptr_eq(&first, &second));

            // the one cache they share is built once and indexed in full
            let progress = indexed(&app.state::<Repos>(), REAL, 3).await;
            assert_eq!(
                (progress.indexed, progress.laid, progress.complete),
                (3, 3, true)
            );

            shutdown(&app.state::<Repos>()).await;
            std::fs::remove_dir_all(dir).unwrap();
            let _ = std::fs::remove_dir_all(root);
        });
    }

    #[test]
    fn nothing_is_opened_after_the_shutdown() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, _refs) = repository("closed", 1);
            let root = scratch("closed-cache");
            let app = app_with_cache(&root);
            let repos = app.state::<Repos>();
            shutdown(&repos).await;

            // as a command that waited for the lock while the exit drained it
            assert!(
                ensure(app.handle(), &repos, WORKSPACE, REAL, &dir)
                    .await
                    .is_err()
            );
            assert!(repos.get(REAL).is_none());
            assert!(!cache::file(app.handle(), WORKSPACE, REAL).unwrap().exists());

            std::fs::remove_dir_all(dir).unwrap();
            let _ = std::fs::remove_dir_all(root);
        });
    }

    /// Waits until the worker of `repo` has indexed and laid out `commits`.
    async fn indexed(repos: &Repos, repo: &str, commits: i64) -> cache::Progress {
        let mut progress = cache::Progress::default();
        for _ in 0..200 {
            let entry = repos.get(repo).expect("the repository is watched");
            progress = cache::progress(&entry.cache.pool).await.unwrap();
            if (progress.indexed, progress.laid, progress.complete) == (commits, commits, true) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        progress
    }

    #[test]
    fn rebuilding_the_caches_empties_them_and_indexes_them_again() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, _refs) = repository("rebuild", 3);
            let root = scratch("rebuild-cache");
            let app = app_with_cache(&root);
            let list = vec![Registered {
                id: REAL.into(),
                workspace: WORKSPACE.into(),
                path: dir.clone(),
            }];
            sync_repositories(app.handle().clone(), app.state::<Repos>(), list)
                .await
                .unwrap();
            let repos = app.state::<Repos>();
            let before = indexed(&repos, REAL, 3).await;
            assert_eq!((before.indexed, before.laid, before.complete), (3, 3, true));
            let old = repos.get(REAL).unwrap();
            let version = |pool| async move {
                cache::meta(pool, "version")
                    .await
                    .unwrap()
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(0)
            };
            let before = version(&old.cache.pool).await;
            assert!(before > 0);
            // a key no walk writes, so only the emptying takes it away
            cache::set_meta(&old.cache.pool, &[("marker", "1".into())])
                .await
                .unwrap();

            rebuild_caches(app.handle().clone(), app.state::<Repos>())
                .await
                .unwrap();

            // a new entry on the same file, which the indexer builds again; a
            // slow runner may leave the old worker in a sync past the wait, and
            // the rebuild then comes once it is through
            for _ in 0..400 {
                if !Arc::ptr_eq(&repos.get(REAL).unwrap(), &old) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            let new = repos.get(REAL).expect("the repository is watched again");
            assert!(!Arc::ptr_eq(&old, &new));
            assert_eq!(new.cache.file, old.cache.file);
            let after = indexed(&repos, REAL, 3).await;
            assert_eq!((after.indexed, after.laid, after.complete), (3, 3, true));
            // from an emptied file, whose rounds counted on from the version
            // it had rather than from zero
            assert_eq!(cache::meta(&new.cache.pool, "marker").await.unwrap(), None);
            assert!(version(&new.cache.pool).await > before);

            shutdown(&repos).await;
            std::fs::remove_dir_all(dir).unwrap();
            let _ = std::fs::remove_dir_all(root);
        });
    }

    #[test]
    fn a_rebuild_leaves_a_busy_worker_to_finish_and_rebuilds_its_cache_after() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, _refs) = repository("busy-rebuild", 3);
            let root = scratch("busy-rebuild-cache");
            let app = app_with_cache(&root);
            let repos = app.state::<Repos>();
            // an entry whose worker is in a batch that outlasts the wait: it
            // has none, so it is gone only once the test says so
            let cache = cache::open(app.handle(), WORKSPACE, REAL).await.unwrap();
            let busy = Arc::new(Repo::new(REAL.into(), dir.clone(), cache));
            repos.open.lock().unwrap().insert(REAL.into(), busy.clone());
            cache::set_meta(&busy.cache.pool, &[("marker", "1".into())])
                .await
                .unwrap();

            // the rebuild gives up on the worker, and the entry stays where
            // the commands find it, with its cache as it was
            let rebuilt = tokio::time::timeout(
                SHUTDOWN * 3,
                rebuild_caches(app.handle().clone(), app.state::<Repos>()),
            )
            .await
            .expect("the rebuild waits for a worker a bounded time");
            rebuilt.unwrap();
            assert!(Arc::ptr_eq(&repos.get(REAL).unwrap(), &busy));
            assert_eq!(
                cache::meta(&busy.cache.pool, "marker").await.unwrap(),
                Some("1".into())
            );

            // once the worker is through, the cache is emptied and a new entry
            // takes the old one's place
            busy.stopped.send_replace(true);
            for _ in 0..200 {
                if !Arc::ptr_eq(&repos.get(REAL).unwrap(), &busy) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            let fresh = repos.get(REAL).unwrap();
            assert!(!Arc::ptr_eq(&fresh, &busy));
            let after = indexed(&repos, REAL, 3).await;
            assert_eq!((after.indexed, after.laid, after.complete), (3, 3, true));
            assert_eq!(
                cache::meta(&fresh.cache.pool, "marker").await.unwrap(),
                None
            );

            shutdown(&repos).await;
            std::fs::remove_dir_all(dir).unwrap();
            let _ = std::fs::remove_dir_all(root);
        });
    }

    #[test]
    fn forgetting_a_busy_repository_deletes_its_cache_once_the_worker_is_through() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, _refs) = repository("busy-forget", 3);
            let root = scratch("busy-forget-cache");
            let app = app_with_cache(&root);
            let repos = app.state::<Repos>();
            // an entry whose worker is in a batch that outlasts the wait, as in
            // the busy rebuild
            let cache = cache::open(app.handle(), WORKSPACE, REAL).await.unwrap();
            let busy = Arc::new(Repo::new(REAL.into(), dir.clone(), cache));
            repos.open.lock().unwrap().insert(REAL.into(), busy.clone());
            let file = busy.cache.file.clone();

            // the command gives up on the worker and lets go of the lock, so
            // the other commands do not wait for the batch
            let removed = tokio::time::timeout(
                SHUTDOWN * 3,
                forget_repo(
                    app.handle().clone(),
                    app.state::<Repos>(),
                    WORKSPACE.into(),
                    REAL.into(),
                ),
            )
            .await
            .expect("forgetting waits for a worker a bounded time")
            .unwrap();
            assert_eq!(removed, file.to_string_lossy());
            assert!(repos.get(REAL).is_none());
            assert!(repos.opening.try_lock().is_ok());
            assert!(file.exists());

            // once the worker is through, its cache is closed and deleted
            busy.stopped.send_replace(true);
            for _ in 0..200 {
                if !file.exists() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            assert!(busy.cache.pool.is_closed());
            assert!(!file.exists());

            let _ = std::fs::remove_dir_all(dir);
            let _ = std::fs::remove_dir_all(root);
        });
    }

    #[test]
    // inspects the file with constant pragmas, which no schema describes
    #[allow(clippy::disallowed_methods)]
    fn a_synced_cache_is_tended_and_shutting_down_tends_and_closes_it() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, _refs) = repository("shutdown", 3);
            let root = scratch("shutdown-cache");
            std::fs::create_dir_all(&root).unwrap();
            let app = app_with_cache(&root);
            let list = vec![Registered {
                id: REAL.into(),
                workspace: WORKSPACE.into(),
                path: dir.clone(),
            }];
            sync_repositories(app.handle().clone(), app.state::<Repos>(), list)
                .await
                .unwrap();
            let repos = app.state::<Repos>();
            let real = repos.get(REAL).expect("the repository is watched");
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
            assert!(repos.get(REAL).is_none());
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
            let _ = std::fs::remove_dir_all(root);
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
            let rows: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM node WHERE view_id = 'all'")
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
    fn a_walk_after_the_refs_moved_is_not_complete_until_it_is_through() {
        tauri::async_runtime::block_on(async {
            let (dir, repo, refs) = repository("rewalk", 3);
            let pool = pool().await;
            walk(&pool, &dir, &refs, 2, &|_, _, _, _| {}, &|| false)
                .await
                .unwrap();
            assert!(cache::progress(&pool).await.unwrap().complete);

            // a new commit, and a walk that stops after its first report
            commit(&repo, "later", &[refs.head.unwrap()]);
            let refs = history::refs(&dir).unwrap();
            let reports = Mutex::new(Vec::new());
            walk(
                &pool,
                &dir,
                &refs,
                2,
                &|_, _, _, complete| reports.lock().unwrap().push(complete),
                &|| !reports.lock().unwrap().is_empty(),
            )
            .await
            .unwrap();
            assert_eq!(*reports.lock().unwrap(), [false]);
            assert!(!cache::progress(&pool).await.unwrap().complete);

            // the rest of the walk is what makes it whole again
            walk(&pool, &dir, &refs, 2, &|_, _, _, _| {}, &|| false)
                .await
                .unwrap();
            let progress = cache::progress(&pool).await.unwrap();
            assert_eq!(
                (progress.indexed, progress.laid, progress.complete),
                (4, 4, true)
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
            let cache = cache::open(app.handle(), WORKSPACE, REAL).await.unwrap();
            let entry = Repo::new(REAL.into(), dir.clone(), cache);

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
            let _ = std::fs::remove_dir_all(root);
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
    fn a_shallow_clone_is_walked_to_where_its_history_ends() {
        tauri::async_runtime::block_on(async {
            let (source, _repo, _refs) = repository("shallow-source", 3);
            let dir = scratch("shallow");
            // libgit2 cannot make a shallow clone, the git CLI can; a local
            // path only honours --depth through the transport --no-local picks
            let cloned = std::process::Command::new("git")
                .args(["clone", "--quiet", "--no-local", "--depth", "1"])
                .arg(&source)
                .arg(&dir)
                .status()
                .unwrap();
            assert!(cloned.success());
            assert!(dir.join(".git").join("shallow").exists());

            let refs = history::refs(&dir).unwrap();
            let pool = pool().await;
            walk(&pool, &dir, &refs, 2, &|_, _, _, _| {}, &|| false)
                .await
                .unwrap();

            // the parent the clone does not have is not looked for
            let progress = cache::progress(&pool).await.unwrap();
            assert_eq!(
                (progress.indexed, progress.laid, progress.complete),
                (1, 1, true)
            );
            std::fs::remove_dir_all(dir).unwrap();
            std::fs::remove_dir_all(source).unwrap();
        });
    }

    /// A change of `kind` to `paths`.
    fn event(kind: notify::EventKind, paths: &[PathBuf]) -> notify::Event {
        paths.iter().fold(notify::Event::new(kind), |event, path| {
            event.add_path(path.clone())
        })
    }

    #[test]
    fn the_watcher_ignores_index_churn() {
        use notify::event::{CreateKind, EventKind, ModifyKind, RenameMode};
        let git = Path::new("/repo/.git");
        let renamed = |paths: &[PathBuf]| {
            relevant(
                &event(EventKind::Modify(ModifyKind::Name(RenameMode::Both)), paths),
                git,
            )
        };
        assert!(!renamed(&[git.join("index")]));
        assert!(!renamed(&[git.join("index.lock")]));
        assert!(!renamed(&[git.join("index"), git.join("index.lock")]));
        // a linked worktree keeps its index in its own directory
        assert!(!renamed(&[git.join("worktrees/feature/index")]));
        assert!(renamed(&[git.join("HEAD")]));
        assert!(renamed(&[git.join("packed-refs")]));
        assert!(renamed(&[git.join("refs/heads/main")]));
        assert!(relevant(
            &event(
                EventKind::Create(CreateKind::File),
                &[git.join("refs/tags/v1")]
            ),
            git
        ));
        // a branch, tag or remote branch named `index` is a ref like any other
        for name in [
            "heads/index",
            "heads/worktrees/x/index",
            "tags/index",
            "remotes/origin/index",
        ] {
            let lock = git.join(format!("refs/{name}.lock"));
            assert!(renamed(&[lock, git.join("refs").join(name)]), "{name}");
        }
        // one relevant path is enough
        assert!(renamed(&[git.join("index"), git.join("refs/heads/main")]));
        // an event without paths is a rescan signal, never ignorable
        assert!(relevant(&event(EventKind::Any, &[]), git));
        assert!(relevant(
            &event(EventKind::Other, &[]).set_flag(notify::event::Flag::Rescan),
            git
        ));
    }

    #[test]
    fn the_watcher_ignores_reads() {
        use notify::event::{AccessKind, AccessMode, EventKind};
        let head = [PathBuf::from("/repo/.git/refs/heads/main")];
        let git = Path::new("/repo/.git");
        // what the inotify backend reports when a sync reads the refs
        assert!(!relevant(
            &event(EventKind::Access(AccessKind::Open(AccessMode::Any)), &head),
            git
        ));
        assert!(!relevant(
            &event(
                EventKind::Access(AccessKind::Close(AccessMode::Read)),
                &head
            ),
            git
        ));
    }

    /// An entry for `dir` with its watch on, and nothing heard yet.
    #[cfg(target_os = "linux")]
    async fn watched(dir: &Path) -> (Arc<Repo>, notify::RecommendedWatcher) {
        let cache = Cache {
            file: dir.join("unused.db"),
            pool: pool().await,
        };
        let repo = Arc::new(Repo::new(REAL.into(), dir.to_owned(), cache));
        let watcher = watch(&repo).unwrap();
        // adding the recursive watches opens the directories they cover
        tokio::time::sleep(Duration::from_millis(300)).await;
        repo.dirty.store(false, Ordering::SeqCst);
        (repo, watcher)
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reading_the_refs_does_not_wake_the_worker() {
        tauri::async_runtime::block_on(async {
            let (dir, repo, _refs) = repository("reads", 2);
            let (entry, _watcher) = watched(&dir).await;

            // what every sync does first
            history::refs(&dir).unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
            assert!(!entry.dirty.load(Ordering::SeqCst));

            // while a new commit still does
            let tip = repo.head().unwrap().target().unwrap();
            commit(&repo, "more", &[tip]);
            tokio::time::sleep(Duration::from_millis(300)).await;
            assert!(entry.dirty.load(Ordering::SeqCst));

            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_watcher_does_not_follow_a_linked_directory() {
        tauri::async_runtime::block_on(async {
            let (dir, _repo, _refs) = repository("linked", 1);
            let outside = scratch("linked-outside");
            std::fs::create_dir_all(outside.join("deep")).unwrap();
            std::os::unix::fs::symlink(&outside, dir.join(".git/refs/x")).unwrap();
            let (entry, _watcher) = watched(&dir).await;

            std::fs::write(outside.join("deep/file"), "changed").unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
            assert!(!entry.dirty.load(Ordering::SeqCst));

            std::fs::remove_dir_all(dir).unwrap();
            std::fs::remove_dir_all(outside).unwrap();
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
