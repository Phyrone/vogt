//! Reads a repository's history in the shape the line map draws.
//!
//! Git does not record which branch a commit was made on. Every commit is
//! given a **line** here: a branch claims the commits on its first-parent
//! chain that no earlier branch claimed, the trunk going first. What is left
//! came in through a merge from a branch that no longer exists; each such
//! chain becomes a line of its own, marked as merged.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use git2::{BranchType, Oid, Patch, Repository, Sort};
use serde::Serialize;

// shortcut: the whole list is read and laid out in one go, so it is capped; page it to go further
const LIMIT: usize = 20_000;
// shortcut: older remote commits than this count as not pushed, replace with merge-base checks per line
const PUSHED_LIMIT: usize = 100_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    /// `user.name` from the git configuration.
    user: String,
    /// Id of the line history is laid out around.
    trunk: String,
    /// Id of the checked-out line.
    head_line: String,
    /// Hash HEAD points at; empty before the first commit.
    head: String,
    lines: Vec<Line>,
    /// Newest first.
    commits: Vec<Commit>,
    stashes: Vec<String>,
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Line {
    id: String,
    name: String,
    /// Has a counterpart on a remote.
    remote: bool,
    /// Merged and deleted: it has commits but no branch any more.
    merged: bool,
}

#[derive(Debug, Serialize)]
struct Commit {
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

#[derive(Debug, Serialize)]
struct Ref {
    kind: &'static str,
    label: String,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct FileChange {
    path: PathBuf,
    added: usize,
    removed: usize,
}

fn history(path: &Path) -> Result<History, git2::Error> {
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

    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    for (_, tip, _) in &branches {
        walk.push(*tip)?;
    }
    if let Some(head) = head {
        walk.push(head)?;
    }
    let listed: Vec<git2::Commit> = walk
        .take(LIMIT)
        .map(|oid| repo.find_commit(oid?))
        .collect::<Result<_, _>>()?;
    let by_id: HashMap<Oid, &git2::Commit> = listed.iter().map(|c| (c.id(), c)).collect();

    let mut line_of: HashMap<Oid, String> = HashMap::new();
    let claim = |line_of: &mut HashMap<Oid, String>, from: Oid, line: &str| {
        let mut next = by_id.get(&from);
        while let Some(commit) = next {
            if line_of.contains_key(&commit.id()) {
                break;
            }
            line_of.insert(commit.id(), line.to_owned());
            next = commit.parent_id(0).ok().and_then(|id| by_id.get(&id));
        }
    };

    let mut lines = Vec::new();
    for (name, tip, remote) in &branches {
        claim(&mut line_of, *tip, name);
        lines.push(Line {
            id: name.clone(),
            name: name.clone(),
            remote: *remote,
            merged: false,
        });
    }
    let head_line = match (&head_branch, head) {
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
            claim(&mut line_of, head, "HEAD");
            if line_of[&head] == "HEAD" {
                lines.push(Line {
                    id: "HEAD".into(),
                    name: "detached HEAD".into(),
                    remote: false,
                    merged: false,
                });
            }
            line_of[&head].clone()
        }
        (None, None) => trunk.clone(),
    };

    // children come before their parents, so a merge is seen before what it merged
    let mut merged_by: HashMap<Oid, &git2::Commit> = HashMap::new();
    for commit in &listed {
        if !line_of.contains_key(&commit.id()) {
            let id = format!("merged-{}", commit.id());
            let name = merged_by
                .get(&commit.id())
                .and_then(|merge| merged_branch(merge.summary()?))
                .unwrap_or_else(|| commit.id().to_string()[..7].to_owned());
            claim(&mut line_of, commit.id(), &id);
            lines.push(Line {
                id,
                name,
                remote: false,
                merged: true,
            });
        }
        for parent in commit.parent_ids().skip(1) {
            merged_by.entry(parent).or_insert(commit);
        }
    }

    let mut refs: HashMap<Oid, Vec<Ref>> = HashMap::new();
    let mut tags = Vec::new();
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
            ("remote", label)
        } else {
            continue;
        };
        refs.entry(commit.id()).or_default().push(Ref {
            kind,
            label: label.to_owned(),
        });
    }

    let mut remote_walk = repo.revwalk()?;
    remote_walk.push_glob("refs/remotes/*")?;
    let pushed: HashSet<Oid> = remote_walk.take(PUSHED_LIMIT).flatten().collect();

    let commits = listed
        .iter()
        .map(|commit| Commit {
            hash: commit.id().to_string(),
            subject: commit.summary().unwrap_or_default().to_owned(),
            body: commit.body().map(str::to_owned),
            author: commit.author().name().unwrap_or_default().to_owned(),
            time: commit.time().seconds(),
            line: line_of[&commit.id()].clone(),
            parents: commit.parent_ids().map(|id| id.to_string()).collect(),
            refs: refs.remove(&commit.id()).unwrap_or_default(),
            pushed: pushed.contains(&commit.id()),
        })
        .collect();

    Ok(History {
        user: repo
            .config()
            .and_then(|config| config.get_string("user.name"))
            .unwrap_or_default(),
        trunk,
        head_line,
        head: head.map(|id| id.to_string()).unwrap_or_default(),
        lines,
        commits,
        stashes,
        tags,
    })
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

#[tauri::command(async)]
pub fn read_history(path: PathBuf) -> Result<History, String> {
    history(&path).map_err(|e| match e.code() {
        git2::ErrorCode::NotFound => "This folder is not a git repository.".into(),
        _ => e.message().to_owned(),
    })
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
        Repository::init_opts(
            &dir,
            git2::RepositoryInitOptions::new().initial_head("main"),
        )
        .unwrap();

        let history = history(&dir).unwrap();

        assert_eq!(
            (history.trunk.as_str(), history.head_line.as_str()),
            ("main", "main")
        );
        assert_eq!(history.head, "");
        assert_eq!(history.lines.len(), 1);
        assert!(history.commits.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn every_commit_gets_a_line() {
        let dir = scratch("lines");
        let repo = Repository::init_opts(
            &dir,
            git2::RepositoryInitOptions::new().initial_head("main"),
        )
        .unwrap();
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

        let history = history(&dir).unwrap();

        assert_eq!(history.trunk, "main");
        assert_eq!(history.head_line, "feature");
        assert_eq!(history.head, feature.to_string());
        assert_eq!(history.tags, ["v1"]);
        let lines: Vec<(&str, bool)> = history
            .lines
            .iter()
            .map(|l| (l.name.as_str(), l.merged))
            .collect();
        assert_eq!(
            lines,
            [("main", false), ("feature", false), ("topic", true)]
        );

        // newest first, a merge before what it merged
        assert_eq!(history.commits[0].hash, feature.to_string());
        assert_eq!(history.commits[1].hash, merge.to_string());
        assert_eq!(history.commits[4].hash, root.to_string());
        let line = |id: Oid| {
            let commit = history
                .commits
                .iter()
                .find(|c| c.hash == id.to_string())
                .unwrap();
            commit.line.as_str()
        };
        assert_eq!(line(feature), "feature");
        assert_eq!([line(merge), line(second), line(root)], ["main"; 3]);
        assert_eq!(line(topic), format!("merged-{topic}"));

        let merged = &history.commits[1];
        assert_eq!(merged.parents, [second.to_string(), topic.to_string()]);
        assert_eq!(merged.refs[0].label, "v1");
        assert!(!merged.pushed);
        assert_eq!(history.commits[4].body.as_deref(), Some("why it exists"));

        let change = |added, removed| FileChange {
            path: "file.txt".into(),
            added,
            removed,
        };
        assert_eq!(files(&dir, &root.to_string()).unwrap(), [change(1, 0)]);
        assert_eq!(files(&dir, &topic.to_string()).unwrap(), [change(2, 0)]);
        assert_eq!(files(&dir, &feature.to_string()).unwrap(), [change(1, 3)]);
        assert!(files(&dir, &merge.to_string()).unwrap().is_empty());
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
