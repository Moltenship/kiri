; Kiri-Setup.exe, a per-user install with no admin prompt. scripts/release-windows.ps1 builds it
; with /DAppVersion, /DNumericVersion (no suffix) and /DExe (the Kiri.exe to install).

[Setup]
; Never change it: later versions find the install by it and upgrade it in place. src/win/update.rs has it too.
AppId={{6D9B7D82-FF2E-4481-BBD7-6A4A225A5970}
AppName=Kiri
AppVersion={#AppVersion}
AppVerName=Kiri {#AppVersion}
AppPublisher=Kiri
AppPublisherURL=https://github.com/Moltenship/kiri
AppSupportURL=https://github.com/Moltenship/kiri/issues
AppUpdatesURL=https://github.com/Moltenship/kiri/releases
VersionInfoVersion={#NumericVersion}
PrivilegesRequired=lowest
; Kiri updates itself by replacing its exe, so the folder has to be the user's.
DefaultDirName={localappdata}\Programs\Kiri
DisableDirPage=yes
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=Kiri-Setup
SetupIconFile=Kiri.ico
UninstallDisplayName=Kiri
UninstallDisplayIcon={app}\Kiri.exe
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
; Kiri is told to quit below. Restart Manager would ask first, and can't close a tray app anyway.
CloseApplications=no

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#Exe}"; DestDir: "{app}"; DestName: "Kiri.exe"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Kiri"; Filename: "{app}\Kiri.exe"

[Run]
Filename: "{app}\Kiri.exe"; Description: "{cm:LaunchProgram,Kiri}"; Flags: nowait postinstall skipifsilent

[Registry]
; Launch at login, which Kiri writes itself. Settings in %APPDATA% stay.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "Kiri"; ValueType: none; Flags: uninsdeletevalue dontcreatekey

[UninstallDelete]
Type: files; Name: "{app}\Kiri.exe.old"
Type: filesandordirs; Name: "{localappdata}\Kiri\Update"
Type: dirifempty; Name: "{localappdata}\Kiri"

[Code]
function KiriRunningFromApp(Wmi: Variant; Kill: Boolean): Boolean;
var
  Procs, Proc: Variant;
  I: Integer;
begin
  Result := False;
  Procs := Wmi.ExecQuery('SELECT ExecutablePath FROM Win32_Process WHERE Name = ''Kiri.exe''');
  for I := 0 to Procs.Count - 1 do begin
    Proc := Procs.ItemIndex(I);
    if not VarIsNull(Proc.ExecutablePath) and (CompareText(Proc.ExecutablePath, ExpandConstant('{app}\Kiri.exe')) = 0) then begin
      Result := True;
      if Kill then
        Proc.Terminate();
    end;
  end;
end;

{ Quitting as the tray menu does takes the tray icon down with Kiri. One still running from the
  install folder five seconds later is stopped. }
procedure StopKiri(const Exe: String);
var
  Code, I: Integer;
  Locator, Wmi: Variant;
begin
  Exec(Exe, '--quit', '', SW_HIDE, ewWaitUntilTerminated, Code);
  try
    Locator := CreateOleObject('WbemScripting.SWbemLocator');
    Wmi := Locator.ConnectServer('.', 'root\CIMV2');
  except
    Exit;
  end;
  for I := 1 to 50 do begin
    if not KiriRunningFromApp(Wmi, False) then
      Exit;
    Sleep(100);
  end;
  KiriRunningFromApp(Wmi, True);
  Sleep(500);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  ExtractTemporaryFile('Kiri.exe');
  StopKiri(ExpandConstant('{tmp}\Kiri.exe'));
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    StopKiri(ExpandConstant('{app}\Kiri.exe'));
end;
