# PDF-Ablage

Virtueller PDF-Drucker für Windows: Alles, was auf den Drucker **„PDF-Ablage“** gedruckt wird, landet ohne Nachfrage als PDF in einem festen Ordner – mit eindeutigem Dateinamen wie `2026-10-31_09-20-15_Rechnung.pdf`.

Läuft als kleines Symbol im Infobereich der Taskleiste.

## Funktionsweise

1. Ein eigener Drucker nutzt den eingebauten Windows-Treiber **Microsoft Print to PDF**.
2. Dessen Anschluss ist eine feste Datei (`C:\ProgramData\PDFDrucker\spool\eingang.pdf`).
3. Die App erkennt jede fertig geschriebene PDF und verschiebt sie mit Datum, Uhrzeit und Dokumentnamen in den Zielordner.

## Installation

**Installer:** unter [Releases](../../releases) die neueste `PDF-Ablage-Setup.exe` herunterladen und ausführen. Installiert wird ohne Adminrechte nach `%LOCALAPPDATA%\Programs\PDF-Ablage`, mit Startmenü-Eintrag und optionalem Autostart. Deinstallieren über *Einstellungen → Apps* (entfernt auch den Drucker).

**Selbst bauen:** Python 3.10+ installieren („Add python.exe to PATH“ anhaken), optional [Inno Setup 6](https://jrsoftware.org/isdl.php), dann `build.bat` doppelklicken. Ergebnis: `dist\PDF-Ablage-Setup.exe` (ohne Inno Setup nur der Programmordner `dist\PDF-Ablage`).

Beim ersten Start fragt die App, ob der Drucker eingerichtet werden soll (Administratorrechte nötig).

## Tray-Menü

- Zielordner öffnen (auch per Doppelklick) / ändern
- PDF nach dem Speichern öffnen
- PDF zusätzlich ausdrucken, Drucker unter „Ausdrucken auf“ wählen
- Benachrichtigung anzeigen
- Mit Windows starten
- Drucker einrichten / entfernen
- Protokoll öffnen

## Updates

Die App prüft beim Start (und über **„Nach Updates suchen …“** im Menü), ob unter [Releases](../../releases) eine neuere Version liegt. Auf Nachfrage lädt sie das neue Setup, installiert es still im Hintergrund und startet neu.

- Solange das Repo **privat** ist, braucht die App einen GitHub-Token: [Fine-grained Token](https://github.com/settings/personal-access-tokens/new) nur für dieses Repo mit **Contents: Read-only** anlegen und in `config.json` bei `github_token` eintragen.

**Neue Version veröffentlichen:** unter [Actions → EXE bauen](../../actions/workflows/build.yml) auf **„Run workflow“** klicken, Version eintragen (z. B. `1.1.0`) und starten. Der Build setzt die Versionsnummer und legt ein Release mit der .exe an. Alternativ einen Git-Tag `v1.1.0` pushen.

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
python pdf_drucker.py                 Tray-App starten
python pdf_drucker.py install         Drucker einrichten (Administrator)
python pdf_drucker.py uninstall       Drucker entfernen (Administrator)
python pdf_drucker.py run             Überwachung ohne Tray
python pdf_drucker.py autostart [--aus]
python pdf_drucker.py config
python pdf_drucker.py print DATEI [DRUCKER]   PDF ausdrucken
python pdf_drucker.py selftest [DRUCKER]      Darstellung/Testdruck prüfen
```

## Hinweise

- Der Ausdruck wird nie an den PDF-Ablage-Drucker selbst geschickt (sonst entstünde eine Endlosschleife). Ist er Windows-Standarddrucker, unter „Ausdrucken auf“ einen echten Drucker wählen.

- Setup und Programm sind nicht signiert: Windows SmartScreen kann beim ersten Start warnen → „Weitere Informationen“ → „Trotzdem ausführen“. Meldet Microsoft Defender einen Fund, ist das ein Fehlalarm; er lässt sich unter [microsoft.com/wdsi/filesubmission](https://www.microsoft.com/en-us/wdsi/filesubmission) melden.
- Werden zwei Dokumente in derselben Sekunde gedruckt, kann das zweite das erste überschreiben, bevor es abgeholt wurde.
