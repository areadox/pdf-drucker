//! PDF-Ablage – virtueller PDF-Drucker für Windows.
//!
//! Alles, was auf den Drucker "PDF-Ablage" gedruckt wird, landet ohne Nachfrage als PDF in einem
//! festen Ordner – benannt nach Datum, Uhrzeit und Dokumentname. Optional wird die PDF zusätzlich
//! auf einem echten Drucker ausgegeben.

#![windows_subsystem = "windows"]

#[macro_use]
mod logging;
mod config;
mod install;
mod logic;
mod pdf;
mod spooler;
mod tray;
mod ui;
mod update;
mod util;
mod watch;

use anyhow::{bail, Context, Result};
use std::path::Path;

const HILFE: &str = "PDF-Ablage – virtueller PDF-Drucker

Befehle:
  PDF-Ablage.exe                          Tray-App starten (Standard)
  PDF-Ablage.exe install [--name NAME] [--ordner PFAD]   Drucker einrichten (Administrator)
  PDF-Ablage.exe uninstall                Drucker entfernen (Administrator)
  PDF-Ablage.exe run                      Überwachung ohne Tray
  PDF-Ablage.exe autostart [--aus]        beim Anmelden starten / nicht mehr starten
  PDF-Ablage.exe config                   Einstellungen anzeigen
  PDF-Ablage.exe print DATEI [DRUCKER]    PDF ausdrucken
  PDF-Ablage.exe selftest [DRUCKER] [--ergebnis DATEI]   Darstellung/Testdruck prüfen
  PDF-Ablage.exe make-test-pdf DATEI      Test-PDF erzeugen
  PDF-Ablage.exe version                  Version anzeigen";

const VALUE_OPTIONS: [&str; 3] = ["--name", "--ordner", "--ergebnis"];

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn positional(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
        } else if VALUE_OPTIONS.contains(&a.as_str()) {
            skip = true;
        } else if !a.starts_with("--") {
            out.push(a.clone());
        }
    }
    out
}

fn run(cmd: &str, args: &[String]) -> Result<i32> {
    match cmd {
        "tray" => tray::run(args.iter().any(|a| a == "--nach-update")),
        "run" => watch::run_console(),
        "install" => install::cmd_install(option(args, "--name"), option(args, "--ordner")),
        "uninstall" => install::cmd_uninstall(),
        "autostart" => {
            let off = args.iter().any(|a| a == "--aus");
            install::set_autostart(!off)?;
            ui::message(if off { "Autostart deaktiviert." } else { "Autostart eingerichtet." }, false);
            Ok(0)
        }
        "config" => {
            ui::message(
                &format!(
                    "Konfigurationsdatei: {}\n{}\nProtokoll: {}",
                    config::config_file().display(),
                    serde_json::to_string_pretty(&config::load())?,
                    config::log_file().display()
                ),
                false,
            );
            Ok(0)
        }
        "version" | "--version" => {
            ui::message(&format!("{} {}", config::APP_NAME, config::APP_VERSION), false);
            Ok(0)
        }
        "print" => {
            let pos = positional(args);
            let file = pos.first().context("Bitte die PDF-Datei angeben.")?;
            let printer = match pos.get(1) {
                Some(p) => p.clone(),
                None => watch::resolve_print_printer(&config::load())?,
            };
            let path = Path::new(file);
            let doc_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let pages = pdf::PdfPrinter::new()?.print(path, &printer, &doc_name)?;
            ui::message(&format!("{pages} Seite(n) an '{printer}' gedruckt."), false);
            Ok(0)
        }
        "selftest" => pdf::cmd_selftest(positional(args).first().cloned(), option(args, "--ergebnis")),
        "make-test-pdf" => {
            let pos = positional(args);
            pdf::write_test_pdf(Path::new(pos.first().context("Bitte den Dateinamen angeben.")?))?;
            Ok(0)
        }
        "help" | "--help" | "-h" | "/?" => {
            ui::message(HILFE, false);
            Ok(0)
        }
        other => bail!("Unbekannter Befehl: {other}\n\n{HILFE}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().cloned().unwrap_or_else(|| "tray".into());
    if cmd != "tray" {
        ui::attach_console();
    }
    logging::init();
    let rest = args.get(1..).unwrap_or(&[]);
    let code = match run(&cmd, rest) {
        Ok(code) => code,
        Err(e) => {
            log_error!("{e:#}");
            ui::message(&format!("{e:#}"), true);
            1
        }
    };
    std::process::exit(code);
}
