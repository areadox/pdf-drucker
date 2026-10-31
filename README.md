# PDF-Ablage

Virtueller PDF-Drucker für Windows: Alles, was auf den Drucker **„PDF-Ablage“** gedruckt wird, landet ohne Nachfrage als PDF in einem festen Ordner – mit eindeutigem Dateinamen wie `2026-10-31_09-20-15_Rechnung.pdf`.

Läuft als kleines Symbol im Infobereich der Taskleiste.

## Funktionsweise

1. Ein eigener Drucker nutzt den eingebauten Windows-Treiber **Microsoft Print to PDF**.
2. Dessen Anschluss ist eine feste Datei (`C:\ProgramData\PDFDrucker\spool\eingang.pdf`).
3. Die App erkennt jede fertig geschriebene PDF und verschiebt sie mit Datum, Uhrzeit und Dokumentnamen in den Zielordner.

## Installation

**Fertige .exe:** unter [Releases](../../releases) die neueste `PDF-Ablage.exe` herunterladen (Entwicklungs-Builds jedes Commits liegen unter [Actions](../../actions)).

**Selbst bauen:** Python 3.10+ installieren („Add python.exe to PATH“ anhaken), dann `build.bat` doppelklicken. Ergebnis: `dist\PDF-Ablage.exe`.

Beim ersten Start fragt die App, ob der Drucker eingerichtet werden soll (Administratorrechte nötig).

## Tray-Menü

- Zielordner öffnen (auch per Doppelklick) / ändern
- PDF nach dem Speichern öffnen
- Benachrichtigung anzeigen
- Mit Windows starten
- Drucker einrichten / entfernen
- Protokoll öffnen

## Updates

Die App prüft beim Start (und über **„Nach Updates suchen …“** im Menü), ob unter [Releases](../../releases) eine neuere Version liegt. Auf Nachfrage lädt sie die neue `PDF-Ablage.exe`, ersetzt sich selbst und startet neu.

- Die .exe nicht unter `C:\Programme` ablegen, sondern z. B. unter `%LOCALAPPDATA%\PDF-Ablage` – sonst darf sie sich nicht ersetzen.
- Solange das Repo **privat** ist, braucht die App einen GitHub-Token: [Fine-grained Token](https://github.com/settings/personal-access-tokens/new) nur für dieses Repo mit **Contents: Read-only** anlegen und in `config.json` bei `github_token` eintragen.

**Neue Version veröffentlichen:** Git-Tag `vX.Y.Z` pushen. Der Build setzt die Versionsnummer aus dem Tag und legt ein Release mit der .exe an.

```
git tag v1.1.0
git push origin v1.1.0
```

## Einstellungen

`C:\ProgramData\PDFDrucker\config.json`

| Schlüssel | Bedeutung |
|---|---|
| `printer_name` | Name des Druckers |
| `target_dir` | Zielordner |
| `filename_pattern` | Dateiname, Platzhalter `{datum}`, `{zeit}`, `{titel}` |
| `open_after_save` | PDF nach dem Speichern öffnen |
| `notify` | Benachrichtigung anzeigen |
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
```

## Hinweise

- Die .exe ist nicht signiert: Windows SmartScreen warnt beim ersten Start → „Weitere Informationen“ → „Trotzdem ausführen“.
- Werden zwei Dokumente in derselben Sekunde gedruckt, kann das zweite das erste überschreiben, bevor es abgeholt wurde.
