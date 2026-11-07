//! Drucker in Windows einrichten/abgleichen/entfernen und Autostart.
//!
//! PDF-Ablage erkennt "seine" Drucker am Anschluss (\\.\pipe\PDFAblage…). Der Abgleich legt
//! fehlende Drucker an, benennt umbenannte um und entfernt Drucker, die nicht mehr in den
//! Einstellungen stehen. Dafür sind Administratorrechte nötig.

use crate::config::{self, Config, APP_NAME};
use crate::{spooler, ui};
use anyhow::{anyhow, bail, Result};
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
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n$ErrorActionPreference = 'Stop'\n\
         try {{\n{script}\nexit 0\n}} catch {{ Write-Output ($_ | Out-String); exit 1 }}"
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

/// PowerShell-Hilfsfunktion: gehört ein Anschluss zu PDF-Ablage?
fn ps_is_our() -> String {
    format!(
        "function IsOur($p) {{ return ([string]$p -like '{}*') -or ([string]$p -eq {}) }}\n",
        config::PIPE_NAME,
        ps_quote(&config::old_spool_file().to_string_lossy())
    )
}

/// Probleme zwischen Einstellungen und Windows-Druckern (leer = alles passt).
pub fn windows_problems(cfg: &Config) -> Vec<String> {
    let mut out = Vec::new();
    for p in &cfg.printers {
        match spooler::port_name(&p.name) {
            None => out.push(format!("„{}“ ist in Windows noch nicht eingerichtet.", p.name)),
            Some(port) if !port.eq_ignore_ascii_case(&p.pipe()) => {
                out.push(format!("„{}“ muss aktualisiert werden.", p.name))
            }
            _ => {}
        }
    }
    out
}

/// Gleicht die Windows-Drucker mit den Einstellungen ab (Administratorrechte nötig).
pub fn sync_printers(cfg: &Config) -> Result<String> {
    let mut names: Vec<String> = Vec::new();
    for p in &cfg.printers {
        config::validate_printer_name(&p.name).map_err(|e| anyhow!(e))?;
        if names.iter().any(|n| n.eq_ignore_ascii_case(&p.name)) {
            bail!("Der Name „{}“ ist doppelt vergeben.", p.name);
        }
        names.push(p.name.clone());
    }
    std::fs::create_dir_all(config::app_dir())?;
    for p in &cfg.printers {
        if let Err(e) = std::fs::create_dir_all(&p.target_dir) {
            log_warn!("Zielordner {} nicht anlegbar: {e}", p.target_dir);
        }
    }
    config::save(cfg)?;

    // Normale Benutzer dürfen die Einstellungen im Programmdaten-Ordner ändern
    // (*S-1-5-32-545 = Gruppe "Benutzer", sprachunabhängig)
    let _ = Command::new("icacls")
        .arg(config::app_dir())
        .args(["/grant", "*S-1-5-32-545:(OI)(CI)M", "/T", "/Q"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let old_port = ps_quote(&config::old_spool_file().to_string_lossy());
    let first_pipe = ps_quote(config::PIPE_NAME);
    let mut script = ps_is_our();
    script.push_str(
        r#"
$drv = (Get-PrinterDriver | Where-Object { $_.Name -like '*Print*PDF*' } | Select-Object -First 1).Name
if (-not $drv) {
    Enable-WindowsOptionalFeature -Online -FeatureName Printing-PrintToPDFServices-Features -NoRestart | Out-Null
    $drv = (Get-PrinterDriver | Where-Object { $_.Name -like '*Print*PDF*' } | Select-Object -First 1).Name
}
if (-not $drv) { throw 'Treiber "Microsoft Print to PDF" nicht gefunden.' }
$managed = @(Get-Printer | Where-Object { IsOur $_.PortName })

function Sync-One($name, $port) {
    if (-not (Get-PrinterPort -Name $port -ErrorAction SilentlyContinue)) { Add-PrinterPort -Name $port }
    $cur = $managed | Where-Object { $_.PortName -eq $port } | Select-Object -First 1
"#,
    );
    script.push_str(&format!(
        "    if (-not $cur -and $port -eq {first_pipe}) {{ $cur = $managed | Where-Object {{ $_.PortName -eq {old_port} }} | Select-Object -First 1 }}\n"
    ));
    script.push_str(
        r#"    $same = Get-Printer -Name $name -ErrorAction SilentlyContinue
    if ($cur) {
        if ($cur.Name -ne $name) {
            if ($same) { throw "Der Name '$name' ist bereits von einem anderen Drucker belegt." }
            Rename-Printer -Name $cur.Name -NewName $name
        }
        Set-Printer -Name $name -PortName $port
    } elseif ($same) {
        if (-not (IsOur $same.PortName)) { throw "Der Name '$name' ist bereits von einem anderen Drucker belegt." }
        Set-Printer -Name $name -PortName $port
    } else {
        Add-Printer -Name $name -DriverName $drv -PortName $port
    }
    Set-Printer -Name $name -KeepPrintedJobs $false
}
"#,
    );
    let wanted: Vec<String> = cfg.printers.iter().map(|p| ps_quote(&p.pipe())).collect();
    for p in &cfg.printers {
        script.push_str(&format!("Sync-One {} {}\n", ps_quote(&p.name), ps_quote(&p.pipe())));
    }
    script.push_str(&format!(
        r#"$wanted = @({})
foreach ($p in @(Get-Printer | Where-Object {{ IsOur $_.PortName }})) {{
    if ($wanted -notcontains $p.PortName) {{ Remove-Printer -Name $p.Name }}
}}
foreach ($port in @(Get-PrinterPort | Where-Object {{ IsOur $_.Name }})) {{
    if ($wanted -notcontains $port.Name) {{ Remove-PrinterPort -Name $port.Name -ErrorAction SilentlyContinue }}
}}
"#,
        wanted.join(", ")
    ));

    let (ok, text) = powershell(&script)?;
    if !ok {
        bail!("Fehler beim Einrichten der Drucker:\n{text}");
    }
    let list: Vec<String> = cfg.printers.iter().map(|p| format!("• {} → {}", p.name, p.target_dir)).collect();
    log_info!("Drucker abgeglichen: {}", list.join("; "));
    Ok(format!("Drucker eingerichtet:\n{}", list.join("\n")))
}

/// Befehl "install": ersten Drucker einrichten (optional mit Name/Ordner) und abgleichen.
pub fn cmd_install(name: Option<String>, folder: Option<String>) -> Result<i32> {
    if !ui::is_admin() {
        ui::message("Bitte als Administrator ausführen.", true);
        return Ok(1);
    }
    let mut cfg = config::load();
    if let Some(first) = cfg.printers.first_mut() {
        if let Some(n) = name {
            first.name = n;
        }
        if let Some(f) = folder {
            first.target_dir = std::path::absolute(PathBuf::from(f))?.to_string_lossy().into_owned();
        }
    }
    ui::message(&sync_printers(&cfg)?, false);
    Ok(0)
}

/// Befehl "sync": alle Drucker gemäß Einstellungen einrichten.
pub fn cmd_sync() -> Result<i32> {
    if !ui::is_admin() {
        ui::message("Bitte als Administrator ausführen.", true);
        return Ok(1);
    }
    ui::message(&sync_printers(&config::load())?, false);
    Ok(0)
}

pub fn cmd_uninstall() -> Result<i32> {
    if !ui::is_admin() {
        ui::message("Bitte als Administrator ausführen.", true);
        return Ok(1);
    }
    let mut script = ps_is_our();
    script.push_str(
        r#"foreach ($p in @(Get-Printer | Where-Object { IsOur $_.PortName })) { Remove-Printer -Name $p.Name }
Restart-Service Spooler -ErrorAction SilentlyContinue
foreach ($port in @(Get-PrinterPort | Where-Object { IsOur $_.Name })) { Remove-PrinterPort -Name $port.Name -ErrorAction SilentlyContinue }
$rest = @(Get-Printer | Where-Object { IsOur $_.PortName })
if ($rest.Count -gt 0) { throw "Nicht entfernt: $($rest.Name -join ', ')" }
"#,
    );
    let (ok, text) = powershell(&script)?;
    let _ = set_autostart(false);
    if !ok {
        bail!("Drucker konnten nicht entfernt werden.\n{text}");
    }
    log_info!("Alle PDF-Ablage-Drucker entfernt");
    ui::message("Alle PDF-Ablage-Drucker wurden entfernt.", false);
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
