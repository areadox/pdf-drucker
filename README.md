# PDF-Ablage

Virtueller PDF-Drucker für Windows: Alles, was auf den Drucker **„PDF-Ablage“** gedruckt wird, landet ohne Nachfrage als PDF in einem festen Ordner – mit eindeutigem Dateinamen wie `2026-10-31_09-20-15_Rechnung.pdf`.

Läuft als kleines Symbol im Infobereich der Taskleiste.

## Funktionsweise

1. Ein eigener Drucker nutzt den eingebauten Windows-Treiber **Microsoft Print to PDF**.
2. Dessen Anschluss ist eine feste Datei (`C:\ProgramData\PDFDrucker\spool\eingang.pdf`).
3. Die App erkennt jede fertig geschriebene PDF und verschiebt sie mit Datum, Uhrzeit und Dokumentnamen in den Zielordner.

## Installation

**Fertige .exe:** unter [Actions](../../actions) den letzten erfolgreichen Build öffnen und `PDF-Ablage` herunterladen (oder unter [Releases](../../releases), falls vorhanden).

**Selbst bauen:** Python 3.10+ installieren („Add python.exe to PATH“ anhaken), dann `build.bat` doppelklicken. Ergebnis: `dist\PDF-Ablage.exe`.

Beim ersten Start fragt die App, ob der Drucker eingerichtet werden soll (Administratorrechte nötig).

## Tray-Menü

- Zielordner öffnen (auch per Doppelklick) / ändern
- PDF nach dem Speichern öffnen
- Benachrichtigung anzeigen
- Mit Windows starten
- Drucker einrichten / entfernen
- Protokoll öffnen

## Einstellungen

`C:\ProgramData\PDFDrucker\config.json`

| Schlüssel | Bedeutung |
|---|---|
| `printer_name` | Name des Druckers |
| `target_dir` | Zielordner |
| `filename_pattern` | Dateiname, Platzhalter `{datum}`, `{zeit}`, `{titel}` |
| `open_after_save` | PDF nach dem Speichern öffnen |
| `notify` | Benachrichtigung anzeigen |

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
