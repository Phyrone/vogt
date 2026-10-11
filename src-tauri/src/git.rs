//! Reads git repositories with libgit2.

use std::path::{Path, PathBuf};

use git2::{ErrorCode, Repository};
use serde::Serialize;

/// One checkout of a repository: the main one, or one added with `git worktree add`.
#[derive(Debug, Serialize)]
pub struct Worktree {
    /// The directory's name.
    name: String,
    path: PathBuf,
    /// `None` while HEAD is detached or has no commit yet.
    branch: Option<String>,
    /// The worktree the listing was asked through.
    current: bool,
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
    // a linked worktree whose directory is gone cannot be opened and is left out
    let linked: Vec<Repository> = main
        .worktrees()?
        .iter()
        .flatten()
        .filter_map(|name| main.find_worktree(name).ok())
        .filter_map(|worktree| Repository::open_from_worktree(&worktree).ok())
        .collect();

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
