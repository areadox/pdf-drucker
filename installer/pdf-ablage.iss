; Inno-Setup-Skript für PDF-Ablage
; Bauen:  iscc /DMyAppVersion=1.2.3 installer\pdf-ablage.iss
; Erwartet den PyInstaller-Ordner unter dist\PDF-Ablage und icon.ico im Hauptordner.

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
#define MyAppName "PDF-Ablage"
#define MyAppExe "PDF-Ablage.exe"
#define MyAppUrl "https://github.com/areadox/pdf-drucker"

[Setup]
AppId={{176E9E6A-AE7D-4296-B7CF-11FB853C201C}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher=areadox
AppPublisherURL={#MyAppUrl}
AppSupportURL={#MyAppUrl}
AppUpdatesURL={#MyAppUrl}/releases
VersionInfoVersion={#MyAppVersion}
VersionInfoDescription={#MyAppName} Setup
; Installation nur für den aktuellen Benutzer -> keine Adminrechte, Updates laufen still
PrivilegesRequired=lowest
DefaultDirName={autopf}\{#MyAppName}
DisableProgramGroupPage=yes
DisableDirPage=auto
UsePreviousAppDir=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=PDF-Ablage-Setup
SetupIconFile=..\icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}
UninstallDisplayName={#MyAppName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=no

[Languages]
Name: "de"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "autostart"; Description: "Beim Windows-Start automatisch starten"
Name: "desktopicon"; Description: "Desktop-Verknüpfung anlegen"; Flags: unchecked

[InstallDelete]
; Alte Programmbibliotheken entfernen, damit nach Updates keine Reste bleiben
Type: filesandordirs; Name: "{app}\_internal"

[Files]
Source: "..\dist\PDF-Ablage\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Registry]
; Gleicher Autostart-Eintrag, den auch die App über ihr Menü setzt.
; Bei stillen Updates nicht anfassen, damit eine Änderung im App-Menü erhalten bleibt.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; \
  ValueName: "{#MyAppName}"; ValueData: """{app}\{#MyAppExe}"""; Tasks: autostart; \
  Check: not IsUpdate; Flags: uninsdeletevalue

[Run]
; Nach einem stillen Update die neue Version wieder starten
Filename: "{app}\{#MyAppExe}"; Parameters: "tray --nach-update"; Check: IsUpdate; Flags: nowait
Filename: "{app}\{#MyAppExe}"; Description: "{#MyAppName} jetzt starten"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Drucker entfernen (fragt nach Administratorrechten; bei "Nein" bleibt der Drucker bestehen)
Filename: "{app}\{#MyAppExe}"; Parameters: "uninstall"; Verb: "runas"; \
  Flags: shellexec waituntilterminated skipifdoesntexist; RunOnceId: "RemovePrinter"

[UninstallDelete]
Type: filesandordirs; Name: "{app}"

[Code]
const
  AppMutex = 'Global\PDFAblageTray';

function IsUpdate: Boolean;
begin
  Result := ExpandConstant('{param:UPDATE|0}') = '1';
end;

function AskUntilClosed: Boolean;
begin
  Result := True;
  while CheckForMutexes(AppMutex) do
  begin
    if MsgBox('PDF-Ablage läuft noch.' + #13#10#13#10 +
              'Bitte über das Symbol im Infobereich der Taskleiste beenden ' +
              '(Rechtsklick → Beenden) und dann auf OK klicken.',
              mbInformation, MB_OKCANCEL) = IDCANCEL then
    begin
      Result := False;
      exit;
    end;
  end;
end;

function InitializeSetup: Boolean;
var
  i: Integer;
begin
  Result := True;
  if not CheckForMutexes(AppMutex) then
    exit;

  if IsUpdate then
  begin
    { Die App beendet sich nach dem Start des Updates selbst – kurz darauf warten }
    for i := 1 to 60 do
    begin
      if not CheckForMutexes(AppMutex) then
        exit;
      Sleep(500);
    end;
    Result := False;
  end
  else
    Result := AskUntilClosed;
end;

function InitializeUninstall: Boolean;
begin
  Result := AskUntilClosed;
end;
