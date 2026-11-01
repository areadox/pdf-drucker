//! PDF ausdrucken: Seiten mit pdfium rendern und über die Windows-Druckschnittstelle (GDI) ausgeben.

use crate::logic;
use crate::util::wide;
use anyhow::{anyhow, bail, Context, Result};
use pdfium_render::prelude::*;
use std::path::Path;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, GetDeviceCaps, SetBrushOrgEx, SetStretchBltMode, StretchDIBits, BITMAPINFO, DIB_RGB_COLORS,
    HALFTONE, HDC, LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH,
    SRCCOPY,
};
use windows_sys::Win32::Storage::Xps::{AbortDoc, EndDoc, EndPage, StartDocW, StartPage, DOCINFOW};

const MAX_DPI: i32 = 300;

fn pe<E: std::fmt::Debug>(e: E) -> anyhow::Error {
    anyhow!("{e:?}")
}

struct Dc(HDC);
impl Drop for Dc {
    fn drop(&mut self) {
        unsafe {
            DeleteDC(self.0);
        }
    }
}

pub struct PdfPrinter {
    pdfium: Pdfium,
}

impl PdfPrinter {
    /// Lädt pdfium.dll aus dem Programmordner.
    pub fn new() -> Result<Self> {
        let exe = std::env::current_exe()?;
        let dir = exe.parent().context("Programmordner unbekannt")?.to_path_buf();
        let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir))
            .or_else(|_| Pdfium::bind_to_system_library())
            .map_err(|e| anyhow!("pdfium.dll konnte nicht geladen werden ({e:?})"))?;
        Ok(Self { pdfium: Pdfium::new(bindings) })
    }

    pub fn page_count(&self, path: &Path) -> Result<usize> {
        let doc = self.pdfium.load_pdf_from_file(path, None).map_err(pe)?;
        Ok(doc.pages().len() as usize)
    }

    /// Druckt alle Seiten auf `printer`. Gibt die Seitenzahl zurück.
    pub fn print(&self, path: &Path, printer: &str, doc_name: &str) -> Result<usize> {
        let doc = self
            .pdfium
            .load_pdf_from_file(path, None)
            .map_err(|e| anyhow!("PDF konnte nicht geöffnet werden: {e:?}"))?;

        let hdc = unsafe { CreateDCW(wide("WINSPOOL").as_ptr(), wide(printer).as_ptr(), null(), null()) };
        if hdc.is_null() {
            bail!("Drucker '{printer}' ist nicht erreichbar.");
        }
        let dc = Dc(hdc);

        let caps = |i| unsafe { GetDeviceCaps(dc.0, i) };
        let dpi = caps(LOGPIXELSX).min(caps(LOGPIXELSY)).clamp(72, MAX_DPI);
        let (paper_w, paper_h) = (caps(PHYSICALWIDTH), caps(PHYSICALHEIGHT));
        let (off_x, off_y) = (caps(PHYSICALOFFSETX), caps(PHYSICALOFFSETY));
        if paper_w <= 0 || paper_h <= 0 {
            bail!("Drucker '{printer}' meldet kein Papierformat.");
        }

        let name = wide(doc_name);
        let info = DOCINFOW {
            cbSize: std::mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: name.as_ptr(),
            lpszOutput: null(),
            lpszDatatype: null(),
            fwType: 0,
        };
        if unsafe { StartDocW(dc.0, &info) } <= 0 {
            bail!("Druckauftrag auf '{printer}' konnte nicht gestartet werden.");
        }
        unsafe {
            SetStretchBltMode(dc.0, HALFTONE);
            SetBrushOrgEx(dc.0, 0, 0, null_mut());
        }

        let result = (|| -> Result<usize> {
            let config = PdfRenderConfig::new().scale_page_by_factor(dpi as f32 / 72.0);
            let mut pages = 0;
            for page in doc.pages().iter() {
                let bitmap = page.render_with_config(&config).map_err(pe)?;
                let (w, h) = (bitmap.width() as usize, bitmap.height() as usize);
                let raw = bitmap.as_raw_bytes();
                let raw: &[u8] = &raw;
                if w == 0 || h == 0 || raw.len() < w * h * 4 {
                    bail!("Seite {} konnte nicht dargestellt werden.", pages + 1);
                }
                let stride = raw.len() / h;
                let mut px = Vec::with_capacity(w * h);
                for row in 0..h {
                    let line = &raw[row * stride..row * stride + w * 4];
                    px.extend(line.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])));
                }
                // Querformat-Seite auf Hochformat-Papier (oder umgekehrt) drehen
                let (w, h, px) = if (w > h) != (paper_w > paper_h) { logic::rotate90(w, h, &px) } else { (w, h, px) };
                let (x, y, dw, dh) = logic::fit_on_paper(paper_w, paper_h, off_x, off_y, w, h);

                let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
                bmi.bmiHeader.biSize = std::mem::size_of_val(&bmi.bmiHeader) as u32;
                bmi.bmiHeader.biWidth = w as i32;
                bmi.bmiHeader.biHeight = -(h as i32); // von oben nach unten
                bmi.bmiHeader.biPlanes = 1;
                bmi.bmiHeader.biBitCount = 32;

                unsafe {
                    if StartPage(dc.0) <= 0 {
                        bail!("Seite {} konnte nicht begonnen werden.", pages + 1);
                    }
                    let lines = StretchDIBits(
                        dc.0,
                        x,
                        y,
                        dw,
                        dh,
                        0,
                        0,
                        w as i32,
                        h as i32,
                        px.as_ptr() as *const _,
                        &bmi,
                        DIB_RGB_COLORS,
                        SRCCOPY,
                    );
                    if lines == 0 {
                        bail!("Seite {} konnte nicht an den Drucker übergeben werden.", pages + 1);
                    }
                    if EndPage(dc.0) <= 0 {
                        bail!("Seite {} konnte nicht abgeschlossen werden.", pages + 1);
                    }
                }
                pages += 1;
            }
            Ok(pages)
        })();

        match result {
            Ok(pages) => {
                unsafe { EndDoc(dc.0) };
                Ok(pages)
            }
            Err(e) => {
                unsafe { AbortDoc(dc.0) };
                Err(e)
            }
        }
    }
}

pub fn write_test_pdf(path: &Path) -> Result<()> {
    std::fs::write(path, logic::test_pdf_bytes()).with_context(|| format!("{} nicht schreibbar", path.display()))
}

/// Prüft PDF-Darstellung und optional einen Testdruck (für den Build und zur Fehlersuche).
pub fn cmd_selftest(printer: Option<String>, result_file: Option<String>) -> Result<i32> {
    use crate::config::{APP_NAME, APP_VERSION};
    let mut lines = vec![format!("{APP_NAME} {APP_VERSION}")];
    let outcome = (|| -> Result<()> {
        let pdf = std::env::temp_dir().join("pdf-ablage-selftest.pdf");
        write_test_pdf(&pdf)?;
        let p = PdfPrinter::new()?;
        lines.push(format!("Darstellung ok: {} Seiten", p.page_count(&pdf)?));
        if let Some(printer) = &printer {
            let n = p.print(&pdf, printer, "PDF-Ablage Selbsttest")?;
            lines.push(format!("Druck ok: {n} Seiten an '{printer}'"));
        }
        Ok(())
    })();
    let code = match outcome {
        Ok(()) => 0,
        Err(e) => {
            lines.push(format!("FEHLER: {e:#}"));
            1
        }
    };
    let text = lines.join("\n");
    match result_file {
        Some(f) => std::fs::write(f, &text)?,
        None => crate::ui::message(&text, code != 0),
    }
    Ok(code)
}
