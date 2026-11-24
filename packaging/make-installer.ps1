# 本地朗读工厂 · 一键生成 Mini / Full 两个自解压安装包
#
# 原理：Rust 编的安装器 stub 已在构建时产出于 dist\release\setup-stub.exe；
#      本脚本把便携包 zip 追加到 stub 尾部（PE overlay），得到单文件 setup.exe。
#      运行时 stub 读自身尾部 payload → 解压到用户目录 → 写 HKCU 注册表 → 建快捷方式。
#
# 用法：  .\packaging\make-installer.ps1
#         .\packaging\make-installer.ps1 -SkipFull   # 只出 Mini 轻量版

param(
    [switch]$SkipFull,
    [string]$MiniVersion = "0.5.0",
    [string]$FullVersion = "0.3.0"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$release = Join-Path $root "dist\release"
$stub = Join-Path $release "setup-stub.exe"

if (-not (Test-Path $stub)) {
    Write-Host "== 未找到 setup-stub.exe，先构建 Mini ==" -ForegroundColor Yellow
    & (Join-Path $root "mini\build.ps1")
    if ($LASTEXITCODE -ne 0) { throw "构建 Mini 失败" }
}

function New-Payload([string]$srcDir, [string]$zipPath) {
    if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
    Compress-Archive -Path (Join-Path $srcDir "*") -DestinationPath $zipPath -CompressionLevel Optimal -Force
    return (Get-Item $zipPath).Length
}

function Append-Payload([string]$stubPath, [string]$zipPath, [string]$setupPath) {
    if (Test-Path $setupPath) { Remove-Item $setupPath -Force }
    Copy-Item $stubPath $setupPath -Force
    $zs = [System.IO.File]::OpenRead($zipPath)
    $len = $zs.Length
    $fs = [System.IO.File]::Open($setupPath, [System.IO.FileMode]::Append, [System.IO.FileAccess]::Write)
    try {
        $zs.CopyTo($fs)
        $fs.Write([BitConverter]::GetBytes([uint64]$len), 0, 8)
        $fs.Write([System.Text.Encoding]::ASCII.GetBytes("BCTLPKG1"), 0, 8)
        $fs.Flush()
    } finally {
        $fs.Close(); $zs.Close()
    }
    return (Get-Item $setupPath).Length
}

Write-Host "== 打包 Mini 载荷 ==" -ForegroundColor Cyan
$miniDir = Join-Path $release "mini-portable"
if (-not (Test-Path (Join-Path $miniDir "BookCtl.Mini.exe"))) { throw "缺少 BookCtl.Mini.exe，先运行 mini\build.ps1" }
$miniZip = Join-Path $release "本地朗读工厂-Mini-$MiniVersion-portable.zip"
$mz = New-Payload $miniDir $miniZip
$miniSetup = Join-Path $release "本地朗读工厂-Mini-$MiniVersion-setup.exe"
$ms = Append-Payload $stub $miniZip $miniSetup
Write-Host ("  zip  {0:N1} MB" -f ($mz / 1MB)) -ForegroundColor Green
Write-Host ("  exe  {0:N1} MB  ->  $miniSetup" -f ($ms / 1MB)) -ForegroundColor Green

if (-not $SkipFull) {
    Write-Host "== 打包 Full 载荷（重量级主线）==" -ForegroundColor Cyan
    $fullZip = Join-Path $root "dist\本地朗读工厂-$FullVersion-win-x64.zip"
    if (-not (Test-Path $fullZip)) {
        Write-Host "  跳过：未找到 $fullZip（先运行 packaging\build-portable.ps1）" -ForegroundColor Yellow
    } else {
        # 直接把现有便携包 zip 作为载荷，不重新打包
        $fullSetup = Join-Path $release "本地朗读工厂-Full-$FullVersion-setup.exe"
        $fsz = Append-Payload $stub $fullZip $fullSetup
        Write-Host ("  zip  {0:N1} MB（复用既有便携包）" -f ((Get-Item $fullZip).Length / 1MB)) -ForegroundColor Green
        Write-Host ("  exe  {0:N1} MB  ->  $fullSetup" -f ($fsz / 1MB)) -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "安装包目录：$release" -ForegroundColor Cyan
Get-ChildItem $release -Filter "*-setup.exe" | ForEach-Object {
    Write-Host ("  {0}  {1:N1} MB" -f $_.Name, ($_.Length / 1MB))
}
Write-Host ""
Write-Host "验证：双击 setup.exe（或 setup.exe /S 静默安装）" -ForegroundColor Yellow
Write-Host "  安装位置：%LOCALAPPDATA%\本地朗读工厂\<Mini|Full>" -ForegroundColor Yellow
Write-Host "  注册表：  HKCU\Software\本地朗读工厂\<Mini|Full>" -ForegroundColor Yellow
Write-Host "  卸载：    安装目录下 Uninstall-Mini.exe / Uninstall-Full.exe，或系统「程序和功能」" -ForegroundColor Yellow
