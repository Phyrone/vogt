//! Turns a repository's remembered commits into the line map the graph draws.
//!
//! `index.rs` walks a repository into the per-repository cache
//! ([`crate::cache`]); this module lays what is remembered out: which commit
//! sits on which line and in which lane, and which edges cross between the
//! rows. The frontend reads the laid-out rows straight from the cache file;
//! the commands here open a repository for it and compute the summary the
//! interface draws around the graph.
//!
//! Git does not record which branch a commit was made on. Every commit is
//! given a **line**: a branch claims the commits on its first-parent chain
//! that no earlier branch claimed, the trunk going first. What is left came in
//! through a merge from a branch that no longer exists; each such chain
//! becomes a line of its own, marked as merged.

use std::{
    collections::{BinaryHeap, HashMap},
    path::{Path, PathBuf},
};

use git2::{BranchType, ErrorCode, Oid, Patch, Repository};
use serde::Serialize;
use sqlx::{Pool, Sqlite};
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::{
    cache::{self, EdgeRow, Fact, NodeRow, Progress, ViewInfo},
    index::{self, Repos},
    layout::{self, Station},
};

/// What laying out needs of a commit: when it was made and who its parents
/// are. Kept apart from [`Fact`], which carries everything the frontend reads.
struct Node {
    time: i64,
    parents: Vec<Oid>,
}

/// One local branch of the repository.
#[derive(Clone, Debug)]
pub struct Branch {
    pub name: String,
    pub tip: Oid,
    pub remote: bool,
}

/// Everything about a repository's refs that the graph and the summary need.
#[derive(Clone, Debug, Default)]
pub struct Refs {
    /// Local branches, in the order lines claim commits: the trunk first, the
    /// checked-out branch next, then by name.
    pub branches: Vec<Branch>,
    pub trunk: String,
    pub head_branch: Option<String>,
    pub head: Option<Oid>,
    /// The HEADs of the worktrees the app has seen; more tips to walk.
    pub heads: Vec<Oid>,
    pub remote_tips: Vec<Oid>,
    pub refs: HashMap<Oid, Vec<Ref>>,
    pub tags: Vec<String>,
    pub user: String,
    pub stashes: Vec<String>,
}

impl Refs {
    /// Every commit the graph is built from: the branches, plus any HEAD a
    /// worktree has pointed at.
    pub fn tips(&self) -> Vec<Oid> {
        self.branches
            .iter()
            .map(|branch| branch.tip)
            .chain(self.head)
            .chain(self.heads.iter().copied())
            .collect()
    }

    /// Changes whenever anything the graph is built from changes.
    pub fn fingerprint(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for branch in &self.branches {
            lines.push(format!("heads/{}\t{}", branch.name, branch.tip));
        }
        for (oid, refs) in &self.refs {
            for reference in refs {
                lines.push(format!("{}\t{}\t{oid}", reference.kind, reference.label));
            }
        }
        if let Some(head) = self.head {
            lines.push(format!("HEAD\t{head}"));
        }
        for head in &self.heads {
            lines.push(format!("worktree\t{head}"));
        }
        lines.sort();
        lines.join("\n")
    }
}

/// A tag or remote branch as a row shows it.
#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct Ref {
    kind: &'static str,
    label: String,
}

/// A line of the graph.
#[derive(Clone, Debug, Serialize, serde::Deserialize, TS)]
#[ts(export)]
pub struct Line {
    id: String,
    name: String,
    /// Has a counterpart on a remote.
    remote: bool,
    /// Merged and deleted: it has commits but no branch any more.
    merged: bool,
}

/// The git directory of a repository and the directory its refs are shared
/// through (they differ for a linked worktree).
pub fn git_dirs(path: &Path) -> Result<(PathBuf, PathBuf), git2::Error> {
    let repo = Repository::open(path)?;
    Ok((repo.path().to_owned(), repo.commondir().to_owned()))
}

/// Reads the refs of the repository at `path`.
pub fn refs(path: &Path) -> Result<Refs, git2::Error> {
    let mut repo = Repository::open(path)?;
    let mut stashes = Vec::new();
    repo.stash_foreach(|_, message, _| {
        stashes.push(message.to_owned());
        true
    })?;

    let head = repo.head().ok().and_then(|head| head.target());
    // set while a branch is checked out, even one without a commit yet
    let head_branch = repo
        .find_reference("HEAD")?
        .symbolic_target()
        .and_then(|target| target.strip_prefix("refs/heads/"))
        .map(str::to_owned);

    let mut branches: Vec<Branch> = repo
        .branches(Some(BranchType::Local))?
        .flatten()
        .filter_map(|(branch, _)| {
            let name = branch.name().ok()??.to_owned();
            Some(Branch {
                name,
                tip: branch.get().target()?,
                remote: branch.upstream().is_ok(),
            })
        })
        .collect();
    let trunk = ["main", "master"]
        .into_iter()
        .find(|name| branches.iter().any(|branch| branch.name == *name))
        .map(str::to_owned)
        .or_else(|| head_branch.clone())
        .or_else(|| branches.first().map(|branch| branch.name.clone()))
        .unwrap_or_else(|| "main".into());
    branches.sort_by_key(|branch| {
        (
            branch.name != trunk,
            Some(&branch.name) != head_branch.as_ref(),
            branch.name.clone(),
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

    Ok(Refs {
        branches,
        trunk,
        head_branch,
        head,
        heads: Vec::new(),
        remote_tips,
        refs,
        tags,
        user: repo
            .config()
            .and_then(|config| config.get_string("user.name"))
            .unwrap_or_default(),
        stashes,
    })
}

/// Reads the facts of `ids` out of git.
pub fn read_facts(path: &Path, ids: &[Oid]) -> Result<Vec<Fact>, git2::Error> {
    let repo = Repository::open(path)?;
    ids.iter()
        .map(|id| {
            let commit = repo.find_commit(*id)?;
            Ok(Fact {
                hash: id.to_string(),
                time: commit.time().seconds(),
                parents: commit
                    .parent_ids()
                    .map(|parent| parent.to_string())
                    .collect(),
                subject: commit.summary().unwrap_or_default().to_owned(),
                author: commit.author().name().unwrap_or_default().to_owned(),
                body: commit.body().map(str::to_owned),
            })
        })
        .collect()
}

/// How big the laid-out graph is.
pub struct Materialized {
    pub total: usize,
}

/// Reads every commit into the compact graph, a page at a time. `page` is how
/// many commits one query returns; the test that pins the boundary between
/// pages keeps it small.
async fn read_graph(
    pool: &Pool<Sqlite>,
    page: usize,
) -> Result<(Vec<Oid>, Vec<Node>, HashMap<u32, String>), String> {
    let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM commits")
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    let mut oids: Vec<Oid> = Vec::with_capacity(count as usize);
    let mut nodes: Vec<Node> = Vec::with_capacity(count as usize);
    let mut subjects: HashMap<u32, String> = HashMap::new();
    // the last row of the page before; the order is time DESC, hash, so the
    // next page is everything older, or the same second with a later hash
    let mut cursor: Option<(i64, String)> = None;
    let limit = page as i64;
    loop {
        // one row shape for both pages; each macro invocation names its record
        // type after this function, so the arms would not match otherwise
        let rows: Vec<(i64, String, String, String)> = match &cursor {
            None => sqlx::query!(
                "SELECT time, hash, parents, subject FROM commits
                 ORDER BY time DESC, hash LIMIT ?",
                limit
            )
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|row| (row.time, row.hash, row.parents, row.subject))
            .collect(),
            Some((time, hash)) => sqlx::query!(
                "SELECT time, hash, parents, subject FROM commits
                 WHERE time < ? OR (time = ? AND hash > ?)
                 ORDER BY time DESC, hash LIMIT ?",
                time,
                time,
                hash,
                limit
            )
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|row| (row.time, row.hash, row.parents, row.subject))
            .collect(),
        };
        if rows.is_empty() {
            break;
        }
        for (time, hash, parents, subject) in rows {
            let Ok(oid) = hash.parse::<Oid>() else {
                continue;
            };
            let parents: Vec<Oid> = serde_json::from_str::<Vec<String>>(&parents)
                .unwrap_or_default()
                .iter()
                .filter_map(|parent| parent.parse().ok())
                .collect();
            if parents.len() > 1 {
                subjects.insert(oids.len() as u32, subject);
            }
            oids.push(oid);
            nodes.push(Node { time, parents });
            cursor = Some((time, hash));
        }
    }
    Ok((oids, nodes, subjects))
}

/// Lays every remembered commit out as the unmasked view and writes it to the
/// cache. `version` is what the view records as its revision.
pub async fn materialize(
    pool: &Pool<Sqlite>,
    refs: &Refs,
    version: i64,
) -> Result<Materialized, String> {
    // The commits are read a page at a time and turned into the compact graph
    // right away: a whole history's subjects and bodies would otherwise sit in
    // memory next to it.
    let (oids, nodes, subjects) = read_graph(pool, 50_000).await?;
    let mut index: HashMap<Oid, u32> = HashMap::with_capacity(oids.len());
    for (i, oid) in oids.iter().enumerate() {
        index.insert(*oid, i as u32);
    }

    // what the current refs reach; commits from deleted branches stay in the
    // facts but leave the view
    let mut reachable = vec![false; nodes.len()];
    let mut stack: Vec<u32> = refs
        .tips()
        .into_iter()
        .filter_map(|tip| index.get(&tip).copied())
        .collect();
    while let Some(i) = stack.pop() {
        if reachable[i as usize] {
            continue;
        }
        reachable[i as usize] = true;
        for parent in &nodes[i as usize].parents {
            if let Some(j) = index.get(parent) {
                stack.push(*j);
            }
        }
    }
    let listed: Vec<u32> = (0..nodes.len() as u32)
        .filter(|i| reachable[*i as usize])
        .collect();
    // newest first, each before its parents; clocks disagree, so time order
    // alone can put a parent above its child
    let listed = children_first(&listed, &nodes, &index);

    // in_view is what the refs reach; listed is the same set in row order
    let mut line_of: Vec<u32> = vec![u32::MAX; nodes.len()];
    let mut lines: Vec<Line> = Vec::new();
    for branch in &refs.branches {
        claim(
            &mut line_of,
            &reachable,
            &nodes,
            &index,
            branch.tip,
            lines.len() as u32,
        );
        lines.push(Line {
            id: branch.name.clone(),
            name: branch.name.clone(),
            remote: branch.remote,
            merged: false,
        });
    }
    let _head_line = match (&refs.head_branch, refs.head) {
        (Some(branch), _) => {
            // a branch without a commit is not among the branches yet
            if !lines.iter().any(|line| &line.id == branch) {
                lines.push(Line {
                    id: branch.clone(),
                    name: branch.clone(),
                    remote: false,
                    merged: false,
                });
            }
            branch.clone()
        }
        // detached: HEAD is on whichever line holds its commit, or on one of its own
        (None, Some(head)) => {
            let line = lines.len() as u32;
            claim(&mut line_of, &reachable, &nodes, &index, head, line);
            match index.get(&head).map(|i| line_of[*i as usize]) {
                Some(line) if line != u32::MAX && (line as usize) < lines.len() => {
                    lines[line as usize].id.clone()
                }
                Some(line) if line != u32::MAX => {
                    lines.push(Line {
                        id: "HEAD".into(),
                        name: "detached HEAD".into(),
                        remote: false,
                        merged: false,
                    });
                    "HEAD".into()
                }
                // not indexed yet; the trunk stands in until it is
                _ => refs.trunk.clone(),
            }
        }
        (None, None) => refs.trunk.clone(),
    };

    // children come before their parents, so a merge is seen before what it merged
    let mut merged_by: HashMap<u32, u32> = HashMap::new();
    for id in &listed {
        if line_of[*id as usize] == u32::MAX {
            let name = merged_by
                .get(id)
                .and_then(|merge| subjects.get(merge))
                .and_then(|subject| merged_branch(subject))
                .unwrap_or_else(|| oids[*id as usize].to_string()[..7].to_owned());
            claim(
                &mut line_of,
                &reachable,
                &nodes,
                &index,
                oids[*id as usize],
                lines.len() as u32,
            );
            lines.push(Line {
                id: format!("merged-{}", oids[*id as usize]),
                name,
                remote: false,
                merged: true,
            });
        }
        for parent in nodes[*id as usize].parents.iter().skip(1) {
            if let Some(parent) = index.get(parent) {
                merged_by.entry(*parent).or_insert(*id);
            }
        }
    }

    let mut row_of: Vec<u32> = vec![u32::MAX; nodes.len()];
    for (row, id) in listed.iter().enumerate() {
        row_of[*id as usize] = row as u32;
    }
    let stations: Vec<Station> = listed
        .iter()
        .map(|id| Station {
            line: line_of[*id as usize] as usize,
            parents: nodes[*id as usize]
                .parents
                .iter()
                .map(|parent| {
                    index
                        .get(parent)
                        .map(|j| row_of[*j as usize])
                        .filter(|row| *row != u32::MAX)
                        .map(|row| row as usize)
                })
                .collect(),
        })
        .collect();
    // a trunk without a line is past the last index, which no station is on
    let trunk = lines
        .iter()
        .position(|line| line.id == refs.trunk)
        .unwrap_or(lines.len());
    let layout = layout::layout(&stations, lines.len(), trunk);

    // commits a remote has; walked over the facts, not read from git again
    let mut pushed = vec![false; nodes.len()];
    let mut stack: Vec<u32> = refs
        .remote_tips
        .iter()
        .filter_map(|tip| index.get(tip).copied())
        .collect();
    while let Some(i) = stack.pop() {
        if pushed[i as usize] {
            continue;
        }
        pushed[i as usize] = true;
        for parent in &nodes[i as usize].parents {
            if let Some(j) = index.get(parent) {
                stack.push(*j);
            }
        }
    }

    let rows: Vec<NodeRow> = listed
        .iter()
        .enumerate()
        .map(|(row, id)| {
            let oid = oids[*id as usize];
            NodeRow {
                row: row as i64,
                hash: oid.to_string(),
                lane: layout.lane[row] as i64,
                line: lines[line_of[*id as usize] as usize].id.clone(),
                pushed: pushed[*id as usize],
                refs: serde_json::to_string(refs.refs.get(&oid).map(Vec::as_slice).unwrap_or(&[]))
                    .unwrap_or_else(|_| "[]".into()),
            }
        })
        .collect();
    let edges: Vec<EdgeRow> = layout
        .edges
        .iter()
        .map(|edge| EdgeRow {
            from: edge.from as i64,
            to: edge.to as i64,
            line: lines[edge.line].id.clone(),
            points: serde_json::to_string(&edge.points).unwrap_or_default(),
        })
        .collect();

    cache::save_view(
        pool,
        ViewInfo::ALL,
        &[],
        listed.len(),
        layout.lanes,
        version,
        &rows,
        &edges,
    )
    .await
    .map_err(|e| e.to_string())?;
    cache::set_meta(
        pool,
        &[
            (
                "lines",
                serde_json::to_string(&lines).map_err(|e| e.to_string())?,
            ),
            ("trunk", refs.trunk.clone()),
        ],
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(Materialized {
        total: listed.len(),
    })
}

/// Lays the commits of the picked lines out as a view of their own. The
/// unmasked view already holds every commit's line and row, so only the lanes
/// and edges have to be worked out again.
pub async fn materialize_mask(
    pool: &Pool<Sqlite>,
    only: &[String],
    version: i64,
) -> Result<ViewInfo, String> {
    let mut only: Vec<String> = only.to_vec();
    only.sort();
    only.dedup();
    let id = format!(
        "mask:{}",
        serde_json::to_string(&only).map_err(|e| e.to_string())?
    );

    // only the picked lines' rows are read; the unmasked view already holds
    // every commit's line and row, so only lanes and edges are worked out again
    let only_json = serde_json::to_string(&only).map_err(|e| e.to_string())?;
    let rows = sqlx::query!(
        "SELECT row, hash, line, pushed, refs FROM node
         WHERE view_id = ? AND line IN (SELECT value FROM json_each(?))
         ORDER BY row",
        ViewInfo::ALL,
        only_json
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let lines: Vec<Line> = cache::meta(pool, "lines")
        .await
        .map_err(|e| e.to_string())?
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    let trunk = cache::meta(pool, "trunk")
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let line_index: HashMap<&str, usize> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| (line.id.as_str(), index))
        .collect();
    let picked: Vec<&_> = rows.iter().collect();

    let hashes: Vec<&str> = picked.iter().map(|row| row.hash.as_str()).collect();
    let mut parents: HashMap<String, Vec<String>> = HashMap::new();
    for chunk in hashes.chunks(5_000) {
        let json = serde_json::to_string(chunk).map_err(|e| e.to_string())?;
        let found = sqlx::query!(
            "SELECT hash, parents FROM commits WHERE hash IN (SELECT value FROM json_each(?))",
            json
        )
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
        parents.extend(found.into_iter().map(|row| {
            (
                row.hash,
                serde_json::from_str(&row.parents).unwrap_or_default(),
            )
        }));
    }

    let mut row_of: HashMap<&str, usize> = HashMap::with_capacity(picked.len());
    for (row, found) in picked.iter().enumerate() {
        row_of.insert(found.hash.as_str(), row);
    }
    let stations: Vec<Station> = picked
        .iter()
        .map(|found| Station {
            line: *line_index.get(found.line.as_str()).unwrap_or(&0),
            parents: parents
                .get(found.hash.as_str())
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .map(|parent| row_of.get(parent.as_str()).copied())
                .collect(),
        })
        .collect();
    let trunk = lines
        .iter()
        .position(|line| line.id == trunk)
        .unwrap_or(lines.len());
    let layout = layout::layout(&stations, lines.len(), trunk);

    let nodes: Vec<NodeRow> = picked
        .iter()
        .enumerate()
        .map(|(row, found)| NodeRow {
            row: row as i64,
            hash: found.hash.clone(),
            lane: layout.lane[row] as i64,
            line: found.line.clone(),
            pushed: found.pushed != 0,
            refs: found.refs.clone(),
        })
        .collect();
    let edges: Vec<EdgeRow> = layout
        .edges
        .iter()
        .map(|edge| EdgeRow {
            from: edge.from as i64,
            to: edge.to as i64,
            line: lines[edge.line].id.clone(),
            points: serde_json::to_string(&edge.points).unwrap_or_default(),
        })
        .collect();
    cache::save_view(
        pool,
        &id,
        &only,
        picked.len(),
        layout.lanes,
        version,
        &nodes,
        &edges,
    )
    .await
    .map_err(|e| e.to_string())?;
    cache::view(pool, &id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "The masked view could not be built.".into())
}

/// Reorders commits so that each comes before its parents, newest first where
/// that leaves a choice.
fn children_first(commits: &[u32], nodes: &[Node], index: &HashMap<Oid, u32>) -> Vec<u32> {
    // rows in `commits`; u32::MAX for everything outside it
    let mut position = vec![u32::MAX; nodes.len()];
    for (row, id) in commits.iter().enumerate() {
        position[*id as usize] = row as u32;
    }
    let parents = |i: u32| -> Vec<u32> {
        nodes[i as usize]
            .parents
            .iter()
            .filter_map(|parent| index.get(parent).map(|j| position[*j as usize]))
            .filter(|row| *row != u32::MAX)
            .collect()
    };
    let mut waiting = vec![0u32; commits.len()];
    for id in commits {
        for parent in parents(*id) {
            waiting[parent as usize] += 1;
        }
    }
    // ready once every child is placed; the heap hands out the newest, then
    // the one listed first
    let key = |position: usize| {
        (
            nodes[commits[position] as usize].time,
            std::cmp::Reverse(position),
        )
    };
    let mut ready: BinaryHeap<_> = (0..commits.len())
        .filter(|position| waiting[*position] == 0)
        .map(key)
        .collect();
    let mut order = Vec::with_capacity(commits.len());
    while let Some((_, std::cmp::Reverse(position))) = ready.pop() {
        order.push(commits[position]);
        for parent in parents(commits[position]) {
            waiting[parent as usize] -= 1;
            if waiting[parent as usize] == 0 {
                ready.push(key(parent as usize));
            }
        }
    }
    order
}

/// Walks a line's first-parent chain from `from`, claiming every commit in
/// view that no earlier line claimed. `line_of` holds the line of each commit
/// index, `u32::MAX` for none yet.
fn claim(
    line_of: &mut [u32],
    in_view: &[bool],
    nodes: &[Node],
    index: &HashMap<Oid, u32>,
    from: Oid,
    line: u32,
) {
    let mut next = index.get(&from).copied();
    while let Some(i) = next.filter(|i| in_view[*i as usize] && line_of[*i as usize] == u32::MAX) {
        line_of[i as usize] = line;
        next = nodes[i as usize]
            .parents
            .first()
            .and_then(|parent| index.get(parent).copied());
    }
}

/// The branch a merge commit says it merged, from the messages git and the
/// forges write.
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

#[derive(Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct FileChange {
    #[ts(type = "string")]
    path: PathBuf,
    added: usize,
    removed: usize,
}

/// What `open_history` answers: the summary the interface draws, and where
/// the rows it shows live.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Opened {
    user: String,
    trunk: String,
    head_line: String,
    /// Hash HEAD points at; empty before the first commit.
    head: String,
    /// Commits on the checked-out line that no remote has.
    ahead: i64,
    lines: Vec<Line>,
    stashes: Vec<String>,
    tags: Vec<String>,
    /// The repository's cache file, for the frontend to query.
    cache_path: String,
    view: ViewInfo,
    progress: Progress,
}

/// Opens the cache of the repository at `path`, starts keeping it in sync,
/// and answers with its summary. `repo` is the registry's id of the
/// repository; `workspace` decides where its cache file lives.
#[tauri::command]
pub async fn open_history<R: tauri::Runtime>(
    app: AppHandle<R>,
    repos: State<'_, Repos>,
    workspace: String,
    repo: String,
    path: PathBuf,
) -> Result<Opened, String> {
    let entry = index::ensure(&app, &repos, &workspace, &repo, &path).await?;
    let read = {
        let path = path.clone();
        tauri::async_runtime::spawn_blocking(move || refs(&path))
            .await
            .map_err(|e| e.to_string())?
    }
    .map_err(|e| match e.code() {
        ErrorCode::NotFound => "This folder is not a git repository.".to_owned(),
        _ => e.message().to_owned(),
    })?;
    index::note_heads(
        &entry,
        read.head.iter().copied().collect::<Vec<_>>().as_slice(),
    )
    .await?;
    index::wait_for_first(&entry).await;

    let pool = &entry.cache.pool;
    let progress = cache::progress(pool).await.map_err(|e| e.to_string())?;
    let view = cache::view(pool, ViewInfo::ALL)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or(ViewInfo {
            id: ViewInfo::ALL.into(),
            only: Vec::new(),
            total: 0,
            lanes: 1,
            version: 0,
        });
    let lines: Vec<Line> = cache::meta(pool, "lines")
        .await
        .map_err(|e| e.to_string())?
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    let head = read.head.map(|head| head.to_string()).unwrap_or_default();
    let head_line = match (&read.head_branch, read.head) {
        (Some(branch), _) => branch.clone(),
        (None, Some(head)) => {
            let hash = head.to_string();
            cache::line_of(pool, &hash)
                .await
                .map_err(|e| e.to_string())?
                .unwrap_or_else(|| read.trunk.clone())
        }
        (None, None) => read.trunk.clone(),
    };
    let ahead = cache::ahead(pool, &head_line)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Opened {
        user: read.user,
        trunk: read.trunk,
        head_line,
        head,
        ahead,
        lines,
        stashes: read.stashes,
        tags: read.tags,
        cache_path: entry.cache.file.to_string_lossy().into_owned(),
        view,
        progress,
    })
}

/// Lays the picked lines out as a view of their own, and answers with it. The
/// same lines at the same version reuse the view that is already there.
#[tauri::command]
pub async fn materialize_view(
    repos: State<'_, Repos>,
    repo: String,
    only: Vec<String>,
) -> Result<ViewInfo, String> {
    let entry = repos
        .get(&repo)
        .ok_or("This repository's cache is not open.")?;
    let pool = &entry.cache.pool;
    let version = cache::meta(pool, "version")
        .await
        .map_err(|e| e.to_string())?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let mut only = only;
    only.sort();
    only.dedup();
    let id = format!(
        "mask:{}",
        serde_json::to_string(&only).map_err(|e| e.to_string())?
    );
    if let Some(view) = cache::view(pool, &id).await.map_err(|e| e.to_string())? {
        if view.version == version {
            return Ok(view);
        }
    }
    materialize_mask(pool, &only, version).await
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

    /// A cache pool with the schema and the given commits in it.
    async fn cache_pool(dir: &Path, commits: &[Oid]) -> Pool<Sqlite> {
        let pool = Pool::<Sqlite>::connect("sqlite::memory:").await.unwrap();
        cache::prepare(&pool).await.unwrap();
        let facts = read_facts(dir, commits).unwrap();
        cache::enqueue(&pool, &facts).await.unwrap();
        pool
    }

    /// The rows of the unmasked view, as `(hash, lane, line)`.
    async fn rows(pool: &Pool<Sqlite>) -> Vec<(String, i64, String)> {
        sqlx::query!(
            "SELECT hash, lane, line FROM node WHERE view_id = ? ORDER BY row",
            ViewInfo::ALL
        )
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.hash, row.lane, row.line))
        .collect()
    }

    #[test]
    fn a_repository_without_commits_has_its_unborn_branch_as_the_only_line() {
        tauri::async_runtime::block_on(async {
            let dir = scratch("unborn");
            init(&dir);
            let pool = cache_pool(&dir, &[]).await;

            let refs = refs(&dir).unwrap();
            let laid = materialize(&pool, &refs, 1).await.unwrap();

            let lines: Vec<Line> =
                serde_json::from_str(&cache::meta(&pool, "lines").await.unwrap().unwrap()).unwrap();
            assert_eq!(laid.total, 0);
            assert_eq!(refs.trunk, "main");
            assert_eq!(lines.len(), 1);
            assert_eq!(lines[0].id, "main");
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn every_commit_gets_a_line_and_a_lane() {
        tauri::async_runtime::block_on(async {
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

            let pool = cache_pool(&dir, &[root, topic, second, merge, feature]).await;
            let refs = refs(&dir).unwrap();
            let laid = materialize(&pool, &refs, 1).await.unwrap();

            assert_eq!(refs.trunk, "main");
            assert_eq!(refs.head_branch.as_deref(), Some("feature"));
            assert_eq!(refs.head, Some(feature));
            assert_eq!(refs.tags, ["v1"]);
            assert_eq!(laid.total, 5);
            assert_eq!(
                cache::view(&pool, ViewInfo::ALL)
                    .await
                    .unwrap()
                    .unwrap()
                    .lanes,
                2
            );
            let lines: Vec<Line> =
                serde_json::from_str(&cache::meta(&pool, "lines").await.unwrap().unwrap()).unwrap();
            let names: Vec<(&str, bool)> = lines
                .iter()
                .map(|line| (line.name.as_str(), line.merged))
                .collect();
            assert_eq!(
                names,
                [("main", false), ("feature", false), ("topic", true)]
            );

            // newest first, a merge before what it merged
            let rows = rows(&pool).await;
            assert_eq!(rows[0].0, feature.to_string());
            assert_eq!(rows[1].0, merge.to_string());
            assert_eq!(rows[4].0, root.to_string());
            let row = |id: Oid| {
                rows.iter()
                    .find(|(hash, ..)| hash == &id.to_string())
                    .unwrap()
            };
            assert_eq!((row(feature).2.as_str(), row(feature).1), ("feature", 1));
            assert_eq!([row(merge).1, row(second).1, row(root).1], [0; 3]);
            assert_eq!(row(topic).2, format!("merged-{topic}"));
            assert_eq!(row(topic).1, 1);
            let merge_hash = merge.to_string();
            let tagged = sqlx::query!(
                "SELECT pushed, refs FROM node WHERE view_id = ? AND hash = ?",
                ViewInfo::ALL,
                merge_hash
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(tagged.pushed, 0);
            assert!(tagged.refs.contains("v1"));
            let edges =
                sqlx::query_scalar!("SELECT COUNT(*) FROM edge WHERE view_id = ?", ViewInfo::ALL)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(edges, 5);

            // a mask holds only the commits made on the picked lines
            let masked = materialize_mask(&pool, &[format!("merged-{topic}")], 1)
                .await
                .unwrap();
            assert_eq!(masked.total, 1);
            let masked_rows = sqlx::query!(
                "SELECT hash FROM node WHERE view_id = ? ORDER BY row",
                masked.id
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(masked_rows.len(), 1);
            assert_eq!(masked_rows[0].hash, topic.to_string());

            let change = |added, removed| FileChange {
                path: "file.txt".into(),
                added,
                removed,
            };
            assert_eq!(files(&dir, &root.to_string()).unwrap(), [change(1, 0)]);
            assert_eq!(files(&dir, &feature.to_string()).unwrap(), [change(1, 3)]);
            assert!(files(&dir, &merge.to_string()).unwrap().is_empty());
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn a_commit_a_deleted_branch_left_behind_leaves_the_view() {
        tauri::async_runtime::block_on(async {
            let dir = scratch("deleted");
            let repo = init(&dir);
            let root = commit(&repo, "main", "root", "a\n", &[]);
            let gone = commit(&repo, "gone", "gone work", "b\n", &[root]);
            let pool = cache_pool(&dir, &[root, gone]).await;
            assert_eq!(
                materialize(&pool, &refs(&dir).unwrap(), 1)
                    .await
                    .unwrap()
                    .total,
                2
            );

            repo.find_branch("gone", BranchType::Local)
                .unwrap()
                .delete()
                .unwrap();

            assert_eq!(
                materialize(&pool, &refs(&dir).unwrap(), 2)
                    .await
                    .unwrap()
                    .total,
                1
            );
            assert_eq!(rows(&pool).await[0].0, root.to_string());
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn reading_the_graph_in_pages_keeps_every_commit_once() {
        use std::collections::HashSet;
        tauri::async_runtime::block_on(async {
            let pool = Pool::<Sqlite>::connect("sqlite::memory:").await.unwrap();
            cache::prepare(&pool).await.unwrap();
            // all at the same second, so a page boundary lands inside one group
            let facts: Vec<cache::Fact> = (1u8..=7)
                .map(|i| cache::Fact {
                    hash: Oid::from_bytes(&[i; 20]).unwrap().to_string(),
                    time: 100,
                    parents: Vec::new(),
                    subject: format!("commit {i}"),
                    author: "Vogt".into(),
                    body: None,
                })
                .collect();
            cache::enqueue(&pool, &facts).await.unwrap();

            let (oids, nodes, _) = read_graph(&pool, 2).await.unwrap();

            assert_eq!(oids.len(), 7);
            assert_eq!(nodes.len(), 7);
            let unique: HashSet<Oid> = oids.iter().copied().collect();
            assert_eq!(unique.len(), 7);
        });
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
