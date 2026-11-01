//! Zugriff auf die Windows-Druckwarteschlange (Druckerliste, Druckaufträge, Dokumentnamen).
//!
//! Der PDF-Ablage-Drucker ist auf "Gedruckte Dokumente beibehalten" gestellt. Dadurch bleibt
//! der Druckauftrag nach dem Schreiben der PDF in der Warteschlange, und wir können seinen
//! Dokumentnamen zuverlässig auslesen. Danach wird der Auftrag gelöscht.

use crate::util::{from_wide_ptr, wide};
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::time::Duration;
use windows_sys::Win32::Graphics::Printing::{
    ClosePrinter, EnumJobsW, EnumPrintersW, GetDefaultPrinterW, GetPrinterW, OpenPrinterW, SetJobW,
    JOB_INFO_1W, PRINTER_INFO_2W, PRINTER_INFO_4W,
};

const JOB_STATUS_PRINTED: u32 = 0x0000_0080;
const JOB_STATUS_COMPLETE: u32 = 0x0000_1000;
const JOB_CONTROL_DELETE: u32 = 5;
const PRINTER_ATTRIBUTE_KEEPPRINTEDJOBS: u32 = 0x0000_0100;
const PRINTER_ENUM_LOCAL: u32 = 0x0000_0002;
const PRINTER_ENUM_CONNECTIONS: u32 = 0x0000_0004;

#[derive(Clone, Debug)]
pub struct Job {
    pub id: u32,
    pub status: u32,
    pub document: String,
}

impl Job {
    fn finished(&self) -> bool {
        self.status & (JOB_STATUS_PRINTED | JOB_STATUS_COMPLETE) != 0
    }
}

/// Ruft eine "zweimal aufrufen"-Funktion der Druck-API auf (erst Größe, dann Daten).
/// Gibt den (ausgerichteten) Puffer und die Anzahl der Einträge zurück.
fn two_pass(f: impl Fn(*mut u8, u32, &mut u32, &mut u32) -> i32) -> Option<(Vec<u64>, u32)> {
    let mut needed = 0u32;
    let mut count = 0u32;
    f(null_mut(), 0, &mut needed, &mut count);
    for _ in 0..3 {
        if needed == 0 {
            return Some((Vec::new(), 0));
        }
        let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
        let cb = needed;
        if f(buf.as_mut_ptr() as *mut u8, cb, &mut needed, &mut count) != 0 {
            return Some((buf, count));
        }
    }
    None
}

pub struct Printer(*mut c_void);

impl Printer {
    pub fn open(name: &str) -> Option<Printer> {
        let w = wide(name);
        let mut h: *mut c_void = null_mut();
        let ok = unsafe { OpenPrinterW(w.as_ptr(), &mut h, null()) };
        if ok == 0 || h.is_null() {
            None
        } else {
            Some(Printer(h))
        }
    }

    pub fn jobs(&self) -> Vec<Job> {
        let h = self.0;
        let Some((buf, count)) =
            two_pass(|p, cb, need, ret| unsafe { EnumJobsW(h, 0, 1000, 1, p, cb, need, ret) })
        else {
            return Vec::new();
        };
        if count == 0 {
            return Vec::new();
        }
        let infos = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const JOB_INFO_1W, count as usize) };
        infos
            .iter()
            .map(|j| Job { id: j.JobId, status: j.Status, document: unsafe { from_wide_ptr(j.pDocument) } })
            .collect()
    }

    pub fn delete_job(&self, id: u32) -> bool {
        unsafe { SetJobW(self.0, id, 0, null(), JOB_CONTROL_DELETE) != 0 }
    }

    pub fn keeps_printed_jobs(&self) -> bool {
        let h = self.0;
        let Some((buf, _)) = two_pass(|p, cb, need, _| unsafe { GetPrinterW(h, 2, p, cb, need) }) else {
            return false;
        };
        if buf.is_empty() {
            return false;
        }
        let info = unsafe { &*(buf.as_ptr() as *const PRINTER_INFO_2W) };
        info.Attributes & PRINTER_ATTRIBUTE_KEEPPRINTEDJOBS != 0
    }
}

impl Drop for Printer {
    fn drop(&mut self) {
        unsafe {
            ClosePrinter(self.0);
        }
    }
}

pub fn printer_exists(name: &str) -> bool {
    Printer::open(name).is_some()
}

pub fn keeps_printed_jobs(name: &str) -> bool {
    Printer::open(name).map(|p| p.keeps_printed_jobs()).unwrap_or(false)
}

/// Alle installierten Drucker (lokal und Netzwerk), alphabetisch, optional ohne einen bestimmten.
pub fn list_printers(exclude: Option<&str>) -> Vec<String> {
    let Some((buf, count)) = two_pass(|p, cb, need, ret| unsafe {
        EnumPrintersW(PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS, null(), 4, p, cb, need, ret)
    }) else {
        return Vec::new();
    };
    if count == 0 {
        return Vec::new();
    }
    let infos = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, count as usize) };
    let mut names: Vec<String> = infos.iter().map(|p| unsafe { from_wide_ptr(p.pPrinterName) }).collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names.retain(|n| !n.is_empty() && exclude.map_or(true, |ex| !n.eq_ignore_ascii_case(ex)));
    names
}

pub fn default_printer() -> Option<String> {
    let mut size = 0u32;
    unsafe { GetDefaultPrinterW(null_mut(), &mut size) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u16; size as usize];
    if unsafe { GetDefaultPrinterW(buf.as_mut_ptr(), &mut size) } == 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

/// Dokumentname des zuletzt fertig gedruckten Auftrags; räumt erledigte Aufträge auf.
pub fn take_document_name(printer: &str) -> Option<String> {
    let p = Printer::open(printer)?;
    let mut chosen: Option<Job> = None;
    for attempt in 0..10 {
        let jobs = p.jobs();
        if let Some(j) = jobs.iter().filter(|j| j.finished()).max_by_key(|j| j.id) {
            chosen = Some(j.clone());
            break;
        }
        // Ohne "Gedruckte Dokumente beibehalten" ist die Warteschlange meist schon leer
        if jobs.is_empty() && attempt >= 2 {
            break;
        }
        if attempt == 9 {
            chosen = jobs.iter().max_by_key(|j| j.id).cloned();
            break;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    let job = chosen?;
    for j in p.jobs() {
        if j.finished() && j.id <= job.id {
            p.delete_job(j.id);
        }
    }
    Some(job.document)
}
