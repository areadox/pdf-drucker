<#
.SYNOPSIS
  Simuliert, was ein Windows-Update mit den PDF-Ablage-Druckern anstellen kann, und prüft
  die Selbstreparatur.

.DESCRIPTION
  Muss als Administrator laufen (PowerShell mit „Als Administrator ausführen“ öffnen).

  Fälle (-Fall):
    Anschluss  Drucker druckt auf „PORTPROMPT:“ – das ist der Speichern-Dialog von
               „Microsoft Print to PDF“, genau das, was nach manchen Updates passiert.
    Treiber    Drucker bekommt einen anderen Treiber.
    Drucker    Drucker wird gelöscht.
    Alles      Alle PDF-Ablage-Drucker und ihr Anschluss werden gelöscht.

  Ohne -Automatisch macht das Skript nur den Drucker „kaputt“. Dann zeigt die laufende
  Tray-App innerhalb von 5 Minuten eine Meldung und eine Administrator-Abfrage – oder sofort
  über Tray-Menü → „Drucker prüfen / reparieren …“.

  Mit -Automatisch prüft das Skript die Reparatur ohne Tray-App selbst:
  „pruefen“ muss den Fehler finden, „reparieren“ ihn beheben, „pruefen“ danach sauber sein.

.EXAMPLE
  .\drucker-defekt-simulieren.ps1 -Fall Anschluss
.EXAMPLE
  .\drucker-defekt-simulieren.ps1 -Fall Alles -Automatisch
#>
param(
    [Parameter(Mandatory)] [ValidateSet('Anschluss', 'Treiber', 'Drucker', 'Alles')] [string]$Fall,
    [string]$Drucker = 'PDF-Ablage',
    [string]$Exe = "$env:LOCALAPPDATA\Programs\PDF-Ablage\PDF-Ablage.exe",
    [switch]$Automatisch
)
$ErrorActionPreference = 'Stop'
$pipe = '\\.\pipe\PDFAblage'

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { throw 'Bitte PowerShell als Administrator starten.' }
if (-not (Test-Path $Exe)) { throw "PDF-Ablage.exe nicht gefunden: $Exe (Pfad mit -Exe angeben)" }

function Unsere { @(Get-Printer | Where-Object { $_.PortName -like "$pipe*" }) }
function App([string]$befehl) {
    $p = Start-Process $Exe -ArgumentList $befehl -Wait -PassThru -NoNewWindow
    return $p.ExitCode
}

"Vorher:"
Get-Printer | Where-Object { $_.Name -like 'PDF-Ablage*' } | Format-Table Name, PortName, DriverName -AutoSize

switch ($Fall) {
    'Anschluss' {
        Set-Printer -Name $Drucker -PortName 'PORTPROMPT:'
    }
    'Treiber' {
        $fremd = (Get-PrinterDriver | Where-Object { $_.Name -notlike '*Print*PDF*' } | Select-Object -First 1).Name
        if (-not $fremd) {
            Add-PrinterDriver -Name 'Generic / Text Only'
            $fremd = 'Generic / Text Only'
        }
        Set-Printer -Name $Drucker -DriverName $fremd
    }
    'Drucker' {
        Remove-Printer -Name $Drucker
    }
    'Alles' {
        Unsere | ForEach-Object { Remove-Printer -Name $_.Name }
        Restart-Service Spooler
        Remove-PrinterPort -Name $pipe -ErrorAction SilentlyContinue
    }
}

"Nachher (simulierter Fehler „$Fall“):"
Get-Printer | Where-Object { $_.Name -like 'PDF-Ablage*' } | Format-Table Name, PortName, DriverName -AutoSize

if (-not $Automatisch) {
    ''
    'Jetzt sollte PDF-Ablage den Fehler melden und eine Administrator-Abfrage zeigen –'
    'spätestens nach 5 Minuten, sofort über Tray-Menü → „Drucker prüfen / reparieren …“.'
    'Danach: PDF-Ablage.exe pruefen   (oder dieses Skript mit -Automatisch erneut aufrufen)'
    "Protokoll: $env:ProgramData\PDFDrucker\pdf_drucker.log"
    return
}

''
'== pruefen (muss den Fehler finden) =='
if ((App 'pruefen') -ne 1) { throw 'Der Fehler wurde NICHT erkannt.' }
'== reparieren =='
if ((App 'reparieren') -ne 0) { throw "Reparatur fehlgeschlagen – siehe $env:ProgramData\PDFDrucker\pdf_drucker.log" }
'== pruefen (muss jetzt sauber sein) =='
if ((App 'pruefen') -ne 0) { throw 'Nach der Reparatur bestehen weiter Probleme.' }
Get-Printer | Where-Object { $_.Name -like 'PDF-Ablage*' } | Format-Table Name, PortName, DriverName -AutoSize
'Selbstreparatur erfolgreich. Zum Abschluss eine Testseite auf den Drucker schicken – sie muss'
'ohne Speichern-Dialog im Zielordner landen.'
