//! Überwacht die Eingangsdatei des Druckers und legt fertige PDFs im Zielordner ab.

use crate::config::{self, Config};
use crate::{logic, pdf, spooler, ui};
use anyhow::{bail, Context, Result};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub enum Event {
    Saved(PathBuf),
    Printed(PathBuf, String),
    PrintFailed(PathBuf, String),
}

pub type Events = Arc<dyn Fn(Event) + Send + Sync>;

pub fn watch(cfg: Arc<Mutex<Config>>, stop: Arc<AtomicBool>, events: Events, print_tx: Option<Sender<PathBuf>>) {
    log_info!(
        "Überwache {} -> {}",
        config::spool_file().display(),
        cfg.lock().map(|c| c.target_dir.clone()).unwrap_or_default()
    );
    while !stop.load(Ordering::Relaxed) {
        let c = cfg.lock().map(|c| c.clone()).unwrap_or_default();
        if let Err(e) = process_once(&c, &events, print_tx.as_ref()) {
            log_error!("Fehler in der Überwachung: {e:#}");
        }
        thread::sleep(Duration::from_secs_f64(c.poll_interval.clamp(0.1, 10.0)));
    }
}

fn process_once(c: &Config, events: &Events, print_tx: Option<&Sender<PathBuf>>) -> Result<()> {
    let spool = config::spool_file();
    if !spool.exists() || !pdf_is_complete(&spool) {
        return Ok(());
    }

    let titel = spooler::take_document_name(&c.printer_name)
        .map(|d| logic::clean_document_name(&d))
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "Druck".into());

    let target = PathBuf::from(&c.target_dir);
    std::fs::create_dir_all(&target).with_context(|| format!("Zielordner {} nicht anlegbar", target.display()))?;
    let now = chrono::Local::now();
    let stem = logic::render_pattern(
        &c.filename_pattern,
        &now.format("%Y-%m-%d").to_string(),
        &now.format("%H-%M-%S").to_string(),
        &titel,
    );
    let dst = logic::unique_path(&target, &logic::safe_name(&stem, 150));
    move_with_retry(&spool, &dst)?;
    log_info!("Gespeichert: {}", dst.display());

    if c.open_after_save {
        ui::open(&dst);
    }
    (events.as_ref())(Event::Saved(dst.clone()));
    if c.print_after_save {
        if let Some(tx) = print_tx {
            let _ = tx.send(dst);
        }
    }
    Ok(())
}

/// Fertig geschrieben = Größe ändert sich nicht mehr und das PDF-Ende-Kennzeichen ist da.
fn pdf_is_complete(path: &Path) -> bool {
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let s1 = size(path);
    if s1 == 0 {
        return false;
    }
    thread::sleep(Duration::from_millis(300));
    if size(path) != s1 {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let start = s1.saturating_sub(1024);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return false;
    }
    let mut tail = Vec::new();
    if f.read_to_end(&mut tail).is_err() {
        return false;
    }
    tail.windows(5).any(|w| w == b"%%EOF")
}

fn move_with_retry(src: &Path, dst: &Path) -> Result<()> {
    const ERROR_NOT_SAME_DEVICE: i32 = 17;
    let mut last = None;
    for _ in 0..20 {
        match std::fs::rename(src, dst) {
            Ok(()) => return Ok(()),
            Err(e) if e.raw_os_error() == Some(ERROR_NOT_SAME_DEVICE) => {
                // Zielordner auf einem anderen Laufwerk: kopieren und Original löschen
                std::fs::copy(src, dst)?;
                std::fs::remove_file(src)?;
                return Ok(());
            }
            Err(e) => {
                last = Some(e);
                thread::sleep(Duration::from_millis(250)); // Spooler hält die Datei noch
            }
        }
    }
    bail!("Konnte {} nicht verschieben: {:?}", src.display(), last)
}

/// Zieldrucker für den zusätzlichen Ausdruck; nie der eigene PDF-Drucker (Endlosschleife).
pub fn resolve_print_printer(c: &Config) -> Result<String> {
    let name = if c.print_printer.trim().is_empty() {
        spooler::default_printer().context("Es ist kein Windows-Standarddrucker eingestellt.")?
    } else {
        c.print_printer.clone()
    };
    if name.eq_ignore_ascii_case(&c.printer_name) {
        bail!(
            "'{name}' ist der PDF-Ablage-Drucker selbst – das ergäbe eine Endlosschleife.\n\
             Bitte im Menü unter „Ausdrucken auf“ einen echten Drucker wählen."
        );
    }
    Ok(name)
}

/// Hintergrund-Thread, der PDFs der Reihe nach ausdruckt.
pub fn start_print_worker(cfg: Arc<Mutex<Config>>, events: Events) -> Sender<PathBuf> {
    let (tx, rx) = mpsc::channel::<PathBuf>();
    thread::spawn(move || {
        let mut printer: Option<pdf::PdfPrinter> = None;
        for path in rx {
            match print_one(&mut printer, &cfg, &path) {
                Ok((pages, target)) => {
                    log_info!(
                        "Gedruckt ({pages} Seite{}) auf '{target}': {}",
                        if pages == 1 { "" } else { "n" },
                        path.display()
                    );
                    (events.as_ref())(Event::Printed(path, target));
                }
                Err(e) => {
                    log_error!("Drucken fehlgeschlagen ({}): {e:#}", path.display());
                    (events.as_ref())(Event::PrintFailed(path, format!("{e:#}")));
                }
            }
        }
    });
    tx
}

fn print_one(printer: &mut Option<pdf::PdfPrinter>, cfg: &Arc<Mutex<Config>>, path: &Path) -> Result<(usize, String)> {
    let c = cfg.lock().map(|c| c.clone()).unwrap_or_default();
    let target = resolve_print_printer(&c)?;
    if printer.is_none() {
        *printer = Some(pdf::PdfPrinter::new()?);
    }
    let doc_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let pages = printer.as_ref().context("pdfium nicht geladen")?.print(path, &target, &doc_name)?;
    Ok((pages, target))
}

/// Überwachung ohne Tray (Befehl "run").
pub fn run_console() -> Result<i32> {
    let cfg = Arc::new(Mutex::new(config::load()));
    let events: Events = Arc::new(|_| {});
    let tx = start_print_worker(cfg.clone(), events.clone());
    watch(cfg, Arc::new(AtomicBool::new(false)), events, Some(tx));
    Ok(0)
}
