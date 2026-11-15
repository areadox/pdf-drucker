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

/// Startet dieses Programm mit Adminrechten (UAC-Abfrage), wartet auf das Ende und gibt den
/// Exit-Code zurück. `None`, wenn die Abfrage abgelehnt wurde oder der Start scheiterte.
pub fn run_elevated_wait(args: &str, timeout: std::time::Duration) -> Option<u32> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let exe = std::env::current_exe().ok()?;
    let (verb, file, params) = (wide("runas"), wide(&exe), wide(args));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_HIDE;
    unsafe {
        if ShellExecuteExW(&mut info) == 0 || info.hProcess.is_null() {
            return None;
        }
        WaitForSingleObject(info.hProcess, timeout.as_millis().min(u32::MAX as u128) as u32);
        let mut code = 1u32;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        Some(code)
    }
}

/// Pfad der .exe des Fensters im Vordergrund (zum Erkennen des druckenden Programms).
pub fn foreground_exe() -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}
