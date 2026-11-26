[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=geokernel project
AppPublisherURL=https://github.com/jash90/geokernel
DefaultDirName={localappdata}\Programs\{#InstallName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
SetupIconFile={#Payload}\geokernel.ico
LicenseFile={#Payload}\{#LicenseFile}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Files]
Source: "{#Payload}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\geokernel-app.exe"; IconFilename: "{app}\geokernel.ico"
