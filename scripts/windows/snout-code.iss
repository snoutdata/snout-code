; The SnoutCode installer for Windows (Inno Setup 6). scripts/package-windows.ps1 compiles it:
;
;   ISCC.exe /DAppVersion=1.0.0 /DArch=x64 /DSourceRoot=<tree> /DRepoRoot=<repository> /O<out> snout-code.iss
;
; It installs for the current user by default (no administrator rights, into
; %LOCALAPPDATA%\Programs\SnoutCode); the first page offers all users instead (Program Files).
; Start menu entry, an optional desktop shortcut, and an uninstaller in Settings > Apps.
;
; Signing (later, with Azure Trusted Signing): pass /DSignTool=azure and a sign tool of that name
; (ISCC /Sazure=$p for signtool with the Trusted Signing dlib), and setup and its uninstaller are
; signed as they are written. The files it installs are signed before (see .github/workflows/windows.yml).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef SourceRoot
  #error Pass /DSourceRoot=<the folder to install>
#endif
#ifndef RepoRoot
  #define RepoRoot "..\.."
#endif

[Setup]
AppId={{6E3C2F4B-8B1D-4C55-9C7E-5A0B9D1F2E61}
AppName=SnoutCode
AppVersion={#AppVersion}
AppVerName=SnoutCode {#AppVersion}
AppPublisher=SNOUTDATA INC.
AppPublisherURL=https://github.com/snoutdata/snout-code
AppSupportURL=https://github.com/snoutdata/snout-code/issues
AppUpdatesURL=https://github.com/snoutdata/snout-code/releases
DefaultDirName={autopf}\SnoutCode
DefaultGroupName=SnoutCode
DisableProgramGroupPage=yes
DisableDirPage=auto
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
MinVersion=10.0.17763
OutputBaseFilename=SnoutCode-{#AppVersion}-windows-{#Arch}-setup
SetupIconFile={#RepoRoot}\assets\icon\snout.ico
UninstallDisplayIcon={app}\snout-code.exe
UninstallDisplayName=SnoutCode
LicenseFile={#RepoRoot}\LICENSE
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
ChangesEnvironment=no
VersionInfoVersion={#AppVersion}
VersionInfoProductName=SnoutCode
VersionInfoCompany=SNOUTDATA INC.
VersionInfoDescription=SnoutCode Setup
#ifdef SignTool
SignTool={#SignTool}
SignedUninstaller=yes
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceRoot}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\SnoutCode"; Filename: "{app}\snout-code.exe"
Name: "{autodesktop}\SnoutCode"; Filename: "{app}\snout-code.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\snout-code.exe"; Description: "{cm:LaunchProgram,SnoutCode}"; Flags: nowait postinstall skipifsilent

[InstallDelete]
; What an older version installed and this one no longer has (the engine's jars change names).
Type: filesandordirs; Name: "{app}\engine"

[UninstallDelete]
; What the engine wrote into its own folder while it ran.
Type: filesandordirs; Name: "{app}\engine"
