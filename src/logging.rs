//! Protokoll in C:\ProgramData\PDFDrucker\pdf_drucker.log (gleiches Format wie die Python-Version).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

static LOG: Mutex<Option<File>> = Mutex::new(None);

pub fn init() {
    let path = crate::config::log_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(f) = OpenOptions::new().create(true).append(true).open(&path) {
        if let Ok(mut g) = LOG.lock() {
            *g = Some(f);
        }
    }
}

pub fn write(level: &str, msg: &str) {
    let line = format!("{} {} {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S,%3f"), level, msg);
    if let Ok(mut g) = LOG.lock() {
        if let Some(f) = g.as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }
    if crate::ui::has_console() {
        eprint!("{line}");
    }
}

macro_rules! log_info {
    ($($t:tt)*) => { $crate::logging::write("INFO", &format!($($t)*)) };
}
macro_rules! log_warn {
    ($($t:tt)*) => { $crate::logging::write("WARNING", &format!($($t)*)) };
}
macro_rules! log_error {
    ($($t:tt)*) => { $crate::logging::write("ERROR", &format!($($t)*)) };
}
