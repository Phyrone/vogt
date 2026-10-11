//! Reads a repository's history and serves it in the shape the line map draws.
//!
//! Opening a repository builds a [`Graph`] once: which commits there are, in
//! which order, on which line and in which lane. The frontend then asks for
//! the rows it shows, so neither the whole history nor its layout ever
//! crosses into the webview or runs on its thread.
//!
//! Git does not record which branch a commit was made on. Every commit is
//! given a **line** here: a branch claims the commits on its first-parent
//! chain that no earlier branch claimed, the trunk going first. What is left
//! came in through a merge from a branch that no longer exists; each such
//! chain becomes a line of its own, marked as merged.

use std::{
    collections::{BinaryHeap, HashMap, HashSet, hash_map::Entry},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use git2::{BranchType, Oid, Patch, Repository};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    cache,
    layout::{Layout, Station, layout},
};

// shortcut: the newest commits are read and laid out in one go when a repository opens, so it is
// capped; extend the graph as the list is scrolled to go further
const LIMIT: usize = 20_000;
// how far the search for pushed commits goes before giving up; it normally ends much earlier
const PUSHED_LIMIT: usize = 100_000;

/// What the graph needs to know about a commit. It never changes for a hash,
/// which is what makes it safe to remember in the database.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// Commit time in seconds since the Unix epoch.
    pub time: i64,
    /// First parent first.
    pub parents: Vec<Oid>,
}

/// Commit nodes, taken from what the database remembers and read from git otherwise.
struct Nodes<'repo> {
    repo: &'repo Repository,
    known: HashMap<Oid, Node>,
    /// Read from git this time, to be remembered.
    fresh: Vec<Oid>,
}

impl Nodes<'_> {
    fn get(&mut self, id: Oid) -> Result<&Node, git2::Error> {
        match self.known.entry(id) {
            Entry::Occupied(node) => Ok(node.into_mut()),
            Entry::Vacant(slot) => {
                let commit = self.repo.find_commit(id)?;
                self.fresh.push(id);
                Ok(slot.insert(Node {
                    time: commit.time().seconds(),
                    parents: commit.parent_ids().collect(),
                }))
            }
        }
    }
}

/// Everything about an open repository except its rows.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// `user.name` from the git configuration.
    user: String,
    /// Id of the line history is laid out around.
    trunk: String,
    /// Id of the checked-out line.
    head_line: String,
    /// Hash HEAD points at; empty before the first commit.
    head: String,
    /// Commits on the checked-out line that no remote has.
    ahead: usize,
    lines: Vec<Line>,
    stashes: Vec<String>,
    tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct Line {
    id: String,
    name: String,
    /// Has a counterpart on a remote.
    remote: bool,
    /// Merged and deleted: it has commits but no branch any more.
    merged: bool,
}

/// The rows `from..to` of a history and the lines of the graph that cross them.
#[derive(Debug, Serialize)]
pub struct Window {
    /// How many rows the history has.
    total: usize,
    /// How many lanes the whole graph has.
    lanes: usize,
    rows: Vec<Row>,
    edges: Vec<Edge>,
}

#[derive(Debug, Serialize)]
struct Row {
    row: usize,
    lane: usize,
    hash: String,
    subject: String,
    body: Option<String>,
    author: String,
    /// Seconds since the Unix epoch.
    time: i64,
    line: String,
    /// First parent first.
    parents: Vec<String>,
    refs: Vec<Ref>,
    pushed: bool,
}

#[derive(Clone, Debug, Serialize)]
struct Ref {
    kind: &'static str,
    label: String,
}

#[derive(Debug, Serialize)]
struct Edge {
    /// Rows of the child and the parent.
    from: usize,
    to: usize,
    /// Id of the line whose colour the edge takes.
    line: String,
    /// Corner points as `[lane, row]`.
    points: Vec<[usize; 2]>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct FileChange {
    path: PathBuf,
    added: usize,
    removed: usize,
}

struct Commit {
    id: Oid,
    /// Index into the summary's lines.
    line: usize,
    parents: Vec<Oid>,
    pushed: bool,
}

/// A selection of the commits and how it is laid out.
struct View {
    /// Indices into the graph's commits, in row order.
    commits: Vec<usize>,
    layout: Layout,
}

/// The history of one open repository, built once and asked for rows.
pub struct Graph {
    path: PathBuf,
    summary: Summary,
    /// Newest first, each before its parents.
    commits: Vec<Commit>,
    refs: HashMap<Oid, Vec<Ref>>,
    all: Arc<View>,
    /// The last view asked for that leaves lines out, with the lines it kept.
    masked: Mutex<Option<(Vec<String>, Arc<View>)>>,
}

impl Graph {
    /// Reads the repository at `path`. `known` is what is remembered of its
    /// commits; what had to be read from git comes back to be remembered.
    pub fn build(
        path: &Path,
        known: HashMap<Oid, Node>,
    ) -> Result<(Self, Vec<(Oid, Node)>), git2::Error> {
        let mut repo = Repository::open(path)?;
        let mut stashes = Vec::new();
        repo.stash_foreach(|_, message, _| {
            stashes.push(message.to_owned());
            true
        })?;
        let repo = repo;

        let head = repo.head().ok().and_then(|head| head.target());
        // set while a branch is checked out, even one without a commit yet
        let head_branch = repo
            .find_reference("HEAD")?
            .symbolic_target()
            .and_then(|target| target.strip_prefix("refs/heads/"))
            .map(str::to_owned);

        let mut branches: Vec<(String, Oid, bool)> = repo
            .branches(Some(BranchType::Local))?
            .flatten()
            .filter_map(|(branch, _)| {
                let name = branch.name().ok()??.to_owned();
                Some((name, branch.get().target()?, branch.upstream().is_ok()))
            })
            .collect();
        let trunk = ["main", "master"]
            .into_iter()
            .find(|name| branches.iter().any(|(branch, ..)| branch == name))
            .map(str::to_owned)
            .or_else(|| head_branch.clone())
            .or_else(|| branches.first().map(|(name, ..)| name.clone()))
            .unwrap_or_else(|| "main".into());
        // the order lines claim commits in: the trunk, the checked-out branch, then by name
        branches.sort_by_key(|(name, ..)| {
            (
                *name != trunk,
                Some(name) != head_branch.as_ref(),
                name.clone(),
            )
        });

        let mut refs: HashMap<Oid, Vec<Ref>> = HashMap::new();
        let mut tags = Vec::new();
        let mut remote_tips = Vec::new();
        for reference in repo.references()?.flatten() {
            let (Some(name), Ok(commit)) = (reference.name(), reference.peel_to_commit()) else {
                continue;
            };
            let (kind, label) = if let Some(label) = name.strip_prefix("refs/tags/") {
                tags.push(label.to_owned());
                ("tag", label)
            } else if let Some(label) = name.strip_prefix("refs/remotes/") {
                if label.ends_with("/HEAD") {
                    continue;
                }
                remote_tips.push(commit.id());
                ("remote", label)
            } else {
                continue;
            };
            refs.entry(commit.id()).or_default().push(Ref {
                kind,
                label: label.to_owned(),
            });
        }

        let mut nodes = Nodes {
            repo: &repo,
            known,
            fresh: Vec::new(),
        };
        let tips = branches.iter().map(|(_, tip, _)| *tip).chain(head);
        let listed = newest(&mut nodes, tips, LIMIT, i64::MIN)?;
        // nothing older than the oldest commit listed can be an ancestor that is listed
        let oldest = listed
            .iter()
            .map(|id| nodes.known[id].time)
            .min()
            .unwrap_or(i64::MAX);
        let pushed: HashSet<Oid> =
            newest(&mut nodes, remote_tips.into_iter(), PUSHED_LIMIT, oldest)?
                .into_iter()
                .collect();
        let Nodes {
            known: nodes,
            fresh,
            ..
        } = nodes;
        let listed = children_first(listed, &nodes);
        let in_view: HashSet<Oid> = listed.iter().copied().collect();

        let mut line_of: HashMap<Oid, usize> = HashMap::new();
        let claim = |line_of: &mut HashMap<Oid, usize>, from: Oid, line: usize| {
            let mut next = Some(from);
            while let Some(id) = next.filter(|id| in_view.contains(id) && !line_of.contains_key(id))
            {
                line_of.insert(id, line);
                next = nodes[&id].parents.first().copied();
            }
        };
        let branch_line = |name: &str, remote| Line {
            id: name.into(),
            name: name.into(),
            remote,
            merged: false,
        };

        let mut lines = Vec::new();
        for (name, tip, remote) in &branches {
            claim(&mut line_of, *tip, lines.len());
            lines.push(branch_line(name, *remote));
        }
        let head_line = match (&head_branch, head) {
            (Some(branch), _) => {
                // a branch without a commit is not among the branches yet
                if !lines.iter().any(|line| &line.id == branch) {
                    lines.push(branch_line(branch, false));
                }
                branch.clone()
            }
            // detached: HEAD is on whichever line holds its commit, or on one of its own
            (None, Some(head)) => {
                claim(&mut line_of, head, lines.len());
                if line_of[&head] == lines.len() {
                    lines.push(Line {
                        name: "detached HEAD".into(),
                        ..branch_line("HEAD", false)
                    });
                }
                lines[line_of[&head]].id.clone()
            }
            (None, None) => trunk.clone(),
        };

        // children come before their parents, so a merge is seen before what it merged
        let mut merged_by: HashMap<Oid, Oid> = HashMap::new();
        for id in &listed {
            if !line_of.contains_key(id) {
                let name = merged_by
                    .get(id)
                    .and_then(|merge| merged_branch(repo.find_commit(*merge).ok()?.summary()?))
                    .unwrap_or_else(|| id.to_string()[..7].to_owned());
                claim(&mut line_of, *id, lines.len());
                lines.push(Line {
                    id: format!("merged-{id}"),
                    name,
                    remote: false,
                    merged: true,
                });
            }
            for parent in nodes[id].parents.iter().skip(1) {
                merged_by.entry(*parent).or_insert(*id);
            }
        }

        let commits: Vec<Commit> = listed
            .iter()
            .map(|id| Commit {
                id: *id,
                line: line_of[id],
                parents: nodes[id].parents.clone(),
                pushed: pushed.contains(id),
            })
            .collect();
        let head_index = lines.iter().position(|line| line.id == head_line);
        let summary = Summary {
            user: repo
                .config()
                .and_then(|config| config.get_string("user.name"))
                .unwrap_or_default(),
            ahead: commits
                .iter()
                .filter(|c| Some(c.line) == head_index && !c.pushed)
                .count(),
            trunk,
            head_line,
            head: head.map(|id| id.to_string()).unwrap_or_default(),
            lines,
            stashes,
            tags,
        };
        let mut graph = Self {
            path: path.into(),
            summary,
            commits,
            refs,
            all: Arc::new(View {
                commits: Vec::new(),
                layout: Layout::default(),
            }),
            masked: Mutex::new(None),
        };
        graph.all = Arc::new(graph.view(None));
        let fresh = fresh
            .into_iter()
            .map(|id| (id, nodes[&id].clone()))
            .collect();
        Ok((graph, fresh))
    }

    pub fn summary(&self) -> &Summary {
        &self.summary
    }

    /// Lays out the commits on the lines `only`, or all of them.
    fn view(&self, only: Option<&[String]>) -> View {
        let keep = |line: usize| only.is_none_or(|ids| ids.contains(&self.summary.lines[line].id));
        let commits: Vec<usize> = (0..self.commits.len())
            .filter(|i| keep(self.commits[*i].line))
            .collect();
        let row: HashMap<Oid, usize> = commits
            .iter()
            .enumerate()
            .map(|(row, i)| (self.commits[*i].id, row))
            .collect();
        let stations: Vec<Station> = commits
            .iter()
            .map(|i| Station {
                line: self.commits[*i].line,
                parents: self.commits[*i]
                    .parents
                    .iter()
                    .map(|id| row.get(id).copied())
                    .collect(),
            })
            .collect();
        let lines = &self.summary.lines;
        // a trunk without a line is past the last index, which no station is on
        let trunk = lines
            .iter()
            .position(|l| l.id == self.summary.trunk)
            .unwrap_or(lines.len());
        View {
            layout: layout(&stations, lines.len(), trunk),
            commits,
        }
    }

    /// The rows `from..to` of the history, masked to the lines `only` unless that is empty.
    pub fn window(&self, from: usize, to: usize, only: &[String]) -> Result<Window, git2::Error> {
        let view = if only.is_empty() {
            self.all.clone()
        } else {
            let mut masked = self.masked.lock().unwrap_or_else(|e| e.into_inner());
            match &*masked {
                Some((lines, view)) if lines == only => view.clone(),
                _ => {
                    let view = Arc::new(self.view(Some(only)));
                    *masked = Some((only.to_vec(), view.clone()));
                    view
                }
            }
        };
        let to = to.min(view.commits.len());
        let from = from.min(to);
        let line = |index: usize| self.summary.lines[index].id.clone();

        let repo = Repository::open(&self.path)?;
        let rows = (from..to)
            .map(|row| {
                let commit = &self.commits[view.commits[row]];
                let read = repo.find_commit(commit.id)?;
                Ok(Row {
                    row,
                    lane: view.layout.lane[row],
                    hash: commit.id.to_string(),
                    subject: read.summary().unwrap_or_default().to_owned(),
                    body: read.body().map(str::to_owned),
                    author: read.author().name().unwrap_or_default().to_owned(),
                    time: read.time().seconds(),
                    line: line(commit.line),
                    parents: commit.parents.iter().map(Oid::to_string).collect(),
                    refs: self.refs.get(&commit.id).cloned().unwrap_or_default(),
                    pushed: commit.pushed,
                })
            })
            .collect::<Result<_, git2::Error>>()?;
        // edges are ordered by the row they start in, so none after `started` can cross the window
        // shortcut: every edge that starts above the window is looked at, index them by the rows
        // they cross once windows deep in a long history get slow
        let started = view.layout.edges.partition_point(|edge| edge.from < to);
        let edges = view.layout.edges[..started]
            .iter()
            .filter(|edge| edge.to >= from)
            .map(|edge| Edge {
                from: edge.from,
                to: edge.to,
                line: line(edge.line),
                points: edge.points.clone(),
            })
            .collect();

        Ok(Window {
            total: view.commits.len(),
            lanes: view.layout.lanes,
            rows,
            edges,
        })
    }
}

/// Up to `limit` commits reachable from `tips`, newest first by commit time,
/// stopping at the first one older than `since`.
///
/// libgit2's own walk loads the whole history before its first commit as soon
/// as it has to sort, which takes minutes on a large repository. This one
/// only reads what it returns and the parents of that.
fn newest(
    nodes: &mut Nodes,
    tips: impl Iterator<Item = Oid>,
    limit: usize,
    since: i64,
) -> Result<Vec<Oid>, git2::Error> {
    let mut seen = HashSet::new();
    let mut queue = BinaryHeap::new();
    for tip in tips {
        if seen.insert(tip) {
            queue.push((nodes.get(tip)?.time, tip));
        }
    }
    let mut commits = Vec::new();
    while let Some((time, id)) = queue.pop() {
        if commits.len() == limit || time < since {
            break;
        }
        for parent in nodes.get(id)?.parents.clone() {
            if seen.insert(parent) {
                queue.push((nodes.get(parent)?.time, parent));
            }
        }
        commits.push(id);
    }
    Ok(commits)
}

/// Reorders commits so that each comes before its parents, newest first
/// where that leaves a choice. Clocks disagree, so time order alone can put a
/// parent above its child.
fn children_first(commits: Vec<Oid>, nodes: &HashMap<Oid, Node>) -> Vec<Oid> {
    let index: HashMap<Oid, usize> = commits.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let parents = |i: usize| {
        nodes[&commits[i]]
            .parents
            .iter()
            .filter_map(|id| index.get(id))
    };
    let mut waiting = vec![0usize; commits.len()];
    for i in 0..commits.len() {
        for parent in parents(i) {
            waiting[*parent] += 1;
        }
    }
    // ready once every child is placed; the heap hands out the newest, then the one listed first
    let key = |i: usize| (nodes[&commits[i]].time, std::cmp::Reverse(i));
    let mut ready: BinaryHeap<_> = (0..commits.len())
        .filter(|i| waiting[*i] == 0)
        .map(key)
        .collect();
    let mut order = Vec::with_capacity(commits.len());
    while let Some((_, std::cmp::Reverse(i))) = ready.pop() {
        order.push(commits[i]);
        for parent in parents(i) {
            waiting[*parent] -= 1;
            if waiting[*parent] == 0 {
                ready.push(key(*parent));
            }
        }
    }
    order
}

/// The branch a merge commit says it merged, from the messages git and the forges write.
fn merged_branch(summary: &str) -> Option<String> {
    if let Some((_, branch)) = summary.split_once(" from ") {
        // "Merge pull request #12 from owner/branch"
        return Some(
            branch
                .split_once('/')
                .map_or(branch, |(_, name)| name)
                .to_owned(),
        );
    }
    // "Merge branch 'name'", "Merge remote-tracking branch 'origin/name' into main"
    Some(summary.split('\'').nth(1)?.to_owned())
}

/// What a commit changed against its first parent. A merge lists nothing:
/// what it brought in is on the line it merged.
fn files(path: &Path, hash: &str) -> Result<Vec<FileChange>, git2::Error> {
    let repo = Repository::open(path)?;
    let commit = repo.find_commit(Oid::from_str(hash)?)?;
    if commit.parent_count() > 1 {
        return Ok(Vec::new());
    }
    let before = commit
        .parent(0)
        .ok()
        .map(|parent| parent.tree())
        .transpose()?;
    let diff = repo.diff_tree_to_tree(before.as_ref(), Some(&commit.tree()?), None)?;
    diff.deltas()
        .enumerate()
        .map(|(index, delta)| {
            // a binary file has no patch and counts no lines
            let (_, added, removed) = match Patch::from_diff(&diff, index)? {
                Some(patch) => patch.line_stats()?,
                None => (0, 0, 0),
            };
            let file = delta.new_file().path().or(delta.old_file().path());
            Ok(FileChange {
                path: file.unwrap_or(Path::new("")).into(),
                added,
                removed,
            })
        })
        .collect()
}

/// The histories of the repositories that are open, by the directory they were read from.
#[derive(Default)]
pub struct Histories(Mutex<HashMap<PathBuf, Arc<Graph>>>);

/// Reads the history of the directory `path` and keeps it for `history_rows`.
/// `repo` is the registry's id of the repository, which its commits are
/// remembered under.
#[tauri::command]
pub async fn open_history(
    app: AppHandle,
    histories: State<'_, Histories>,
    repo: String,
    path: PathBuf,
) -> Result<Summary, String> {
    let pool = cache::pool(&app).await;
    // the cache only saves reading: without it everything comes from git
    let known = match &pool {
        Some(pool) => cache::load(pool, &repo).await.unwrap_or_default(),
        None => HashMap::new(),
    };
    let directory = path.clone();
    let (graph, fresh) =
        tauri::async_runtime::spawn_blocking(move || Graph::build(&directory, known))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| match e.code() {
                git2::ErrorCode::NotFound => "This folder is not a git repository.".into(),
                _ => e.message().to_owned(),
            })?;
    if let Some(pool) = &pool {
        let _ = cache::store(pool, &repo, &fresh).await;
    }
    let summary = graph.summary().clone();
    histories
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(path, Arc::new(graph));
    Ok(summary)
}

/// The rows `from..to` of a history opened with `open_history`, masked to
/// the lines `only` unless that is empty.
#[tauri::command(async)]
pub fn history_rows(
    histories: State<'_, Histories>,
    path: PathBuf,
    from: usize,
    to: usize,
    only: Vec<String>,
) -> Result<Window, String> {
    let graph = histories
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&path)
        .cloned();
    graph
        .ok_or("This repository's history is not open.")?
        .window(from, to, &only)
        .map_err(|e| e.message().into())
}

#[tauri::command(async)]
pub fn commit_files(path: PathBuf, hash: String) -> Result<Vec<FileChange>, String> {
    files(&path, &hash).map_err(|e| e.message().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vogt-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn init(dir: &Path) -> Repository {
        Repository::init_opts(dir, git2::RepositoryInitOptions::new().initial_head("main")).unwrap()
    }

    /// Commits a tree holding `content` as `file.txt` onto `branch`.
    fn commit(
        repo: &Repository,
        branch: &str,
        message: &str,
        content: &str,
        parents: &[Oid],
    ) -> Oid {
        let signature = git2::Signature::now("Vogt", "vogt@example.com").unwrap();
        let mut tree = repo.treebuilder(None).unwrap();
        tree.insert("file.txt", repo.blob(content.as_bytes()).unwrap(), 0o100644)
            .unwrap();
        let tree = repo.find_tree(tree.write().unwrap()).unwrap();
        let parents: Vec<git2::Commit> = parents
            .iter()
            .map(|id| repo.find_commit(*id).unwrap())
            .collect();
        repo.commit(
            Some(&format!("refs/heads/{branch}")),
            &signature,
            &signature,
            message,
            &tree,
            &parents.iter().collect::<Vec<_>>(),
        )
        .unwrap()
    }

    #[test]
    fn a_repository_without_commits_has_its_unborn_branch_as_the_only_line() {
        let dir = scratch("unborn");
        init(&dir);

        let (graph, _) = Graph::build(&dir, HashMap::new()).unwrap();

        let summary = graph.summary();
        assert_eq!(
            (summary.trunk.as_str(), summary.head_line.as_str()),
            ("main", "main")
        );
        assert_eq!(summary.head, "");
        assert_eq!(summary.lines.len(), 1);
        assert_eq!(graph.window(0, 50, &[]).unwrap().total, 0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn every_commit_gets_a_line_and_a_lane() {
        let dir = scratch("lines");
        let repo = init(&dir);
        let root = commit(&repo, "main", "root\n\nwhy it exists\n", "a\n", &[]);
        let topic = commit(&repo, "topic", "topic work", "a\nb\nc\n", &[root]);
        let second = commit(&repo, "main", "second", "a\nb\n", &[root]);
        let merge = commit(
            &repo,
            "main",
            "Merge branch 'topic'",
            "a\nb\nc\n",
            &[second, topic],
        );
        repo.find_branch("topic", BranchType::Local)
            .unwrap()
            .delete()
            .unwrap();
        let feature = commit(&repo, "feature", "feature work", "x\n", &[merge]);
        repo.set_head("refs/heads/feature").unwrap();
        repo.tag_lightweight("v1", &repo.find_object(merge, None).unwrap(), false)
            .unwrap();

        let (graph, fresh) = Graph::build(&dir, HashMap::new()).unwrap();

        let summary = graph.summary();
        assert_eq!(summary.trunk, "main");
        assert_eq!(summary.head_line, "feature");
        assert_eq!(summary.head, feature.to_string());
        assert_eq!(summary.ahead, 1);
        assert_eq!(summary.tags, ["v1"]);
        let lines: Vec<(&str, bool)> = summary
            .lines
            .iter()
            .map(|l| (l.name.as_str(), l.merged))
            .collect();
        assert_eq!(
            lines,
            [("main", false), ("feature", false), ("topic", true)]
        );
        // everything was read from git, and comes back to be remembered
        assert_eq!(fresh.len(), 5);

        let window = graph.window(0, 50, &[]).unwrap();
        assert_eq!((window.total, window.lanes), (5, 2));
        // newest first, a merge before what it merged
        assert_eq!(window.rows[0].hash, feature.to_string());
        assert_eq!(window.rows[1].hash, merge.to_string());
        assert_eq!(window.rows[4].hash, root.to_string());
        let row = |id: Oid| {
            window
                .rows
                .iter()
                .find(|r| r.hash == id.to_string())
                .unwrap()
        };
        assert_eq!(
            (row(feature).line.as_str(), row(feature).lane),
            ("feature", 1)
        );
        assert_eq!([row(merge).lane, row(second).lane, row(root).lane], [0; 3]);
        assert_eq!(row(topic).line, format!("merged-{topic}"));
        assert_eq!(row(topic).lane, 1);
        assert_eq!(row(merge).parents, [second.to_string(), topic.to_string()]);
        assert_eq!(row(merge).refs[0].label, "v1");
        assert!(!row(merge).pushed);
        assert_eq!(row(root).body.as_deref(), Some("why it exists"));
        assert_eq!(window.edges.len(), 5);

        // a window holds its rows and every edge that crosses them
        let middle = graph.window(2, 3, &[]).unwrap();
        assert_eq!(middle.rows.len(), 1);
        assert_eq!(middle.total, 5);
        assert!(
            middle
                .edges
                .iter()
                .all(|edge| edge.from <= 2 && edge.to >= 2)
        );
        assert!(!middle.edges.is_empty());

        // masked to a line, only its commits are rows
        let masked = graph.window(0, 50, &[format!("merged-{topic}")]).unwrap();
        assert_eq!(masked.total, 1);
        assert_eq!(masked.rows[0].hash, topic.to_string());
        assert!(masked.edges.is_empty());

        let change = |added, removed| FileChange {
            path: "file.txt".into(),
            added,
            removed,
        };
        assert_eq!(files(&dir, &root.to_string()).unwrap(), [change(1, 0)]);
        assert_eq!(files(&dir, &feature.to_string()).unwrap(), [change(1, 3)]);
        assert!(files(&dir, &merge.to_string()).unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn remembered_commits_are_not_read_from_git_again() {
        let dir = scratch("remembered");
        let repo = init(&dir);
        let root = commit(&repo, "main", "root", "a\n", &[]);
        commit(&repo, "main", "second", "b\n", &[root]);
        let (first, fresh) = Graph::build(&dir, HashMap::new()).unwrap();

        let (again, read) = Graph::build(&dir, fresh.into_iter().collect()).unwrap();

        assert!(read.is_empty());
        let hashes = |graph: &Graph| -> Vec<String> {
            graph
                .window(0, 50, &[])
                .unwrap()
                .rows
                .into_iter()
                .map(|row| row.hash)
                .collect()
        };
        assert_eq!(hashes(&again), hashes(&first));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_merge_message_names_the_branch_it_merged() {
        let name = |summary| merged_branch(summary);
        assert_eq!(
            name("Merge branch 'feat/graph' into main").as_deref(),
            Some("feat/graph")
        );
        assert_eq!(
            name("Merge pull request #7 from mira/fix/abort").as_deref(),
            Some("fix/abort")
        );
        assert_eq!(name("merge the two"), None);
    }
}
