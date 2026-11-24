# 打包脚本（PowerShell 5.1 兼容，无 && ）
# 用法：
#   powershell -ExecutionPolicy Bypass -File packaging\build-portable.ps1
#   powershell -ExecutionPolicy Bypass -File packaging\build-portable.ps1 -Version 0.3.0
param(
    [string]$Version = "0.3.0",
    [string]$Configuration = "Release",
    [switch]$SkipCli
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$dist = Join-Path $root "dist"
$name = "本地朗读工厂-$Version-win-x64"
$out = Join-Path $dist $name

Write-Host "== 清理 $out" -ForegroundColor Cyan
if (Test-Path $out) { Remove-Item $out -Recurse -Force }
New-Item -ItemType Directory -Force -Path $out | Out-Null

Write-Host "== 发布桌面程序（自包含，免装 .NET 与 Windows App Runtime）" -ForegroundColor Cyan
dotnet publish (Join-Path $root "src\BookCtl.App\BookCtl.App.csproj") `
    -c $Configuration -r win-x64 --self-contained true `
    -p:PublishReadyToRun=false -p:PublishSingleFile=false -p:DebugType=none -v q --nologo -tl:false `
    -o $out
if ($LASTEXITCODE -ne 0) { throw "发布 App 失败" }

if (-not $SkipCli) {
    Write-Host "== 发布 CLI / MCP / Worker" -ForegroundColor Cyan
    foreach ($p in @("BookCtl.Cli", "BookCtl.Mcp", "BookCtl.Worker")) {
        dotnet publish (Join-Path $root "src\$p\$p.csproj") `
            -c $Configuration -r win-x64 --self-contained true `
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

# 版本信息
$edition = @"
{
  "product": "本地朗读工厂",
  "binary": "bookctl",
  "version": "$Version",
  "edition": "pro-portable",
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
Write-Host "便携版压缩包: $zip（压缩前请按需签名，见 packaging\sign.ps1）" -ForegroundColor Green
