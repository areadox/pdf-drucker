//! Meldungen, Rückfragen, Konsole, Dateien öffnen, Adminrechte.

use crate::config::APP_NAME;
use crate::util::wide;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Console::{AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE};
use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONQUESTION, MB_SETFOREGROUND, MB_TOPMOST,
    MB_YESNO, SW_SHOWNORMAL,
};

static CONSOLE: AtomicBool = AtomicBool::new(false);

/// Für Kommandozeilen-Aufrufe: Ausgaben in die aufrufende Konsole statt in Fenster.
pub fn attach_console() {
    unsafe {
        let h = GetStdHandle(STD_OUTPUT_HANDLE);
        if !h.is_null() && h != INVALID_HANDLE_VALUE {
            CONSOLE.store(true, Ordering::Relaxed);
        } else if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            CONSOLE.store(true, Ordering::Relaxed);
        }
    }
}

pub fn has_console() -> bool {
    CONSOLE.load(Ordering::Relaxed)
}

/// Keine Fenster anzeigen (Konsole vorhanden oder PDF_ABLAGE_NO_GUI gesetzt, z. B. im Build-Test).
pub fn no_gui() -> bool {
    has_console() || std::env::var_os("PDF_ABLAGE_NO_GUI").is_some()
}

pub fn message(text: &str, error: bool) {
    if no_gui() {
        if error {
            eprintln!("{text}");
        } else {
            println!("{text}");
        }
        return;
    }
    let icon = if error { MB_ICONERROR } else { MB_ICONINFORMATION };
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(text).as_ptr(),
            wide(APP_NAME).as_ptr(),
            icon | MB_TOPMOST | MB_SETFOREGROUND,
        );
    }
}

pub fn ask(text: &str, title: &str) -> bool {
    if no_gui() {
        return false;
    }
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(text).as_ptr(),
            wide(title).as_ptr(),
            MB_YESNO | MB_ICONQUESTION | MB_TOPMOST | MB_SETFOREGROUND,
        ) == IDYES
    }
}

/// Datei oder Ordner mit dem zugehörigen Programm öffnen.
pub fn open<P: AsRef<std::ffi::OsStr> + ?Sized>(path: &P) {
    unsafe {
        ShellExecuteW(null_mut(), wide("open").as_ptr(), wide(path).as_ptr(), null(), null(), SW_SHOWNORMAL);
    }
}

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

/// Startet dieses Programm mit Adminrechten (UAC-Abfrage) und den angegebenen Argumenten.
pub fn run_elevated(args: &str) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let r = unsafe {
        ShellExecuteW(
            null_mut(),
            wide("runas").as_ptr(),
            wide(&exe).as_ptr(),
            wide(args).as_ptr(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    r as isize > 32
}
