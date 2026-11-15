# PDF-Ablage

**Ein Zweck, keine Werbung, keine Bündel-Software – funktioniert einfach.**

PDF-Ablage ist ein virtueller PDF-Drucker für Windows. Alles, was Sie auf den Drucker **„PDF-Ablage“** drucken, landet ohne Nachfrage als PDF in einem festen Ordner – benannt nach Datum, Uhrzeit und Dokument, z. B. `2026-11-15_09-41-07_Rechnung 4711.pdf`.

Warum es PDF-Ablage gibt: Viele PDF-Drucker sind heute große Pakete mit Editor, Konverter, Werbung oder Zusatzangeboten. PDF-Ablage macht nur eine Sache – Ausdrucke zuverlässig als PDF ablegen. Das Setup ist rund 6 MB groß, enthält keine weiteren Programme, zeigt keine Werbung und sendet keine Daten (keine Telemetrie; ins Internet geht die App nur für die Update-Prüfung bei GitHub, die Sie abschalten können). Und weil ein PDF-Drucker, der nach einem Windows-Update plötzlich wieder einen Speichern-Dialog zeigt, im Alltag schlimmer ist als gar keiner, prüft PDF-Ablage seine Einrichtung regelmäßig selbst und repariert sie.

## Vergleich

| | PDF-Ablage | Microsoft Print to PDF | PDFCreator (kostenlose Version) | PDF24 Creator |
|---|---|---|---|---|
| Automatisch speichern ohne Dialog | ja | nein, fragt jedes Mal nach dem Dateinamen | ja (Profil-Einstellung) | ja (Einstellung je PDF-Drucker) |
| Werbung | nein | nein | ja, laut Hersteller in der kostenlosen Version | ¹ |
| Größe des Setups | ca. 6 MB | in Windows enthalten | ca. 150 MB | ca. 420 MB |
| Weitere Funktionen | nur Ablage, optional zusätzlicher Ausdruck | keine | Profile, Zusammenfügen, E-Mail, Aktionen | Editor, Konverter, OCR, Fax-Drucker u. v. m. |
| Prüft und repariert die eigene Druckereinrichtung | ja, beim Start und alle 5 Minuten | – | ¹ | ¹ |

¹ Dazu haben wir keine Herstellerangabe gefunden.
Stand: November 2026, Angaben laut Herstellerseiten bzw. Download-Portalen (heise Download, FossHub). Die anderen Programme können deutlich mehr als PDF-Ablage – wenn Sie PDFs bearbeiten, zusammenfügen oder umwandeln möchten, sind sie die bessere Wahl.

## Typische Einsatzfälle

- **Kleine Büros und Werkstätten:** Rechnungen, Lieferscheine und Auftragsbestätigungen werden beim Drucken automatisch als PDF abgelegt – ohne dass jemand jedes Mal einen Dateinamen eintippt.
- **Ältere Branchen- oder Warenwirtschaftssoftware**, die selbst kein PDF erzeugen kann: Einfach auf „PDF-Ablage“ drucken, die Rechnung liegt sofort im Rechnungsordner.
- **Ablage an mehreren Orten gleichzeitig**, z. B. lokal, in der Nextcloud und auf dem NAS.
- **Papier und PDF zugleich:** Die PDF wird gespeichert und zusätzlich auf einem echten Drucker ausgegeben.

Sie können **bis zu fünf PDF-Drucker** einrichten, jeder mit eigenem Namen, eigener Dateinamen-Vorlage und eigenen Speicherorten – z. B. „PDF-Ablage Rechnungen“ in den Rechnungsordner und „PDF-Ablage Belege“ in den Belegordner. Jeder Drucker kann **bis zu fünf Speicherorte** haben; die PDF wird in allen abgelegt.

## Installation

1. Unter [Releases](../../releases) die neueste `PDF-Ablage-Setup.exe` herunterladen und ausführen. Installiert wird ohne Administratorrechte nach `%LOCALAPPDATA%\Programs\PDF-Ablage`, mit Startmenü-Eintrag und optionalem Autostart. Das Setup enthält nur PDF-Ablage selbst – keine Zusatzangebote.
2. Beim ersten Start richtet PDF-Ablage den Drucker in Windows ein. Dafür erscheint einmal eine **Administrator-Abfrage**, die Sie bestätigen.
3. Fertig: Im Druckdialog jedes Programms steht jetzt „PDF-Ablage“. Die PDFs landen standardmäßig in `Dokumente\PDF-Ablage`.

**„Mit Windows starten“ ist Standard** – die App schaltet es beim ersten Start einmalig ein (auch nach einem Update). Schalten Sie es im Tray-Menü aus, bleibt es aus.

**Deinstallieren** über *Einstellungen → Apps*; dabei werden auch die Drucker entfernt. Ihre PDFs bleiben erhalten.

**Selbst bauen:** [Rust](https://rustup.rs) installieren, optional [Inno Setup 6](https://jrsoftware.org/isdl.php), dann `build.bat` doppelklicken. Das Skript lädt `pdfium.dll` herunter und erzeugt `dist\PDF-Ablage-Setup.exe` (ohne Inno Setup nur den Programmordner `dist\PDF-Ablage`).

## Einstellungen

Öffnen über **„Einstellungen …“** im Tray-Menü, per Doppelklick auf das Symbol im Infobereich oder durch erneutes Starten von PDF-Ablage über das Startmenü.

- **Links:** Liste der PDF-Drucker, **Hinzufügen** (bis zu 5) und **Entfernen**
- **Rechts**, für den gewählten Drucker:
  - **Name** – so erscheint der Drucker im Druckdialog
  - **Speicherorte** – bis zu 5; die PDF wird in allen gespeichert. Der erste ist der **Hauptordner**: Den öffnet „Zielordner öffnen“, und aus ihm wird die PDF geöffnet bzw. ausgedruckt.
  - **Dateiname** – Vorlage mit Platzhaltern, siehe unten
  - **PDF nach dem Speichern öffnen**
  - **PDF zusätzlich ausdrucken auf:** Windows-Standarddrucker oder ein bestimmter Drucker
- **Speichern** übernimmt alles sofort. Wurden Drucker hinzugefügt, umbenannt oder entfernt, richtet die App sie danach in Windows ein (eine Administrator-Abfrage).

### Tray-Menü

- Einstellungen … (auch per Doppelklick)
- Zielordner öffnen (bei mehreren Druckern als Untermenü)
- Benachrichtigung anzeigen
- Mit Windows starten
- **Drucker prüfen / reparieren …**
- Alle Drucker entfernen …
- Protokoll öffnen
- Nach Updates suchen … / Automatisch nach Updates suchen

### Einstellungsdatei

`C:\ProgramData\PDFDrucker\config.json` – normalerweise über das Einstellungsfenster bearbeitet, lässt sich aber auch direkt ändern.

| Schlüssel | Bedeutung |
|---|---|
| `printers` | Liste der PDF-Drucker (1 bis 5), siehe unten |
| `notify` | Benachrichtigung nach dem Speichern anzeigen (Fehler werden immer gemeldet) |
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
| `filename_pattern` | Dateinamen-Vorlage |
| `open_after_save` | PDF nach dem Speichern öffnen |
| `print_after_save` | gespeicherte PDF zusätzlich ausdrucken |
| `print_printer` | Drucker dafür (leer = Windows-Standarddrucker) |

Nach Änderungen an Namen oder an der Anzahl der Drucker: `PDF-Ablage.exe sync` als Administrator ausführen oder im Tray-Menü „Drucker prüfen / reparieren …“ wählen.

## Dateinamen-Vorlage

Standard: `{datum}_{uhrzeit}_{titel}.pdf` → `2026-11-15_09-41-07_Rechnung 4711.pdf`

| Platzhalter | Wird ersetzt durch | Beispiel |
|---|---|---|
| `{titel}` | Dokumentname aus dem Druckauftrag, ohne Programm-Vorsatz und Endung („Microsoft Word - Angebot.docx“ → „Angebot“) | `Angebot` |
| `{datum}` | Datum | `2026-11-15` |
| `{uhrzeit}` | Uhrzeit (Stunden-Minuten-Sekunden) | `09-41-07` |
| `{programm}` | Programm, aus dem gedruckt wurde | `Word`, `Excel`, `Edge`, `Faktura` |

- Groß-/Kleinschreibung der Platzhalter ist egal. `{zeit}` aus älteren Versionen funktioniert weiterhin wie `{uhrzeit}`.
- Die Endung `.pdf` dürfen Sie in die Vorlage schreiben oder weglassen – sie wird immer genau einmal angehängt.
- Ein unbekannter Platzhalter (z. B. Tippfehler `{tittel}`) wird beim Speichern der Einstellungen gemeldet.
- **`{programm}`** wird aus dem Dokumentnamen erkannt (Word, Excel, PowerPoint) und sonst aus dem Programmfenster, das beim Drucken im Vordergrund war. Das trifft fast immer; druckt ein Programm aber im Hintergrund, während Sie in einem anderen Fenster arbeiten, steht dort das andere Programm. Lässt sich nichts erkennen, entfällt der Platzhalter samt Trennzeichen: `{datum}_{programm}_{titel}` ergibt dann `2026-11-15_Angebot`.

Damit jeder Dateiname unter Windows gültig ist, gilt außerdem:

- **Ungültige Zeichen** `< > : " / \ | ? *` und Steuerzeichen werden durch `_` ersetzt, mehrfache Leerzeichen zusammengefasst, Punkte und Leerzeichen am Anfang und Ende entfernt.
- **Reservierte Namen** wie `CON`, `NUL` oder `COM1` bekommen ein `_` vorangestellt.
- **Zu lange Namen** werden gekürzt: höchstens 150 Zeichen, und der vollständige Pfad bleibt unter 260 Zeichen, damit Explorer, Backup-Programme und ältere Software die Datei problemlos öffnen.
- **Gleicher Name:** Es wird nie überschrieben. Gibt es die Datei schon, heißt die neue `…_2.pdf`, `…_3.pdf` usw.
- PDFs werden erst unter einem Zwischennamen (`.part`) geschrieben und dann abgelegt – so synchronisieren Cloud-Ordner keine halben Dateien.

## Zuverlässigkeit nach Windows-Updates

Windows-Updates können Drucker verändern: Der Anschluss wird zurückgesetzt (dann öffnet „Microsoft Print to PDF“ plötzlich wieder einen Speichern-Dialog), ein Drucker verschwindet oder der Treiber wird ausgetauscht. PDF-Ablage prüft deshalb **beim Start und danach alle 5 Minuten**, ob

- jeder eingestellte Drucker in Windows vorhanden ist,
- er noch auf PDF-Ablage zeigt (Anschluss `\\.\pipe\PDFAblage`),
- er den Treiber „Microsoft Print to PDF“ nutzt,
- der Anschluss und der Treiber in Windows registriert sind.

Stimmt etwas nicht, meldet PDF-Ablage das sofort per Benachrichtigung und startet die Reparatur. Dafür zeigt Windows eine **Administrator-Abfrage**; nach dem Bestätigen richtet PDF-Ablage die Drucker neu ein und meldet das Ergebnis. Lehnen Sie die Abfrage ab, erhalten Sie eine Fehlermeldung, und PDF-Ablage fragt bei denselben Problemen nicht alle 5 Minuten erneut – über **„Drucker prüfen / reparieren …“** im Tray-Menü können Sie es jederzeit wieder anstoßen. Alles wird im Protokoll festgehalten.

Die Prüfung selbst braucht keine Administratorrechte und kostet praktisch keine Rechenzeit.

### Selbstreparatur selbst testen

Im Ordner `scripts` liegt `drucker-defekt-simulieren.ps1`. Es verändert den Drucker so, wie es ein Windows-Update tun kann. PowerShell **als Administrator** öffnen und im Repo-Ordner ausführen:

```powershell
# Ggf. einmalig für diese Sitzung Skripte erlauben
Set-ExecutionPolicy -Scope Process Bypass

# Anschluss auf den Speichern-Dialog umstellen (der häufigste Fall nach Updates)
.\scripts\drucker-defekt-simulieren.ps1 -Fall Anschluss

# Weitere Fälle: anderer Treiber, Drucker gelöscht, Drucker und Anschluss gelöscht
.\scripts\drucker-defekt-simulieren.ps1 -Fall Treiber
.\scripts\drucker-defekt-simulieren.ps1 -Fall Drucker
.\scripts\drucker-defekt-simulieren.ps1 -Fall Alles
```

Danach sollte die laufende Tray-App innerhalb von 5 Minuten eine Meldung und die Administrator-Abfrage zeigen – sofort, wenn Sie im Tray-Menü „Drucker prüfen / reparieren …“ wählen. Anschließend eine Testseite auf „PDF-Ablage“ drucken: Sie muss ohne Dialog im Zielordner landen.

Mit `-Automatisch` prüft das Skript die Reparatur ohne Tray-App: `pruefen` muss den Fehler finden, `reparieren` ihn beheben und `pruefen` danach sauber sein. Liegt PDF-Ablage nicht im Standardordner, geben Sie den Pfad mit `-Exe` an, einen anderen Drucker mit `-Drucker`.

Dieselben vier Fälle werden auch bei jedem Build automatisch auf Windows getestet (siehe unten).

## Fehlerbehebung

**Beim Drucken erscheint ein Speichern-Dialog.**
Der Drucker zeigt nicht mehr auf PDF-Ablage. Im Tray-Menü „Drucker prüfen / reparieren …“ wählen und die Administrator-Abfrage bestätigen. Läuft PDF-Ablage nicht, zuerst über das Startmenü starten.

**Die PDF kommt nicht an, der Auftrag steht mit „Fehler“ in der Druckwarteschlange.**
PDF-Ablage läuft nicht. Die App über das Startmenü starten – wartende Aufträge werden automatisch abgeholt. Damit das nicht wieder passiert, im Tray-Menü „Mit Windows starten“ einschalten.

**„Speicherort nicht erreichbar“.**
Ein Ordner (z. B. ein Netzlaufwerk) war nicht verfügbar. Die PDF liegt trotzdem in allen anderen Speicherorten. Den Ordner prüfen oder in den Einstellungen ändern.

**Der zusätzliche Ausdruck funktioniert nicht.**
In den Einstellungen bei „PDF zusätzlich ausdrucken auf“ einen echten Drucker wählen. Auf einen PDF-Ablage-Drucker wird nie ausgedruckt (das ergäbe eine Endlosschleife) – ist einer davon Windows-Standarddrucker, bitte einen bestimmten Drucker auswählen.

**Windows SmartScreen oder Microsoft Defender warnt.**
Setup und Programm sind nicht signiert. Bei SmartScreen „Weitere Informationen“ → „Trotzdem ausführen“. Meldet Defender einen Fund, ist das ein Fehlalarm; Sie können ihn unter [microsoft.com/wdsi/filesubmission](https://www.microsoft.com/en-us/wdsi/filesubmission) melden.

**Wo finde ich Details?**
Im Protokoll: Tray-Menü → „Protokoll öffnen“ bzw. `C:\ProgramData\PDFDrucker\pdf_drucker.log`. Dort stehen jede gespeicherte PDF, jede Druckerprüfung mit gefundenen Problemen und das Ergebnis jeder Reparatur. Wird das Protokoll größer als 2 MB, wird es beim nächsten Start nach `pdf_drucker.old.log` verschoben. Mit `PDF-Ablage.exe pruefen` in einer Eingabeaufforderung sehen Sie den aktuellen Zustand der Drucker.

## Funktionsweise

1. Jeder PDF-Ablage-Drucker nutzt den eingebauten Windows-Treiber **Microsoft Print to PDF** – PDF-Ablage installiert keinen eigenen Treiber.
2. Alle PDF-Ablage-Drucker teilen sich einen Anschluss, die Named Pipe `\\.\pipe\PDFAblage`. Windows übergibt jeden Druckauftrag einzeln an die App und arbeitet die Aufträge nacheinander ab – auch viele direkt hintereinander oder auf mehreren Druckern gleichzeitig gedruckte Dokumente kommen vollständig und getrennt an.
3. Beim Empfang liest die App aus der Druckwarteschlange, welcher Drucker gerade druckt und wie das Dokument heißt – damit passen Drucker und Name immer zur Datei. Kommt einmal eine leere Übertragung an, startet die App den Auftrag neu.
4. Die PDF wird nach der Dateinamen-Vorlage des Druckers in jedem seiner Speicherorte gespeichert.
5. Für den zusätzlichen Ausdruck rendert sie die Seiten mit [pdfium](https://pdfium.googlesource.com/pdfium/) und schickt sie über die Windows-Druckschnittstelle an den gewählten Drucker – ohne Acrobat oder andere PDF-Programme.
6. Eingerichtet werden Drucker und Anschluss mit den PowerShell-Befehlen von Windows (`Add-PrinterPort`, `Add-Printer`, `Set-Printer`); geprüft wird über die Druck-API von Windows ohne Administratorrechte.

## Updates

Die App prüft beim Start (und über **„Nach Updates suchen …“** im Menü), ob unter [Releases](../../releases) eine neuere Version liegt. Auf Nachfrage lädt sie das neue Setup, installiert es still im Hintergrund und startet neu. Die automatische Prüfung lässt sich im Tray-Menü abschalten.

- Solange das Repo **privat** ist, braucht die App einen GitHub-Token: [Fine-grained Token](https://github.com/settings/personal-access-tokens/new) nur für dieses Repo mit **Contents: Read-only** anlegen und in `config.json` bei `github_token` eintragen.

**Neue Version veröffentlichen:** unter [Actions → EXE bauen](../../actions/workflows/build.yml) auf **„Run workflow“** klicken, Version eintragen (z. B. `2.4.0`) und starten. Der Build testet die App auf Windows (alte Einstellungen übernehmen, drei Drucker einrichten, 10 Dokumente gleichzeitig auf alle drei drucken und Ordner, Dateinamen und Vorlagen prüfen, Ablage in drei Speicherorten und Verhalten bei einem nicht erreichbaren Speicherort prüfen, zusätzlichen Ausdruck prüfen, Drucker umbenennen und entfernen, **Selbstreparatur bei verbogenem Anschluss, gelöschtem Drucker, fremdem Treiber und fehlendem Anschluss**, Drucken ohne laufende App, Tray-App und Einstellungsfenster starten), setzt die Versionsnummer und legt ein Release mit dem Setup an. Alternativ einen Git-Tag `v2.4.0` pushen.

## Kommandozeile

```
PDF-Ablage.exe                          Tray-App starten (läuft sie schon: Einstellungen öffnen)
PDF-Ablage.exe einstellungen            Einstellungsfenster öffnen
PDF-Ablage.exe pruefen                  Drucker, Anschluss und Treiber prüfen (Exit-Code 1 bei Problemen)
PDF-Ablage.exe reparieren               Probleme beheben (Administrator, ohne Fenster, Ergebnis im Protokoll)
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
| `src/tray.rs` | Tray-Symbol, Menü, Benachrichtigungen, regelmäßige Druckerprüfung und Selbstreparatur |
| `src/settings.rs` | Einstellungsfenster |
| `src/config.rs` | Einstellungen (bis zu 5 Drucker), Übernahme alter Einstellungen (mit Tests) |
| `src/watch.rs` | Empfang über die Named Pipe, Ablage in allen Speicherorten, Neustart leerer Übertragungen, Druck-Warteschlange |
| `src/spooler.rs` | Druckerliste, Druckaufträge, Dokumentname, Anschlüsse und Treiber |
| `src/pdf.rs` | PDF rendern (pdfium) und drucken (GDI) |
| `src/install.rs` | Drucker in Windows anlegen/umbenennen/reparieren/entfernen, Prüfung, Autostart |
| `src/update.rs` | Updates über GitHub Releases |
| `src/logic.rs` | Dateinamen-Vorlage und -Bereinigung, Druckerprüfung, Versionen, Test-PDF (mit Tests) |
| `src/logging.rs` | Protokoll |
| `installer/pdf-ablage.iss` | Inno-Setup-Installer |
| `scripts/drucker-defekt-simulieren.ps1` | kaputten Drucker simulieren, Selbstreparatur testen |
| `docs/website-text.md` | Text für areadox.de (Deutsch/Englisch) |

## Hinweise

- `pdfium.dll` stammt aus [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) (BSD-Lizenz, liegt als `pdfium-LICENSE.txt` bei).
- PDF-Ablage sammelt keine Nutzungsdaten. Netzwerkzugriffe gibt es nur für die Update-Prüfung bei GitHub.
