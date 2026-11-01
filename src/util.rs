//! Hilfen für UTF-16-Zeichenketten der Windows-API.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// Nullterminierte UTF-16-Zeichenkette für die Windows-API.
pub fn wide<S: AsRef<OsStr> + ?Sized>(s: &S) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// Liest eine nullterminierte UTF-16-Zeichenkette.
///
/// # Safety
/// `p` muss null sein oder auf eine gültige, nullterminierte Zeichenkette zeigen.
pub unsafe fn from_wide_ptr(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

/// Kopiert Text (gekürzt) in einen festen UTF-16-Puffer, z. B. für NOTIFYICONDATAW.
pub fn copy_into(buf: &mut [u16], text: &str) {
    let max = buf.len().saturating_sub(1);
    let units: Vec<u16> = text.encode_utf16().take(max).collect();
    buf[..units.len()].copy_from_slice(&units);
    buf[units.len()] = 0;
}
