#Requires -Version 5.1
<#
.SYNOPSIS
    把 dist\release 下「已经构建好」的安装包发布到 GitHub Releases。

.DESCRIPTION
    分工（交接给下一个 AI 时请记住）：
      * 源码 + 历史版本  -> git 仓库（main + v0.3.0 / v0.4.0 / v0.5.0 / v0.6.0 tags）
      * 直接下载即用的发行版 -> GitHub Releases 的 assets（本脚本负责）
    安装包一律不进 git 仓库（.gitignore 已排除 dist/ 与 *.zip/*.exe 产物）。

    认证：SSH 密钥只能 git push；创建 Release / 上传 asset 必须走 REST API，
    需要一个有 Contents 读写权限的 GitHub PAT（fine-grained token 亦可）。
    凭据优先级：-Token / $env:GITHUB_TOKEN > 本机
    %USERPROFILE%\.workbuddy\credentials\github-pat.txt（仓库外，绝不入库）。

.EXAMPLE
    $env:GITHUB_TOKEN = '<PAT>'
    .\packaging\publish-release.ps1
    .\packaging\publish-release.ps1 -DryRun        # 不联网，只核对发行文件清单
    .\packaging\publish-release.ps1 -SkipLarge     # 跳过 >100MB 的 Full 包
#>
param(
    [string]$Repo      = 'tartan01/epubtts-miniature-parakeet',
    [string]$Tag       = 'v0.6.0',
    [string]$Title     = '本地朗读工厂 v0.6.0 · 阅读器 / Mini / Full',
    [string]$Token     = $env:GITHUB_TOKEN,
    [string]$NotesFile = '',
    [switch]$SkipLarge,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

# 凭据优先级：显式 -Token / 环境变量 > 本机凭据文件（仓库外，绝不入库）
$credSource = 'env:GITHUB_TOKEN'
if (-not $Token) {
    $credFile = Join-Path $env:USERPROFILE '.workbuddy\credentials\github-pat.txt'
    if (Test-Path $credFile) {
        $Token = ([IO.File]::ReadAllText($credFile, [Text.Encoding]::UTF8)).Trim()
        $credSource = 'local credential file (outside repo)'
    } else {
        $credSource = 'none'
    }
}

$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist\release'
if (-not $NotesFile) { $NotesFile = Join-Path $PSScriptRoot 'release-notes-v0.6.0.md' }

# 发行文件清单（顺序 = Release 页面顺序）
$assets = @(
    '本地朗读工厂-Mini-0.6.0-setup.exe',
    '本地朗读工厂-阅读器-0.6.0-setup.exe',
    '本地朗读工厂-Full-0.3.0-setup.exe',
    '本地朗读工厂-Mini-0.6.0-portable.zip',
    '本地朗读工厂-阅读器-0.6.0-portable.zip'
)
if ($SkipLarge) {
    $assets = @($assets | Where-Object {
        $p = Join-Path $dist $_
        (Test-Path $p) -and ((Get-Item $p).Length -lt 100MB)
    })
}

# ---- 1) 本地校验（无需网络） ----
$missing = @()
foreach ($a in $assets) {
    $p = Join-Path $dist $a
    if (-not (Test-Path $p)) { $missing += $a }
    else { '{0,12:N0} B  {1}' -f (Get-Item $p).Length, $a }
}
if ($missing.Count -gt 0) { throw "缺少发行文件（先跑 mini\build.ps1 + packaging\make-installer.ps1）：$($missing -join ', ')" }
if (-not (Test-Path $NotesFile)) { throw "找不到 release notes：$NotesFile" }
if ($DryRun) { "DryRun OK：清单齐全，未联网。凭据来源：$credSource"; return }
if (-not $Token) { throw '未设置 GITHUB_TOKEN。请提供有 Contents 读写权限的 GitHub PAT（SSH key 不能调 REST API）。' }

$tmp = Join-Path $env:TEMP 'bookctl-release-upload'
if (-not (Test-Path $tmp)) { New-Item -ItemType Directory -Force -Path $tmp | Out-Null }
$common = @(
    '-sS', '-H', "Authorization: Bearer $Token",
    '-H', 'Accept: application/vnd.github+json',
    '-H', 'X-GitHub-Api-Version: 2022-11-28'
)
$api = "https://api.github.com/repos/$Repo"

function Invoke-Gh {
    param([string]$Method, [string]$Url, [string]$DataFile = $null, [string]$ContentType = $null)
    $curlArgs = @('-X', $Method) + $common
    if ($ContentType) { $curlArgs += @('-H', "Content-Type: $ContentType") }
    if ($DataFile)    { $curlArgs += @('--data-binary', "@$DataFile") }
    $curlArgs += @('--max-time', '1800', $Url)
    $out = & curl.exe @curlArgs 2>&1
    if ($LASTEXITCODE -ne 0) { throw "curl 失败（exit $LASTEXITCODE）：$out" }
    return ($out -join "`n")
}

# ---- 2) 查 / 建 Release ----
$rel = $null
try { $rel = (Invoke-Gh -Method 'GET' -Url "$api/releases/tags/$Tag") | ConvertFrom-Json } catch {}
if ($rel -and $rel.id) {
    "复用已有 Release：$Tag (id=$($rel.id))"
} else {
    $notes = [IO.File]::ReadAllText($NotesFile, [Text.Encoding]::UTF8)
    $payload = @{
        tag_name = $Tag; name = $Title; body = $notes
        draft = $false; prerelease = $false; make_latest = 'true'
    } | ConvertTo-Json -Depth 5
    $bodyFile = Join-Path $tmp 'release.json'
    [IO.File]::WriteAllText($bodyFile, $payload, (New-Object Text.UTF8Encoding($false)))
    $created = Invoke-Gh -Method 'POST' -Url "$api/releases" -DataFile $bodyFile -ContentType 'application/json'
    $rel = $created | ConvertFrom-Json
    if (-not $rel.id) { throw "创建 Release 失败：$created" }
    "已创建 Release：$Tag (id=$($rel.id))"
}

# ---- 3) 同名资产先删，保证脚本可重复运行 ----
$existing = @()
try { $existing = @((Invoke-Gh -Method 'GET' -Url "$api/releases/$($rel.id)/assets?per_page=100") | ConvertFrom-Json) } catch {}

foreach ($a in $assets) {
    $src = Join-Path $dist $a
    # 复制到纯 ASCII 临时文件，避免中文路径经 curl.exe 传参时的编码问题
    $idx = [array]::IndexOf($assets, $a)
    $stage = Join-Path $tmp ("asset$idx.bin")
    Copy-Item $src $stage -Force
    $old = $existing | Where-Object { $_.name -eq $a } | Select-Object -First 1
    if ($old) {
        "  删除同名旧资产：$a"
        $null = Invoke-Gh -Method 'DELETE' -Url "$api/releases/assets/$($old.id)"
    }
    $enc = [uri]::EscapeDataString($a)
    $url = "https://uploads.github.com/repos/$Repo/releases/$($rel.id)/assets?name=$enc"
    "  上传 $a ..."
    $res = Invoke-Gh -Method 'POST' -Url $url -DataFile $stage -ContentType 'application/octet-stream'
    $obj = $null
    try { $obj = $res | ConvertFrom-Json } catch {}
    if ($obj -and $obj.browser_download_url) { "    -> $($obj.browser_download_url)" }
    else { throw "上传 $a 失败：$res" }
    Remove-Item $stage -Force -ErrorAction SilentlyContinue
}

"完成：$($rel.html_url)"
