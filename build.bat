@echo off
rem Baut PDF-Ablage (Rust) und - falls Inno Setup installiert ist - den Installer.
rem Voraussetzung: Rust (https://rustup.rs)
rem Optional:      Inno Setup 6 (https://jrsoftware.org/isdl.php) fuer PDF-Ablage-Setup.exe

cd /d "%~dp0"
echo === Programm bauen ===
cargo build --release || goto :fehler

if not exist pdfium\bin\pdfium.dll (
  echo === pdfium herunterladen ===
  powershell -NoProfile -Command "Invoke-WebRequest https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-win-x64.tgz -OutFile pdfium.tgz" || goto :fehler
  if not exist pdfium mkdir pdfium
  tar -xzf pdfium.tgz -C pdfium || goto :fehler
)

echo === Programmordner zusammenstellen ===
if not exist dist\PDF-Ablage mkdir dist\PDF-Ablage
copy /y target\release\PDF-Ablage.exe dist\PDF-Ablage\ >nul || goto :fehler
copy /y pdfium\bin\pdfium.dll dist\PDF-Ablage\ >nul || goto :fehler
if exist pdfium\LICENSE copy /y pdfium\LICENSE dist\PDF-Ablage\pdfium-LICENSE.txt >nul

set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" (
  echo.
  echo Inno Setup nicht gefunden - Installer wird uebersprungen.
  echo Programm: %~dp0dist\PDF-Ablage\PDF-Ablage.exe
  pause
  exit /b 0
)

echo === Installer bauen ===
"%ISCC%" installer\pdf-ablage.iss || goto :fehler

echo.
echo Fertig: %~dp0dist\PDF-Ablage-Setup.exe
pause
exit /b 0

:fehler
echo.
echo Build fehlgeschlagen.
pause
exit /b 1
