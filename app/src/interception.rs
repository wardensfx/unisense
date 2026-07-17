//! Bindings FFI vers la bibliotheque Interception (interception.dll).
//!
//! On charge la DLL dynamiquement avec `libloading` plutot que de lier
//! statiquement contre `interception.lib`. Ca evite d'avoir a fournir le
//! fichier .lib du SDK au moment de la compilation : il suffit que
//! `interception.dll` soit presente au runtime (installee avec le driver,
//! cf. README). Les structures ci-dessous reproduisent exactement le layout
//! de `interception.h` (oblitum/Interception) : ne pas reordonner les champs.
//!
//! Ce module reproduit toute la surface de l'API C (y compris des items non
//! utilises par unisense aujourd'hui, ex: `is_keyboard`) : c'est une couche
//! de bindings, pas juste ad-hoc pour l'appli.
#![allow(dead_code)]

use anyhow::{Context, Result};
use libloading::{Library, Symbol};
use std::ffi::c_void;

pub const MAX_KEYBOARD: i32 = 10;
pub const MAX_MOUSE: i32 = 10;

pub fn keyboard(index: i32) -> i32 {
    index + 1
}

pub fn mouse(index: i32) -> i32 {
    index + 1 + MAX_KEYBOARD
}

pub type InterceptionContext = *mut c_void;
pub type InterceptionDevice = i32;
pub type InterceptionPrecedence = i32;
pub type InterceptionFilter = u16;

// On garde le filtre "tout" pour ne rien perdre : un stroke souris peut
// combiner mouvement ET etat de bouton dans la meme structure, et on a
// besoin de voir les boutons pour la detection de hotkey souris (5.1).
pub const FILTER_MOUSE_ALL: InterceptionFilter = 0xFFFF;
pub const FILTER_MOUSE_NONE: InterceptionFilter = 0x0000;

pub const MOUSE_MOVE_RELATIVE: u16 = 0x000;
pub const MOUSE_MOVE_ABSOLUTE: u16 = 0x001;

pub const MOUSE_LEFT_BUTTON_DOWN: u16 = 0x001;
pub const MOUSE_LEFT_BUTTON_UP: u16 = 0x002;
pub const MOUSE_RIGHT_BUTTON_DOWN: u16 = 0x004;
pub const MOUSE_RIGHT_BUTTON_UP: u16 = 0x008;
pub const MOUSE_MIDDLE_BUTTON_DOWN: u16 = 0x010;
pub const MOUSE_MIDDLE_BUTTON_UP: u16 = 0x020;
pub const MOUSE_BUTTON_4_DOWN: u16 = 0x040;
pub const MOUSE_BUTTON_4_UP: u16 = 0x080;
pub const MOUSE_BUTTON_5_DOWN: u16 = 0x100;
pub const MOUSE_BUTTON_5_UP: u16 = 0x200;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct InterceptionMouseStroke {
    pub state: u16,
    pub flags: u16,
    pub rolling: i16,
    pub x: i32,
    pub y: i32,
    pub information: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct InterceptionKeyStroke {
    pub code: u16,
    pub state: u16,
    pub information: u32,
}

/// Reproduit l'union C `InterceptionStroke`. Le champ actif depend du type
/// de device (verifier avec `is_mouse`/`is_keyboard` avant de lire).
#[repr(C)]
#[derive(Clone, Copy)]
pub union InterceptionStroke {
    pub mouse: InterceptionMouseStroke,
    pub key: InterceptionKeyStroke,
}

impl Default for InterceptionStroke {
    fn default() -> Self {
        InterceptionStroke {
            mouse: InterceptionMouseStroke::default(),
        }
    }
}

type FnCreateContext = unsafe extern "system" fn() -> InterceptionContext;
type FnDestroyContext = unsafe extern "system" fn(InterceptionContext);
type FnSetFilter = unsafe extern "system" fn(
    InterceptionContext,
    predicate: unsafe extern "system" fn(InterceptionDevice) -> i32,
    filter: InterceptionFilter,
);
type FnWait = unsafe extern "system" fn(InterceptionContext) -> InterceptionDevice;
type FnWaitTimeout =
    unsafe extern "system" fn(InterceptionContext, milliseconds: u32) -> InterceptionDevice;
type FnSend = unsafe extern "system" fn(
    InterceptionContext,
    InterceptionDevice,
    *const InterceptionStroke,
    u32,
) -> i32;
type FnReceive = unsafe extern "system" fn(
    InterceptionContext,
    InterceptionDevice,
    *mut InterceptionStroke,
    u32,
) -> i32;
type FnIsMouse = unsafe extern "system" fn(InterceptionDevice) -> i32;
type FnIsKeyboard = unsafe extern "system" fn(InterceptionDevice) -> i32;
type FnIsInvalid = unsafe extern "system" fn(InterceptionDevice) -> i32;

/// Predicat passe a `interception_set_filter`. Le predicat C n'a pas de
/// parametre utilisateur : on ne peut pas y capturer les pointeurs de
/// fonction charges dynamiquement, donc on reimplemente `is_mouse` a partir
/// des plages d'ID stables definies par les macros `INTERCEPTION_MOUSE(i)`
/// de interception.h (devices 11..20 = souris, 1..10 = claviers).
unsafe extern "system" fn accept_all_mice(device: InterceptionDevice) -> i32 {
    if device >= mouse(0) && device <= mouse(MAX_MOUSE - 1) {
        1
    } else {
        0
    }
}

pub struct Interception {
    _lib: Library, // doit rester en vie tant que `context` est utilise
    context: InterceptionContext,
    destroy_context: FnDestroyContext,
    set_filter: FnSetFilter,
    wait_timeout: FnWaitTimeout,
    send: FnSend,
    receive: FnReceive,
    is_mouse: FnIsMouse,
    is_keyboard: FnIsKeyboard,
    is_invalid: FnIsInvalid,
}

// Le contexte Interception (HANDLE de driver + IOCP en interne) est
// utilisable depuis n'importe quel thread ; on l'utilise depuis un seul
// thread dedie de toute facon.
unsafe impl Send for Interception {}

impl Interception {
    /// Charge `interception.dll` (doit etre dans le PATH, a cote de
    /// l'executable, ou installee dans system32 par le driver) et cree un
    /// contexte de capture.
    pub fn new() -> Result<Self> {
        unsafe {
            let lib = Library::new("interception.dll").context(
                "impossible de charger interception.dll : le driver Interception est-il installe ? (voir README)",
            )?;

            macro_rules! load {
                ($name:literal) => {{
                    let sym: Symbol<_> = lib.get($name).with_context(|| {
                        format!("symbole manquant dans interception.dll: {:?}", $name)
                    })?;
                    *sym
                }};
            }

            let create_context: FnCreateContext = load!(b"interception_create_context\0");
            let destroy_context: FnDestroyContext = load!(b"interception_destroy_context\0");
            let set_filter: FnSetFilter = load!(b"interception_set_filter\0");
            let wait_timeout: FnWaitTimeout = load!(b"interception_wait_with_timeout\0");
            let send: FnSend = load!(b"interception_send\0");
            let receive: FnReceive = load!(b"interception_receive\0");
            let is_mouse: FnIsMouse = load!(b"interception_is_mouse\0");
            let is_keyboard: FnIsKeyboard = load!(b"interception_is_keyboard\0");
            let is_invalid: FnIsInvalid = load!(b"interception_is_invalid\0");

            let context = create_context();
            if context.is_null() {
                anyhow::bail!(
                    "interception_create_context a renvoye NULL : le service driver n'est peut-etre pas demarre (execute en administrateur ?)"
                );
            }

            Ok(Self {
                _lib: lib,
                context,
                destroy_context,
                set_filter,
                wait_timeout,
                send,
                receive,
                is_mouse,
                is_keyboard,
                is_invalid,
            })
        }
    }

    /// Active la capture sur toutes les souris (predicat "accepte tout" +
    /// filtre "tous les evenements"). Les claviers ne sont pas filtres : ils
    /// continuent de fonctionner nativement, unisense ne s'en occupe pas
    /// (les hotkeys clavier passent par `RegisterHotKey`, pas par
    /// Interception). `interception_set_filter` s'applique via un predicat
    /// sur `is_mouse`, donc un seul appel couvre tous les emplacements 0..9.
    pub fn filter_all_mice(&self) {
        unsafe {
            (self.set_filter)(self.context, accept_all_mice, FILTER_MOUSE_ALL);
        }
    }

    /// Attend un evenement pendant au plus `timeout_ms`. Retourne `None` en
    /// cas de timeout (permet de verifier periodiquement les flags d'arret).
    pub fn wait(&self, timeout_ms: u32) -> Option<InterceptionDevice> {
        let device = unsafe { (self.wait_timeout)(self.context, timeout_ms) };
        if unsafe { (self.is_invalid)(device) } != 0 {
            None
        } else {
            Some(device)
        }
    }

    pub fn is_mouse(&self, device: InterceptionDevice) -> bool {
        unsafe { (self.is_mouse)(device) != 0 }
    }

    pub fn is_keyboard(&self, device: InterceptionDevice) -> bool {
        unsafe { (self.is_keyboard)(device) != 0 }
    }

    pub fn receive(&self, device: InterceptionDevice, stroke: &mut InterceptionStroke) -> bool {
        unsafe { (self.receive)(self.context, device, stroke as *mut _, 1) == 1 }
    }

    pub fn send(&self, device: InterceptionDevice, stroke: &InterceptionStroke) {
        unsafe {
            (self.send)(self.context, device, stroke as *const _, 1);
        }
    }
}

impl Drop for Interception {
    fn drop(&mut self) {
        unsafe {
            (self.destroy_context)(self.context);
        }
    }
}
