use tauri::App;
use tauri_plugin_sql::Migration;

mod cache;
mod git;
mod history;
mod index;
mod layout;

/// The database's name for tauri-plugin-sql; `src/lib/db/index.ts` loads the same one.
const DATABASE: &str = "sqlite:vogt.db";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_stronghold::Builder::new(|pass| todo!()).build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations(DATABASE, migrations())
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(index::Repos::default())
        .setup(init_app)
        .invoke_handler(tauri::generate_handler![
            git::list_worktrees,
            git::create_repository,
            history::open_history,
            history::materialize_view,
            history::commit_files,
            index::sync_repositories,
            index::forget_repo,
            index::rebuild_caches
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_app<R: tauri::Runtime>(app: &mut App<R>) -> Result<(), Box<dyn std::error::Error>> {
    // the tag only tells other tools to leave the caches alone; losing it is
    // not worth failing the start over
    if let Err(failure) = cache::tag(app.handle()) {
        eprintln!("vogt: could not tag the cache directory: {failure}");
    }
    Ok(())
}

/// The migrations `pnpm db:generate` writes to `drizzle/`, applied when the
/// frontend opens the database.
fn migrations() -> Vec<Migration> {
    vogt_macros::sql_migrations!("../drizzle")
}

/// The migrations `pnpm db:repo:generate` writes to `drizzle-repo/`, applied
/// by `cache::prepare` when a repository's cache file is created.
pub fn repo_migrations() -> Vec<Migration> {
    vogt_macros::sql_migrations!("../drizzle-repo")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_numbered(dir: &str, migrations: Vec<Migration>) {
        let generated = std::fs::read_dir(format!("{}/../{dir}", env!("CARGO_MANIFEST_DIR")))
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().path().extension() == Some("sql".as_ref()))
            .count();
        let versions: Vec<i64> = migrations.iter().map(|m| m.version).collect();

        assert_eq!(versions, (1..=generated as i64).collect::<Vec<_>>());
    }

    #[test]
    fn every_generated_migration_is_included() {
        assert_numbered("drizzle", migrations());
    }

    #[test]
    fn every_repo_migration_is_included() {
        assert_numbered("drizzle-repo", repo_migrations());
    }
}
