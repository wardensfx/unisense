//! Thread de capture : lit les strokes souris bruts via Interception,
//! applique le facteur d'echelle courant, et reinjecte. Tourne jusqu'a
//! arret demande via `running`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use unisense_core::scaling::AxisAccumulator;

use crate::hotkey::{HotkeySpec, MouseButton};
use crate::interception::{Interception, InterceptionStroke, MOUSE_MOVE_ABSOLUTE};
use crate::state::AppState;

/// Bouton souris configure comme hotkey, avec l'action a executer sur front
/// descendant. Le clic lui-meme est toujours transmis inchange au jeu (on
/// ne consomme jamais un clic gauche/droit configure par erreur en hotkey,
/// on se contente d'observer l'evenement en plus de le forwarder).
#[derive(Clone)]
pub enum MouseHotkeyAction {
    TogglePassthrough,
    CycleGame,
    SelectGame(usize),
}

pub struct MouseHotkey {
    pub button: MouseButton,
    pub action: MouseHotkeyAction,
}

pub fn mouse_hotkeys_from_specs(specs: &[(HotkeySpec, MouseHotkeyAction)]) -> Vec<MouseHotkey> {
    specs
        .iter()
        .filter_map(|(spec, action)| match spec {
            HotkeySpec::MouseButton(button) => Some(MouseHotkey {
                button: *button,
                action: action.clone(),
            }),
            HotkeySpec::Keyboard { .. } => None,
        })
        .collect()
}

pub fn run(
    state: Arc<Mutex<AppState>>,
    mouse_hotkeys: Vec<MouseHotkey>,
    running: Arc<AtomicBool>,
    on_error: impl Fn(String) + Send + 'static,
) {
    let interception = match Interception::new() {
        Ok(ctx) => ctx,
        Err(e) => {
            on_error(format!("{e:?}"));
            running.store(false, Ordering::SeqCst);
            return;
        }
    };
    interception.filter_all_mice();

    while running.load(Ordering::SeqCst) {
        // Timeout court pour reverifier `running` regulierement (permet un
        // arret propre depuis le menu tray "Quitter").
        let Some(device) = interception.wait(200) else {
            continue;
        };
        if !interception.is_mouse(device) {
            continue;
        }

        let mut stroke = InterceptionStroke::default();
        if !interception.receive(device, &mut stroke) {
            continue;
        }

        // SAFETY: `device` a ete verifie mouse via `is_mouse` ci-dessus, le
        // champ actif de l'union est donc bien `mouse`.
        let mut mouse = unsafe { stroke.mouse };

        for hk in &mouse_hotkeys {
            if hk.button.is_down_in(mouse.state) {
                let mut s = state.lock().unwrap();
                match hk.action {
                    MouseHotkeyAction::TogglePassthrough => s.toggle_passthrough(),
                    MouseHotkeyAction::CycleGame => s.cycle_game(),
                    MouseHotkeyAction::SelectGame(i) => s.select_game(i),
                }
            }
        }

        // Le mode ABSOLUTE (tablette graphique, certains KVM/RDP) n'est pas
        // exprime en "comptes" relatifs : appliquer notre facteur n'aurait
        // pas de sens, on laisse passer tel quel.
        if mouse.flags & MOUSE_MOVE_ABSOLUTE == 0 && (mouse.x != 0 || mouse.y != 0) {
            let mut s = state.lock().unwrap();
            let factor = s.current_factor();
            let (acc_x, acc_y) = s
                .accumulators
                .entry(device)
                .or_insert_with(|| (AxisAccumulator::default(), AxisAccumulator::default()));
            mouse.x = acc_x.apply(mouse.x, factor);
            mouse.y = acc_y.apply(mouse.y, factor);
        }

        stroke.mouse = mouse;
        interception.send(device, &stroke);
    }
}
