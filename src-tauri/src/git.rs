//! Reads git repositories with libgit2.

use std::path::{Path, PathBuf};

use git2::{ErrorCode, Repository, RepositoryInitOptions};
use serde::Serialize;
use ts_rs::TS;

/// One checkout of a repository: the main one, or one added with `git worktree add`.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Worktree {
    /// The directory's name.
    name: String,
    #[ts(type = "string")]
    path: PathBuf,
    /// `None` while HEAD is detached or has no commit yet.
    branch: Option<String>,
    /// The worktree the listing was asked through.
    current: bool,
}

/// Every worktree of the repository at `path`, the main one first, as opened
/// repositories. `path` may be any of them; a linked worktree whose directory
/// is gone is left out. A directory that is not a repository has none.
pub(crate) fn worktree_repos(path: &Path) -> Result<Vec<Repository>, git2::Error> {
    let main = match Repository::open(path) {
        Ok(repo) => Repository::open(repo.commondir())?,
        Err(e) if e.code() == ErrorCode::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let linked = linked_worktree_repos(&main)?;
    Ok(std::iter::once(main).chain(linked).collect())
}

/// The linked worktrees of `main`, as opened repositories; a worktree whose
/// directory is gone cannot be opened and is left out.
fn linked_worktree_repos(main: &Repository) -> Result<Vec<Repository>, git2::Error> {
    Ok(main
        .worktrees()?
        .iter()
        .flatten()
        .filter_map(|name| main.find_worktree(name).ok())
        .filter_map(|worktree| Repository::open_from_worktree(&worktree).ok())
        .collect())
}

/// Every worktree of the repository at `path`, the main one first. `path` may
/// be any of them. A directory that is not a repository has none.
fn worktrees(path: &Path) -> Result<Vec<Worktree>, git2::Error> {
    let opened = match Repository::open(path) {
        Ok(repo) => repo,
        Err(e) if e.code() == ErrorCode::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let main = Repository::open(opened.commondir())?;
    let linked = linked_worktree_repos(&main)?;

    Ok(std::iter::once(main)
        .chain(linked)
        .filter_map(|repo| {
            // a bare main repository has no checkout of its own
            let dir = repo.workdir()?;
            Some(Worktree {
                name: dir.file_name()?.to_string_lossy().into_owned(),
                path: dir.into(),
                branch: repo
                    .head()
                    .ok()
                    .filter(|head| head.is_branch())
                    .and_then(|head| head.shorthand().map(Into::into)),
                current: repo.path() == opened.path(),
            })
        })
        .collect())
}

#[tauri::command(async)]
pub fn list_worktrees(path: PathBuf) -> Result<Vec<Worktree>, String> {
    worktrees(&path).map_err(|e| e.message().into())
}

/// Creates the directory `name` inside `parent` and starts a repository in it.
/// An existing directory is only taken over while it is empty.
fn create(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err("Give the repository a name without slashes.".into());
    }
    let path = parent.join(name);
    let taken = path.is_file() || path.read_dir().is_ok_and(|mut dir| dir.next().is_some());
    if taken {
        return Err(format!(
            "{} already exists and is not empty. Choose another name or location.",
            path.display()
        ));
    }
    std::fs::create_dir_all(&path)
        .map_err(|e| format!("Could not create {}: {e}", path.display()))?;
    Repository::init_opts(&path, RepositoryInitOptions::new().initial_head("main"))
        .map_err(|e| e.message().to_string())?;
    Ok(path)
}

#[tauri::command(async)]
pub fn create_repository(parent: PathBuf, name: String) -> Result<PathBuf, String> {
    create(&parent, &name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system's temporary one.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vogt-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn summary(path: &Path) -> Vec<(String, Option<String>, bool)> {
        worktrees(path)
            .unwrap()
            .into_iter()
            .map(|w| (w.name, w.branch, w.current))
            .collect()
    }

    #[test]
    fn a_directory_that_is_no_repository_has_no_worktrees() {
        let dir = scratch("plain");
        assert!(worktrees(&dir).unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_new_repository_starts_on_main_in_its_own_directory() {
        let dir = scratch("create");

        let path = create(&dir, " fresh ").unwrap();

        assert_eq!(path, dir.join("fresh"));
        let repo = Repository::open(&path).unwrap();
        let head = repo.find_reference("HEAD").unwrap();
        assert_eq!(head.symbolic_target(), Some("refs/heads/main"));
        // it now holds a .git directory, so it is no longer free to take
        assert!(create(&dir, "fresh").is_err());
        for name in ["", "..", "a/b"] {
            assert!(create(&dir, name).is_err(), "{name:?} is not a name");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn worktrees_are_listed_from_any_of_them() {
        let dir = scratch("worktrees");
        let repo = Repository::init(dir.join("main")).unwrap();
        let signature = git2::Signature::now("Vogt", "vogt@example.com").unwrap();
        let tree = repo
            .find_tree(repo.treebuilder(None).unwrap().write().unwrap())
            .unwrap();
        repo.commit(Some("HEAD"), &signature, &signature, "init", &tree, &[])
            .unwrap();
        let trunk = repo.head().unwrap().shorthand().unwrap().to_owned();
        repo.worktree("feature", &dir.join("feature"), None)
            .unwrap();

        let main = (String::from("main"), Some(trunk), true);
        let feature = (
            String::from("feature"),
            Some(String::from("feature")),
            false,
        );
        assert_eq!(summary(&dir.join("main")), [main.clone(), feature.clone()]);
        assert_eq!(
            summary(&dir.join("feature")),
            [(main.0, main.1, false), (feature.0, feature.1, true)]
        );

        // a worktree deleted from disk without telling git
        std::fs::remove_dir_all(dir.join("feature")).unwrap();
        assert_eq!(summary(&dir.join("main")).len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
