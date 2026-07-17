//! Parsing des specs de hotkey ("Ctrl+Alt+F9", "Mouse4", ...) vers soit un
//! hotkey clavier global (RegisterHotKey, gere par la boucle de messages
//! Win32 dans main.rs), soit un bouton souris (detecte directement dans la
//! boucle de capture Interception, cf. capture.rs).
//!
//! Pourquoi deux mecanismes distincts : `RegisterHotKey` ne recoit pas les
//! clics souris (documentation Win32), donc un hotkey "Mouse4" ne peut etre
//! detecte qu'en inspectant nous-memes les strokes bruts que l'on intercepte
//! deja pour le scaling.

use anyhow::{bail, Result};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
};

use crate::interception;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeySpec {
    Keyboard { vk: u16, modifiers: u32 },
    MouseButton(MouseButton),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}

impl MouseButton {
    /// Retourne true si `state` (champ InterceptionMouseStroke::state)
    /// contient un front descendant (bouton presse) pour ce bouton.
    pub fn is_down_in(&self, state: u16) -> bool {
        let mask = match self {
            MouseButton::Left => interception::MOUSE_LEFT_BUTTON_DOWN,
            MouseButton::Right => interception::MOUSE_RIGHT_BUTTON_DOWN,
            MouseButton::Middle => interception::MOUSE_MIDDLE_BUTTON_DOWN,
            MouseButton::Button4 => interception::MOUSE_BUTTON_4_DOWN,
            MouseButton::Button5 => interception::MOUSE_BUTTON_5_DOWN,
        };
        state & mask != 0
    }
}

pub fn parse(spec: &str) -> Result<HotkeySpec> {
    let parts: Vec<&str> = spec.split('+').map(|s| s.trim()).collect();
    let Some((&key, mods)) = parts.split_last() else {
        bail!("hotkey vide");
    };

    if let Some(button) = parse_mouse_button(key) {
        if !mods.is_empty() {
            bail!("les modificateurs (Ctrl/Alt/Shift) ne sont pas supportes pour les boutons souris: {spec}");
        }
        return Ok(HotkeySpec::MouseButton(button));
    }

    let vk = parse_vk(key).ok_or_else(|| anyhow::anyhow!("touche inconnue dans hotkey: {key}"))?;

    let mut modifiers: u32 = MOD_NOREPEAT.0 as u32;
    for m in mods {
        modifiers |= match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => MOD_CONTROL.0 as u32,
            "alt" => MOD_ALT.0 as u32,
            "shift" => MOD_SHIFT.0 as u32,
            "win" | "meta" => MOD_WIN.0 as u32,
            other => bail!("modificateur inconnu dans hotkey '{spec}': {other}"),
        };
    }

    Ok(HotkeySpec::Keyboard { vk, modifiers })
}

fn parse_mouse_button(key: &str) -> Option<MouseButton> {
    Some(match key.to_ascii_lowercase().as_str() {
        "mouseleft" | "mouse1" => MouseButton::Left,
        "mouseright" | "mouse2" => MouseButton::Right,
        "mousemiddle" | "mouse3" => MouseButton::Middle,
        "mouse4" | "mousex1" | "mouseback" => MouseButton::Button4,
        "mouse5" | "mousex2" | "mouseforward" => MouseButton::Button5,
        _ => return None,
    })
}

/// Traduit un nom de touche humain vers un code VK Win32. Couvre F1-F24,
/// les lettres/chiffres, et quelques touches courantes. Etendre au besoin.
fn parse_vk(key: &str) -> Option<u16> {
    let upper = key.to_ascii_uppercase();

    if let Some(n) = upper.strip_prefix('F') {
        if let Ok(n) = n.parse::<u16>() {
            if (1..=24).contains(&n) {
                // VK_F1 = 0x70, F2 = 0x71, ...
                return Some(0x70 + (n - 1));
            }
        }
    }

    if upper.len() == 1 {
        let c = upper.chars().next().unwrap();
        if c.is_ascii_alphanumeric() {
            // VK des lettres/chiffres correspond a leur code ASCII majuscule.
            return Some(c as u16);
        }
    }

    let named = match upper.as_str() {
        "SPACE" => 0x20,
        "TAB" => 0x09,
        "ESC" | "ESCAPE" => 0x1B,
        "ENTER" | "RETURN" => 0x0D,
        "INSERT" => 0x2D,
        "DELETE" | "DEL" => 0x2E,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" => 0x21,
        "PAGEDOWN" => 0x22,
        "CAPSLOCK" => 0x14,
        "SCROLLLOCK" => 0x91,
        "PAUSE" => 0x13,
        "NUMLOCK" => 0x90,
        _ => return None,
    };
    Some(named)
}

/// `RegisterHotKey` attend le code VK sous forme de `u32` brut (pas le
/// newtype `VIRTUAL_KEY`, qui sert a d'autres API comme `keybd_event`).
pub fn vk_as_u32(vk: u16) -> u32 {
    vk as u32
}

pub fn modifiers_as_hot_key_modifiers(modifiers: u32) -> HOT_KEY_MODIFIERS {
    HOT_KEY_MODIFIERS(modifiers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_function_key() {
        assert_eq!(
            parse("F9").unwrap(),
            HotkeySpec::Keyboard {
                vk: 0x78,
                modifiers: MOD_NOREPEAT.0 as u32
            }
        );
    }

    #[test]
    fn parses_modifier_combo() {
        match parse("Ctrl+Alt+F9").unwrap() {
            HotkeySpec::Keyboard { vk, modifiers } => {
                assert_eq!(vk, 0x78);
                assert_eq!(modifiers & MOD_CONTROL.0 as u32, MOD_CONTROL.0 as u32);
                assert_eq!(modifiers & MOD_ALT.0 as u32, MOD_ALT.0 as u32);
            }
            _ => panic!("expected keyboard hotkey"),
        }
    }

    #[test]
    fn parses_mouse_button() {
        assert_eq!(parse("Mouse4").unwrap(), HotkeySpec::MouseButton(MouseButton::Button4));
    }
}
