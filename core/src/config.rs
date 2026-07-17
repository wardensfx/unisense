//! Chargement du fichier de config YAML (config/games.yaml par defaut).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// DPI materiel courant de la souris (reglage OSD/logiciel du peripherique).
    /// Windows ne peut pas lire cette valeur sur le bus HID : c'est a
    /// l'utilisateur de la renseigner ici, et de la mettre a jour s'il
    /// change le DPI sur sa souris.
    pub mouse_dpi: f64,

    /// Cible universelle : centimetres de deplacement de la souris pour un
    /// tour complet (360 degres) de la camera, quel que soit le jeu actif.
    pub target_cm_per_360: f64,

    /// Hotkey qui bascule passthrough <-> dernier jeu actif. Formats
    /// acceptes : "F9", "Ctrl+Alt+F9", "Mouse4", "Mouse5", "MouseMiddle".
    #[serde(default = "default_toggle_hotkey")]
    pub toggle_hotkey: String,

    /// Hotkey qui fait defiler la liste des jeux configures (mode "jeu actif").
    #[serde(default = "default_cycle_hotkey")]
    pub cycle_game_hotkey: String,

    /// Si vrai, unisense demarre en passthrough (facteur 1.0) plutot que
    /// sur le premier jeu de la liste.
    #[serde(default)]
    pub start_in_passthrough: bool,
}

fn default_toggle_hotkey() -> String {
    "Mouse4".to_string()
}

fn default_cycle_hotkey() -> String {
    "Ctrl+Alt+F9".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEntry {
    /// Nom affiche dans le tray / menu.
    pub name: String,

    /// Degres de rotation de la camera par compte de souris brut ("mickey"),
    /// mesures avec la sensibilite in-game fixee a `reference_sensitivity`.
    /// C'est la constante publiee par mouse-sensitivity.com (colonne "yaw"
    /// ou equivalent) ou mesuree soi-meme (voir README).
    pub deg_per_count_at_ref: f64,

    /// Valeur de la sensibilite in-game a laquelle `deg_per_count_at_ref` a
    /// ete mesuree (ex: 1.0). L'utilisateur doit regler le jeu sur cette
    /// valeur pour que le calcul soit exact.
    pub reference_sensitivity: f64,

    /// Sensibilite in-game reellement utilisee, si differente de la
    /// reference (utile quand le jeu ne permet pas de saisir une valeur
    /// precise, ex: slider a crans entiers). Le facteur est corrige par le
    /// ratio current/reference. Laisser absent = current == reference.
    ///
    /// ATTENTION : cette correction lineaire ne vaut que si la formule de
    /// sensibilite du jeu est elle-meme lineaire en la valeur du slider
    /// (vrai pour la plupart des moteurs Source/Quake/Unreal ; FAUX par
    /// exemple pour Minecraft dont la courbe est cubique). Pour un jeu a
    /// courbe non lineaire, verrouiller current_sensitivity == reference et
    /// ne jamais toucher au slider in-game.
    #[serde(default)]
    pub current_sensitivity: Option<f64>,

    /// Lien vers la source de la constante (pour verification / mise a jour
    /// si le jeu patche sa formule de sensibilite). Purement informatif,
    /// jamais lu par le code : utile pour la maintenance du YAML.
    #[allow(dead_code)]
    #[serde(default)]
    pub source: Option<String>,

    /// Hotkey optionnelle pour selectionner directement ce jeu sans passer
    /// par le cycle. Meme format que `toggle_hotkey`.
    #[serde(default)]
    pub hotkey: Option<String>,

    /// Detection automatique menu/gameplay par lecture memoire (5.2,
    /// experimental). Voir README "Detection automatique".
    #[serde(default)]
    pub auto_detect: Option<AutoDetect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoDetect {
    /// Nom du process (ex: "GameName.exe").
    pub process_name: String,
    /// Nom du module dans lequel chercher l'offset (souvent l'exe lui-meme
    /// ou une DLL du moteur). None = module principal du process.
    #[serde(default)]
    pub module_name: Option<String>,
    /// Offset (en octets) depuis la base du module.
    pub offset: u64,
    /// Chaine d'offsets de pointeurs a suivre apres l'offset de base
    /// (pointer chain), vide si acces direct.
    #[serde(default)]
    pub pointer_chain: Vec<u64>,
    /// Valeur (octets) attendue a cette adresse quand le jeu est EN JEU
    /// (gameplay). Toute autre valeur => considere "menu".
    pub in_game_bytes: Vec<u8>,
    /// Intervalle de sondage en millisecondes.
    #[serde(default = "default_poll_ms")]
    pub poll_interval_ms: u64,
}

fn default_poll_ms() -> u64 {
    250
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub settings: Settings,
    pub games: Vec<GameEntry>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("lecture impossible de {}", path.display()))?;
        let config: Config = serde_yaml::from_str(&text)
            .with_context(|| format!("YAML invalide dans {}", path.display()))?;
        if config.games.is_empty() {
            anyhow::bail!("la config ne contient aucun jeu (section 'games' vide)");
        }
        Ok(config)
    }

    /// Ecrit la config au format YAML. Utilise par la GUI (`gui/`) pour
    /// sauvegarder les modifications faites dans l'editeur.
    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_yaml::to_string(self).context("serialisation YAML")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creation du dossier {}", parent.display()))?;
        }
        std::fs::write(path, text)
            .with_context(|| format!("ecriture impossible de {}", path.display()))
    }
}

impl GameEntry {
    /// Sensibilite in-game effective a utiliser dans le calcul du facteur.
    pub fn effective_sensitivity(&self) -> f64 {
        self.current_sensitivity.unwrap_or(self.reference_sensitivity)
    }
}
