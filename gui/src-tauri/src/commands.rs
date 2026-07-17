//! Commandes exposees au frontend (`invoke("...")` cote JS).

use std::path::PathBuf;

use tauri::State;
use unisense_core::config::Config;

use crate::state::AppState;
use crate::sysinfo::{self, DiffEntry, ProcessEntry};

/// Config d'exemple embarquee au moment de la compilation : sert de point
/// de depart ("config de base") sans dependre du reseau ni d'un chemin
/// relatif fragile au runtime.
const EXAMPLE_CONFIG_YAML: &str = include_str!("../../../config/games.example.yaml");

/// Meme convention que l'app tray : `config/games.yaml` a cote de
/// l'executable. Les deux binaires (tray et GUI) sont destines a etre
/// deployes cote a cote et a partager le meme fichier.
fn config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("config").join("games.yaml")))
        .unwrap_or_else(|| PathBuf::from("config/games.yaml"))
}

#[tauri::command]
pub fn config_path_string() -> String {
    config_path().display().to_string()
}

#[tauri::command]
pub fn load_config() -> Result<Config, String> {
    Config::load(&config_path()).map_err(|e| format!("{e:?}"))
}

#[tauri::command]
pub fn load_example_config() -> Result<Config, String> {
    serde_yaml::from_str(EXAMPLE_CONFIG_YAML).map_err(|e| format!("config d'exemple invalide: {e}"))
}

#[tauri::command]
pub fn save_config(config: Config) -> Result<(), String> {
    if config.games.is_empty() {
        return Err("ajoutez au moins un jeu avant d'enregistrer".to_string());
    }
    config.save(&config_path()).map_err(|e| format!("{e:?}"))
}

#[tauri::command]
pub fn list_processes() -> Result<Vec<ProcessEntry>, String> {
    sysinfo::list_processes().map_err(|e| format!("{e:?}"))
}

#[tauri::command]
pub fn list_modules(pid: u32) -> Result<Vec<String>, String> {
    sysinfo::list_modules(pid).map_err(|e| format!("{e:?}"))
}

#[tauri::command]
pub fn mem_snapshot(state: State<AppState>, pid: u32) -> Result<String, String> {
    let snap = sysinfo::take_snapshot(pid).map_err(|e| format!("{e:?}"))?;
    Ok(state.store_snapshot(snap))
}

#[tauri::command]
pub fn mem_diff(
    state: State<AppState>,
    snapshot_a: String,
    snapshot_b: String,
) -> Result<Vec<DiffEntry>, String> {
    state
        .with_two_snapshots(&snapshot_a, &snapshot_b, |a, b| {
            sysinfo::diff_snapshots(a, b)
        })
        .ok_or_else(|| "snapshot introuvable (deja efface ?)".to_string())
}

#[tauri::command]
pub fn mem_clear_snapshots(state: State<AppState>) {
    state.clear_snapshots();
}
