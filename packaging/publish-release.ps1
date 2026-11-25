#Requires -Version 5.1
<#
.SYNOPSIS
    把 dist\release 下「已经构建好」的安装包发布到 GitHub Releases。

.DESCRIPTION
    分工（交接给下一个 AI 时请记住）：
      * 源码 + 历史版本  -> git 仓库（main + v0.3.0 / v0.4.0 / v0.5.0 / v0.6.0 tags）
      * 直接下载即用的发行版 -> GitHub Releases 的 assets（本脚本负责）
    安装包一律不进 git 仓库（.gitignore 已排除 dist/ 与 *.zip 产物）。

    重要坑：GitHub Releases 会过滤资产名里的非 ASCII 字符（?name= query 与
    PATCH 的 JSON body 都会把中文剥成 "-Mini-0.6.0-setup.exe" 这类残缺名）。
    所以 Release 上的资产统一用 ASCII 名 BookCtl-*.exe，再用 PATCH 给资产加
    中文 label（label 允许 Unicode），页面显示中文、下载文件名保持清晰。

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
$utf8NoBom = New-Object Text.UTF8Encoding($false)

# 资产表：Release 名（ASCII）/ 本地中文文件名 / 页面中文 label
$assetTable = @(
    [pscustomobject]@{ Name = 'BookCtl-Mini-0.6.0-setup.exe';      File = '本地朗读工厂-Mini-0.6.0-setup.exe';      Label = '本地朗读工厂 Mini 0.6.0 · 批量模式（一键安装包）' }
    [pscustomobject]@{ Name = 'BookCtl-Reader-0.6.0-setup.exe';    File = '本地朗读工厂-阅读器-0.6.0-setup.exe';    Label = '本地朗读工厂 阅读器 0.6.0 · 边看边读（一键安装包）' }
    [pscustomobject]@{ Name = 'BookCtl-Full-0.3.0-setup.exe';      File = '本地朗读工厂-Full-0.3.0-setup.exe';      Label = '本地朗读工厂 Full 0.3.0 · 重量级主线（197 MB）' }
    [pscustomobject]@{ Name = 'BookCtl-Mini-0.6.0-portable.zip';   File = '本地朗读工厂-Mini-0.6.0-portable.zip';   Label = '本地朗读工厂 Mini 0.6.0 · 免安装便携版' }
    [pscustomobject]@{ Name = 'BookCtl-Reader-0.6.0-portable.zip'; File = '本地朗读工厂-阅读器-0.6.0-portable.zip'; Label = '本地朗读工厂 阅读器 0.6.0 · 免安装便携版' }
)
if ($SkipLarge) {
    $assetTable = @($assetTable | Where-Object {
        $p = Join-Path $dist $_.File
        (Test-Path $p) -and ((Get-Item $p).Length -lt 100MB)
    })
}

# ---- 1) 本地校验（无需网络） ----
$missing = @()
foreach ($a in $assetTable) {
    $p = Join-Path $dist $a.File
    if (-not (Test-Path $p)) { $missing += $a.File }
    else { '{0,12:N0} B  {1}  <- {2}' -f (Get-Item $p).Length, $a.Name, $a.File }
}
if ($missing.Count -gt 0) { throw "缺少发行文件（先跑 mini\build.ps1 + packaging\make-installer.ps1）：$($missing -join ', ')" }
if (-not (Test-Path $NotesFile)) { throw "找不到 release notes：$NotesFile" }
if ($DryRun) { "DryRun OK：清单齐全，未联网。凭据来源：$credSource"; return }
if (-not $Token) { throw '未设置 GITHUB_TOKEN 且本机凭据文件不存在。请提供有 Contents 读写权限的 GitHub PAT（SSH key 不能调 REST API）。' }

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
    # 197MB 上传偶发 curl (55) Connection was reset（服务端其实可能已收全），自动重试
    $curlArgs += @('--retry', '3', '--retry-delay', '2', '--max-time', '1800', $Url)
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
        draft = $false; prerelease = $false; make_latest = $true
    } | ConvertTo-Json -Depth 5
    $bodyFile = Join-Path $tmp 'release.json'
    [IO.File]::WriteAllText($bodyFile, $payload, $utf8NoBom)
    $created = Invoke-Gh -Method 'POST' -Url "$api/releases" -DataFile $bodyFile -ContentType 'application/json; charset=utf-8'
    $rel = $created | ConvertFrom-Json
    if (-not $rel.id) { throw "创建 Release 失败：$created" }
    "已创建 Release：$Tag (id=$($rel.id))"
}

# Release 正文也一并同步（改过 notes 后重跑不会留旧说明）
$notesNow = [IO.File]::ReadAllText($NotesFile, [Text.Encoding]::UTF8)
$bodyPatch = @{ body = $notesNow } | ConvertTo-Json -Depth 5
$bodyPatchFile = Join-Path $tmp 'release-body.json'
[IO.File]::WriteAllText($bodyPatchFile, $bodyPatch, $utf8NoBom)
$null = Invoke-Gh -Method 'PATCH' -Url "$api/releases/$($rel.id)" -DataFile $bodyPatchFile -ContentType 'application/json; charset=utf-8'

# ---- 3) 同名资产先删，保证脚本可重复运行 ----
$existing = @()
try { $existing = @((Invoke-Gh -Method 'GET' -Url "$api/releases/$($rel.id)/assets?per_page=100") | ConvertFrom-Json) } catch {}

$idx = 0
foreach ($a in $assetTable) {
    $src = Join-Path $dist $a.File
    # 复制到纯 ASCII 临时文件，避免中文路径经 curl.exe 传参时的编码问题
    $stage = Join-Path $tmp ("asset$idx.bin")
    Copy-Item $src $stage -Force

    $old = $existing | Where-Object { $_.name -eq $a.Name } | Select-Object -First 1
    if ($old) {
        "  删除同名旧资产：$($a.Name)"
        $null = Invoke-Gh -Method 'DELETE' -Url "$api/releases/assets/$($old.id)"
    }

    $enc = [uri]::EscapeDataString($a.Name)   # ASCII 名，编码后仍是 ASCII
    $url = "https://uploads.github.com/repos/$Repo/releases/$($rel.id)/assets?name=$enc"
    "  上传 $($a.Name) ..."
    $res = Invoke-Gh -Method 'POST' -Url $url -DataFile $stage -ContentType 'application/octet-stream'
    $obj = $null
    try { $obj = $res | ConvertFrom-Json } catch {}
    if (-not ($obj -and $obj.id)) { throw "上传 $($a.Name) 失败：$res" }

    # 中文 label 用 JSON body 设置（不用 query，页面显示中文、文件名保持 ASCII）
    $labelPatch = @{ name = $a.Name; label = $a.Label } | ConvertTo-Json -Depth 4
    $labelFile = Join-Path $tmp ("label$idx.json")
    [IO.File]::WriteAllText($labelFile, $labelPatch, $utf8NoBom)
    $null = Invoke-Gh -Method 'PATCH' -Url "$api/releases/assets/$($obj.id)" -DataFile $labelFile -ContentType 'application/json; charset=utf-8'
    "    -> $($obj.browser_download_url)"

    Remove-Item $stage -Force -ErrorAction SilentlyContinue
    Remove-Item $labelFile -Force -ErrorAction SilentlyContinue
    $idx++
}

"完成：$($rel.html_url)"
