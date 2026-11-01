//! Pfade und Einstellungen (kompatibel zur config.json der Python-Version).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const APP_NAME: &str = "PDF-Ablage";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "areadox/pdf-drucker";
pub const SETUP_ASSET: &str = "PDF-Ablage-Setup.exe";

/// Entwicklungs-Builds (ohne Versionsnummer) suchen nicht nach Updates.
pub fn is_dev_build() -> bool {
    APP_VERSION == "0.0.0"
}

pub fn app_dir() -> PathBuf {
    let base = std::env::var_os("PROGRAMDATA").unwrap_or_else(|| r"C:\ProgramData".into());
    PathBuf::from(base).join("PDFDrucker")
}
pub fn spool_dir() -> PathBuf {
    app_dir().join("spool")
}
pub fn spool_file() -> PathBuf {
    spool_dir().join("eingang.pdf")
}
pub fn config_file() -> PathBuf {
    app_dir().join("config.json")
}
pub fn log_file() -> PathBuf {
    app_dir().join("pdf_drucker.log")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub printer_name: String,
    pub target_dir: String,
    /// Platzhalter: {datum} {zeit} {titel}
    pub filename_pattern: String,
    pub poll_interval: f64,
    pub open_after_save: bool,
    pub notify: bool,
    /// Gespeicherte PDF zusätzlich ausdrucken ("" = Windows-Standarddrucker)
    pub print_after_save: bool,
    pub print_printer: String,
    pub auto_update_check: bool,
    /// Nur nötig, solange das GitHub-Repo privat ist (Fine-grained Token, nur "Contents: Read")
    pub github_token: String,
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        Self {
            printer_name: APP_NAME.into(),
            target_dir: home.join("Documents").join(APP_NAME).to_string_lossy().into_owned(),
            filename_pattern: "{datum}_{zeit}_{titel}".into(),
            poll_interval: 0.5,
            open_after_save: false,
            notify: true,
            print_after_save: false,
            print_printer: String::new(),
            auto_update_check: true,
            github_token: String::new(),
        }
    }
}

pub fn load() -> Config {
    let path = config_file();
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
            Ok(cfg) => cfg,
            Err(e) => {
                log_warn!("config.json nicht lesbar ({e}), nutze Standardwerte.");
                Config::default()
            }
        },
        Err(_) => Config::default(),
    }
}

pub fn save(cfg: &Config) -> Result<()> {
    std::fs::create_dir_all(app_dir())?;
    std::fs::write(config_file(), serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}
