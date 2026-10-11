use tauri::App;
use tauri_plugin_sql::Migration;

mod git;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_stronghold::Builder::new(|pass| todo!()).build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:vogt.db", migrations())
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(init_app)
        .invoke_handler(tauri::generate_handler![
            git::list_worktrees,
            git::create_repository
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_app<R: tauri::Runtime>(app: &mut App<R>) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

/// The migrations `pnpm db:generate` writes to `drizzle/`, applied when the
/// frontend opens the database.
fn migrations() -> Vec<Migration> {
    vogt_macros::sql_migrations!("../drizzle")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_generated_migration_is_included() {
        let generated = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../drizzle"))
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().path().extension() == Some("sql".as_ref()))
            .count();
        let versions: Vec<i64> = migrations().iter().map(|m| m.version).collect();

        assert_eq!(versions, (1..=generated as i64).collect::<Vec<_>>());
    }
}
