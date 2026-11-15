//! Symbol im Infobereich der Taskleiste: Menü, Benachrichtigungen, Updates.

use crate::config::{self, Config, APP_NAME, APP_VERSION};
use crate::util::{copy_into, wide};
use crate::watch::{Event, Events, Receiver};
use crate::{install, settings, ui, update};
use anyhow::{bail, Result};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_ERROR, NIIF_INFO, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW, FindWindowW, GetCursorPos,
    GetMessageW, GetSystemMetrics, LoadIconW, LoadImageW, PostMessageW, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, SetForegroundWindow, SetMenuDefaultItem, TrackPopupMenu, TranslateMessage, HICON, HMENU,
    IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING,
    MF_UNCHECKED, MSG, SM_CXSMICON, SM_CYSMICON, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP,
    WM_CONTEXTMENU, WM_LBUTTONDBLCLK, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
};

const WM_TRAY: u32 = WM_APP + 1;
const WM_NOTICE: u32 = WM_APP + 2;
const WM_APP_QUIT: u32 = WM_APP + 3;
/// Einstellungen öffnen (auch von einem zweiten Programmstart aus gesendet)
const WM_OPEN_SETTINGS: u32 = WM_APP + 4;
const CLASS: &str = "PDFAblageTrayWnd";

// Menü-Befehle
const ID_SETTINGS: usize = 10;
const ID_OPEN_FIRST: usize = 100; // 100.. = Zielordner von Drucker 1..5
const ID_NOTIFY: usize = 22;
const ID_AUTOSTART: usize = 23;
const ID_REPAIR: usize = 30;
const ID_UNINSTALL: usize = 31;
const ID_LOG: usize = 32;
const ID_UPDATE: usize = 40;
const ID_AUTOUPDATE: usize = 41;
const ID_QUIT: usize = 99;

struct State {
    cfg: Arc<Mutex<Config>>,
    icon: usize,
    taskbar_created: u32,
    receiver: OnceLock<Arc<Receiver>>,
}

static STATE: OnceLock<State> = OnceLock::new();
static HWND_VAL: AtomicUsize = AtomicUsize::new(0);
static UPDATE_RUNNING: AtomicBool = AtomicBool::new(false);
static REPAIR_RUNNING: AtomicBool = AtomicBool::new(false);
/// Zuletzt gemeldete Probleme – dieselben Probleme lösen nicht alle paar Minuten erneut eine
/// Administrator-Abfrage aus, wenn sie einmal abgelehnt wurde.
static LAST_PROBLEMS: Mutex<String> = Mutex::new(String::new());
/// Abstand der Hintergrundprüfung
const CHECK_INTERVAL: Duration = Duration::from_secs(5 * 60);

struct Notice {
    title: String,
    text: String,
    error: bool,
}

fn state() -> &'static State {
    STATE.get().expect("Tray nicht initialisiert")
}

fn cfg() -> Config {
    state().cfg.lock().map(|c| c.clone()).unwrap_or_default()
}

fn update_cfg(f: impl FnOnce(&mut Config)) {
    if let Ok(mut c) = state().cfg.lock() {
        f(&mut c);
        if let Err(e) = config::save(&c) {
            log_error!("Einstellungen nicht gespeichert: {e:#}");
            ui::message(&format!("Konnte die Einstellung nicht speichern:\n{e:#}"), true);
        }
    }
}

/// Benachrichtigung anzeigen (aus jedem Thread aufrufbar).
pub fn post_notice(title: &str, text: &str, error: bool) {
    let hwnd = HWND_VAL.load(Ordering::Relaxed);
    if hwnd == 0 {
        return;
    }
    let ptr = Box::into_raw(Box::new(Notice { title: title.into(), text: text.into(), error }));
    if unsafe { PostMessageW(hwnd as HWND, WM_NOTICE, 0, ptr as LPARAM) } == 0 {
        drop(unsafe { Box::from_raw(ptr) });
    }
}

fn quit() {
    let hwnd = HWND_VAL.load(Ordering::Relaxed);
    if hwnd != 0 {
        unsafe { PostMessageW(hwnd as HWND, WM_APP_QUIT, 0, 0) };
    }
}

fn single_instance(wait: Duration) -> bool {
    let name = wide("Global\\PDFAblageTray"); // gleicher Name wie im Installer
    let deadline = Instant::now() + wait;
    loop {
        let h = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
        let err = unsafe { GetLastError() };
        if !h.is_null() && err != ERROR_ALREADY_EXISTS {
            return true; // Handle bleibt bis Programmende offen
        }
        if !h.is_null() {
            unsafe { CloseHandle(h) };
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn nid(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut n: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    n.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    n.hWnd = hwnd;
    n.uID = 1;
    n
}

fn add_icon(hwnd: HWND) {
    let mut n = nid(hwnd);
    n.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    n.uCallbackMessage = WM_TRAY;
    n.hIcon = state().icon as HICON;
    copy_into(&mut n.szTip, &format!("{APP_NAME} {APP_VERSION}"));
    unsafe { Shell_NotifyIconW(NIM_ADD, &n) };
}

fn remove_icon(hwnd: HWND) {
    let n = nid(hwnd);
    unsafe { Shell_NotifyIconW(NIM_DELETE, &n) };
}

fn balloon(hwnd: HWND, notice: &Notice) {
    let mut n = nid(hwnd);
    n.uFlags = NIF_INFO;
    copy_into(&mut n.szInfoTitle, &notice.title);
    copy_into(&mut n.szInfo, &notice.text);
    n.dwInfoFlags = if notice.error { NIIF_ERROR } else { NIIF_INFO };
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &n) };
}

fn load_icon() -> HICON {
    unsafe {
        let hinst = GetModuleHandleW(null());
        let icon = LoadImageW(
            hinst,
            1usize as *const u16, // Ressourcen-ID 1 (build.rs)
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        ) as HICON;
        if icon.is_null() {
            LoadIconW(null_mut(), IDI_APPLICATION)
        } else {
            icon
        }
    }
}

unsafe fn item(menu: HMENU, flags: u32, id: usize, text: &str) {
    let w = wide(text);
    AppendMenuW(menu, flags, id, w.as_ptr());
}

unsafe fn check(menu: HMENU, id: usize, text: &str, on: bool) {
    item(menu, MF_STRING | if on { MF_CHECKED } else { MF_UNCHECKED }, id, text);
}

unsafe fn separator(menu: HMENU) {
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
}

fn show_menu(hwnd: HWND) {
    let c = cfg();
    let cmd = unsafe {
        let menu = CreatePopupMenu();
        item(menu, MF_STRING | MF_GRAYED, 1, &format!("{APP_NAME} {APP_VERSION}"));
        separator(menu);
        item(menu, MF_STRING, ID_SETTINGS, "Einstellungen …");
        if c.printers.len() <= 1 {
            item(menu, MF_STRING, ID_OPEN_FIRST, "Zielordner öffnen");
        } else {
            let sub = CreatePopupMenu();
            for (i, p) in c.printers.iter().enumerate() {
                item(sub, MF_STRING, ID_OPEN_FIRST + i, &p.name);
            }
            item(menu, MF_STRING | MF_POPUP, sub as usize, "Zielordner öffnen");
        }
        SetMenuDefaultItem(menu, ID_SETTINGS as u32, 0);
        separator(menu);
        check(menu, ID_NOTIFY, "Benachrichtigung anzeigen", c.notify);
        check(menu, ID_AUTOSTART, "Mit Windows starten", install::autostart_enabled());
        separator(menu);
        item(menu, MF_STRING, ID_REPAIR, "Drucker prüfen / reparieren …");
        item(menu, MF_STRING, ID_UNINSTALL, "Alle Drucker entfernen …");
        item(menu, MF_STRING, ID_LOG, "Protokoll öffnen");
        separator(menu);
        item(menu, MF_STRING, ID_UPDATE, "Nach Updates suchen …");
        check(menu, ID_AUTOUPDATE, "Automatisch nach Updates suchen", c.auto_update_check);
        separator(menu);
        item(menu, MF_STRING, ID_QUIT, "Beenden");

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu); // zerstört auch das Untermenü
        cmd
    };
    if cmd > 0 {
        handle_command(cmd as usize);
    }
}

fn open_target(index: usize) {
    let c = cfg();
    let Some(p) = c.printers.get(index) else { return };
    let _ = std::fs::create_dir_all(&p.target_dir);
    ui::open(&p.target_dir);
}

fn open_settings() {
    let st = state();
    let receiver = st.receiver.get().cloned();
    let hook: settings::Hook = Arc::new(move || {
        if let Some(r) = &receiver {
            r.ensure_servers();
        }
    });
    settings::open(st.cfg.clone(), Some(hook), false);
}

fn handle_command(id: usize) {
    match id {
        ID_SETTINGS => open_settings(),
        n if (ID_OPEN_FIRST..ID_OPEN_FIRST + config::MAX_PRINTERS).contains(&n) => open_target(n - ID_OPEN_FIRST),
        ID_NOTIFY => update_cfg(|c| c.notify = !c.notify),
        ID_AUTOSTART => {
            if let Err(e) = install::set_autostart(!install::autostart_enabled()) {
                ui::message(&format!("Autostart konnte nicht geändert werden:\n{e:#}"), true);
            }
        }
        ID_REPAIR => {
            thread::spawn(|| check_and_repair(true));
        }
        ID_UNINSTALL => {
            if ui::ask(
                "Alle PDF-Ablage-Drucker aus Windows entfernen?\n\nDie Einstellungen und gespeicherten PDFs bleiben erhalten; \
                 über „Drucker prüfen / reparieren …“ lassen sich die Drucker wieder anlegen.",
                APP_NAME,
            ) {
                ui::run_elevated("uninstall");
            }
        }
        ID_LOG => ui::open(&config::log_file()),
        ID_UPDATE => {
            thread::spawn(|| update_check(false));
        }
        ID_AUTOUPDATE => update_cfg(|c| c.auto_update_check = !c.auto_update_check),
        ID_QUIT => quit(),
        _ => {}
    }
}

fn update_check(silent: bool) {
    if UPDATE_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            UPDATE_RUNNING.store(false, Ordering::SeqCst);
        }
    }
    let _reset = Reset;

    if config::is_dev_build() {
        if !silent {
            ui::message("Dies ist eine Entwicklungsversion – Updates gibt es nur für veröffentlichte Versionen.", false);
        }
        return;
    }
    let c = cfg();
    match update::check_update(&c) {
        Err(e) => {
            log_warn!("Update-Prüfung fehlgeschlagen: {e:#}");
            if !silent {
                ui::message(&format!("Update-Prüfung fehlgeschlagen:\n{e:#}"), true);
            }
        }
        Ok(None) => {
            if !silent {
                ui::message(&format!("Du hast bereits die neueste Version ({APP_VERSION})."), false);
            }
        }
        Ok(Some(info)) => {
            let mut notes: String = info.notes.chars().take(600).collect();
            if info.notes.chars().count() > 600 {
                notes.push('…');
            }
            let text = format!(
                "Version {} ist verfügbar (installiert: {APP_VERSION}).\n\n{}Jetzt herunterladen und installieren?",
                info.version,
                if notes.is_empty() { String::new() } else { format!("{notes}\n\n") }
            );
            if !ui::ask(&text, &format!("{APP_NAME} – Update")) {
                return;
            }
            post_notice(APP_NAME, "Update wird heruntergeladen …", false);
            match update::download_and_run(&info, &c) {
                Ok(()) => quit(), // Setup wartet, bis die App beendet ist
                Err(e) => {
                    log_error!("Update fehlgeschlagen: {e:#}");
                    ui::message(&format!("Update fehlgeschlagen:\n{e:#}"), true);
                }
            }
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if let Some(st) = STATE.get() {
        if st.taskbar_created != 0 && msg == st.taskbar_created {
            add_icon(hwnd); // Explorer wurde neu gestartet
            return 0;
        }
    }
    match msg {
        WM_TRAY => {
            match (lp as u32) & 0xFFFF {
                WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                WM_LBUTTONDBLCLK => open_settings(),
                _ => {}
            }
            0
        }
        WM_NOTICE => {
            let notice = Box::from_raw(lp as *mut Notice);
            balloon(hwnd, &notice);
            0
        }
        WM_OPEN_SETTINGS => {
            open_settings();
            0
        }
        WM_APP_QUIT => {
            remove_icon(hwnd);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn create_window() -> Result<HWND> {
    unsafe {
        let hinst = GetModuleHandleW(null());
        let class = wide(CLASS);
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = hinst;
        wc.lpszClassName = class.as_ptr();
        RegisterClassW(&wc);
        let title = wide(APP_NAME);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinst,
            null(),
        );
        if hwnd.is_null() {
            bail!("Fenster für das Tray-Symbol konnte nicht erstellt werden.");
        }
        Ok(hwnd)
    }
}

/// "Mit Windows starten" ist Standard: einmalig einschalten (auch nach einem Update).
/// Schaltet man es danach im Menü aus, bleibt es aus.
fn apply_autostart_default(cfg: &Arc<Mutex<Config>>) {
    let Ok(mut c) = cfg.lock() else { return };
    if c.autostart_default_applied {
        return;
    }
    match install::set_autostart(true) {
        Ok(()) => {
            log_info!("Autostart eingeschaltet (Standard).");
            c.autostart_default_applied = true;
            if let Err(e) = config::save(&c) {
                log_warn!("Einstellungen nicht gespeichert: {e:#}");
            }
        }
        Err(e) => log_warn!("Autostart konnte nicht eingeschaltet werden: {e:#}"),
    }
}

/// Prüft Drucker, Anschluss und Treiber und repariert sie bei Bedarf (Administrator-Abfrage).
///
/// `manual`: über das Tray-Menü angestoßen – dann gibt es immer eine Rückmeldung, auch wenn
/// alles stimmt, und die Reparatur wird auch bei schon einmal gemeldeten Problemen versucht.
fn check_and_repair(manual: bool) {
    if REPAIR_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            REPAIR_RUNNING.store(false, Ordering::SeqCst);
        }
    }
    let _reset = Reset;

    let c = cfg();
    let problems = install::windows_problems(&c);
    let mut last = LAST_PROBLEMS.lock().unwrap_or_else(|e| e.into_inner());
    if problems.is_empty() {
        if !last.is_empty() {
            log_info!("Druckerprüfung: wieder alles in Ordnung.");
        }
        last.clear();
        if manual {
            post_notice("Drucker geprüft", "Alle PDF-Ablage-Drucker sind korrekt eingerichtet.", false);
        }
        return;
    }
    let text = problems.join("\n");
    if !manual {
        // Gerade läuft eine Einrichtung aus dem Einstellungsfenster, oder dieselben Probleme
        // wurden schon gemeldet (Abfrage abgelehnt) – dann nicht erneut nachfragen
        if install::sync_recent() || *last == text {
            return;
        }
    }
    *last = text.clone();
    drop(last);
    log_warn!("Druckerprüfung: {}", problems.join(" | "));

    let first_setup = c.printers.iter().all(|p| p.installed_name.is_empty());
    if !manual && ui::no_gui() {
        return; // Testumgebung ohne Fenster: nur protokollieren
    }
    let (title, hint) = if first_setup {
        ("Drucker einrichten", "Bitte die Administrator-Abfrage bestätigen.")
    } else {
        ("Drucker wird repariert", "Windows hat die Einrichtung verändert. Bitte die Administrator-Abfrage bestätigen.")
    };
    post_notice(title, &format!("{text}\n{hint}"), !first_setup);

    match ui::run_elevated_wait("reparieren", Duration::from_secs(10 * 60)) {
        None => {
            log_warn!("Reparatur nicht ausgeführt (Administrator-Abfrage abgelehnt oder nicht möglich).");
            post_notice(
                "Drucker nicht repariert",
                "Ohne Administratorrechte geht es nicht. Bis dahin speichert PDF-Ablage nicht automatisch. \
                 Erneut versuchen: Tray-Menü → „Drucker prüfen / reparieren …“",
                true,
            );
        }
        Some(code) => {
            let after = install::windows_problems(&cfg());
            if after.is_empty() {
                log_info!("Drucker repariert (Exit {code}).");
                if let Ok(mut l) = LAST_PROBLEMS.lock() {
                    l.clear();
                }
                let done = if first_setup { "Drucker eingerichtet" } else { "Drucker repariert" };
                post_notice(done, "PDF-Ablage speichert wieder automatisch.", false);
            } else {
                log_error!("Reparatur fehlgeschlagen (Exit {code}): {}", after.join(" | "));
                post_notice(
                    "Reparatur fehlgeschlagen",
                    &format!("{}\nDetails: Tray-Menü → „Protokoll öffnen“", after.join("\n")),
                    true,
                );
            }
        }
    }
}

/// Prüft beim Start und danach regelmäßig im Hintergrund die Einrichtung in Windows.
fn start_printer_watch() {
    thread::spawn(|| loop {
        check_and_repair(false);
        thread::sleep(CHECK_INTERVAL);
    });
}

/// Läuft die Tray-App schon? Dann dort die Einstellungen öffnen.
pub fn signal_running_instance() -> bool {
    let hwnd = unsafe { FindWindowW(wide(CLASS).as_ptr(), null()) };
    if hwnd.is_null() {
        return false;
    }
    unsafe { PostMessageW(hwnd, WM_OPEN_SETTINGS, 0, 0) != 0 }
}

pub fn run(after_update: bool) -> Result<i32> {
    let wait = if after_update { Duration::from_secs(20) } else { Duration::ZERO };
    if !single_instance(wait) {
        // Zweiter Start (z. B. über das Startmenü): Einstellungen der laufenden App öffnen
        if !signal_running_instance() {
            ui::message(&format!("{APP_NAME} läuft bereits (Symbol im Infobereich der Taskleiste)."), false);
        }
        return Ok(0);
    }
    log_info!("{APP_NAME} {APP_VERSION} gestartet");
    install::migrate_old_autostart();

    let cfg = Arc::new(Mutex::new(config::load()));
    apply_autostart_default(&cfg);
    let _ = STATE.set(State {
        cfg: cfg.clone(),
        icon: load_icon() as usize,
        taskbar_created: unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) },
        receiver: OnceLock::new(),
    });

    let hwnd = create_window()?;
    HWND_VAL.store(hwnd as usize, Ordering::Relaxed);
    add_icon(hwnd);

    let events: Events = {
        let cfg = cfg.clone();
        Arc::new(move |e: Event| {
            let notify = cfg.lock().map(|c| c.notify).unwrap_or(true);
            let name = |p: &std::path::Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            match e {
                Event::Saved(p, 1) if notify => post_notice("PDF gespeichert", &name(&p), false),
                Event::Saved(p, n) if notify => {
                    post_notice("PDF gespeichert", &format!("{}\nin {n} Speicherorten", name(&p)), false)
                }
                // Fehler immer melden, auch wenn Benachrichtigungen aus sind
                Event::SaveFailed(dir, err) => post_notice("Speicherort nicht erreichbar", &format!("{dir}\n{err}"), true),
                Event::Printed(p, printer) if notify => {
                    post_notice("Ausgedruckt", &format!("{}\n→ {printer}", name(&p)), false)
                }
                // Fehler immer melden, auch wenn Benachrichtigungen aus sind
                Event::PrintFailed(p, err) => {
                    post_notice("Drucken fehlgeschlagen", &format!("{}\n{err}", name(&p)), true)
                }
                _ => {}
            }
        })
    };
    let _ = state().receiver.set(Receiver::start(cfg.clone(), events));
    start_printer_watch();
    {
        let cfg = cfg.clone();
        thread::spawn(move || {
            if after_update {
                post_notice(APP_NAME, &format!("Auf Version {APP_VERSION} aktualisiert."), false);
            } else if cfg.lock().map(|c| c.auto_update_check).unwrap_or(false) && !config::is_dev_build() {
                thread::sleep(Duration::from_secs(5)); // Start nicht ausbremsen
                update_check(true);
            }
        });
    }

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if settings::is_dialog_message(&msg) {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    log_info!("Beendet.");
    Ok(0)
}
