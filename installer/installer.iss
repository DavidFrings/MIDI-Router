#define MyAppName "MIDI Router"
#define MyAppPublisher "David Frings"
#define MyAppURL "https://github.com/DavidFrings/MIDI-Router"
#define MyAppExeName "midi-router.exe"
[Setup]
AppId={{86E22B5D-90BD-4322-8068-50483B44A96B}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\David Frings\MIDI Router
UninstallDisplayIcon={app}\{#MyAppExeName}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes
LicenseFile={#ProjectDir}\LICENSE
InfoAfterFile={#ProjectDir}\installer\install-readme.txt
PrivilegesRequiredOverridesAllowed=dialog
OutputDir={#ProjectDir}\installer
OutputBaseFilename=midi-router-installer
SolidCompression=yes
WizardStyle=modern
[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
[Files]
Source: "{#ProjectDir}\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectDir}\example.config.toml"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectDir}\configs\akai-apc40-mk2\akai-apc40-mk2.config.toml"; DestDir: "{app}"; Flags: ignoreversion
[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; Tasks: desktopicon
[Run]
Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
[Code]
// The router reads config.toml from its working directory ({app}, see WorkingDir above).
var
  ConfigPage: TInputOptionWizardPage;
  ImportPage: TInputFileWizardPage;

function ConfigPath: String;
begin
  Result := ExpandConstant('{app}\config.toml');
end;

procedure InitializeWizard;
begin
  ConfigPage := CreateInputOptionPage(wpSelectTasks,
    'Configuration', 'Which config.toml should MIDI Router use?',
    'MIDI Router reads config.toml from its install folder.', True, False);
  ConfigPage.Add('Keep the current config.toml');
  ConfigPage.Add('Use the Akai APC40 mkII template');
  ConfigPage.Add('Import my own config.toml');

  ImportPage := CreateInputFilePage(ConfigPage.ID,
    'Import configuration', 'Select the config.toml to import.', '');
  ImportPage.Add('Config file:', 'TOML files|*.toml|All files|*.*', '.toml');
end;

procedure CurPageChanged(CurPageID: Integer);
var
  HasConfig: Boolean;
begin
  if CurPageID <> ConfigPage.ID then Exit;
  HasConfig := FileExists(ConfigPath);
  ConfigPage.CheckListBox.ItemEnabled[0] := HasConfig;
  // Updates keep the existing config by default; fresh installs get the template.
  if (ConfigPage.SelectedValueIndex < 0) or ((ConfigPage.SelectedValueIndex = 0) and not HasConfig) then
  begin
    if HasConfig then
      ConfigPage.SelectedValueIndex := 0
    else
      ConfigPage.SelectedValueIndex := 1;
  end;
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := (PageID = ImportPage.ID) and (ConfigPage.SelectedValueIndex <> 2);
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = ImportPage.ID) and not FileExists(ImportPage.Values[0]) then
  begin
    MsgBox('Please select an existing config file.', mbError, MB_OK);
    Result := False;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Choice: Integer;
  Source: String;
begin
  if CurStep <> ssPostInstall then Exit;
  // Silent installs never show the page: Choice is -1, keep an existing config.
  Choice := ConfigPage.SelectedValueIndex;
  if Choice = 2 then
    Source := ImportPage.Values[0]
  else if (Choice = 1) or not FileExists(ConfigPath) then
    Source := ExpandConstant('{app}\akai-apc40-mk2.config.toml')
  else
    Exit;
  if not FileCopy(Source, ConfigPath, False) then
    SuppressibleMsgBox('Could not copy ' + Source + ' to ' + ConfigPath + '.', mbError, MB_OK, IDOK);
end;
