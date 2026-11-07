//! Empfängt die Druckaufträge über Named Pipes (eine pro Drucker) und legt sie als PDF ab.
//!
//! Windows übergibt jeden Druckauftrag einzeln an die Pipe seines Druckers (eine Verbindung
//! pro Auftrag). Dadurch kann nichts überschrieben werden, auch wenn mehrere Dokumente direkt
//! hintereinander oder auf mehreren Druckern gleichzeitig gedruckt werden.

use crate::config::{self, Config, PrinterConfig, PIPE_NAME};
use crate::util::wide;
use crate::{logic, pdf, spooler, ui};
use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, PIPE_ACCESS_INBOUND};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

pub enum Event {
    /// Gespeichert: erste Datei, Anzahl der Speicherorte
    Saved(PathBuf, usize),
    /// Ein Speicherort war nicht beschreibbar: Ordner, Fehler
    SaveFailed(String, String),
    Printed(PathBuf, String),
    PrintFailed(PathBuf, String),
}

pub type Events = Arc<dyn Fn(Event) + Send + Sync>;

/// Ein empfangener Druckauftrag: Drucker-Nr., Daten, Dokumentname.
type Job = (u8, Vec<u8>, Option<String>);

/// Nimmt Druckaufträge für alle eingestellten Drucker entgegen.
pub struct Receiver {
    cfg: Arc<Mutex<Config>>,
    job_tx: Sender<Job>,
    /// Drucker, für die der Empfang schon gemeldet wurde
    announced: Mutex<HashSet<u8>>,
    server_started: AtomicBool,
    events: Events,
    /// Neustarts leerer Aufträge je (Drucker-Nr., Auftrags-ID)
    restarts: Mutex<HashMap<(u8, u32), u8>>,
}

const MAX_RESTARTS: u8 = 3;

impl Receiver {
    pub fn start(cfg: Arc<Mutex<Config>>, events: Events) -> Arc<Receiver> {
        let print_tx = start_print_worker(events.clone());
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        {
            // Aufträge werden der Reihe nach abgelegt
            let (cfg, events) = (cfg.clone(), events.clone());
            thread::spawn(move || {
                for (id, data, doc) in job_rx {
                    let c = lock(&cfg);
                    let p = c.printer(id).cloned().unwrap_or_else(|| c.first());
                    if let Err(e) = save_job(&c, &p, &data, doc.as_deref(), &events, &print_tx) {
                        log_error!("Druckauftrag für „{}“ konnte nicht gespeichert werden: {e:#}", p.name);
                    }
                }
            });
        }
        let r = Arc::new(Receiver {
            cfg,
            job_tx,
            announced: Mutex::new(HashSet::new()),
            server_started: AtomicBool::new(false),
            events,
            restarts: Mutex::new(HashMap::new()),
        });
        r.ensure_servers();
        r
    }

    /// Startet den Empfang (einmal für alle Drucker) und meldet neu hinzugekommene Drucker.
    pub fn ensure_servers(self: &Arc<Self>) {
        let printers = lock(&self.cfg).printers;
        let mut announced = self.announced.lock().unwrap_or_else(|e| e.into_inner());
        for p in printers {
            if announced.insert(p.id) {
                log_info!("Empfange Druckaufträge für „{}“ über {} -> {}", p.name, PIPE_NAME, p.all_dirs().join(" + "));
            }
        }
        if !self.server_started.swap(true, Ordering::SeqCst) {
            let me = self.clone();
            thread::spawn(move || me.serve());
        }
    }

    fn serve(self: Arc<Self>) {
        let pipe = PIPE_NAME;
        let name = wide(pipe);
        let mut failures = 0u32;
        loop {
            // Neue Pipe-Instanz anlegen, auf den nächsten Auftrag warten
            let h = unsafe {
                CreateNamedPipeW(
                    name.as_ptr(),
                    PIPE_ACCESS_INBOUND,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    PIPE_UNLIMITED_INSTANCES,
                    0,
                    // Kleiner Puffer: Windows wartet beim Schreiben auf uns, so bleibt der Auftrag
                    // "wird gedruckt", bis wir seinen Dokumentnamen gelesen haben
                    64 * 1024,
                    0,
                    null(),
                )
            };
            if h == INVALID_HANDLE_VALUE {
                failures += 1;
                if failures == 1 || failures % 30 == 0 {
                    log_error!("Pipe {pipe} konnte nicht angelegt werden (Fehler {}).", unsafe { GetLastError() });
                }
                thread::sleep(Duration::from_secs(2));
                continue;
            }
            failures = 0;
            let connected =
                unsafe { ConnectNamedPipe(h, null_mut()) } != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
            if !connected {
                unsafe { CloseHandle(h) };
                continue;
            }
            // Lesen in eigenem Thread, damit sofort die nächste Pipe-Instanz bereitsteht
            let (h, me) = (h as usize, self.clone());
            thread::spawn(move || me.receive(h as HANDLE));
        }
    }

    /// Liest einen Auftrag aus der verbundenen Pipe.
    fn receive(&self, h: HANDLE) {
        // Genau jetzt läuft der Auftrag, der in die Pipe schreibt -> Drucker und Dokumentname
        // passen sicher (alle Drucker teilen sich den Anschluss, es läuft immer nur ein Auftrag)
        let c = lock(&self.cfg);
        let names: Vec<(u8, String)> = c.printers.iter().map(|p| (p.id, p.name.clone())).collect();
        let found = spooler::current_job_any(&names);
        let id = found.as_ref().map(|(id, _)| *id).unwrap_or_else(|| c.first().id);
        let printer = c.printer(id).map(|p| p.name.clone());
        let job = found.map(|(_, j)| j);
        let data = read_all(h);
        unsafe {
            DisconnectNamedPipe(h);
            CloseHandle(h);
        }
        let data = match data {
            Ok(d) => d,
            Err(e) => {
                log_error!("Druckauftrag konnte nicht gelesen werden: {e:#}");
                return;
            }
        };
        if data.is_empty() {
            // Windows hat die Verbindung ohne Daten beendet (kommt unter hoher Last vor):
            // Auftrag neu starten, damit er nicht verloren geht
            if let (Some(printer), Some(job)) = (printer, job) {
                let n = {
                    let mut r = self.restarts.lock().unwrap_or_else(|e| e.into_inner());
                    let n = r.entry((id, job.id)).or_insert(0);
                    *n += 1;
                    *n
                };
                if n <= MAX_RESTARTS {
                    log_warn!("Leere Übertragung von „{}“ ({}), starte Auftrag neu ({n}/{MAX_RESTARTS}).", job.document, printer);
                    let events = self.events.clone();
                    thread::spawn(move || {
                        thread::sleep(Duration::from_secs(2));
                        if !spooler::restart_job(&printer, job.id) {
                            log_error!("Auftrag „{}“ konnte nicht neu gestartet werden.", job.document);
                            (events.as_ref())(Event::SaveFailed(
                                printer,
                                format!("„{}“ ist leer angekommen und wurde nicht gespeichert. Bitte erneut drucken.", job.document),
                            ));
                        }
                    });
                } else {
                    log_error!("Auftrag „{}“ ({printer}) kam auch nach {MAX_RESTARTS} Versuchen leer an.", job.document);
                    (self.events.as_ref())(Event::SaveFailed(
                        printer,
                        format!("„{}“ ist leer angekommen und wurde nicht gespeichert.", job.document),
                    ));
                }
            } else {
                log_warn!("Leerer Druckauftrag ohne zugehörigen Auftrag ignoriert.");
            }
            return;
        }
        let _ = self.job_tx.send((id, data, job.map(|j| j.document)));
    }
}

fn lock(cfg: &Arc<Mutex<Config>>) -> Config {
    cfg.lock().map(|c| c.clone()).unwrap_or_default()
}

fn read_all(h: HANDLE) -> Result<Vec<u8>> {
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
    p: &PrinterConfig,
    data: &[u8],
    doc: Option<&str>,
    events: &Events,
    print_tx: &Sender<(PathBuf, String)>,
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

    let now = chrono::Local::now();
    let stem = logic::render_pattern(
        &p.filename_pattern,
        &now.format("%Y-%m-%d").to_string(),
        &now.format("%H-%M-%S").to_string(),
        &titel,
    );
    let stem = logic::safe_name(&stem, 150);

    // In jedem Speicherort ablegen; ein nicht erreichbarer Ordner verhindert die anderen nicht
    let mut saved: Vec<PathBuf> = Vec::new();
    for dir in p.all_dirs() {
        match save_to(Path::new(&dir), &stem, data) {
            Ok(dst) => {
                log_info!("Gespeichert ({}): {}", p.name, dst.display());
                saved.push(dst);
            }
            Err(e) => {
                log_error!("Speicherort {dir} ({}): {e:#}", p.name);
                (events.as_ref())(Event::SaveFailed(dir, format!("{e:#}")));
            }
        }
    }
    let Some(first) = saved.first().cloned() else {
        bail!("Die PDF konnte in keinem Speicherort abgelegt werden.");
    };

    if p.open_after_save {
        ui::open(&first);
    }
    (events.as_ref())(Event::Saved(first.clone(), saved.len()));
    if p.print_after_save {
        match resolve_print_printer(c, p) {
            Ok(target) => {
                let _ = print_tx.send((first, target));
            }
            Err(e) => (events.as_ref())(Event::PrintFailed(first, format!("{e:#}"))),
        }
    }
    Ok(())
}

/// Legt die PDF in `dir` ab (eindeutiger Name, erst als .part, dann umbenannt).
fn save_to(dir: &Path, stem: &str, data: &[u8]) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("Ordner {} nicht erreichbar oder nicht anlegbar", dir.display()))?;
    let dst = logic::unique_path(dir, stem);
    // Erst unter Zwischennamen schreiben, damit z. B. Nextcloud keine halbe Datei synchronisiert
    let part = dst.with_extension("pdf.part");
    std::fs::write(&part, data).with_context(|| format!("{} nicht schreibbar", part.display()))?;
    if let Err(e) = std::fs::rename(&part, &dst) {
        let _ = std::fs::remove_file(&part);
        return Err(e).with_context(|| format!("{} nicht anlegbar", dst.display()));
    }
    Ok(dst)
}

/// Zieldrucker für den zusätzlichen Ausdruck; nie ein PDF-Ablage-Drucker (Endlosschleife).
pub fn resolve_print_printer(c: &Config, p: &PrinterConfig) -> Result<String> {
    let name = if p.print_printer.trim().is_empty() {
        spooler::default_printer().context("Es ist kein Windows-Standarddrucker eingestellt.")?
    } else {
        p.print_printer.clone()
    };
    if c.is_own_printer(&name) {
        bail!(
            "'{name}' ist selbst ein PDF-Ablage-Drucker – das ergäbe eine Endlosschleife.\n\
             Bitte in den Einstellungen bei „{}“ einen echten Drucker wählen.",
            p.name
        );
    }
    Ok(name)
}

/// Hintergrund-Thread, der PDFs der Reihe nach ausdruckt.
fn start_print_worker(events: Events) -> Sender<(PathBuf, String)> {
    let (tx, rx) = mpsc::channel::<(PathBuf, String)>();
    thread::spawn(move || {
        let mut printer: Option<pdf::PdfPrinter> = None;
        for (path, target) in rx {
            match print_one(&mut printer, &path, &target) {
                Ok(pages) => {
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

fn print_one(printer: &mut Option<pdf::PdfPrinter>, path: &Path, target: &str) -> Result<usize> {
    if printer.is_none() {
        *printer = Some(pdf::PdfPrinter::new()?);
    }
    let doc_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    printer.as_ref().context("pdfium nicht geladen")?.print(path, target, &doc_name)
}

/// Empfang ohne Tray (Befehl "run").
pub fn run_console() -> Result<i32> {
    let cfg = Arc::new(Mutex::new(config::load()));
    let _receiver = Receiver::start(cfg, Arc::new(|_| {}));
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
