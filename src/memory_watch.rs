//! Detection automatique menu/gameplay par lecture memoire (point 5.2 du
//! cahier des charges), EXPERIMENTAL.
//!
//! Contrairement au reste de l'outil, cette fonctionnalite est intrinsequement
//! fragile : elle depend d'une adresse memoire specifique a une version
//! precise d'un jeu, qui casse au moindre patch. Elle est opt-in par jeu :
//! seuls les jeux dont l'entree YAML contient un bloc `auto_detect` sont
//! surveilles (aucun jeu ne l'a par defaut). unisense NE FOURNIT AUCUN
//! offset pre-rempli pour un jeu quelconque : c'est a l'utilisateur de
//! les trouver lui-meme (Cheat Engine ou equivalent, cf. README) et de les
//! placer dans le bloc `auto_detect` de son jeu dans la config.
//!
//! Le principe : on lit periodiquement un octet (ou une petite sequence) a
//! une adresse `base_module + offset [+ chaine de pointeurs]`, et on compare
//! au pattern `in_game_bytes` attendu en gameplay. Toute autre valeur bascule
//! sur passthrough. Ceci ne modifie rien dans le process cible (lecture
//! seule, ReadProcessMemory).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
    MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_VM_READ};

use crate::config::AutoDetect;
use crate::state::AppState;

pub fn spawn_watchers(
    games: &[(usize, AutoDetect)],
    state: Arc<Mutex<AppState>>,
    running: Arc<AtomicBool>,
) {
    for (index, detect) in games.iter().cloned() {
        let state = state.clone();
        let running = running.clone();
        std::thread::spawn(move || watch_loop(index, detect, state, running));
    }
}

fn watch_loop(game_index: usize, detect: AutoDetect, state: Arc<Mutex<AppState>>, running: Arc<AtomicBool>) {
    let poll = Duration::from_millis(detect.poll_interval_ms.max(50));
    let mut was_in_game = false;

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(poll);

        let in_game = read_state(&detect).unwrap_or(false);
        if in_game == was_in_game {
            continue;
        }
        was_in_game = in_game;

        let mut s = state.lock().unwrap();
        if in_game {
            s.select_game(game_index);
        } else {
            s.set_passthrough();
        }
    }
}

fn read_state(detect: &AutoDetect) -> Option<bool> {
    let pid = find_pid_by_name(&detect.process_name)?;
    let module_base = find_module_base(pid, detect.module_name.as_deref())?;

    let process = unsafe { OpenProcess(PROCESS_VM_READ, false, pid).ok()? };
    let _guard = HandleGuard(process);

    let mut address = module_base + detect.offset;
    for step_offset in &detect.pointer_chain {
        let mut ptr_buf = [0u8; 8];
        if !read_bytes(process, address, &mut ptr_buf) {
            return None;
        }
        address = u64::from_le_bytes(ptr_buf) + step_offset;
    }

    let mut buf = vec![0u8; detect.in_game_bytes.len()];
    if !read_bytes(process, address, &mut buf) {
        return None;
    }

    Some(buf == detect.in_game_bytes)
}

struct HandleGuard(HANDLE);
impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn read_bytes(process: HANDLE, address: u64, out: &mut [u8]) -> bool {
    let mut read = 0usize;
    unsafe {
        ReadProcessMemory(
            process,
            address as *const _,
            out.as_mut_ptr() as *mut _,
            out.len(),
            Some(&mut read as *mut usize),
        )
        .is_ok()
            && read == out.len()
    }
}

fn find_pid_by_name(name: &str) -> Option<u32> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let _guard = HandleGuard(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_err() {
            return None;
        }
        loop {
            let exe_name = wide_to_string(&entry.szExeFile);
            if exe_name.eq_ignore_ascii_case(name) {
                return Some(entry.th32ProcessID);
            }
            if Process32NextW(snapshot, &mut entry).is_err() {
                return None;
            }
        }
    }
}

fn find_module_base(pid: u32, module_name: Option<&str>) -> Option<u64> {
    unsafe {
        let snapshot =
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid).ok()?;
        let _guard = HandleGuard(snapshot);
        let mut entry = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        if Module32FirstW(snapshot, &mut entry).is_err() {
            return None;
        }
        loop {
            let name = wide_to_string(&entry.szModule);
            let is_match = match module_name {
                Some(wanted) => name.eq_ignore_ascii_case(wanted),
                // Sans nom precise, le premier module de l'enumeration est
                // toujours l'executable principal.
                None => true,
            };
            if is_match {
                return Some(entry.modBaseAddr as u64);
            }
            if Module32NextW(snapshot, &mut entry).is_err() {
                return None;
            }
        }
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}
