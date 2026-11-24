# 本地朗读工厂 · Mini 构建脚本（PowerShell 5.1 兼容，UTF-8 BOM）
# 本机没有 MSVC / Windows SDK：用 rustup 的 GNU 工具链 + rust-lld 自包含链接。
# 用法：  .\mini\build.ps1            # 编译 Mini 主程序 + 安装器 stub
#         .\mini\build.ps1 -Clean     # 先清理产物目录

param(
    [switch]$Clean,
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$env:RUSTUP_HOME = Join-Path $root ".rustup"
$env:CARGO_HOME  = Join-Path $root ".cargo"
$rustc = Join-Path $env:CARGO_HOME "bin\rustc.exe"

if (-not (Test-Path $rustc)) {
    throw "找不到 Rust 工具链：$rustc`n先运行 packaging\install-toolchain.cmd 安装。"
}

if ($OutDir -eq "") { $OutDir = Join-Path $root "dist\release" }
if ($Clean -and (Test-Path $OutDir)) {
    Get-ChildItem $OutDir -Force | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
$miniDir = Join-Path $OutDir "mini-portable"
New-Item -ItemType Directory -Force -Path $miniDir | Out-Null

# 统一的编译参数：体积优先、无符号、自包含链接
$common = @(
    "--edition", "2021",
    "-A", "warnings",
    "-C", "opt-level=s",
    "-C", "lto=fat",
    "-C", "codegen-units=1",
    "-C", "panic=abort",
    "-C", "strip=symbols",
    "-C", "linker=rust-lld",
    "-C", "link-self-contained=yes"
)

# PS 5.1 在 $ErrorActionPreference='Stop' 下会把原生命令的 stderr 当成终止错误，
# 必须临时切回 Continue 并只依据退出码判断。
function Invoke-Rustc([string[]]$toolArgs) {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    # 必须吞掉 rustc 的 stdout，否则函数返回值会变成 @(输出..., 退出码)
    & $rustc @toolArgs | Out-Null
    $code = $LASTEXITCODE
    $ErrorActionPreference = $prev
    return $code
}

Write-Host "== 编译 Mini 主程序 ==" -ForegroundColor Cyan
$mainOut = Join-Path $miniDir "BookCtl.Mini.exe"
$mainArgs = $common + @((Join-Path $root "mini\src\main.rs"), "-o", $mainOut)
$code = Invoke-Rustc $mainArgs
if ($code -ne 0) { throw "Mini 主程序编译失败（exit $code）" }

Write-Host "== 编译安装器 stub ==" -ForegroundColor Cyan
$stubOut = Join-Path $OutDir "setup-stub.exe"
$stubArgs = $common + @((Join-Path $root "mini\setup\main.rs"), "-o", $stubOut)
$code = Invoke-Rustc $stubArgs
if ($code -ne 0) { throw "安装器编译失败（exit $code）" }

# Mini 便携目录的附带文件
Copy-Item (Join-Path $root "README.md") (Join-Path $miniDir "README.md") -Force -ErrorAction SilentlyContinue
Copy-Item (Join-Path $root "architecture-epub2audiobook.md") (Join-Path $miniDir "architecture.md") -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path (Join-Path $miniDir "models") | Out-Null
$readme = @"
本地朗读工厂 · Mini（轻量入口）
================================

双击 BookCtl.Mini.exe 打开界面：
  1) 把 .epub 从桌面直接拖进窗口；
  2) 选运行环境（自动 / CPU / GPU / 混合）；
  3) 在「模型下载管理」里选安装盘（C/D/E…）并点「安装 / 下载模型」；
     没网或某个源失败会自动切换到下一个内置源；
  4) 点「打开模型目录」可直接定位到模型文件所在文件夹；
  5) 点「一键生成朗读」，完成后点「打开输出文件夹」。

命令行（与主线同一套 JSON envelope）：
  BookCtl.Mini.exe capabilities
  BookCtl.Mini.exe device list
  BookCtl.Mini.exe source list
  BookCtl.Mini.exe source pull sherpa-vits-zh-fanchen --root D:\本地朗读工厂模型
  BookCtl.Mini.exe run --input 书.epub --device auto
  BookCtl.Mini.exe mcp          # MCP stdio 服务

依赖：仅 Windows 10/11 自带组件（curl.exe / tar.exe / PowerShell System.Speech），
      本目录内的 exe 不需要 .NET、不需要 Python、不需要安装任何运行时。

PowerShell 5.1 用户看中文日志乱码时（宿主按 GBK 解码子进程 stderr）：
  先执行 chcp 65001，或
  [Console]::OutputEncoding = [Text.Encoding]::UTF8
GUI 界面与 stdout 的 JSON 信封始终是干净 UTF-8，不受影响。
"@
[System.IO.File]::WriteAllText((Join-Path $miniDir "使用说明.txt"), $readme,
    (New-Object System.Text.UTF8Encoding($false)))

Write-Host ""
Write-Host "Mini 主程序 : $mainOut" -ForegroundColor Green
Write-Host "安装器 stub : $stubOut" -ForegroundColor Green
$sz = (Get-Item $mainOut).Length
Write-Host ("Mini 体积   : {0:N0} KB" -f ($sz / 1KB)) -ForegroundColor Green
