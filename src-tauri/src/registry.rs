//! The workspaces and repositories the app knows about.
//!
//! Ids are generated here and never derived from a path or a name, so a
//! repository keeps its id when it is moved or renamed.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use nanoid::nanoid;
use serde::Serialize;
use tauri::State;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Repo {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

/// A workspace as the frontend sees it, with its repositories resolved.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub repos: Vec<Repo>,
}

struct WorkspaceRecord {
    id: String,
    name: String,
    /// Ids of the repositories in the workspace, in display order.
    repos: Vec<String>,
}

/// Repositories are stored once and referenced by id, so one repository can
/// sit in several workspaces.
#[derive(Default)]
pub struct Registry {
    workspaces: Vec<WorkspaceRecord>,
    repos: Vec<Repo>,
}

impl Registry {
    pub fn workspaces(&self) -> Vec<Workspace> {
        self.workspaces.iter().map(|w| self.resolve(w)).collect()
    }

    pub fn create_workspace(&mut self, name: &str) -> Result<Workspace, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("A workspace needs a name".into());
        }
        let record = WorkspaceRecord {
            id: nanoid!(),
            name: name.into(),
            repos: Vec::new(),
        };
        let workspace = self.resolve(&record);
        self.workspaces.push(record);
        Ok(workspace)
    }

    /// Adds the directory at `path` to a workspace. A path that is already
    /// registered keeps its id.
    pub fn add_repo(&mut self, workspace: &str, path: &Path) -> Result<Repo, String> {
        if !path.is_dir() {
            return Err(format!("{} is not a directory", path.display()));
        }
        let index = self.workspace_index(workspace)?;
        let repo = match self.repos.iter().find(|r| r.path == path) {
            Some(repo) => repo.clone(),
            None => {
                let repo = Repo {
                    id: nanoid!(),
                    name: path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string()),
                    path: path.into(),
                };
                self.repos.push(repo.clone());
                repo
            }
        };
        let members = &mut self.workspaces[index].repos;
        if !members.contains(&repo.id) {
            members.push(repo.id.clone());
        }
        Ok(repo)
    }

    /// Takes a repository out of a workspace, and forgets it once no
    /// workspace holds it. Nothing on disk is touched.
    pub fn remove_repo(&mut self, workspace: &str, repo: &str) -> Result<(), String> {
        let index = self.workspace_index(workspace)?;
        self.workspaces[index].repos.retain(|id| id != repo);
        if !self
            .workspaces
            .iter()
            .any(|w| w.repos.iter().any(|id| id == repo))
        {
            self.repos.retain(|r| r.id != repo);
        }
        Ok(())
    }

    fn workspace_index(&self, id: &str) -> Result<usize, String> {
        self.workspaces
            .iter()
            .position(|w| w.id == id)
            .ok_or_else(|| "This workspace no longer exists".into())
    }

    fn resolve(&self, record: &WorkspaceRecord) -> Workspace {
        Workspace {
            id: record.id.clone(),
            name: record.name.clone(),
            repos: record
                .repos
                .iter()
                .filter_map(|id| self.repos.iter().find(|r| &r.id == id).cloned())
                .collect(),
        }
    }
}

/// The registry as Tauri state. It lives in memory for now; persistence
/// belongs behind these same commands.
pub struct Store(Mutex<Registry>);

impl Store {
    /// A registry with one empty workspace, so there is always one to show.
    pub fn new() -> Self {
        let mut registry = Registry::default();
        registry
            .create_workspace("Default")
            .expect("the name is not empty");
        Self(Mutex::new(registry))
    }

    fn with<T>(&self, run: impl FnOnce(&mut Registry) -> T) -> T {
        run(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

#[tauri::command]
pub fn list_workspaces(store: State<'_, Store>) -> Vec<Workspace> {
    store.with(|registry| registry.workspaces())
}

#[tauri::command]
pub fn create_workspace(store: State<'_, Store>, name: String) -> Result<Workspace, String> {
    store.with(|registry| registry.create_workspace(&name))
}

#[tauri::command]
pub fn add_repo(store: State<'_, Store>, workspace: String, path: PathBuf) -> Result<Repo, String> {
    store.with(|registry| registry.add_repo(&workspace, &path))
}

#[tauri::command]
pub fn remove_repo(store: State<'_, Store>, workspace: String, repo: String) -> Result<(), String> {
    store.with(|registry| registry.remove_repo(&workspace, &repo))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory that exists wherever the tests run.
    fn dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn a_workspace_needs_a_name() {
        assert!(Registry::default().create_workspace("  ").is_err());
    }

    #[test]
    fn a_repository_is_named_after_its_directory() {
        let mut registry = Registry::default();
        let workspace = registry.create_workspace("Work").unwrap();

        let repo = registry.add_repo(&workspace.id, &dir()).unwrap();

        assert_eq!(repo.name, "src-tauri");
        assert_eq!(registry.workspaces()[0].repos, [repo]);
    }

    #[test]
    fn only_directories_can_be_added() {
        let mut registry = Registry::default();
        let workspace = registry.create_workspace("Work").unwrap();

        assert!(
            registry
                .add_repo(&workspace.id, &dir().join("Cargo.toml"))
                .is_err()
        );
    }

    #[test]
    fn a_path_keeps_its_id_across_workspaces() {
        let mut registry = Registry::default();
        let work = registry.create_workspace("Work").unwrap();
        let play = registry.create_workspace("Play").unwrap();

        let first = registry.add_repo(&work.id, &dir()).unwrap();
        let again = registry.add_repo(&work.id, &dir()).unwrap();
        let second = registry.add_repo(&play.id, &dir()).unwrap();

        assert_eq!(first.id, again.id);
        assert_eq!(first.id, second.id);
        assert_eq!(registry.workspaces()[0].repos.len(), 1);
    }

    #[test]
    fn a_repository_is_forgotten_with_its_last_workspace() {
        let mut registry = Registry::default();
        let work = registry.create_workspace("Work").unwrap();
        let play = registry.create_workspace("Play").unwrap();
        let repo = registry.add_repo(&work.id, &dir()).unwrap();
        registry.add_repo(&play.id, &dir()).unwrap();

        registry.remove_repo(&work.id, &repo.id).unwrap();
        assert_eq!(registry.repos.len(), 1);
        assert!(registry.workspaces()[0].repos.is_empty());

        registry.remove_repo(&play.id, &repo.id).unwrap();
        assert!(registry.repos.is_empty());
    }
}
