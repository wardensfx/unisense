// Pas de console en release (app tray pure) ; garde la console en debug
// pour voir les println! de diagnostic pendant le developpement.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod hotkey;
mod interception;
mod memory_watch;
mod state;
mod tray;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use unisense_core::config::Config;

use capture::{mouse_hotkeys_from_specs, MouseHotkeyAction};
use hotkey::HotkeySpec;
use state::AppState;
use tray::{keyboard_hotkeys_from_specs, HotkeyBinding};

fn main() {
    if let Err(e) = run() {
        // En mode fenetre (release) il n'y a pas de console pour lire
        // stderr : on affiche donc aussi une MessageBox pour que l'erreur
        // soit visible (ex: driver Interception absent).
        eprintln!("[unisense] erreur fatale: {e:?}");
        show_fatal_error(&format!("{e:?}"));
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let config_path = resolve_config_path();
    let config = Config::load(&config_path)
        .with_context(|| format!("chargement de la config depuis {}", config_path.display()))?;

    let game_names: Vec<String> = config.games.iter().map(|g| g.name.clone()).collect();
    let state = Arc::new(Mutex::new(AppState::from_config(&config)));
    let running = Arc::new(AtomicBool::new(true));

    // --- Resolution des hotkeys (clavier -> RegisterHotKey, souris -> capture.rs) ---
    let mut all_specs: Vec<(HotkeySpec, HotkeyAction)> = Vec::new();
    all_specs.push((
        hotkey::parse(&config.settings.toggle_hotkey).context("settings.toggle_hotkey invalide")?,
        HotkeyAction::TogglePassthrough,
    ));
    all_specs.push((
        hotkey::parse(&config.settings.cycle_game_hotkey)
            .context("settings.cycle_game_hotkey invalide")?,
        HotkeyAction::CycleGame,
    ));
    for (i, g) in config.games.iter().enumerate() {
        if let Some(spec) = &g.hotkey {
            let parsed = hotkey::parse(spec)
                .with_context(|| format!("hotkey invalide pour le jeu '{}'", g.name))?;
            all_specs.push((parsed, HotkeyAction::SelectGame(i)));
        }
    }

    let keyboard_specs: Vec<(HotkeySpec, HotkeyBinding)> = all_specs
        .iter()
        .filter(|(s, _)| matches!(s, HotkeySpec::Keyboard { .. }))
        .map(|(s, a)| (*s, a.to_tray_binding()))
        .collect();
    let mouse_specs: Vec<(HotkeySpec, MouseHotkeyAction)> = all_specs
        .iter()
        .filter(|(s, _)| matches!(s, HotkeySpec::MouseButton(_)))
        .map(|(s, a)| (*s, a.to_capture_action()))
        .collect();

    let keyboard_hotkeys = keyboard_hotkeys_from_specs(keyboard_specs);
    let mouse_hotkeys = mouse_hotkeys_from_specs(&mouse_specs);

    // --- Detection automatique memoire (optionnelle, 5.2) ---
    let auto_detect_games: Vec<(usize, unisense_core::config::AutoDetect)> = config
        .games
        .iter()
        .enumerate()
        .filter_map(|(i, g)| g.auto_detect.clone().map(|d| (i, d)))
        .collect();
    if !auto_detect_games.is_empty() {
        memory_watch::spawn_watchers(&auto_detect_games, state.clone(), running.clone());
    }

    // --- Thread de capture Interception ---
    {
        let state = state.clone();
        let running = running.clone();
        std::thread::spawn(move || {
            capture::run(state, mouse_hotkeys, running, |err| {
                eprintln!("[unisense] capture: {err}");
                show_fatal_error(&format!("Impossible de demarrer la capture souris:\n{err}"));
            });
        });
    }

    // --- Boucle UI (tray + hotkeys clavier), bloque jusqu'a "Quitter" ---
    tray::run(state, running, game_names, keyboard_hotkeys)
}

/// Action de haut niveau associee a une hotkey, independante du mecanisme
/// (clavier vs souris) qui la declenche.
#[derive(Clone, Copy)]
enum HotkeyAction {
    TogglePassthrough,
    CycleGame,
    SelectGame(usize),
}

impl HotkeyAction {
    fn to_tray_binding(self) -> HotkeyBinding {
        match self {
            HotkeyAction::TogglePassthrough => HotkeyBinding::TogglePassthrough,
            HotkeyAction::CycleGame => HotkeyBinding::CycleGame,
            HotkeyAction::SelectGame(i) => HotkeyBinding::SelectGame(i),
        }
    }

    fn to_capture_action(self) -> MouseHotkeyAction {
        match self {
            HotkeyAction::TogglePassthrough => MouseHotkeyAction::TogglePassthrough,
            HotkeyAction::CycleGame => MouseHotkeyAction::CycleGame,
            HotkeyAction::SelectGame(i) => MouseHotkeyAction::SelectGame(i),
        }
    }
}

/// Cherche `config/games.yaml` a cote de l'executable (pas du repertoire
/// courant : important pour un lancement au demarrage de Windows / raccourci).
/// Argument CLI optionnel pour surcharger.
fn resolve_config_path() -> PathBuf {
    if let Some(arg) = std::env::args().nth(1) {
        return PathBuf::from(arg);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("config").join("games.yaml")))
        .unwrap_or_else(|| PathBuf::from("config/games.yaml"))
}

fn show_fatal_error(message: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    unsafe {
        let _ = MessageBoxW(
            None,
            &HSTRING::from(message),
            &HSTRING::from("unisense - erreur"),
            MB_OK | MB_ICONERROR,
        );
    }
}
