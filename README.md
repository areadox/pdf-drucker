# PDF-Ablage

Virtueller PDF-Drucker für Windows: Alles, was auf den Drucker **„PDF-Ablage“** gedruckt wird, landet ohne Nachfrage als PDF in einem festen Ordner – benannt nach Datum, Uhrzeit und Dokument, z. B. `2026-11-01_16-20-15_Rechnung.pdf`. Optional wird jede PDF zusätzlich auf einem echten Drucker ausgegeben.

Läuft als kleines Symbol im Infobereich der Taskleiste. Geschrieben in Rust.

## Funktionsweise

1. Ein eigener Drucker nutzt den eingebauten Windows-Treiber **Microsoft Print to PDF**.
2. Dessen Anschluss ist eine feste Datei (`C:\ProgramData\PDFDrucker\spool\eingang.pdf`).
3. Der Drucker ist auf **„Gedruckte Dokumente beibehalten“** gestellt. So kann die App nach dem Druck den Dokumentnamen aus der Warteschlange lesen; danach löscht sie den erledigten Auftrag.
4. Die App erkennt jede fertig geschriebene PDF und verschiebt sie mit Datum, Uhrzeit und Dokumentnamen in den Zielordner.
5. Für den zusätzlichen Ausdruck rendert sie die Seiten mit [pdfium](https://pdfium.googlesource.com/pdfium/) und schickt sie über die Windows-Druckschnittstelle an den gewählten Drucker – ohne Acrobat oder andere PDF-Programme.

## Installation

**Installer:** unter [Releases](../../releases) die neueste `PDF-Ablage-Setup.exe` herunterladen und ausführen. Installiert wird ohne Adminrechte nach `%LOCALAPPDATA%\Programs\PDF-Ablage`, mit Startmenü-Eintrag und optionalem Autostart. Deinstallieren über *Einstellungen → Apps* (entfernt auch den Drucker).

Beim ersten Start fragt die App, ob der Drucker eingerichtet werden soll (Administratorrechte nötig). Ist der Drucker noch von einer älteren Version eingerichtet, fragt sie einmalig, ob er aktualisiert werden soll – erst danach enthalten die Dateinamen den Dokumentnamen.

**Selbst bauen:** [Rust](https://rustup.rs) installieren, optional [Inno Setup 6](https://jrsoftware.org/isdl.php), dann `build.bat` doppelklicken. Das Skript lädt `pdfium.dll` herunter und erzeugt `dist\PDF-Ablage-Setup.exe` (ohne Inno Setup nur den Programmordner `dist\PDF-Ablage`).

## Tray-Menü

- Zielordner öffnen (auch per Doppelklick) / ändern
- PDF nach dem Speichern öffnen
- PDF zusätzlich ausdrucken, Drucker unter „Ausdrucken auf“ wählen
- Benachrichtigung anzeigen
- Mit Windows starten
- Drucker einrichten / entfernen
- Protokoll öffnen
- Nach Updates suchen / automatisch nach Updates suchen

## Updates

Die App prüft beim Start (und über **„Nach Updates suchen …“** im Menü), ob unter [Releases](../../releases) eine neuere Version liegt. Auf Nachfrage lädt sie das neue Setup, installiert es still im Hintergrund und startet neu.

- Solange das Repo **privat** ist, braucht die App einen GitHub-Token: [Fine-grained Token](https://github.com/settings/personal-access-tokens/new) nur für dieses Repo mit **Contents: Read-only** anlegen und in `config.json` bei `github_token` eintragen.

**Neue Version veröffentlichen:** unter [Actions → EXE bauen](../../actions/workflows/build.yml) auf **„Run workflow“** klicken, Version eintragen (z. B. `2.1.0`) und starten. Der Build testet die App auf Windows (Drucker einrichten, Testdokument drucken, Dateiname und Ausdruck prüfen, Drucker entfernen), setzt die Versionsnummer und legt ein Release mit dem Setup an. Alternativ einen Git-Tag `v2.1.0` pushen.

## Einstellungen

`C:\ProgramData\PDFDrucker\config.json`

| Schlüssel | Bedeutung |
|---|---|
| `printer_name` | Name des Druckers |
| `target_dir` | Zielordner |
| `filename_pattern` | Dateiname, Platzhalter `{datum}`, `{zeit}`, `{titel}` |
| `open_after_save` | PDF nach dem Speichern öffnen |
| `notify` | Benachrichtigung anzeigen |
| `print_after_save` | gespeicherte PDF zusätzlich ausdrucken |
| `print_printer` | Drucker dafür (leer = Windows-Standarddrucker) |
| `auto_update_check` | beim Start nach Updates suchen |
| `github_token` | Lese-Token, nur bei privatem Repo nötig |

## Kommandozeile

```
PDF-Ablage.exe                          Tray-App starten
PDF-Ablage.exe install [--name NAME] [--ordner PFAD]   Drucker einrichten (Administrator)
PDF-Ablage.exe uninstall                Drucker entfernen (Administrator)
PDF-Ablage.exe run                      Überwachung ohne Tray
PDF-Ablage.exe autostart [--aus]
PDF-Ablage.exe config
PDF-Ablage.exe print DATEI [DRUCKER]    PDF ausdrucken
PDF-Ablage.exe selftest [DRUCKER]       Darstellung/Testdruck prüfen
PDF-Ablage.exe version
```

## Projektaufbau

| Datei | Inhalt |
|---|---|
| `src/main.rs` | Befehle der Kommandozeile |
| `src/tray.rs` | Tray-Symbol, Menü, Benachrichtigungen |
| `src/watch.rs` | Überwachung der Eingangsdatei, Ablage, Druck-Warteschlange |
| `src/spooler.rs` | Druckerliste, Druckaufträge, Dokumentname |
| `src/pdf.rs` | PDF rendern (pdfium) und drucken (GDI) |
| `src/install.rs` | Drucker einrichten/entfernen, Autostart |
| `src/update.rs` | Updates über GitHub Releases |
| `src/logic.rs` | Dateinamen, Versionen, Test-PDF (mit Tests) |
| `installer/pdf-ablage.iss` | Inno-Setup-Installer |

## Hinweise

- Der Ausdruck wird nie an den PDF-Ablage-Drucker selbst geschickt (sonst entstünde eine Endlosschleife). Ist er Windows-Standarddrucker, unter „Ausdrucken auf“ einen echten Drucker wählen.
- Setup und Programm sind nicht signiert: Windows SmartScreen kann beim ersten Start warnen → „Weitere Informationen“ → „Trotzdem ausführen“. Meldet Microsoft Defender einen Fund, ist das ein Fehlalarm; er lässt sich unter [microsoft.com/wdsi/filesubmission](https://www.microsoft.com/en-us/wdsi/filesubmission) melden.
- Werden zwei Dokumente in derselben Sekunde gedruckt, kann das zweite das erste überschreiben, bevor es abgeholt wurde.
- `pdfium.dll` stammt aus [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) (BSD-Lizenz, liegt als `pdfium-LICENSE.txt` bei).
