mod registry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(registry::Store::new())
        .invoke_handler(tauri::generate_handler![
            registry::list_workspaces,
            registry::create_workspace,
            registry::add_repo,
            registry::remove_repo
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
