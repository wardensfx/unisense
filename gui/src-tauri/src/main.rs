#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;
mod sysinfo;

use state::AppState;

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::config_path_string,
            commands::load_config,
            commands::load_example_config,
            commands::save_config,
            commands::list_processes,
            commands::list_modules,
            commands::mem_snapshot,
            commands::mem_diff,
            commands::mem_clear_snapshots,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au demarrage de l'interface unisense");
}
