//! Etat partage entre le thread de capture Interception, le thread de
//! detection memoire optionnelle, et le thread UI (message loop Win32 /
//! tray / hotkeys clavier).

use std::collections::HashMap;

use crate::config::Config;
use crate::scaling::{compute_factor, AxisAccumulator};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Passthrough,
    Game(usize),
}

pub struct GameRuntime {
    pub name: String,
    pub factor: f64,
}

pub struct AppState {
    pub mode: Mode,
    /// Dernier jeu selectionne, pour que le hotkey de toggle sache ou
    /// revenir quand on quitte le passthrough.
    pub last_game: usize,
    pub games: Vec<GameRuntime>,
    /// Un accumulateur (x, y) par device souris physique (plusieurs souris
    /// branchees = plusieurs InterceptionDevice distincts).
    pub accumulators: HashMap<i32, (AxisAccumulator, AxisAccumulator)>,
}

impl AppState {
    pub fn from_config(config: &Config) -> Self {
        let games = config
            .games
            .iter()
            .map(|g| GameRuntime {
                name: g.name.clone(),
                factor: compute_factor(
                    config.settings.target_cm_per_360,
                    config.settings.mouse_dpi,
                    g.deg_per_count_at_ref,
                    g.reference_sensitivity,
                    g.effective_sensitivity(),
                ),
            })
            .collect();

        let mode = if config.settings.start_in_passthrough {
            Mode::Passthrough
        } else {
            Mode::Game(0)
        };

        Self {
            mode,
            last_game: 0,
            games,
            accumulators: HashMap::new(),
        }
    }

    pub fn current_factor(&self) -> f64 {
        match self.mode {
            Mode::Passthrough => 1.0,
            Mode::Game(i) => self.games.get(i).map(|g| g.factor).unwrap_or(1.0),
        }
    }

    pub fn mode_label(&self) -> String {
        match self.mode {
            Mode::Passthrough => "Passthrough".to_string(),
            Mode::Game(i) => self
                .games
                .get(i)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "?".to_string()),
        }
    }

    pub fn toggle_passthrough(&mut self) {
        self.mode = match self.mode {
            Mode::Passthrough => Mode::Game(self.last_game),
            Mode::Game(i) => {
                self.last_game = i;
                Mode::Passthrough
            }
        };
        self.reset_accumulators();
    }

    pub fn set_passthrough(&mut self) {
        if let Mode::Game(i) = self.mode {
            self.last_game = i;
        }
        self.mode = Mode::Passthrough;
        self.reset_accumulators();
    }

    pub fn select_game(&mut self, index: usize) {
        if index < self.games.len() {
            self.mode = Mode::Game(index);
            self.last_game = index;
            self.reset_accumulators();
        }
    }

    pub fn cycle_game(&mut self) {
        if self.games.is_empty() {
            return;
        }
        let next = match self.mode {
            Mode::Passthrough => self.last_game,
            Mode::Game(i) => (i + 1) % self.games.len(),
        };
        self.select_game(next);
    }

    /// A appeler a chaque changement de mode/facteur : le reste
    /// fractionnaire accumule pour l'ancien facteur n'a pas de sens pour le
    /// nouveau, et le laisser trainer produirait un micro-saut au prochain
    /// mouvement.
    fn reset_accumulators(&mut self) {
        for (ax, ay) in self.accumulators.values_mut() {
            ax.reset();
            ay.reset();
        }
    }
}
