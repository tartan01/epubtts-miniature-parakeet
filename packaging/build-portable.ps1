# 打包脚本（PowerShell 5.1 兼容，无 && ）
# 用法：
#   powershell -ExecutionPolicy Bypass -File packaging\build-portable.ps1
#   powershell -ExecutionPolicy Bypass -File packaging\build-portable.ps1 -Version 0.3.0
param(
    [string]$Version = "0.3.0",
    [string]$Configuration = "Release",
    [switch]$SkipCli,
    [switch]$Slim
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$dist = Join-Path $root "dist"
$name = "本地朗读工厂-$Version-win-x64"
if ($Slim) { $name = "$name-slim" }
$out = Join-Path $dist $name

# 本机 dotnet 不在 PATH（AGENTS.md §2），优先用绝对路径，找不到再回退 PATH。
$dotnet = "C:\Program Files\dotnet\dotnet.exe"
if (-not (Test-Path $dotnet)) { $dotnet = "dotnet" }

# 不清理旧目录：宿主对批量删除有 safe-delete 阈值（≥50 个文件要人工确认），
# Remove-Item -Recurse 会直接被拦。publish 本身就是增量覆盖，这里只保证目录存在。
# 需要彻底重来请手动删掉 $out 再运行。
New-Item -ItemType Directory -Force -Path $out | Out-Null

Write-Host "== 发布桌面程序（自包含，免装 .NET 与 Windows App Runtime）" -ForegroundColor Cyan
& $dotnet publish (Join-Path $root "src\BookCtl.App\BookCtl.App.csproj") `
    -c $Configuration -r win-x64 --self-contained true `
    -p:PublishReadyToRun=false -p:PublishSingleFile=false -p:DebugType=none -v q --nologo -tl:false `
    -o $out
if ($LASTEXITCODE -ne 0) { throw "发布 App 失败" }

# WinUI 3 的 `dotnet publish` 不会自动带上 XAML 编译产物（*.xbf）与应用资源索引（BookCtl.App.pri），
# 缺失会让 Microsoft.UI.Xaml.dll 在启动瞬间崩溃（0xC000027B / stowed exception）。
# 这是便携包"双击没反应"的根因（缺陷 #20），必须从构建输出目录补齐。
$appBin = Join-Path $root "src\BookCtl.App\bin\$Configuration\net10.0-windows10.0.19041.0\win-x64"
if (-not (Test-Path $appBin)) {
    throw "找不到 App 构建输出目录：$appBin（先 dotnet build -c $Configuration）"
}
Copy-Item (Join-Path $appBin "*.xbf") $out -Force
Copy-Item (Join-Path $appBin "BookCtl.App.pri") $out -Force
Write-Host "== 已补齐 XAML 资源：*.xbf + BookCtl.App.pri" -ForegroundColor Cyan

# -Slim：GUI 仍完全自包含；三个命令行工具改 framework-dependent，
# 避免把同一份 .NET 运行时复制三遍（实测省约 236 MB）。
$selfContained = "true"
if ($Slim) { $selfContained = "false" }

if (-not $SkipCli) {
    Write-Host "== 发布 CLI / MCP / Worker (self-contained=$selfContained)" -ForegroundColor Cyan
    foreach ($p in @("BookCtl.Cli", "BookCtl.Mcp", "BookCtl.Worker")) {
        & $dotnet publish (Join-Path $root "src\$p\$p.csproj") `
            -c $Configuration -r win-x64 --self-contained $selfContained `
            -p:PublishReadyToRun=false -p:DebugType=none -v q --nologo -tl:false `
            -o (Join-Path $out "tools\$p")
        if ($LASTEXITCODE -ne 0) { throw "发布 $p 失败" }
    }
}

Write-Host "== 拷贝配置、引擎与说明" -ForegroundColor Cyan
Copy-Item (Join-Path $root "config") (Join-Path $out "config") -Recurse -Force
# 引擎目录按需拷贝：不要先把巨大的 .venv 拷进来再删（便携包体积/耗时）
$engineDst = Join-Path $out "engine"
New-Item -ItemType Directory -Force -Path $engineDst | Out-Null
Get-ChildItem (Join-Path $root "engine") -Force |
    Where-Object { $_.Name -notin @(".venv", "__pycache__") } |
    Copy-Item -Destination $engineDst -Recurse -Force
Copy-Item (Join-Path $root "README.md") $out -Force -ErrorAction SilentlyContinue
Copy-Item (Join-Path $root "architecture-epub2audiobook.md") $out -Force -ErrorAction SilentlyContinue

# 便携模式标记：数据与模型写在程序目录，U 盘可带走
New-Item -ItemType File -Force -Path (Join-Path $out "portable.marker") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $out "sources") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $out "data") | Out-Null

# ---- Slim 裁剪：去掉本项目用不到的组件与非中英文多语言资源 ----
if ($Slim) {
    Write-Host "== Slim 裁剪：未使用的 ML 组件" -ForegroundColor Cyan

    # Windows App SDK 附带的本地 ML 推理组件。本项目 TTS 走 Python sidecar，
    # 主程序不加载它们；删掉不影响 GUI 启动（已实测）。
    foreach ($f in @("onnxruntime.dll", "DirectML.dll", "Microsoft.Asg.SemanticIndex.AiFabric.Compatibility.dll")) {
        $p = Join-Path $out $f
        if (Test-Path $p) { Remove-Item $p -Force }
    }

    # 多语言 MUI 实测仅 ~3.4 MB（172 个）：裁掉收益远小于宿主 safe-delete 的批量删除代价，
    # 所以不裁。以后真要裁，应在 publish 前用 SatelliteResourceLanguages / MRT 白名单控制，
    # 而不是事后批量 Remove-Item。

    $toolsDir = Join-Path $out "tools"
    New-Item -ItemType Directory -Force -Path $toolsDir | Out-Null
    $note = @"
The GUI (BookCtl.App.exe) is fully self-contained. Nothing to install.

CLI / MCP / Worker in this slim build are framework-dependent, to avoid
shipping three extra copies of the .NET runtime (about 236 MB saved).
They need .NET 10 Desktop Runtime installed on the machine.

Need a fully offline, zero-prerequisite build? Run build-portable.ps1
without -Slim (the standard portable edition).
"@
    [System.IO.File]::WriteAllText(
        (Join-Path $toolsDir "PREREQUISITE.txt"),
        $note,
        (New-Object System.Text.UTF8Encoding($false)))
}

# 版本信息
$editionName = "pro-portable"
if ($Slim) { $editionName = "pro-portable-slim" }
$edition = @"
{
  "product": "本地朗读工厂",
  "binary": "bookctl",
  "version": "$Version",
  "edition": "$editionName",
  "built_at": "$(Get-Date -Format o)",
  "portable": true
}
"@
# 显式写无 BOM UTF-8：PowerShell 5.1 的 -Encoding UTF8 会带 BOM，部分 JSON 解析器会报错
[System.IO.File]::WriteAllText(
    (Join-Path $out "edition.json"),
    $edition,
    (New-Object System.Text.UTF8Encoding($false)))

Write-Host "== 生成 zip" -ForegroundColor Cyan
$zip = Join-Path $dist "$name.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $out "*") -DestinationPath $zip -CompressionLevel Optimal
Write-Host "便携版目录: $out"
if ($Slim) { Write-Host "Slim 版：GUI 零依赖；CLI/MCP/Worker 需 .NET 10 运行时（见 tools\PREREQUISITE.txt）" -ForegroundColor Yellow }
Write-Host "便携版压缩包: $zip（压缩前请按需签名，见 packaging\sign.ps1）" -ForegroundColor Green
