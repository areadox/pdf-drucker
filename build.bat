@echo off
rem Baut den Programmordner und – falls Inno Setup installiert ist – den Installer.
rem Voraussetzung: Python 3.10+ (beim Setup "Add python.exe to PATH" anhaken)
rem Optional:      Inno Setup 6 (https://jrsoftware.org/isdl.php) für PDF-Ablage-Setup.exe

cd /d "%~dp0"
echo === Pakete installieren ===
python -m pip install --upgrade pyinstaller pystray pillow pywin32 pypdfium2 || goto :fehler

echo === Symbol und Versionsinfo erzeugen ===
python pdf_drucker.py make-icon icon.ico || goto :fehler
python pdf_drucker.py make-versionfile version_info.txt || goto :fehler

echo === Programmordner bauen ===
python -m PyInstaller --noconfirm --onedir --windowed ^
  --name PDF-Ablage --icon icon.ico --version-file version_info.txt ^
  --hidden-import pystray._win32 ^
  --collect-all pypdfium2 --collect-all pypdfium2_raw ^
  pdf_drucker.py || goto :fehler

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
