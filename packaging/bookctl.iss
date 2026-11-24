; 本地朗读工厂 · Windows 安装包脚本（Inno Setup 6+）
; 构建：
;   1) powershell -ExecutionPolicy Bypass -File packaging\build-portable.ps1
;   2) "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" packaging\bookctl.iss /DMyVersion=0.3.0
; 产物：dist\本地朗读工厂-<版本>-setup.exe
;
; 说明：
; - 全程 per-user 安装（PrivilegesRequired=lowest），不需要管理员，符合商店外分发习惯；
; - 安装版不写 portable.marker，数据落在 %LOCALAPPDATA%\bookctl，避免 Program Files 只读问题；
; - 卸载时保留用户产物（data 与已下载模型），只在用户勾选时删除。

#ifndef MyVersion
  #define MyVersion "0.3.0"
#endif

#define AppName "本地朗读工厂"
#define AppExe "BookCtl.App.exe"
#define SourceDir "..\dist\本地朗读工厂-" + MyVersion + "-win-x64"

[Setup]
AppId={{8F3C1B42-7A55-4C0E-9B6D-2E1F7A9C4D31}
AppName={#AppName}
AppVersion={#MyVersion}
AppPublisher=本地朗读工厂
DefaultDirName={autopf}\LocalReadingFactory
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\dist
OutputBaseFilename=本地朗读工厂-{#MyVersion}-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\{#AppExe}
; 签名：在 ISCC 命令行加 /Ssigntool="signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 $f"
; 或在 sign.ps1 中对 setup.exe 单独签名

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "快捷方式:"
Name: "associate"; Description: "把 .epub 关联到本地朗读工厂"; GroupDescription: "文件关联:"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[InstallDelete]
; 安装版删除便携标记，让数据改走 %LOCALAPPDATA%
Type: files; Name: "{app}\portable.marker"

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{group}\卸载 {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Classes\.epub\OpenWithProgids"; ValueType: string; ValueName: "LocalReadingFactory.epub"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\LocalReadingFactory.epub\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Flags: uninsdeletekey; Tasks: associate

[Run]
Filename: "{app}\{#AppExe}"; Description: "启动 {#AppName}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}\data"
