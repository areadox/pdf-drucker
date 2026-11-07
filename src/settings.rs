//! Einstellungsfenster: bis zu fünf PDF-Drucker anlegen, umbenennen, entfernen und konfigurieren.
//!
//! Läuft im Haupt-Thread der Tray-App (oder eigenständig über den Befehl "einstellungen").

use crate::config::{self, Config, PrinterConfig, APP_NAME, MAX_PRINTERS};
use crate::util::wide;
use crate::{install, spooler, ui};
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex, Once};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{CreateFontIndirectW, DeleteObject, HFONT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, SystemParametersInfoForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetDlgItem, GetMessageW,
    GetSystemMetrics, GetWindowTextLengthW, GetWindowTextW, IsDialogMessageW, LoadCursorW, LoadImageW,
    PostQuitMessage, RegisterClassW, SendMessageW, SetForegroundWindow, SetWindowTextW, ShowWindow,
    TranslateMessage, HMENU, IDC_ARROW, IMAGE_ICON, LR_DEFAULTCOLOR, MSG, NONCLIENTMETRICSW, SM_CXSCREEN,
    SM_CYSCREEN, SPI_GETNONCLIENTMETRICS, SW_RESTORE, SW_SHOW, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_SETFONT,
    WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP,
    WS_VISIBLE, WS_VSCROLL,
};

/// Wird nach dem Speichern aufgerufen (Tray startet dann den Empfang für neue Drucker).
pub type Hook = Arc<dyn Fn() + Send + Sync>;

// Steuerelemente
const IDOK: i32 = 1;
const IDCANCEL: i32 = 2;
const IDC_LIST: i32 = 101;
const IDC_ADD: i32 = 102;
const IDC_REMOVE: i32 = 103;
const IDC_NAME: i32 = 110;
const IDC_DIR: i32 = 111;
const IDC_BROWSE: i32 = 112;
const IDC_PATTERN: i32 = 113;
const IDC_OPEN: i32 = 114;
const IDC_PRINT: i32 = 115;
const IDC_TARGET: i32 = 116;
const IDC_SAVE: i32 = 120;
const IDC_CLOSE: i32 = 121;
const IDC_STATUS: i32 = 122;
const IDC_LABEL: i32 = 0xFFFF;

// Stile und Meldungen der Standard-Steuerelemente
const ES_AUTOHSCROLL: u32 = 0x0080;
const BS_DEFPUSHBUTTON: u32 = 0x0001;
const BS_AUTOCHECKBOX: u32 = 0x0003;
const LBS_NOTIFY: u32 = 0x0001;
const LBS_NOINTEGRALHEIGHT: u32 = 0x0100;
const CBS_DROPDOWNLIST: u32 = 0x0003;
const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;
const EM_SETSEL: u32 = 0x00B1;
const LB_ADDSTRING: u32 = 0x0180;
const LB_RESETCONTENT: u32 = 0x0184;
const LB_SETCURSEL: u32 = 0x0186;
const LB_GETCURSEL: u32 = 0x0188;
const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_RESETCONTENT: u32 = 0x014B;
const CB_SETCURSEL: u32 = 0x014E;
const BN_CLICKED: u32 = 0;
const LBN_SELCHANGE: u32 = 1;

const CLASS: &str = "PDFAblageSettingsWnd";

struct Settings {
    hwnd: HWND,
    font: HFONT,
    cfg: Arc<Mutex<Config>>,
    on_saved: Option<Hook>,
    standalone: bool,
    working: Vec<PrinterConfig>,
    current: usize,
    /// Einträge der Druckerauswahl ab Index 1 (Index 0 = Windows-Standarddrucker)
    targets: Vec<String>,
}

thread_local! {
    static SETTINGS: RefCell<Option<Settings>> = const { RefCell::new(None) };
}

/// Führt `f` mit dem Fensterzustand aus (nicht, wenn er gerade anderweitig benutzt wird).
fn with<R>(f: impl FnOnce(&mut Settings) -> R) -> Option<R> {
    SETTINGS.with(|s| s.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

fn item(hwnd: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(hwnd, id) }
}

fn send(hwnd: HWND, id: i32, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe { SendMessageW(item(hwnd, id), msg, wp, lp) }
}

fn text(hwnd: HWND, id: i32) -> String {
    unsafe {
        let h = item(hwnd, id);
        let len = GetWindowTextLengthW(h).max(0) as usize;
        let mut buf = vec![0u16; len + 1];
        let n = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buf[..n])
    }
}

fn set_text(hwnd: HWND, id: i32, s: &str) {
    unsafe { SetWindowTextW(item(hwnd, id), wide(s).as_ptr()) };
}

fn checked(hwnd: HWND, id: i32) -> bool {
    send(hwnd, id, BM_GETCHECK, 0, 0) == 1
}

fn set_checked(hwnd: HWND, id: i32, on: bool) {
    send(hwnd, id, BM_SETCHECK, on as usize, 0);
}

fn enable(hwnd: HWND, id: i32, on: bool) {
    unsafe { EnableWindow(item(hwnd, id), on as i32) };
}

impl Settings {
    /// Felder des gewählten Druckers in die Arbeitskopie übernehmen.
    fn store(&mut self) {
        let h = self.hwnd;
        let targets = self.targets.clone();
        let Some(p) = self.working.get_mut(self.current) else { return };
        p.name = text(h, IDC_NAME).trim().to_string();
        p.target_dir = text(h, IDC_DIR).trim().to_string();
        p.filename_pattern = text(h, IDC_PATTERN).trim().to_string();
        p.open_after_save = checked(h, IDC_OPEN);
        p.print_after_save = checked(h, IDC_PRINT);
        let sel = send(h, IDC_TARGET, CB_GETCURSEL, 0, 0);
        p.print_printer = if sel <= 0 { String::new() } else { targets.get(sel as usize - 1).cloned().unwrap_or_default() };
    }

    /// Felder mit dem gewählten Drucker füllen.
    fn load(&mut self) {
        let h = self.hwnd;
        let Some(p) = self.working.get(self.current).cloned() else { return };
        set_text(h, IDC_NAME, &p.name);
        set_text(h, IDC_DIR, &p.target_dir);
        set_text(h, IDC_PATTERN, &p.filename_pattern);
        set_checked(h, IDC_OPEN, p.open_after_save);
        set_checked(h, IDC_PRINT, p.print_after_save);

        // Druckerauswahl: Standarddrucker + echte Drucker (keine PDF-Ablage-Drucker)
        let own: Vec<String> = self.working.iter().map(|w| w.name.clone()).collect();
        let saved_own: Vec<String> = self.cfg.lock().map(|c| c.printers.iter().map(|w| w.name.clone()).collect()).unwrap_or_default();
        let mut targets: Vec<String> = spooler::list_printers(None)
            .into_iter()
            .filter(|n| !own.iter().chain(saved_own.iter()).any(|o| o.eq_ignore_ascii_case(n)))
            .collect();
        if !p.print_printer.is_empty() && !targets.contains(&p.print_printer) {
            targets.push(p.print_printer.clone());
        }
        send(h, IDC_TARGET, CB_RESETCONTENT, 0, 0);
        let default = spooler::default_printer().unwrap_or_else(|| "–".into());
        let first = wide(&format!("Windows-Standarddrucker ({default})"));
        send(h, IDC_TARGET, CB_ADDSTRING, 0, first.as_ptr() as LPARAM);
        for t in &targets {
            let w = wide(t);
            send(h, IDC_TARGET, CB_ADDSTRING, 0, w.as_ptr() as LPARAM);
        }
        let sel = if p.print_printer.is_empty() {
            0
        } else {
            targets.iter().position(|t| *t == p.print_printer).map_or(0, |i| i + 1)
        };
        send(h, IDC_TARGET, CB_SETCURSEL, sel, 0);
        self.targets = targets;
    }

    /// Liste links neu aufbauen, Knöpfe freigeben/sperren.
    fn refresh(&mut self) {
        let h = self.hwnd;
        send(h, IDC_LIST, LB_RESETCONTENT, 0, 0);
        for p in &self.working {
            let label = wide(if p.name.is_empty() { "(ohne Namen)" } else { &p.name });
            send(h, IDC_LIST, LB_ADDSTRING, 0, label.as_ptr() as LPARAM);
        }
        send(h, IDC_LIST, LB_SETCURSEL, self.current, 0);
        enable(h, IDC_ADD, self.working.len() < MAX_PRINTERS);
        enable(h, IDC_REMOVE, self.working.len() > 1);
    }

    fn add(&mut self) {
        self.store();
        let Some(id) = Config::free_id(&self.working) else { return };
        let mut p = PrinterConfig::new(id);
        let mut n = id;
        while self.working.iter().any(|w| w.name.eq_ignore_ascii_case(&p.name)) {
            n += 1;
            p = PrinterConfig { id, ..PrinterConfig::new(n) };
        }
        self.working.push(p);
        self.current = self.working.len() - 1;
        self.refresh();
        self.load();
        unsafe {
            SetFocus(item(self.hwnd, IDC_NAME));
        }
        send(self.hwnd, IDC_NAME, EM_SETSEL, 0, -1);
    }

    fn remove_current(&mut self) {
        if self.working.len() <= 1 {
            return;
        }
        self.working.remove(self.current);
        self.current = self.current.min(self.working.len() - 1);
        self.refresh();
        self.load();
    }
}

fn validate(working: &[PrinterConfig], saved: &Config) -> Result<(), String> {
    let mut names: Vec<&str> = Vec::new();
    for p in working {
        config::validate_printer_name(&p.name)?;
        if names.iter().any(|n| n.eq_ignore_ascii_case(&p.name)) {
            return Err(format!("Der Name „{}“ ist doppelt vergeben.", p.name));
        }
        names.push(&p.name);
        if p.target_dir.is_empty() {
            return Err(format!("Bitte für „{}“ einen Zielordner angeben.", p.name));
        }
        // Name darf nicht zu einem fremden Drucker gehören
        if let Some(port) = spooler::port_name(&p.name) {
            if !config::is_our_port(&port) && !saved.is_own_printer(&p.name) {
                return Err(format!("Der Name „{}“ gehört bereits zu einem anderen Drucker.", p.name));
            }
        }
    }
    for p in working {
        if p.print_after_save && working.iter().any(|o| o.name.eq_ignore_ascii_case(&p.print_printer)) {
            return Err(format!(
                "„{}“ kann nicht auf einen PDF-Ablage-Drucker ausdrucken (Endlosschleife).",
                p.name
            ));
        }
    }
    Ok(())
}

fn save() {
    let Some((hwnd, working, cfg, hook)) = with(|st| {
        st.store();
        (st.hwnd, st.working.clone(), st.cfg.clone(), st.on_saved.clone())
    }) else {
        return;
    };
    let old = cfg.lock().map(|c| c.clone()).unwrap_or_default();
    if let Err(msg) = validate(&working, &old) {
        ui::message(&msg, true);
        return;
    }
    let mut new = old.clone();
    new.printers = working;
    if let Err(e) = config::save(&new) {
        ui::message(&format!("Einstellungen konnten nicht gespeichert werden:\n{e:#}"), true);
        return;
    }
    if let Ok(mut c) = cfg.lock() {
        *c = new.clone();
    }
    log_info!("Einstellungen gespeichert ({} Drucker)", new.printers.len());
    if let Some(hook) = hook {
        hook();
    }

    let pairs = |c: &Config| c.printers.iter().map(|p| (p.id, p.name.to_lowercase())).collect::<Vec<_>>();
    let needs_sync = pairs(&old) != pairs(&new) || !install::windows_problems(&new).is_empty();
    let status = if !needs_sync {
        "Gespeichert."
    } else if ui::run_elevated("sync") {
        "Gespeichert. Die Drucker werden jetzt in Windows eingerichtet …"
    } else {
        "Gespeichert, aber noch nicht in Windows übernommen (Administratorrechte nötig). \
         Nachholen über „Drucker in Windows einrichten …“ im Tray-Menü."
    };
    set_text(hwnd, IDC_STATUS, status);
    with(|st| st.refresh());
}

fn browse() {
    let Some((hwnd, current)) = with(|st| (st.hwnd, text(st.hwnd, IDC_DIR))) else { return };
    let picked = rfd::FileDialog::new()
        .set_title("Zielordner für PDFs wählen")
        .set_directory(&current)
        .pick_folder();
    if let Some(folder) = picked {
        set_text(hwnd, IDC_DIR, &folder.to_string_lossy());
    }
}

fn close(hwnd: HWND) {
    let dirty = with(|st| {
        st.store();
        let saved = st.cfg.lock().map(|c| c.printers.clone()).unwrap_or_default();
        st.working != saved
    })
    .unwrap_or(false);
    if dirty && !ui::ask("Die Änderungen wurden noch nicht gespeichert.\n\nTrotzdem schließen?", APP_NAME) {
        return;
    }
    unsafe { DestroyWindow(hwnd) };
}

fn on_command(hwnd: HWND, id: i32, code: u32) {
    match (id, code) {
        (IDC_LIST, LBN_SELCHANGE) => {
            with(|st| {
                let sel = send(st.hwnd, IDC_LIST, LB_GETCURSEL, 0, 0);
                if sel >= 0 && (sel as usize) < st.working.len() && sel as usize != st.current {
                    st.store();
                    st.current = sel as usize;
                    st.refresh();
                    st.load();
                }
            });
        }
        (IDC_ADD, BN_CLICKED) => {
            with(|st| st.add());
        }
        (IDC_REMOVE, BN_CLICKED) => {
            let name = with(|st| st.working.get(st.current).map(|p| p.name.clone())).flatten();
            if let Some(name) = name {
                let q = format!(
                    "Drucker „{name}“ entfernen?\n\nEr wird beim Speichern auch aus Windows entfernt. \
                     Die bereits gespeicherten PDFs bleiben erhalten."
                );
                if ui::ask(&q, APP_NAME) {
                    with(|st| st.remove_current());
                }
            }
        }
        (IDC_BROWSE, BN_CLICKED) => browse(),
        (IDC_SAVE, BN_CLICKED) | (IDOK, _) => save(),
        (IDC_CLOSE, BN_CLICKED) | (IDCANCEL, _) => close(hwnd),
        _ => {}
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            on_command(hwnd, (wp & 0xFFFF) as i32, ((wp >> 16) & 0xFFFF) as u32);
            0
        }
        WM_CLOSE => {
            close(hwnd);
            0
        }
        WM_DESTROY => {
            let st = SETTINGS.with(|s| s.try_borrow_mut().ok().and_then(|mut g| g.take()));
            if let Some(st) = st {
                DeleteObject(st.font as _);
                if st.standalone {
                    PostQuitMessage(0);
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Für die Nachrichtenschleife: Tab, Enter und Esc im Einstellungsfenster.
pub fn is_dialog_message(msg: &MSG) -> bool {
    let hwnd = SETTINGS.with(|s| s.try_borrow().ok().and_then(|g| g.as_ref().map(|st| st.hwnd)));
    match hwnd {
        Some(h) => unsafe { IsDialogMessageW(h, msg) != 0 },
        None => false,
    }
}

/// Öffnet das Einstellungsfenster (oder holt es nach vorne, wenn es schon offen ist).
pub fn open(cfg: Arc<Mutex<Config>>, on_saved: Option<Hook>, standalone: bool) {
    let existing = SETTINGS.with(|s| s.try_borrow().ok().and_then(|g| g.as_ref().map(|st| st.hwnd)));
    if let Some(h) = existing {
        unsafe {
            ShowWindow(h, SW_RESTORE);
            SetForegroundWindow(h);
        }
        return;
    }
    unsafe {
        let hinst = GetModuleHandleW(null());
        let class = wide(CLASS);
        static REGISTER: Once = Once::new();
        REGISTER.call_once(|| {
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(wndproc);
            wc.hInstance = hinst;
            wc.lpszClassName = class.as_ptr();
            wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
            wc.hbrBackground = 16usize as _; // COLOR_BTNFACE + 1
            wc.hIcon = LoadImageW(hinst, 1usize as *const u16, IMAGE_ICON, 32, 32, LR_DEFAULTCOLOR) as _;
            RegisterClassW(&wc);
        });

        let dpi = GetDpiForSystem().max(96);
        let s = dpi as f64 / 96.0;
        let sc = |v: i32| (v as f64 * s).round() as i32;

        let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS, ncm.cbSize, &mut ncm as *mut _ as *mut _, 0, dpi);
        let font = CreateFontIndirectW(&ncm.lfMessageFont);

        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let mut rect = RECT { left: 0, top: 0, right: sc(660), bottom: sc(340) };
        AdjustWindowRectEx(&mut rect, style, 0, 0);
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        let x = ((GetSystemMetrics(SM_CXSCREEN) - w) / 2).max(0);
        let y = ((GetSystemMetrics(SM_CYSCREEN) - h) / 2).max(0);
        let title = wide(&format!("{APP_NAME} – Einstellungen"));
        let hwnd = CreateWindowExW(0, class.as_ptr(), title.as_ptr(), style, x, y, w, h, null_mut(), null_mut(), hinst, null());
        if hwnd.is_null() {
            log_error!("Einstellungsfenster konnte nicht erstellt werden.");
            return;
        }

        let ctl = |class: &str, text: &str, style: u32, ex: u32, r: (i32, i32, i32, i32), id: i32| {
            let c = CreateWindowExW(
                ex,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                sc(r.0),
                sc(r.1),
                sc(r.2),
                sc(r.3),
                hwnd,
                id as usize as HMENU,
                hinst,
                null(),
            );
            SendMessageW(c, WM_SETFONT, font as WPARAM, 1);
            c
        };

        // Links: Druckerliste
        ctl("STATIC", "PDF-Drucker (höchstens 5):", 0, 0, (12, 12, 190, 18), IDC_LABEL);
        ctl("LISTBOX", "", WS_TABSTOP | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, WS_EX_CLIENTEDGE, (12, 32, 190, 254), IDC_LIST);
        ctl("BUTTON", "Hinzufügen", WS_TABSTOP, 0, (12, 294, 93, 28), IDC_ADD);
        ctl("BUTTON", "Entfernen", WS_TABSTOP, 0, (109, 294, 93, 28), IDC_REMOVE);

        // Rechts: Einstellungen des gewählten Druckers
        let x = 220;
        ctl("STATIC", "Name des Druckers (so erscheint er beim Drucken):", 0, 0, (x, 12, 428, 18), IDC_LABEL);
        ctl("EDIT", "", WS_TABSTOP | ES_AUTOHSCROLL, WS_EX_CLIENTEDGE, (x, 32, 428, 24), IDC_NAME);
        ctl("STATIC", "Zielordner:", 0, 0, (x, 66, 428, 18), IDC_LABEL);
        ctl("EDIT", "", WS_TABSTOP | ES_AUTOHSCROLL, WS_EX_CLIENTEDGE, (x, 86, 338, 24), IDC_DIR);
        ctl("BUTTON", "Wählen …", WS_TABSTOP, 0, (x + 344, 85, 84, 26), IDC_BROWSE);
        ctl("STATIC", "Dateiname (Platzhalter: {datum} {zeit} {titel}):", 0, 0, (x, 120, 428, 18), IDC_LABEL);
        ctl("EDIT", "", WS_TABSTOP | ES_AUTOHSCROLL, WS_EX_CLIENTEDGE, (x, 140, 428, 24), IDC_PATTERN);
        ctl("BUTTON", "PDF nach dem Speichern öffnen", WS_TABSTOP | BS_AUTOCHECKBOX, 0, (x, 176, 428, 22), IDC_OPEN);
        ctl("BUTTON", "PDF zusätzlich ausdrucken auf:", WS_TABSTOP | BS_AUTOCHECKBOX, 0, (x, 204, 428, 22), IDC_PRINT);
        ctl("COMBOBOX", "", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 0, (x + 20, 230, 408, 240), IDC_TARGET);

        // Unten
        ctl("STATIC", "", 0, 0, (x, 268, 428, 20), IDC_STATUS);
        ctl("BUTTON", "Speichern", WS_TABSTOP | BS_DEFPUSHBUTTON, 0, (x + 254, 294, 84, 28), IDC_SAVE);
        ctl("BUTTON", "Schließen", WS_TABSTOP, 0, (x + 344, 294, 84, 28), IDC_CLOSE);

        let working = cfg.lock().map(|c| c.printers.clone()).unwrap_or_default();
        SETTINGS.with(|s| {
            *s.borrow_mut() = Some(Settings {
                hwnd,
                font,
                cfg,
                on_saved,
                standalone,
                working,
                current: 0,
                targets: Vec::new(),
            })
        });
        with(|st| {
            st.refresh();
            st.load();
        });
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(item(hwnd, IDC_LIST));
    }
}

/// Einstellungen ohne laufende Tray-App (Befehl "einstellungen").
pub fn run_standalone() {
    open(Arc::new(Mutex::new(config::load())), None, true);
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if is_dialog_message(&msg) {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
