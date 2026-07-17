//! Logique partagee entre l'app tray (`app/`) et la GUI de configuration
//! (`gui/`) : schema YAML et calcul du facteur d'echelle cm/360. Aucune
//! dependance Win32 ici, pour rester utilisable telle quelle des deux cotes.

pub mod config;
pub mod scaling;
