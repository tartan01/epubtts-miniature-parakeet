# 本地朗读工厂 · 一键生成 Mini / 阅读器 / Full 三个自解压安装包
#
# 原理：Rust 编的安装器 stub 已在构建时产出于 dist\release\setup-stub.exe；
#      本脚本把便携包 zip 追加到 stub 尾部（PE overlay），得到单文件 setup.exe。
#      运行时 stub 读自身尾部 payload → 解压到用户目录 → 写 HKCU 注册表 → 建快捷方式。
#
# 用法：  .\packaging\make-installer.ps1
#         .\packaging\make-installer.ps1 -SkipFull   # 只出 Mini 轻量版

param(
    [switch]$SkipFull,
    [string]$MiniVersion = "0.6.0",
    [string]$ReaderVersion = "0.6.0",
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

Write-Host "== 打包 阅读器 载荷（同一份代码，主 exe 改名 BookCtl.Reader.exe）==" -ForegroundColor Cyan
$readerDir = Join-Path $release "reader-portable"
if (-not (Test-Path $readerDir)) { New-Item -ItemType Directory -Force -Path $readerDir | Out-Null }
Get-ChildItem $readerDir -Force | ForEach-Object { Remove-Item $_.FullName -Recurse -Force }
Copy-Item (Join-Path $miniDir "BookCtl.Mini.exe") (Join-Path $readerDir "BookCtl.Reader.exe") -Force
$rreadme = @"
本地朗读工厂 · 阅读器（边看边读）
=================================

双击 BookCtl.Reader.exe 打开阅读器：
  1) 把 .epub 从桌面直接拖进窗口，或点「打开电子书…」；
  2) 左边选章节，中间看正文，右边点段落直接跳转；
  3) 点「开始朗读」：正文里当前朗读的段落会高亮，
     朗读推进时正文自动跟随滚动（「跟随：关」可关掉，自己自由翻页）；
  4) 后台会自动把「当前位置往后 6 段 + 下一章前 2 段」的语音提前生成好，
     底部「预取」可调到 2 / 4 / 6 / 10 / 16 段；
  5) 人看一段要几秒，机器合成一段只要几百毫秒，所以基本不用等。

按钮说明：
  上一段 / 下一段    逐段跳转（朗读中跳转会立即从新位置继续）
  语速 − / +         0.5x ~ 2.0x；换语速会用独立缓存，不混用
  重合成本段         删除该段 WAV 重新合成（生成失败时用）
  跟随：开/关        文字是否随朗读自动滚动
  整本导出…          走批量流水线，用 Full 模型导出整本有声书（装了 Full 才有模型音质）

命令行（与 Mini / 主线同一套 JSON envelope）：
  BookCtl.Reader.exe reader --input 书.epub --dry-run
  BookCtl.Reader.exe reader --input 书.epub --headless --count 5
  BookCtl.Reader.exe reader status --input 书.epub
  BookCtl.Reader.exe reader prefetch --input 书.epub --from 10 --count 6
  BookCtl.Reader.exe reader --input 书.epub --play-test      # 验证播放链路

缓存位置（卸载不删）：%LOCALAPPDATA%\本地朗读工厂\ReaderCache\
依赖：仅 Windows 10/11 自带组件（tar.exe / PowerShell System.Speech / winmm.dll）。
"@
[System.IO.File]::WriteAllText((Join-Path $readerDir "使用说明.txt"), $rreadme,
    (New-Object System.Text.UTF8Encoding($false)))
$readerZip = Join-Path $release "本地朗读工厂-阅读器-$ReaderVersion-portable.zip"
$rz = New-Payload $readerDir $readerZip
$readerSetup = Join-Path $release "本地朗读工厂-阅读器-$ReaderVersion-setup.exe"
$rs = Append-Payload $stub $readerZip $readerSetup
Write-Host ("  zip  {0:N1} MB" -f ($rz / 1MB)) -ForegroundColor Green
Write-Host ("  exe  {0:N1} MB  ->  $readerSetup" -f ($rs / 1MB)) -ForegroundColor Green

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
Write-Host "  安装位置：%LOCALAPPDATA%\本地朗读工厂\<Mini|Reader|Full>" -ForegroundColor Yellow
Write-Host "  注册表：  HKCU\Software\本地朗读工厂\<Mini|Reader|Full>" -ForegroundColor Yellow
Write-Host "  卸载：    安装目录下 Uninstall-<Edition>.exe，或系统「程序和功能」" -ForegroundColor Yellow
Write-Host "  三个版本可同时安装：Mini=批量模式，阅读器=边看边读，Full=重量级模型主线" -ForegroundColor Yellow
