//! Drucker einrichten/entfernen und Autostart.

use crate::config::{self, APP_NAME};
use crate::{spooler, ui};
use anyhow::{bail, Result};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// PowerShell-Skript unsichtbar ausführen. Gibt (Erfolg, Ausgabe) zurück.
fn powershell(script: &str) -> Result<(bool, String)> {
    let full = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n$ErrorActionPreference = 'Stop'\n{script}"
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &full])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok((out.status.success(), text.trim().to_string()))
}

pub fn install_printer(name: &str, target_dir: &str) -> Result<String> {
    let mut cfg = config::load();
    cfg.printer_name = name.to_string();
    cfg.target_dir = target_dir.to_string();
    std::fs::create_dir_all(config::app_dir())?;
    std::fs::create_dir_all(target_dir)?;
    config::save(&cfg)?;

    // Normale Benutzer dürfen Dateien im Programmdaten-Ordner ändern/verschieben
    // (*S-1-5-32-545 = Gruppe "Benutzer", sprachunabhängig)
    let _ = Command::new("icacls")
        .arg(config::app_dir())
        .args(["/grant", "*S-1-5-32-545:(OI)(CI)M", "/T", "/Q"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let port = ps_quote(config::PIPE_NAME);
    let old_port = ps_quote(&config::old_spool_file().to_string_lossy());
    let n = ps_quote(name);
    let script = format!(
        r#"
$drv = (Get-PrinterDriver | Where-Object {{ $_.Name -like '*Print*PDF*' }} | Select-Object -First 1).Name
if (-not $drv) {{
    Enable-WindowsOptionalFeature -Online -FeatureName Printing-PrintToPDFServices-Features -NoRestart | Out-Null
    $drv = (Get-PrinterDriver | Where-Object {{ $_.Name -like '*Print*PDF*' }} | Select-Object -First 1).Name
}}
if (-not $drv) {{ throw 'Treiber "Microsoft Print to PDF" nicht gefunden.' }}
if (-not (Get-PrinterPort -Name {port} -ErrorAction SilentlyContinue)) {{
    Add-PrinterPort -Name {port}
}}
if (Get-Printer -Name {n} -ErrorAction SilentlyContinue) {{
    Set-Printer -Name {n} -PortName {port}
}} else {{
    Add-Printer -Name {n} -DriverName $drv -PortName {port}
}}
# Erledigte Aufträge nicht aufbewahren (frühere Versionen brauchten das für den Dokumentnamen)
Set-Printer -Name {n} -KeepPrintedJobs $false
# Alten Datei-Anschluss früherer Versionen entfernen
Remove-PrinterPort -Name {old_port} -ErrorAction SilentlyContinue
"#
    );
    let (ok, text) = powershell(&script)?;
    if !ok {
        bail!("Fehler bei der Einrichtung des Druckers:\n{text}");
    }
    log_info!("Drucker '{name}' eingerichtet, Zielordner {target_dir}");
    Ok(format!("Drucker '{name}' eingerichtet.\nPDFs landen in:\n{target_dir}"))
}

pub fn cmd_install(name: Option<String>, folder: Option<String>) -> Result<i32> {
    if !ui::is_admin() {
        ui::message("Bitte als Administrator ausführen.", true);
        return Ok(1);
    }
    let cfg = config::load();
    let name = name.unwrap_or(cfg.printer_name);
    let target = match folder {
        Some(f) => std::path::absolute(PathBuf::from(f))?.to_string_lossy().into_owned(),
        None => cfg.target_dir,
    };
    let text = install_printer(&name, &target)?;
    ui::message(&text, false);
    Ok(0)
}

pub fn cmd_uninstall() -> Result<i32> {
    if !ui::is_admin() {
        ui::message("Bitte als Administrator ausführen.", true);
        return Ok(1);
    }
    let cfg = config::load();
    let script = format!(
        "Remove-Printer -Name {} -ErrorAction SilentlyContinue\n\
         Restart-Service Spooler -ErrorAction SilentlyContinue\n\
         Remove-PrinterPort -Name {} -ErrorAction SilentlyContinue\n\
         Remove-PrinterPort -Name {} -ErrorAction SilentlyContinue",
        ps_quote(&cfg.printer_name),
        ps_quote(config::PIPE_NAME),
        ps_quote(&config::old_spool_file().to_string_lossy())
    );
    let (ok, text) = powershell(&script)?;
    let _ = set_autostart(false);
    if !ok || spooler::printer_exists(&cfg.printer_name) {
        bail!("Drucker '{}' konnte nicht entfernt werden.\n{text}", cfg.printer_name);
    }
    log_info!("Drucker '{}' entfernt", cfg.printer_name);
    ui::message(&format!("Drucker '{}' entfernt.", cfg.printer_name), false);
    Ok(0)
}

pub fn autostart_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>(APP_NAME))
        .is_ok()
}

pub fn set_autostart(on: bool) -> Result<()> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
    if on {
        let exe = std::env::current_exe()?;
        key.set_value(APP_NAME, &format!("\"{}\"", exe.display()))?;
    } else {
        match key.delete_value(APP_NAME) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Ganz alte Versionen nutzten eine .bat im Autostart-Ordner – auf Registry umstellen.
pub fn migrate_old_autostart() {
    let Some(appdata) = std::env::var_os("APPDATA") else { return };
    let old = PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup\PDF-Drucker.bat");
    if old.exists() && std::fs::remove_file(&old).is_ok() {
        let _ = set_autostart(true);
    }
}
