# PDF-Ablage

Virtueller PDF-Drucker für Windows: Alles, was auf den Drucker **„PDF-Ablage“** gedruckt wird, landet ohne Nachfrage als PDF in einem festen Ordner – benannt nach Datum, Uhrzeit und Dokument, z. B. `2026-11-01_16-20-15_Rechnung.pdf`. Optional wird jede PDF zusätzlich auf einem echten Drucker ausgegeben.

Es lassen sich **bis zu fünf solcher Drucker** einrichten, jeder mit eigenem Namen, Dateinamen und eigenen Optionen – z. B. „PDF-Ablage Rechnungen“ in den Rechnungsordner und „PDF-Ablage Belege“ in den Belegordner. Jeder Drucker kann **bis zu fünf Speicherorte** haben; die PDF wird in allen abgelegt (z. B. lokal, in der Nextcloud und auf dem NAS).

Läuft als kleines Symbol im Infobereich der Taskleiste. Geschrieben in Rust.

## Funktionsweise

1. Jeder PDF-Ablage-Drucker nutzt den eingebauten Windows-Treiber **Microsoft Print to PDF**.
2. Alle PDF-Ablage-Drucker teilen sich einen Anschluss, die Named Pipe `\\.\pipe\PDFAblage`. Windows übergibt jeden Druckauftrag einzeln an die App und arbeitet die Aufträge nacheinander ab – auch viele direkt hintereinander oder auf mehreren Druckern gleichzeitig gedruckte Dokumente kommen vollständig und getrennt an. (Getrennte Anschlüsse je Drucker führten dazu, dass der Windows-PDF-Treiber bei gleichzeitigen Aufträgen gelegentlich leere Übertragungen lieferte.)
3. Beim Empfang liest die App aus der Druckwarteschlange, welcher Drucker gerade druckt und wie das Dokument heißt – damit passen Drucker und Name immer zur Datei. Kommt trotzdem einmal eine leere Übertragung an, startet die App den Auftrag neu.
4. Die PDF wird nach dem Dateinamen-Muster des Druckers in jedem seiner Speicherorte gespeichert (erst unter `.part`, dann umbenannt – so synchronisieren Cloud-Ordner keine halben Dateien).
5. Für den zusätzlichen Ausdruck rendert sie die Seiten mit [pdfium](https://pdfium.googlesource.com/pdfium/) und schickt sie über die Windows-Druckschnittstelle an den gewählten Drucker – ohne Acrobat oder andere PDF-Programme.

## Installation

**Installer:** unter [Releases](../../releases) die neueste `PDF-Ablage-Setup.exe` herunterladen und ausführen. Installiert wird ohne Adminrechte nach `%LOCALAPPDATA%\Programs\PDF-Ablage`, mit Startmenü-Eintrag und optionalem Autostart. Deinstallieren über *Einstellungen → Apps* (entfernt auch die Drucker).

Beim ersten Start fragt die App, ob der Drucker eingerichtet werden soll (Administratorrechte nötig). Ist ein Drucker noch von einer älteren Version eingerichtet, fragt sie einmalig, ob er aktualisiert werden soll. Einstellungen älterer Versionen werden automatisch für den ersten Drucker übernommen.

**„Mit Windows starten“ ist Standard** – die App schaltet es beim ersten Start einmalig ein (auch nach einem Update). Wer es im Menü ausschaltet, behält diese Einstellung.

**Selbst bauen:** [Rust](https://rustup.rs) installieren, optional [Inno Setup 6](https://jrsoftware.org/isdl.php), dann `build.bat` doppelklicken. Das Skript lädt `pdfium.dll` herunter und erzeugt `dist\PDF-Ablage-Setup.exe` (ohne Inno Setup nur den Programmordner `dist\PDF-Ablage`).

## Einstellungsfenster

Öffnen über **„Einstellungen …“** im Tray-Menü (auch per Doppelklick aufs Symbol) oder einfach durch erneutes Starten von PDF-Ablage über das Startmenü.

- **Links:** Liste der PDF-Drucker, **Hinzufügen** (bis zu 5) und **Entfernen**
- **Rechts**, für den gewählten Drucker:
  - **Name** – so erscheint der Drucker im Druckdialog
  - **Speicherorte** – bis zu 5, mit „Hinzufügen …“, „Ändern …“ (oder Doppelklick) und „Entfernen“; die PDF wird in allen gespeichert. Der erste ist der **Hauptordner**: Den öffnet „Zielordner öffnen“, und aus ihm wird die PDF geöffnet bzw. ausgedruckt.
  - **Dateiname** – Muster mit `{datum}`, `{zeit}`, `{titel}`
  - **PDF nach dem Speichern öffnen**
  - **PDF zusätzlich ausdrucken auf:** Windows-Standarddrucker oder ein bestimmter Drucker
- **Speichern** übernimmt alles sofort. Wurden Drucker hinzugefügt, umbenannt oder entfernt, richtet die App sie danach in Windows ein (eine Administrator-Abfrage).

## Tray-Menü

- Einstellungen … (Standard, auch per Doppelklick)
- Zielordner öffnen (bei mehreren Druckern als Untermenü)
- Benachrichtigung anzeigen
- Mit Windows starten
- Drucker in Windows einrichten … / Alle Drucker entfernen …
- Protokoll öffnen
- Nach Updates suchen / automatisch nach Updates suchen

## Updates

Die App prüft beim Start (und über **„Nach Updates suchen …“** im Menü), ob unter [Releases](../../releases) eine neuere Version liegt. Auf Nachfrage lädt sie das neue Setup, installiert es still im Hintergrund und startet neu.

- Solange das Repo **privat** ist, braucht die App einen GitHub-Token: [Fine-grained Token](https://github.com/settings/personal-access-tokens/new) nur für dieses Repo mit **Contents: Read-only** anlegen und in `config.json` bei `github_token` eintragen.

**Neue Version veröffentlichen:** unter [Actions → EXE bauen](../../actions/workflows/build.yml) auf **„Run workflow“** klicken, Version eintragen (z. B. `2.4.0`) und starten. Der Build testet die App auf Windows (alte Einstellungen übernehmen, drei Drucker einrichten, 10 Dokumente gleichzeitig auf alle drei drucken und Ordner, Dateinamen und Muster prüfen, Ablage in drei Speicherorten und Verhalten bei einem nicht erreichbaren Speicherort prüfen, zusätzlichen Ausdruck prüfen, Drucker umbenennen und entfernen, Drucken ohne laufende App, Tray-App und Einstellungsfenster starten), setzt die Versionsnummer und legt ein Release mit dem Setup an. Alternativ einen Git-Tag `v2.4.0` pushen.

## Einstellungen

`C:\ProgramData\PDFDrucker\config.json`

Normalerweise über das Einstellungsfenster; die Datei lässt sich aber auch direkt bearbeiten.

| Schlüssel | Bedeutung |
|---|---|
| `printers` | Liste der PDF-Drucker (1 bis 5), siehe unten |
| `notify` | Benachrichtigung anzeigen |
| `auto_update_check` | beim Start nach Updates suchen |
| `autostart_default_applied` | Autostart wurde einmalig als Standard eingeschaltet (intern) |
| `github_token` | Lese-Token, nur bei privatem Repo nötig |

Je Drucker in `printers`:

| Schlüssel | Bedeutung |
|---|---|
| `id` | 1–5, bleibt beim Umbenennen gleich |
| `name` | Name des Druckers in Windows |
| `installed_name` | Name beim letzten Einrichten (intern, erkennt Umbenennungen) |
| `target_dir` | Hauptordner (erster Speicherort) |
| `extra_dirs` | weitere Speicherorte (bis zu 4) |
| `filename_pattern` | Dateiname, Platzhalter `{datum}`, `{zeit}`, `{titel}` |
| `open_after_save` | PDF nach dem Speichern öffnen |
| `print_after_save` | gespeicherte PDF zusätzlich ausdrucken |
| `print_printer` | Drucker dafür (leer = Windows-Standarddrucker) |

Nach Änderungen an Namen oder an der Anzahl der Drucker: `PDF-Ablage.exe sync` als Administrator (oder im Tray-Menü „Drucker in Windows einrichten …“).

## Kommandozeile

```
PDF-Ablage.exe                          Tray-App starten (läuft sie schon: Einstellungen öffnen)
PDF-Ablage.exe einstellungen            Einstellungsfenster öffnen
PDF-Ablage.exe sync                     alle Drucker gemäß Einstellungen einrichten (Administrator)
PDF-Ablage.exe install [--name NAME] [--ordner PFAD]   ersten Drucker einrichten (Administrator)
PDF-Ablage.exe uninstall                alle PDF-Ablage-Drucker entfernen (Administrator)
PDF-Ablage.exe run                      Empfang ohne Tray
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
| `src/settings.rs` | Einstellungsfenster |
| `src/config.rs` | Einstellungen (bis zu 5 Drucker), Übernahme alter Einstellungen (mit Tests) |
| `src/watch.rs` | Empfang über die Named Pipe, Ablage in allen Speicherorten, Neustart leerer Übertragungen, Druck-Warteschlange |
| `src/spooler.rs` | Druckerliste, Druckaufträge, Dokumentname |
| `src/pdf.rs` | PDF rendern (pdfium) und drucken (GDI) |
| `src/install.rs` | Drucker in Windows anlegen/umbenennen/entfernen, Autostart |
| `src/update.rs` | Updates über GitHub Releases |
| `src/logic.rs` | Dateinamen, Versionen, Test-PDF (mit Tests) |
| `installer/pdf-ablage.iss` | Inno-Setup-Installer |

## Hinweise

- Der Ausdruck wird nie an einen PDF-Ablage-Drucker geschickt (sonst entstünde eine Endlosschleife). Ist einer davon Windows-Standarddrucker, in den Einstellungen bei „PDF zusätzlich ausdrucken auf“ einen echten Drucker wählen.
- Setup und Programm sind nicht signiert: Windows SmartScreen kann beim ersten Start warnen → „Weitere Informationen“ → „Trotzdem ausführen“. Meldet Microsoft Defender einen Fund, ist das ein Fehlalarm; er lässt sich unter [microsoft.com/wdsi/filesubmission](https://www.microsoft.com/en-us/wdsi/filesubmission) melden.
- Läuft die App nicht, bleibt ein Druckauftrag mit „Fehler“ in der Windows-Druckwarteschlange stehen und wird automatisch abgeholt, sobald die App wieder läuft. Deshalb ist „Mit Windows starten“ Standard.
- Ist ein Speicherort nicht erreichbar (z. B. Netzlaufwerk aus), wird die PDF trotzdem in allen anderen gespeichert; die App meldet den Fehler mit einer Benachrichtigung und im Protokoll.
- Werden zwei Dokumente in derselben Sekunde mit gleichem Namen gedruckt, bekommt das zweite die Endung `_2`.
- `pdfium.dll` stammt aus [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) (BSD-Lizenz, liegt als `pdfium-LICENSE.txt` bei).
