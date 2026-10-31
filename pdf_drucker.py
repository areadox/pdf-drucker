#!/usr/bin/env python3
"""
PDF-Ablage – eigener virtueller Drucker, der PDFs automatisch in einen Zielordner legt.

Als .exe (oder ohne Argumente gestartet) läuft das Tool als Symbol im Infobereich
der Taskleiste. Dort lassen sich Drucker, Zielordner und Autostart per Menü steuern.

Funktionsweise:
  - Legt einen Drucker an, der den Windows-Treiber "Microsoft Print to PDF" nutzt.
  - Der Drucker druckt immer in eine feste Eingangsdatei (lokaler Port = Dateipfad).
  - Ein Hintergrundprozess überwacht diese Datei und verschiebt jede fertige PDF
    mit eindeutigem Namen in den Zielordner.

Befehle (optional, für die Kommandozeile):
  pdf_drucker.py                    Tray-App starten (Standard)
  pdf_drucker.py install [--name NAME] [--ordner PFAD]   (Administrator)
  pdf_drucker.py uninstall                               (Administrator)
  pdf_drucker.py run                Überwachung ohne Tray im Konsolenfenster
  pdf_drucker.py autostart [--aus]  beim Anmelden starten / nicht mehr starten
  pdf_drucker.py config             Einstellungen anzeigen
  pdf_drucker.py make-icon DATEI    .ico-Datei für den Build erzeugen

Pakete für die Tray-App:  pip install pystray pillow pywin32
"""
import argparse
import ctypes
import json
import logging
import os
import re
import shutil
import subprocess
import sys
import threading
import time
from datetime import datetime
from pathlib import Path

try:
    import win32print  # optional, für Dokumentnamen
except ImportError:
    win32print = None

APP_NAME = "PDF-Ablage"
APP_DIR = Path(os.environ.get("PROGRAMDATA", r"C:\ProgramData")) / "PDFDrucker"
SPOOL_DIR = APP_DIR / "spool"
SPOOL_FILE = SPOOL_DIR / "eingang.pdf"
CONFIG_FILE = APP_DIR / "config.json"
LOG_FILE = APP_DIR / "pdf_drucker.log"
STARTUP_FILE = (Path(os.environ.get("APPDATA", "")) / "Microsoft" / "Windows"
                / "Start Menu" / "Programs" / "Startup" / "PDF-Drucker.bat")

FROZEN = getattr(sys, "frozen", False)  # True, wenn als .exe gebaut
NO_WINDOW = 0x08000000  # CREATE_NO_WINDOW: keine schwarzen Konsolenfenster

DEFAULT_CONFIG = {
    "printer_name": APP_NAME,
    "target_dir": str(Path.home() / "Documents" / APP_NAME),
    # Platzhalter: {datum} {zeit} {titel}
    "filename_pattern": "{datum}_{zeit}_{titel}",
    "poll_interval": 0.5,
    "open_after_save": False,
    "notify": True,
}

log = logging.getLogger("pdf_drucker")


# ---------------------------------------------------------------- Hilfsfunktionen

def load_config() -> dict:
    cfg = dict(DEFAULT_CONFIG)
    if CONFIG_FILE.exists():
        try:
            cfg.update(json.loads(CONFIG_FILE.read_text(encoding="utf-8")))
        except (OSError, json.JSONDecodeError) as e:
            log.warning("config.json nicht lesbar (%s), nutze Standardwerte.", e)
    return cfg


def save_config(cfg: dict) -> None:
    APP_DIR.mkdir(parents=True, exist_ok=True)
    CONFIG_FILE.write_text(json.dumps(cfg, indent=2, ensure_ascii=False), encoding="utf-8")


def setup_logging() -> None:
    APP_DIR.mkdir(parents=True, exist_ok=True)
    handlers = [logging.FileHandler(LOG_FILE, encoding="utf-8")]
    if sys.stderr:  # in der .exe ohne Konsole gibt es kein stderr
        handlers.append(logging.StreamHandler())
    logging.basicConfig(level=logging.INFO,
                        format="%(asctime)s %(levelname)s %(message)s", handlers=handlers)


def is_admin() -> bool:
    try:
        return bool(ctypes.windll.shell32.IsUserAnAdmin())
    except Exception:
        return False


def run_elevated(*args: str) -> bool:
    """Startet dieses Programm mit Admin-Rechten neu (UAC-Abfrage)."""
    if FROZEN:
        exe, params = sys.executable, " ".join(args)
    else:
        exe = sys.executable
        params = f'"{Path(__file__).resolve()}" ' + " ".join(args)
    return ctypes.windll.shell32.ShellExecuteW(None, "runas", exe, params, None, 1) > 32


def ps(script: str) -> subprocess.CompletedProcess:
    """PowerShell-Befehl unsichtbar ausführen."""
    return subprocess.run(
        ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script],
        capture_output=True, text=True, creationflags=NO_WINDOW,
    )


def ps_quote(s: str) -> str:
    return "'" + s.replace("'", "''") + "'"


def safe_name(text: str, max_len: int = 80) -> str:
    text = re.sub(r'[<>:"/\\|?*\x00-\x1f]', "_", text).strip(" .")
    text = re.sub(r"\s+", " ", text)
    return text[:max_len] or "Druck"


def unique_path(folder: Path, stem: str) -> Path:
    candidate = folder / f"{stem}.pdf"
    n = 2
    while candidate.exists():
        candidate = folder / f"{stem}_{n}.pdf"
        n += 1
    return candidate


def message(text: str, title: str = APP_NAME, error: bool = False) -> None:
    """Meldung als Fenster (in der .exe) bzw. in der Konsole."""
    if FROZEN or not sys.stdout:
        flags = 0x10 if error else 0x40  # MB_ICONERROR / MB_ICONINFORMATION
        ctypes.windll.user32.MessageBoxW(None, text, title, flags)
    else:
        print(text)


def printer_exists(name: str) -> bool:
    r = ps(f"if (Get-Printer -Name {ps_quote(name)} -ErrorAction SilentlyContinue) {{ 'ja' }}")
    return "ja" in r.stdout


def startup_command() -> str:
    if FROZEN:
        return f'start "" "{sys.executable}"'
    pythonw = Path(sys.executable).with_name("pythonw.exe")
    exe = pythonw if pythonw.exists() else Path(sys.executable)
    return f'start "" "{exe}" "{Path(__file__).resolve()}"'


def set_autostart(on: bool) -> None:
    if on:
        STARTUP_FILE.parent.mkdir(parents=True, exist_ok=True)
        STARTUP_FILE.write_text(f"@echo off\n{startup_command()}\n", encoding="utf-8")
    elif STARTUP_FILE.exists():
        STARTUP_FILE.unlink()


def make_icon_image(size: int = 64):
    """Einfaches Symbol: Blatt mit gefalteter Ecke und rotem PDF-Band."""
    from PIL import Image, ImageDraw
    s = size / 64
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.polygon([(12 * s, 4 * s), (40 * s, 4 * s), (52 * s, 16 * s), (52 * s, 60 * s), (12 * s, 60 * s)],
              fill=(255, 255, 255, 255), outline=(90, 90, 90, 255))
    d.polygon([(40 * s, 4 * s), (40 * s, 16 * s), (52 * s, 16 * s)], fill=(210, 210, 210, 255),
              outline=(90, 90, 90, 255))
    d.rectangle([6 * s, 30 * s, 46 * s, 48 * s], fill=(200, 30, 30, 255))
    for y in (22, 53):  # angedeutete Textzeilen
        d.line([(18 * s, y * s), (46 * s, y * s)], fill=(160, 160, 160, 255), width=max(1, int(2 * s)))
    # "PDF" als Blockbuchstaben, damit keine Schriftart nötig ist
    w = max(1, int(2.5 * s))
    x, y0, y1 = 11 * s, 34 * s, 44 * s
    d.line([(x, y0), (x, y1)], fill="white", width=w)                       # P
    d.line([(x, y0), (x + 6 * s, y0), (x + 6 * s, 39 * s), (x, 39 * s)], fill="white", width=w)
    x = 22 * s                                                               # D
    d.line([(x, y0), (x, y1)], fill="white", width=w)
    d.line([(x, y0), (x + 4 * s, y0), (x + 7 * s, 37 * s), (x + 7 * s, 41 * s), (x + 4 * s, y1), (x, y1)],
           fill="white", width=w)
    x = 34 * s                                                               # F
    d.line([(x, y0), (x, y1)], fill="white", width=w)
    d.line([(x, y0), (x + 7 * s, y0)], fill="white", width=w)
    d.line([(x, 39 * s), (x + 5 * s, 39 * s)], fill="white", width=w)
    return img


# ---------------------------------------------------------------- Installation

def install_printer(name: str, target_dir: str) -> tuple[bool, str]:
    cfg = load_config()
    cfg["printer_name"] = name
    cfg["target_dir"] = target_dir

    SPOOL_DIR.mkdir(parents=True, exist_ok=True)
    Path(target_dir).mkdir(parents=True, exist_ok=True)
    save_config(cfg)

    # Normale Benutzer dürfen Dateien im Programmordner ändern/verschieben
    # (*S-1-5-32-545 = Gruppe "Benutzer", sprachunabhängig)
    subprocess.run(["icacls", str(APP_DIR), "/grant", "*S-1-5-32-545:(OI)(CI)M", "/T", "/Q"],
                   capture_output=True, creationflags=NO_WINDOW)

    port = str(SPOOL_FILE)
    r = ps(f"""
$ErrorActionPreference = 'Stop'
$drv = (Get-PrinterDriver | Where-Object {{ $_.Name -like '*Print*PDF*' }} | Select-Object -First 1).Name
if (-not $drv) {{
    Enable-WindowsOptionalFeature -Online -FeatureName Printing-PrintToPDFServices-Features -NoRestart | Out-Null
    $drv = (Get-PrinterDriver | Where-Object {{ $_.Name -like '*Print*PDF*' }} | Select-Object -First 1).Name
}}
if (-not $drv) {{ throw 'Treiber "Microsoft Print to PDF" nicht gefunden.' }}
if (-not (Get-PrinterPort -Name {ps_quote(port)} -ErrorAction SilentlyContinue)) {{
    Add-PrinterPort -Name {ps_quote(port)}
}}
if (Get-Printer -Name {ps_quote(name)} -ErrorAction SilentlyContinue) {{
    Set-Printer -Name {ps_quote(name)} -PortName {ps_quote(port)}
}} else {{
    Add-Printer -Name {ps_quote(name)} -DriverName $drv -PortName {ps_quote(port)}
}}
""")
    if r.returncode != 0:
        return False, "Fehler bei der Installation:\n" + (r.stderr or r.stdout).strip()
    return True, f"Drucker '{name}' eingerichtet.\nPDFs landen in:\n{target_dir}"


def cmd_install(args) -> int:
    if not is_admin():
        message("Bitte als Administrator ausführen.", error=True)
        return 1
    cfg = load_config()
    name = args.name or cfg["printer_name"]
    target = str(Path(args.ordner).expanduser().resolve()) if args.ordner else cfg["target_dir"]
    ok, text = install_printer(name, target)
    message(text, error=not ok)
    return 0 if ok else 1


def cmd_uninstall(args) -> int:
    if not is_admin():
        message("Bitte als Administrator ausführen.", error=True)
        return 1
    cfg = load_config()
    r = ps(f"""
Remove-Printer -Name {ps_quote(cfg['printer_name'])} -ErrorAction SilentlyContinue
Restart-Service Spooler -ErrorAction SilentlyContinue
Remove-PrinterPort -Name {ps_quote(str(SPOOL_FILE))} -ErrorAction SilentlyContinue
""")
    set_autostart(False)
    if r.returncode == 0:
        message(f"Drucker '{cfg['printer_name']}' entfernt.")
        return 0
    message(r.stderr, error=True)
    return 1


def cmd_autostart(args) -> int:
    set_autostart(not args.aus)
    print("Autostart deaktiviert." if args.aus else f"Autostart eingerichtet: {STARTUP_FILE}")
    return 0


def cmd_config(args) -> int:
    print(f"Konfigurationsdatei: {CONFIG_FILE}")
    print(json.dumps(load_config(), indent=2, ensure_ascii=False))
    print(f"Log: {LOG_FILE}")
    return 0


def cmd_make_icon(args) -> int:
    img = make_icon_image(256)
    img.save(args.datei, sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print(f"Symbol gespeichert: {args.datei}")
    return 0


# ---------------------------------------------------------------- Überwachung

class JobNameTracker:
    """Merkt sich den Dokumentnamen des aktuellen Druckauftrags (benötigt pywin32)."""

    def __init__(self, printer: str):
        self.printer = printer
        self.last_name = None

    def poll(self) -> None:
        if not win32print:
            return
        try:
            h = win32print.OpenPrinter(self.printer)
            try:
                jobs = win32print.EnumJobs(h, 0, 20, 1)
            finally:
                win32print.ClosePrinter(h)
            if jobs:
                self.last_name = jobs[-1].get("pDocument") or self.last_name
        except Exception:
            pass

    def take(self) -> str:
        name, self.last_name = self.last_name, None
        if not name:
            return "Druck"
        name = re.sub(r"^Microsoft (Word|Excel|PowerPoint) - ", "", name)
        name = re.sub(r"\.(docx?|xlsx?|pptx?|txt|pdf|html?)$", "", name, flags=re.I)
        return safe_name(name)


def pdf_is_complete(path: Path) -> bool:
    try:
        size = path.stat().st_size
        if size == 0:
            return False
        time.sleep(0.3)
        if path.stat().st_size != size:
            return False
        with open(path, "rb") as f:
            f.seek(max(0, size - 1024))
            return b"%%EOF" in f.read()
    except OSError:
        return False


def move_with_retry(src: Path, dst: Path, tries: int = 20) -> bool:
    for _ in range(tries):
        try:
            shutil.move(str(src), str(dst))
            return True
        except PermissionError:
            time.sleep(0.25)  # Spooler hält die Datei noch
    return False


def watch(cfg: dict, stop: threading.Event, on_saved=None) -> None:
    """Überwacht die Eingangsdatei, bis stop gesetzt wird. cfg darf sich währenddessen ändern."""
    tracker = JobNameTracker(cfg["printer_name"])
    log.info("Überwache %s -> %s", SPOOL_FILE, cfg["target_dir"])
    if not win32print:
        log.info("pywin32 nicht installiert – {titel} wird zu 'Druck'.")

    while not stop.is_set():
        try:
            tracker.poll()
            if SPOOL_FILE.exists() and pdf_is_complete(SPOOL_FILE):
                target = Path(cfg["target_dir"])
                target.mkdir(parents=True, exist_ok=True)
                now = datetime.now()
                stem = cfg["filename_pattern"].format(
                    datum=now.strftime("%Y-%m-%d"),
                    zeit=now.strftime("%H-%M-%S"),
                    titel=tracker.take(),
                )
                dst = unique_path(target, safe_name(stem, 150))
                if move_with_retry(SPOOL_FILE, dst):
                    log.info("Gespeichert: %s", dst)
                    if cfg.get("open_after_save"):
                        os.startfile(dst)
                    if on_saved:
                        on_saved(dst)
                else:
                    log.error("Konnte %s nicht verschieben.", SPOOL_FILE)
        except Exception:
            log.exception("Fehler in der Überwachung")
        stop.wait(cfg.get("poll_interval", 0.5))


def cmd_run(args) -> int:
    setup_logging()
    stop = threading.Event()
    try:
        watch(load_config(), stop)
    except KeyboardInterrupt:
        log.info("Beendet.")
    return 0


# ---------------------------------------------------------------- Tray-App

def single_instance() -> bool:
    """Verhindert, dass die Tray-App zweimal läuft."""
    ctypes.windll.kernel32.CreateMutexW(None, False, "Global\\PDFAblageTray")
    return ctypes.windll.kernel32.GetLastError() != 183  # ERROR_ALREADY_EXISTS


def ask_folder(initial: str) -> str | None:
    """Nativer Windows-Ordnerdialog in eigenem Prozess (stört die Tray-Schleife nicht)."""
    script = f"""
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.FolderBrowserDialog
$dlg.Description = 'Zielordner für PDFs wählen'
$dlg.ShowNewFolderButton = $true
$dlg.SelectedPath = {ps_quote(initial)}
$owner = New-Object System.Windows.Forms.Form
$owner.TopMost = $true
if ($dlg.ShowDialog($owner) -eq [System.Windows.Forms.DialogResult]::OK) {{ $dlg.SelectedPath }}
$owner.Dispose()
"""
    r = subprocess.run(
        ["powershell", "-NoProfile", "-STA", "-ExecutionPolicy", "Bypass", "-Command", script],
        capture_output=True, encoding="utf-8", errors="replace", creationflags=NO_WINDOW,
    )
    if r.returncode != 0:
        log.error("Ordnerdialog fehlgeschlagen: %s", r.stderr.strip())
    folder = r.stdout.strip()
    return folder or None


def cmd_tray(args) -> int:
    import pystray

    setup_logging()
    if not single_instance():
        message(f"{APP_NAME} läuft bereits (Symbol im Infobereich der Taskleiste).")
        return 0

    cfg = load_config()
    stop = threading.Event()

    if not printer_exists(cfg["printer_name"]):
        yes = ctypes.windll.user32.MessageBoxW(
            None,
            f"Der Drucker '{cfg['printer_name']}' ist noch nicht eingerichtet.\n\n"
            f"Jetzt einrichten? (Administratorrechte nötig)\n\nZielordner:\n{cfg['target_dir']}",
            APP_NAME, 0x04 | 0x20)  # MB_YESNO | MB_ICONQUESTION
        if yes == 6:  # IDYES
            run_elevated("install")

    def notify(path: Path) -> None:
        if cfg.get("notify"):
            try:
                icon.notify(path.name, "PDF gespeichert")
            except Exception:
                pass

    def open_folder(_icon=None, _item=None):
        Path(cfg["target_dir"]).mkdir(parents=True, exist_ok=True)
        os.startfile(cfg["target_dir"])

    dialog_lock = threading.Lock()

    def _change_folder():
        if not dialog_lock.acquire(blocking=False):
            return  # Dialog ist schon offen
        try:
            folder = ask_folder(cfg["target_dir"])
            if folder:
                cfg["target_dir"] = str(Path(folder))
                try:
                    save_config(cfg)
                    log.info("Neuer Zielordner: %s", folder)
                    icon.notify(folder, "Neuer Zielordner")
                except OSError as e:
                    message(f"Konnte die Einstellung nicht speichern:\n{e}", error=True)
        finally:
            dialog_lock.release()

    def change_folder(_icon, _item):
        threading.Thread(target=_change_folder, daemon=True).start()

    def toggle(key):
        def action(_icon, _item):
            cfg[key] = not cfg.get(key)
            try:
                save_config(cfg)
            except OSError:
                pass
        return action

    def toggle_autostart(_icon, _item):
        set_autostart(not STARTUP_FILE.exists())

    def quit_app(_icon, _item):
        stop.set()
        icon.stop()

    menu = pystray.Menu(
        pystray.MenuItem("Zielordner öffnen", open_folder, default=True),
        pystray.MenuItem("Zielordner ändern …", change_folder),
        pystray.Menu.SEPARATOR,
        pystray.MenuItem("PDF nach dem Speichern öffnen", toggle("open_after_save"),
                         checked=lambda _: bool(cfg.get("open_after_save"))),
        pystray.MenuItem("Benachrichtigung anzeigen", toggle("notify"),
                         checked=lambda _: bool(cfg.get("notify"))),
        pystray.MenuItem("Mit Windows starten", toggle_autostart,
                         checked=lambda _: STARTUP_FILE.exists()),
        pystray.Menu.SEPARATOR,
        pystray.MenuItem("Drucker einrichten …", lambda *_: run_elevated("install")),
        pystray.MenuItem("Drucker entfernen …", lambda *_: run_elevated("uninstall")),
        pystray.MenuItem("Protokoll öffnen", lambda *_: os.startfile(LOG_FILE)),
        pystray.Menu.SEPARATOR,
        pystray.MenuItem("Beenden", quit_app),
    )
    icon = pystray.Icon(APP_NAME, make_icon_image(64), APP_NAME, menu)

    threading.Thread(target=watch, args=(cfg, stop, notify), daemon=True).start()
    icon.run()
    return 0


# ---------------------------------------------------------------- CLI

def main() -> int:
    p = argparse.ArgumentParser(description="Virtueller PDF-Drucker mit festem Zielordner")
    sub = p.add_subparsers(dest="cmd")

    pi = sub.add_parser("install", help="Drucker einrichten (Admin)")
    pi.add_argument("--name", help="Name des Druckers")
    pi.add_argument("--ordner", help="Zielordner für die PDFs")
    pi.set_defaults(func=cmd_install)

    sub.add_parser("uninstall", help="Drucker entfernen (Admin)").set_defaults(func=cmd_uninstall)
    sub.add_parser("run", help="Überwachung ohne Tray starten").set_defaults(func=cmd_run)
    sub.add_parser("tray", help="Tray-App starten (Standard)").set_defaults(func=cmd_tray)

    pa = sub.add_parser("autostart", help="Beim Anmelden automatisch starten")
    pa.add_argument("--aus", action="store_true", help="Autostart entfernen")
    pa.set_defaults(func=cmd_autostart)

    sub.add_parser("config", help="Einstellungen anzeigen").set_defaults(func=cmd_config)

    pm = sub.add_parser("make-icon", help=".ico-Datei erzeugen (für den Build)")
    pm.add_argument("datei")
    pm.set_defaults(func=cmd_make_icon)

    args = p.parse_args()
    if args.cmd == "make-icon":
        return args.func(args)
    if os.name != "nt":
        print("Dieses Tool läuft nur unter Windows.")
        return 1
    return (args.func if args.cmd else cmd_tray)(args)


if __name__ == "__main__":
    sys.exit(main())
