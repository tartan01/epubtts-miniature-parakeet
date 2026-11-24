# 代码签名脚本（Windows / signtool）
# 用途：给便携版 exe、dll 与 setup.exe 签名，消除 "未知发布者" 与 SmartScreen 拦截。
#
# 三种证书形态（任选其一，用环境变量切换）：
#   1) PFX 文件（最常用，OV/EV 证书导出）:
#        $env:BOOKCTL_PFX="D:\certs\localreading.pfx"; $env:BOOKCTL_PFX_PASS="******"
#   2) 证书存储中的指纹（EV 硬件令牌 / USB Key 安装到 CurrentUser\My）:
#        $env:BOOKCTL_CERT_SHA1="A1B2C3..."
#   3) Azure Trusted Signing（云签名，需 azure CLI 登录 + 配置文件）:
#        $env:BOOKCTL_TS_ENDPOINT="https://xxx.codesigning.azure.net"
#        $env:BOOKCTL_TS_ACCOUNT="myaccount"; $env:BOOKCTL_TS_PROFILE="myprofile"
#
# 用法：
#   powershell -ExecutionPolicy Bypass -File packaging\sign.ps1 -Path dist\本地朗读工厂-0.3.0-win-x64
#   powershell -ExecutionPolicy Bypass -File packaging\sign.ps1 -Path dist\本地朗读工厂-0.3.0-setup.exe
param(
    [Parameter(Mandatory = $true)][string]$Path,
    [string]$TimestampUrl = "http://timestamp.digicert.com"
)

$ErrorActionPreference = "Stop"

function Find-SignTool {
    $candidates = @()
    $kits = "C:\Program Files (x86)\Windows Kits\10\bin"
    if (Test-Path $kits) {
        $candidates += Get-ChildItem $kits -Directory -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending |
            ForEach-Object { Join-Path $_.FullName "x64\signtool.exe" }
    }
    $candidates += (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
    foreach ($c in $candidates) { if ($c -and (Test-Path $c)) { return $c } }
    throw "找不到 signtool.exe：请安装 Windows SDK（winget install Microsoft.WindowsSDK.10.0.26100）"
}

$signtool = Find-SignTool
Write-Host "signtool: $signtool" -ForegroundColor Cyan

# 组装签名参数
$signArgs = @()
if ($env:BOOKCTL_TS_ENDPOINT) {
    $signArgs += @("sign", "/fd", "sha256", "/tr", $TimestampUrl, "/td", "sha256",
        "/a", "/dlib", "Azure.CodeSigning.Dlib.dll",
        "/dmdf", $env:BOOKCTL_TS_METADATA)
    Write-Host "模式：Azure Trusted Signing" -ForegroundColor Cyan
}
elseif ($env:BOOKCTL_PFX) {
    $signArgs += @("sign", "/fd", "sha256", "/tr", $TimestampUrl, "/td", "sha256",
        "/f", $env:BOOKCTL_PFX)
    if ($env:BOOKCTL_PFX_PASS) { $signArgs += @("/p", $env:BOOKCTL_PFX_PASS) }
    Write-Host "模式：PFX 文件" -ForegroundColor Cyan
}
elseif ($env:BOOKCTL_CERT_SHA1) {
    $signArgs += @("sign", "/fd", "sha256", "/tr", $TimestampUrl, "/td", "sha256",
        "/sha1", $env:BOOKCTL_CERT_SHA1)
    Write-Host "模式：证书存储指纹" -ForegroundColor Cyan
}
else {
    throw "未配置证书：请设置 BOOKCTL_PFX(+BOOKCTL_PFX_PASS) / BOOKCTL_CERT_SHA1 / BOOKCTL_TS_* 之一"
}

$targets = @()
if (Test-Path $Path -PathType Container) {
    $targets = Get-ChildItem $Path -Recurse -Include *.exe, *.dll |
        Where-Object { $_.FullName -notmatch '\\engine\\.venv\\' }
}
else {
    $targets = @(Get-Item $Path)
}

Write-Host "待签名文件：$($targets.Count) 个" -ForegroundColor Cyan
$failed = 0
foreach ($t in $targets) {
    & $signtool @signArgs $t.FullName
    if ($LASTEXITCODE -ne 0) { Write-Warning "签名失败：$($t.Name)"; $failed++ }
}

# 校验
Write-Host "== 校验签名" -ForegroundColor Cyan
foreach ($t in $targets) {
    & $signtool verify /pa /q $t.FullName
    if ($LASTEXITCODE -ne 0) { Write-Warning "校验失败：$($t.Name)" }
}

if ($failed -gt 0) { Write-Warning "$failed 个文件签名失败" }
else { Write-Host "全部签名完成，可重新打包 zip / 重新生成 setup.exe" -ForegroundColor Green }
