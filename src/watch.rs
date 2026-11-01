//! Empfängt die Druckaufträge über die Named Pipe und legt sie als PDF im Zielordner ab.
//!
//! Windows übergibt jeden Druckauftrag einzeln an die Pipe (eine Verbindung pro Auftrag).
//! Dadurch kann nichts überschrieben werden, auch wenn mehrere Dokumente direkt
//! hintereinander gedruckt werden.

use crate::config::{self, Config, PIPE_NAME};
use crate::util::wide;
use crate::{logic, pdf, spooler, ui};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, PIPE_ACCESS_INBOUND};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

pub enum Event {
    Saved(PathBuf),
    Printed(PathBuf, String),
    PrintFailed(PathBuf, String),
}

pub type Events = Arc<dyn Fn(Event) + Send + Sync>;

/// Nimmt Druckaufträge entgegen, bis `stop` gesetzt wird (bzw. das Programm endet).
pub fn watch(cfg: Arc<Mutex<Config>>, stop: Arc<AtomicBool>, events: Events, print_tx: Option<Sender<PathBuf>>) {
    log_info!(
        "Empfange Druckaufträge über {PIPE_NAME} -> {}",
        cfg.lock().map(|c| c.target_dir.clone()).unwrap_or_default()
    );
    // Aufträge werden in einem eigenen Thread der Reihe nach abgelegt
    let (job_tx, job_rx) = mpsc::channel::<(Vec<u8>, Option<String>)>();
    {
        let cfg = cfg.clone();
        thread::spawn(move || {
            for (data, doc) in job_rx {
                let c = cfg.lock().map(|c| c.clone()).unwrap_or_default();
                if let Err(e) = save_job(&c, &data, doc.as_deref(), &events, print_tx.as_ref()) {
                    log_error!("Druckauftrag konnte nicht gespeichert werden: {e:#}");
                }
            }
        });
    }

    let name = wide(PIPE_NAME);
    let mut failures = 0u32;
    while !stop.load(Ordering::Relaxed) {
        // Neue Pipe-Instanz anlegen, auf den nächsten Auftrag warten
        let h = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_INBOUND,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                PIPE_UNLIMITED_INSTANCES,
                0,
                1 << 20,
                0,
                null(),
            )
        };
        if h == INVALID_HANDLE_VALUE {
            failures += 1;
            if failures == 1 || failures % 30 == 0 {
                log_error!("Pipe {PIPE_NAME} konnte nicht angelegt werden (Fehler {}).", unsafe { GetLastError() });
            }
            thread::sleep(Duration::from_secs(2));
            continue;
        }
        failures = 0;
        let connected = unsafe { ConnectNamedPipe(h, null_mut()) } != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        if !connected {
            unsafe { CloseHandle(h) };
            continue;
        }
        // Genau jetzt läuft der Auftrag, der in die Pipe schreibt -> Dokumentname passt sicher
        let printer = cfg.lock().map(|c| c.printer_name.clone()).unwrap_or_default();
        let doc = spooler::current_document_name(&printer);
        // Lesen in eigenem Thread, damit sofort die nächste Pipe-Instanz bereitsteht
        let (h, job_tx) = (h as usize, job_tx.clone());
        thread::spawn(move || {
            let data = read_all(h as _);
            unsafe {
                DisconnectNamedPipe(h as _);
                CloseHandle(h as _);
            }
            match data {
                Ok(data) => {
                    let _ = job_tx.send((data, doc));
                }
                Err(e) => log_error!("Druckauftrag konnte nicht gelesen werden: {e:#}"),
            }
        });
    }
}

fn read_all(h: windows_sys::Win32::Foundation::HANDLE) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let mut read = 0u32;
        let ok = unsafe { ReadFile(h, buf.as_mut_ptr(), buf.len() as u32, &mut read, null_mut()) };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            if err == ERROR_BROKEN_PIPE {
                break; // Windows hat den Auftrag fertig übertragen
            }
            bail!("Lesefehler {err}");
        }
        data.extend_from_slice(&buf[..read as usize]);
    }
    Ok(data)
}

fn save_job(
    c: &Config,
    data: &[u8],
    doc: Option<&str>,
    events: &Events,
    print_tx: Option<&Sender<PathBuf>>,
) -> Result<()> {
    if data.is_empty() {
        log_warn!("Leerer Druckauftrag ignoriert ({}).", doc.unwrap_or("ohne Namen"));
        return Ok(());
    }
    if !data.starts_with(b"%PDF") {
        log_warn!("Druckauftrag sieht nicht wie eine PDF aus – wird trotzdem gespeichert.");
    }
    let titel = doc
        .map(logic::clean_document_name)
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
    // Erst unter Zwischennamen schreiben, damit z. B. Nextcloud keine halbe Datei synchronisiert
    let part = dst.with_extension("pdf.part");
    std::fs::write(&part, data).with_context(|| format!("{} nicht schreibbar", part.display()))?;
    std::fs::rename(&part, &dst)?;
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
