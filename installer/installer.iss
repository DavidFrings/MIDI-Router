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
Source: "{#ProjectDir}\target\release\updater.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectDir}\example.config.toml"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectDir}\configs\akai-apc40-mk2\akai-apc40-mk2.config.toml"; DestDir: "{app}"; Flags: ignoreversion
[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
