@echo off
rem Baut PDF-Ablage.exe aus pdf_drucker.py
rem Voraussetzung: Python 3.10+ von python.org (beim Setup "Add python.exe to PATH" anhaken)

cd /d "%~dp0"
echo === Pakete installieren ===
python -m pip install --upgrade pyinstaller pystray pillow pywin32 || goto :fehler

echo === Symbol erzeugen ===
python pdf_drucker.py make-icon icon.ico || goto :fehler

echo === EXE bauen ===
python -m PyInstaller --noconfirm --onefile --windowed ^
  --name PDF-Ablage --icon icon.ico ^
  --hidden-import pystray._win32 ^
  pdf_drucker.py || goto :fehler

echo.
echo Fertig: %~dp0dist\PDF-Ablage.exe
pause
exit /b 0

:fehler
echo.
echo Build fehlgeschlagen.
pause
exit /b 1
