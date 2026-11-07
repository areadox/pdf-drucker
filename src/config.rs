//! Pfade und Einstellungen. Bis zu fünf PDF-Drucker mit jeweils eigenen Einstellungen.
//!
//! Ältere Versionen hatten nur einen Drucker mit Einstellungen auf oberster Ebene der
//! config.json – diese werden beim Laden automatisch in den ersten Drucker übernommen.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const APP_NAME: &str = "PDF-Ablage";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "areadox/pdf-drucker";
pub const SETUP_ASSET: &str = "PDF-Ablage-Setup.exe";
pub const MAX_PRINTERS: usize = 5;
pub const DEFAULT_PATTERN: &str = "{datum}_{zeit}_{titel}";

/// Entwicklungs-Builds (ohne Versionsnummer) suchen nicht nach Updates.
pub fn is_dev_build() -> bool {
    APP_VERSION == "0.0.0"
}

pub fn app_dir() -> PathBuf {
    let base = std::env::var_os("PROGRAMDATA").unwrap_or_else(|| r"C:\ProgramData".into());
    PathBuf::from(base).join("PDFDrucker")
}

/// Anschluss des ersten Druckers: Windows schreibt jeden Druckauftrag einzeln in diese Pipe.
pub const PIPE_NAME: &str = r"\\.\pipe\PDFAblage";

/// Pipe für Drucker Nr. `id` (1 behält den bisherigen Namen).
pub fn pipe_name(id: u8) -> String {
    if id <= 1 {
        PIPE_NAME.to_string()
    } else {
        format!("{PIPE_NAME}-{id}")
    }
}

/// Anschluss älterer Versionen (feste Datei), wird bei der Einrichtung entfernt.
pub fn old_spool_file() -> PathBuf {
    app_dir().join("spool").join("eingang.pdf")
}

/// Gehört ein Drucker-Anschluss zu PDF-Ablage?
pub fn is_our_port(port: &str) -> bool {
    let p = port.to_ascii_lowercase();
    p.starts_with(&PIPE_NAME.to_ascii_lowercase())
        || p == old_spool_file().to_string_lossy().to_ascii_lowercase()
}

pub fn config_file() -> PathBuf {
    app_dir().join("config.json")
}
pub fn log_file() -> PathBuf {
    app_dir().join("pdf_drucker.log")
}

fn documents_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    home.join("Documents")
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrinterConfig {
    /// 1–5, bestimmt den Anschluss (Pipe) – bleibt beim Umbenennen gleich
    pub id: u8,
    /// Name des Druckers in Windows
    pub name: String,
    pub target_dir: String,
    /// Platzhalter: {datum} {zeit} {titel}
    pub filename_pattern: String,
    pub open_after_save: bool,
    /// Gespeicherte PDF zusätzlich ausdrucken ("" = Windows-Standarddrucker)
    pub print_after_save: bool,
    pub print_printer: String,
}

impl PrinterConfig {
    pub fn new(id: u8) -> Self {
        let name = if id <= 1 { APP_NAME.to_string() } else { format!("{APP_NAME} {id}") };
        Self {
            id,
            target_dir: documents_dir().join(&name).to_string_lossy().into_owned(),
            name,
            filename_pattern: DEFAULT_PATTERN.into(),
            open_after_save: false,
            print_after_save: false,
            print_printer: String::new(),
        }
    }

    pub fn pipe(&self) -> String {
        pipe_name(self.id)
    }
}

impl Default for PrinterConfig {
    fn default() -> Self {
        Self::new(1)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(default)]
    pub printers: Vec<PrinterConfig>,
    pub notify: bool,
    pub auto_update_check: bool,
    /// Autostart wurde einmalig als Standard eingeschaltet (danach gilt die Menü-Einstellung)
    pub autostart_default_applied: bool,
    /// Nur nötig, solange das GitHub-Repo privat ist (Fine-grained Token, nur "Contents: Read")
    pub github_token: String,

    // Einstellungen älterer Versionen (nur lesen, werden in printers[0] übernommen)
    #[serde(skip_serializing)]
    printer_name: Option<String>,
    #[serde(skip_serializing)]
    target_dir: Option<String>,
    #[serde(skip_serializing)]
    filename_pattern: Option<String>,
    #[serde(skip_serializing)]
    open_after_save: Option<bool>,
    #[serde(skip_serializing)]
    print_after_save: Option<bool>,
    #[serde(skip_serializing)]
    print_printer: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            printers: Vec::new(),
            notify: true,
            auto_update_check: true,
            autostart_default_applied: false,
            github_token: String::new(),
            printer_name: None,
            target_dir: None,
            filename_pattern: None,
            open_after_save: None,
            print_after_save: None,
            print_printer: None,
        }
    }
}

impl Config {
    /// Alte Einzel-Einstellungen übernehmen, IDs prüfen, auf 5 Drucker begrenzen.
    fn normalize(&mut self) {
        if self.printers.is_empty() {
            let mut p = PrinterConfig::new(1);
            if let Some(v) = self.printer_name.take() {
                p.name = v;
            }
            if let Some(v) = self.target_dir.take() {
                p.target_dir = v;
            }
            if let Some(v) = self.filename_pattern.take() {
                p.filename_pattern = v;
            }
            if let Some(v) = self.open_after_save.take() {
                p.open_after_save = v;
            }
            if let Some(v) = self.print_after_save.take() {
                p.print_after_save = v;
            }
            if let Some(v) = self.print_printer.take() {
                p.print_printer = v;
            }
            self.printers.push(p);
        }
        self.printers.truncate(MAX_PRINTERS);
        let mut used = Vec::new();
        for p in &mut self.printers {
            if p.id == 0 || p.id as usize > MAX_PRINTERS || used.contains(&p.id) {
                p.id = (1..=MAX_PRINTERS as u8).find(|i| !used.contains(i)).unwrap_or(1);
            }
            used.push(p.id);
            if p.filename_pattern.trim().is_empty() {
                p.filename_pattern = DEFAULT_PATTERN.into();
            }
        }
    }

    /// Kleinste freie Nummer für einen neuen Drucker.
    pub fn free_id(printers: &[PrinterConfig]) -> Option<u8> {
        (1..=MAX_PRINTERS as u8).find(|i| !printers.iter().any(|p| p.id == *i))
    }

    pub fn printer(&self, id: u8) -> Option<&PrinterConfig> {
        self.printers.iter().find(|p| p.id == id)
    }

    pub fn first(&self) -> PrinterConfig {
        self.printers.first().cloned().unwrap_or_default()
    }

    pub fn is_own_printer(&self, name: &str) -> bool {
        self.printers.iter().any(|p| p.name.eq_ignore_ascii_case(name))
    }
}

pub fn load() -> Config {
    let path = config_file();
    let mut cfg = match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
            Ok(cfg) => cfg,
            Err(e) => {
                log_warn!("config.json nicht lesbar ({e}), nutze Standardwerte.");
                Config::default()
            }
        },
        Err(_) => Config::default(),
    };
    cfg.normalize();
    cfg
}

pub fn save(cfg: &Config) -> Result<()> {
    std::fs::create_dir_all(app_dir())?;
    std::fs::write(config_file(), serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}

/// Prüft einen Druckernamen auf Zeichen, die Windows nicht erlaubt.
pub fn validate_printer_name(name: &str) -> Result<(), String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Der Druckername darf nicht leer sein.".into());
    }
    if n.chars().count() > 200 {
        return Err("Der Druckername ist zu lang (höchstens 200 Zeichen).".into());
    }
    if n.contains(['\\', ',', '!']) {
        return Err(format!("„{n}“: Druckernamen dürfen kein \\ , oder ! enthalten."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Config {
        let mut c: Config = serde_json::from_str(json).unwrap();
        c.normalize();
        c
    }

    #[test]
    fn alte_einstellungen_werden_uebernommen() {
        let c = parse(
            r#"{"printer_name":"Büro","target_dir":"C:\\Nextcloud\\PDFS","filename_pattern":"{titel}",
                "open_after_save":true,"print_after_save":true,"print_printer":"Brother","notify":false,
                "poll_interval":0.5}"#,
        );
        assert_eq!(c.printers.len(), 1);
        let p = &c.printers[0];
        assert_eq!((p.id, p.name.as_str(), p.target_dir.as_str()), (1, "Büro", r"C:\Nextcloud\PDFS"));
        assert_eq!(p.filename_pattern, "{titel}");
        assert!(p.open_after_save && p.print_after_save);
        assert_eq!(p.print_printer, "Brother");
        assert!(!c.notify);
        // Alte Felder werden nicht mehr geschrieben
        let out = serde_json::to_string(&c).unwrap();
        assert!(!out.contains("printer_name") && out.contains("printers"));
    }

    #[test]
    fn neue_installation() {
        let c = parse("{}");
        assert_eq!(c.printers.len(), 1);
        assert_eq!(c.printers[0].name, "PDF-Ablage");
        assert_eq!(c.printers[0].pipe(), r"\\.\pipe\PDFAblage");
    }

    #[test]
    fn ids_und_grenzen() {
        let c = parse(r#"{"printers":[{"id":2,"name":"A"},{"id":2,"name":"B"},{"name":"C"},{"id":9,"name":"D"},
            {"id":1,"name":"E"},{"id":3,"name":"F"}]}"#);
        assert_eq!(c.printers.len(), MAX_PRINTERS);
        let mut ids: Vec<u8> = c.printers.iter().map(|p| p.id).collect();
        ids.sort();
        assert_eq!(ids, vec![1, 2, 3, 4, 5]);
        assert_eq!(Config::free_id(&c.printers), None);
        assert_eq!(pipe_name(3), r"\\.\pipe\PDFAblage-3");
    }

    #[test]
    fn namen_pruefen() {
        assert!(validate_printer_name("PDF-Ablage Rechnungen").is_ok());
        assert!(validate_printer_name("  ").is_err());
        assert!(validate_printer_name(r"a\b").is_err());
        assert!(validate_printer_name("a,b").is_err());
    }
}
