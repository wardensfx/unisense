//! Fenetre invisible + icone systray + hotkeys clavier globales.
//!
//! Tout ce fichier tourne sur le thread principal, qui possede la boucle de
//! messages Win32 (necessaire pour `RegisterHotKey` et `Shell_NotifyIcon`).
//! Le contexte applicatif est attache a la fenetre via GWLP_USERDATA, seule
//! facon standard de faire parvenir de l'etat a une wndproc `extern "system"`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::RegisterHotKey;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::hotkey::HotkeySpec;
use crate::state::AppState;

const WM_TRAYICON: u32 = WM_APP + 1;
const TRAY_UID: u32 = 1;

const ID_EXIT: usize = 1;
const ID_PASSTHROUGH: usize = 2;
const ID_GAME_BASE: usize = 100;

pub enum HotkeyBinding {
    TogglePassthrough,
    CycleGame,
    SelectGame(usize),
}

/// Une entree clavier a enregistrer avec `RegisterHotKey`. Les hotkeys
/// souris sont gerees ailleurs (cf. capture.rs), pas ici.
pub struct KeyboardHotkey {
    pub vk: u16,
    pub modifiers: u32,
    pub binding: HotkeyBinding,
}

pub fn keyboard_hotkeys_from_specs(specs: Vec<(HotkeySpec, HotkeyBinding)>) -> Vec<KeyboardHotkey> {
    specs
        .into_iter()
        .filter_map(|(spec, binding)| match spec {
            HotkeySpec::Keyboard { vk, modifiers } => Some(KeyboardHotkey {
                vk,
                modifiers,
                binding,
            }),
            HotkeySpec::MouseButton(_) => None,
        })
        .collect()
}

struct Context {
    state: Arc<Mutex<AppState>>,
    running: Arc<AtomicBool>,
    game_names: Vec<String>,
    hotkeys: Vec<KeyboardHotkey>,
    hwnd: HWND,
}

/// Bloque jusqu'a ce que l'utilisateur quitte (menu tray "Quitter") ou que
/// `running` passe a false (ex: erreur fatale dans le thread de capture).
pub fn run(
    state: Arc<Mutex<AppState>>,
    running: Arc<AtomicBool>,
    game_names: Vec<String>,
    hotkeys: Vec<KeyboardHotkey>,
) -> anyhow::Result<()> {
    unsafe {
        let hinstance: windows::Win32::Foundation::HINSTANCE = GetModuleHandleW(None)?.into();
        let class_name = w!("UnisenseTrayWindow");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("unisense"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            hinstance,
            None,
        )?;

        let ctx = Box::new(Context {
            state,
            running: running.clone(),
            game_names,
            hotkeys,
            hwnd,
        });

        for (i, hk) in ctx.hotkeys.iter().enumerate() {
            let id = i as i32 + 1;
            let ok = RegisterHotKey(
                hwnd,
                id,
                crate::hotkey::modifiers_as_hot_key_modifiers(hk.modifiers),
                crate::hotkey::vk_as_u32(hk.vk),
            );
            if ok.is_err() {
                eprintln!("[unisense] echec RegisterHotKey pour l'entree #{i} (deja pris par une autre appli ?)");
            }
        }

        add_tray_icon(hwnd);
        update_tooltip(hwnd, &ctx.state.lock().unwrap().mode_label());

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(ctx) as isize);

        // GetMessageW bloque indefiniment tant qu'aucun message n'arrive.
        // Un timer periodique garantit qu'on revient reevaluer `running`
        // meme sans interaction utilisateur (ex: le thread de capture a
        // rencontre une erreur fatale et a mis running=false).
        const WATCHDOG_TIMER_ID: usize = 1;
        SetTimer(hwnd, WATCHDOG_TIMER_ID, 500, None);

        let mut msg = MSG::default();
        while running.load(Ordering::SeqCst) {
            let got = GetMessageW(&mut msg, None, 0, 0);
            if got.0 <= 0 {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = KillTimer(hwnd, WATCHDOG_TIMER_ID);

        remove_tray_icon(hwnd);
        running.store(false, Ordering::SeqCst);

        // Reprend possession du Context pour le laisser se liberer proprement.
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Context;
        if !ptr.is_null() {
            drop(Box::from_raw(ptr));
        }
    }

    Ok(())
}

unsafe fn add_tray_icon(hwnd: HWND) {
    let icon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: icon,
        ..Default::default()
    };
    set_tip(&mut nid, "unisense");
    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
}

unsafe fn update_tooltip(hwnd: HWND, mode_label: &str) {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        uFlags: NIF_TIP,
        ..Default::default()
    };
    set_tip(&mut nid, &format!("unisense - {mode_label}"));
    let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
}

fn set_tip(nid: &mut NOTIFYICONDATAW, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let cap = nid.szTip.len();
    let len = wide.len().min(cap);
    nid.szTip[..len].copy_from_slice(&wide[..len]);
    if len == cap {
        nid.szTip[cap - 1] = 0; // troncature : force la terminaison NUL
    }
}

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn show_menu(ctx: &Context) {
    let Ok(menu) = CreatePopupMenu() else { return };

    let (mode_is_passthrough, active_game) = {
        let s = ctx.state.lock().unwrap();
        (
            matches!(s.mode, crate::state::Mode::Passthrough),
            match s.mode {
                crate::state::Mode::Game(i) => Some(i),
                crate::state::Mode::Passthrough => None,
            },
        )
    };

    let passthrough_flags = if mode_is_passthrough {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    let _ = AppendMenuW(
        menu,
        passthrough_flags,
        ID_PASSTHROUGH,
        w!("Passthrough (natif)"),
    );
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());

    for (i, name) in ctx.game_names.iter().enumerate() {
        let checked = active_game == Some(i);
        let flags = if checked {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        let wide = to_wide(name);
        let _ = AppendMenuW(menu, flags, ID_GAME_BASE + i, PCWSTR(wide.as_ptr()));
    }

    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, w!("Quitter"));

    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);

    // Necessaire pour que le menu se ferme correctement si l'utilisateur
    // clique ailleurs (comportement documente de TrackPopupMenu).
    let _ = SetForegroundWindow(ctx.hwnd);
    let _ = TrackPopupMenu(
        menu,
        TPM_RIGHTALIGN | TPM_BOTTOMALIGN,
        pt.x,
        pt.y,
        0,
        ctx.hwnd,
        None,
    );
    let _ = PostMessageW(ctx.hwnd, WM_NULL, WPARAM(0), LPARAM(0));

    let _ = DestroyMenu(menu);
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Context;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let ctx = &mut *ptr;

        match msg {
            WM_TRAYICON => {
                let event = lparam.0 as u32;
                match event {
                    WM_LBUTTONUP => {
                        ctx.state.lock().unwrap().toggle_passthrough();
                        let label = ctx.state.lock().unwrap().mode_label();
                        update_tooltip(hwnd, &label);
                    }
                    WM_RBUTTONUP => show_menu(ctx),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_HOTKEY => {
                let id = wparam.0 as i32;
                if let Some(hk) = ctx.hotkeys.get((id - 1) as usize) {
                    let mut s = ctx.state.lock().unwrap();
                    match &hk.binding {
                        HotkeyBinding::TogglePassthrough => s.toggle_passthrough(),
                        HotkeyBinding::CycleGame => s.cycle_game(),
                        HotkeyBinding::SelectGame(i) => s.select_game(*i),
                    }
                    drop(s);
                    let label = ctx.state.lock().unwrap().mode_label();
                    update_tooltip(hwnd, &label);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xFFFF;
                if id == ID_EXIT {
                    ctx.running.store(false, Ordering::SeqCst);
                    let _ = DestroyWindow(hwnd);
                } else if id == ID_PASSTHROUGH {
                    ctx.state.lock().unwrap().set_passthrough();
                    let label = ctx.state.lock().unwrap().mode_label();
                    update_tooltip(hwnd, &label);
                } else if id >= ID_GAME_BASE {
                    ctx.state.lock().unwrap().select_game(id - ID_GAME_BASE);
                    let label = ctx.state.lock().unwrap().mode_label();
                    update_tooltip(hwnd, &label);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
