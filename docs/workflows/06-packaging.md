# 06 · 封装、便携版 / 安装版与代码签名

> 脚本：`packaging/build-portable.ps1`、`bookctl.iss`、`sign.ps1`、
> **`packaging/make-installer.ps1` + `mini/build.ps1`（v0.5 Mini）**；说明 `packaging/README.md`。
> 架构定义：`architecture-epub2audiobook.md §B.6` + **附录 C（Mini）**、README §9 / **§10**。

> **v0.6 起有两种源码线、三个一键 `setup.exe`**，共用同一套自研 Rust 安装器 stub：
> `本地朗读工厂-Mini-0.6.0-setup.exe`（0.7 MB，批量）、
> `本地朗读工厂-阅读器-0.6.0-setup.exe`（0.7 MB，边看边读）、
> `本地朗读工厂-Full-0.3.0-setup.exe`（197.4 MB，重量级主线）。
> 详见 **§8**（Mini/Full）与 **§9**（阅读器）。Inno Setup（§3）仍保留作为主线备选方案，本机未安装。

## 1. 两个版本的差异

| | 便携版（pro-portable） | 安装版（exe setup） |
|---|---|---|
| 触发 | 程序目录存在 `portable.marker` | Inno Setup 安装时**删除**该 marker |
| 数据目录 | `<程序目录>\data` | `%LOCALAPPDATA%\bookctl\workspace` |
| 模型源 | `<程序目录>\sources` | `%LOCALAPPDATA%\bookctl\sources` |
| Edition | `pro` | `pro` |
| 安装权限 | 免安装，U 盘可带走 | `PrivilegesRequired=lowest`（per-user，无需管理员） |
| 文件关联 | — | `.epub` 关联到 `BookCtl.App.exe` |

两版都是 **self-contained**：目标机不需要装 .NET 10，也不需要 Windows App Runtime。

判定实现在 `Contracts.ApplyPortable(cfg)`：
从 `AppContext.BaseDirectory` **逐级向上 ≤6 层**找 `portable.marker`（因为 CLI/MCP/Worker 发布在
`tools\<proj>\` 子目录），以 marker 所在目录为便携根。

## 2. 便携版构建

**一条命令出两种形态**（便携 zip + 可选安装包）：

```powershell
.\packaging\make-release.cmd            # 标准便携版（完全自包含）
.\packaging\make-release.cmd -Slim      # 轻量便携版（CLI 工具需 .NET 10）
```

或直接调发布脚本：

```powershell
$env:PATH = 'C:\Program Files\dotnet;' + $env:PATH
.\packaging\build-portable.ps1 -Configuration Release -Version 0.3.0
.\packaging\build-portable.ps1 -Configuration Release -Version 0.3.0 -Slim   # 输出 ...-slim
```

脚本做：
1. `dotnet publish` App → 输出根目录（self-contained，双击即用）；
2. `dotnet publish` CLI / MCP / Worker → `tools\<proj>\`（`-Slim` 时改 framework-dependent，省 3 份运行时）；
3. 拷贝 `config/`、`engine/`（**跳过 `.venv` / `__pycache__`**）、README、架构 MD；
4. 创建 `portable.marker`、`sources/`、`data/`、`edition.json`；
5. `-Slim` 额外删掉 `onnxruntime.dll` / `DirectML.dll`（本项目不加载）并写 `tools\PREREQUISITE.txt`；
6. `Compress-Archive -Force` 生成 `dist\本地朗读工厂-<ver>-win-x64[-slim].zip`。

产物目录：
```
dist\本地朗读工厂-0.3.0-win-x64\
├── BookCtl.App.exe          ← 双击启动（portable pro）
├── config/  engine/  tools\BookCtl.Cli|Mcp|Worker\
├── data/  sources/  portable.marker  edition.json
└── README.md  architecture-epub2audiobook.md
```
实测 zip ≈ **196 MB**。

### 两个必须记住的坑

1. **`.ps1` / `.iss` 必须带 UTF-8 BOM**。Windows PowerShell 5.1 按 GBK 解析无 BOM 的中文脚本，
   here-string 会被打乱，直接 `意外的标记":"` 语法崩溃（缺陷 #11）。
   而 `edition.json` 要**无 BOM**，用：
   ```powershell
   [System.IO.File]::WriteAllText($path, $edition, (New-Object System.Text.UTF8Encoding($false)))
   ```
2. **不要先删 `dist/` 旧目录**（1100+ 文件批量删除会被安全策略拦截）。
   v0.4 起脚本已改为增量覆盖 + `Compress-Archive -Force`，不再 `Remove-Item -Recurse`；
   需要彻底重来请手动删目录。等价的手动命令见 `HANDOVER.md` / 本文件 §6。

## 3. 安装版（Inno Setup 6）

```powershell
winget install JRSoftware.InnoSetup
iscc packaging\bookctl.iss
```

`bookctl.iss` 要点：
- `OutputBaseFilename=本地朗读工厂-<version>-setup`
- `PrivilegesRequired=lowest`
- `.epub` 文件关联 + 桌面快捷方式
- `[InstallDelete] Type: files; Name: "{app}\portable.marker"` → 安装后走用户级数据目录

## 4. 代码签名

```powershell
.\packaging\sign.ps1 [-Path <dir|file>]
```

支持三种凭据来源，**全部只读环境变量，私钥不进仓库**：
| 变量 | 用途 |
|---|---|
| `BOOKCTL_PFX` + `BOOKCTL_PFX_PASS` | 本地 PFX |
| `BOOKCTL_CERT_SHA1` | 证书存储指纹 |
| `BOOKCTL_TS_*` | Azure Trusted Signing |

命令形态：
```
signtool sign /fd sha256 /tr <RFC3161 时间戳> /td sha256 ...
signtool verify /pa /q
```
建议：先签目录内所有 exe/dll，**再对 setup.exe 单独签一次**。
不签名会被 SmartScreen 拦截，用户体验差。

> 本期**未实际签名**：用户尚未提供证书 / PFX / Azure Trusted Signing 参数。

## 5. Pro 版差异化（预留）

`BookCtlConfig.Edition` 已存在（默认 `community`；便携/安装版由 `ApplyPortable` 或 `edition.json` 置 `pro`）。
功能门禁、授权校验、增值功能尚未实现，见 `docs/roadmap.md`。

## 6. 原地增量发布（打包脚本被拦截时的等价命令）

```powershell
$root='C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07'
$dn='C:\Program Files\dotnet\dotnet.exe'
$out=Join-Path $root 'dist\本地朗读工厂-0.3.0-win-x64'
$ProgressPreference='SilentlyContinue'
& $dn publish "$root\src\BookCtl.App\BookCtl.App.csproj" -c Release -r win-x64 --self-contained true `
  -p:PublishReadyToRun=false -p:PublishSingleFile=false -p:DebugType=none -v q --nologo -tl:false -o $out
foreach($p in @('BookCtl.Cli','BookCtl.Mcp','BookCtl.Worker')){
  & $dn publish "$root\src\$p\$p.csproj" -c Release -r win-x64 --self-contained true `
    -p:PublishReadyToRun=false -p:DebugType=none -v q --nologo -tl:false -o (Join-Path $out "tools\$p")
}
Copy-Item "$root\config" (Join-Path $out 'config') -Recurse -Force
Get-ChildItem "$root\engine" -Force | Where-Object { $_.Name -notin @('.venv','__pycache__') } |
  Copy-Item -Destination (Join-Path $out 'engine') -Recurse -Force
[System.IO.File]::WriteAllText((Join-Path $out 'edition.json'), $edition, (New-Object System.Text.UTF8Encoding($false)))
Compress-Archive -Path (Join-Path $out '*') -DestinationPath "$root\dist\本地朗读工厂-0.3.0-win-x64.zip" `
  -CompressionLevel Optimal -Force
```

## 7. 发布前检查清单

- [ ] `dotnet build bookctl.sln -c Debug` 0 error / 0 warning
- [ ] `manifest.status = completed`（不是 `running`）
- [ ] 便携版 CLI：`workspace = dist\...\data`、`edition = pro`
- [ ] `portable.marker` 存在于程序根目录；安装版 `.iss` 会删它
- [ ] `engine/` 不含 `.venv`（体积）
- [ ] `edition.json` 无 BOM、`.ps1`/`.iss` 有 BOM
- [ ] 所有 exe/dll 已签名（若有证书）
- [ ] `.epub` 文件关联在安装版可用
- [ ] zip 内不含 `data\*.db` / 大音频 / 私钥

---

## 8. v0.5 · Mini / Full 两个一键 `setup.exe`（自研 overlay 安装器）

### 8.1 为什么自研

本机未装 Inno Setup（winget 安装需提权），且用户要求「两个版本都封装成 `.exe`，
一键安装、装完像常规 Windows 软件一样在用户目录生成文件夹并写注册表」。
方案 = Rust stub + PE overlay payload（7z SFX / NSIS 的标准做法，PE 允许尾部附加数据）：

```
<PE stub exe> <payload zip 字节> <payload 长度 u64 小端> <magic "BCTLPKG1">
                └──────────────── read_overlay() 从尾部读回 ────────────────┘
```

运行时 stub 读 `current_exe()` 尾部 → 解压 → 写 HKCU → 建快捷方式 → 复制自身**头部**
（不含 payload）作为卸载程序。HKCU 而非 HKLM ⇒ **不需要管理员权限**。

### 8.2 构建命令

```powershell
.\packaging\install-toolchain.cmd     # 首次：装 Rust GNU 工具链（rsproxy.cn 镜像）
.\mini\build.ps1                      # → dist\release\mini-portable\BookCtl.Mini.exe
                                      #   + dist\release\setup-stub.exe
.\packaging\make-installer.ps1        # → 三个 setup.exe（Full 复用 dist\*-win-x64.zip）
.\packaging\make-installer.ps1 -SkipFull   # 只出 Mini + 阅读器（省 197MB 重打）
```

`mini/.cargo/config.toml` 指定 `rust-lld` + `link-self-contained`，本机无 MSVC / Windows SDK
也能链接，产物不依赖 VC++ Redistributable。`mini/_probe/probe.rs` 是换机验证链接的最小探针。

### 8.3 产物

| 文件 | 体积 | 说明 |
|---|---|---|
| `dist\release\本地朗读工厂-Mini-0.6.0-setup.exe` | **761,141 B** | 批量模式，拷到另一台电脑测试用这个 |
| `dist\release\本地朗读工厂-阅读器-0.6.0-setup.exe` | **720,628 B** | 阅读器（见 §9） |
| `dist\release\本地朗读工厂-Full-0.3.0-setup.exe` | **206,943,055 B** | 重量级主线（复用既有便携包 zip） |
| `dist\release\本地朗读工厂-{Mini,阅读器}-0.6.0-portable.zip` | ≈390 / 350 KB | 免安装便携版 |
| `dist\release\setup-stub.exe` | 370,688 B | 未附加 payload 的裸 stub |

> `dist/release/` 被 `.gitignore` 排除，**不入库**；跨机测试只拷需要的 setup.exe（三个可同时安装）。

### 8.4 安装行为（HKCU，无需管理员）

- 目录：`%LOCALAPPDATA%\本地朗读工厂\<Mini|Full>`
- 注册表：
  - `HKCU\Software\本地朗读工厂\<Mini|Full>`：`InstallPath` / `Version` / `Edition` / `ExePath` / `ModelRoot` / `CliPath`
  - `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\本地朗读工厂 <Edition>`
  - `.epub` 关联：`HKCU\Software\Classes\…`（`OpenWithProgids` + `shell\open\command`）
- 快捷方式：开始菜单 + 桌面**两处**
- Full 载荷自带 `portable.marker` → 安装后**删除**，数据改走 `%LOCALAPPDATA%`

命令行：`setup.exe /S` 静默；`setup.exe /D=<路径>` 指定目录；`/MODELROOT=<路径>` 指定模型根。

### 8.5 两阶段卸载（必须知道）

卸载器 stub 就待在自己要删的目录里，Windows 不允许删除运行中的 exe：

1. 第一次运行把自己复制到 `%TEMP%\Uninstall-<Edition>.exe`，由副本带
   `/UNINSTALL /FROM_TMP`，**等待 600ms** 后执行删除；
2. 副本删程序目录、三处注册表、开始菜单与桌面两处快捷方式；
3. **保留用户模型与已生成的有声书**（只删程序目录）。

`Uninstall-*.exe` **无参数双击也必须能卸载**（`main()` 按文件名前缀判定，缺陷 #26）；
「程序和功能」走注册表 `UninstallString` 的显式 `/UNINSTALL /S`。

### 8.6 验收记录（2026-11-24 深夜）

| 检查 | 结果 |
|---|---|
| Mini `/S` 安装 | `exit=0`；`BookCtl.Mini.exe` 616,960 B；HKCU 产品信息 / Uninstall / `.epub` 关联 / 两处快捷方式全部核验 |
| Mini 安装版 `doctor` / `version` | `full_cli=true` / `0.5.0` |
| Mini 安装版委托 Full | `chapters=3 / paragraphs=14 / seconds=10.1 / engine=mock-via-Full` |
| Mini 卸载（`/UNINSTALL /S`） | `dir / reg_product / reg_uninstall / lnk_start / lnk_desktop` 全 False |
| Mini 重装 | `exit=0`，`version=0.5.0` |
| Full `/S` 安装 | `exit=0`（11.5 s）；`BookCtl.App.exe` + `tools\BookCtl.Cli\bookctl.exe` 在、marker 已删、注册表 `CliPath` 已写 |

> 非交互宿主里双击卸载会弹「卸载完成」MessageBox 并阻塞，自动化验收统一用 `/UNINSTALL /S`。

### 8.7 Mini 发布检查清单

- [ ] `.\mini\build.ps1` 编译 0 错误，`BookCtl.Mini.exe` ≈603 KB
- [ ] `capabilities` / `device list` / `doctor` / `source list` / dry-run 全部 JSON envelope 正常
- [ ] MCP `initialize` / `tools/list` 正常（6 tools）
- [ ] 委托 Full 后 `engine` 是动态标签（如 `mock-via-Full`），统计非 0（缺陷 #24）
- [ ] 三个 setup.exe 都已重新生成（改过 `mini/setup/main.rs` 必须重打）
- [ ] 安装 / 卸载闭环实测，卸载保留用户数据
- [ ] 便携包 `使用说明.txt` 含 `chcp 65001` 提示（缺陷 #25）
- [ ] `git status` 里没有 `.rustup/` / `.cargo/` / `mini/target/` / `dist/release/` / 任何 exe

---

## 9. v0.6 · 第三个包「阅读器」（同一代码、换名发布）

### 9.1 为什么只是改名

阅读器与 Mini 是**同一份 exe**：`mini/src/main.rs` 按 `current_exe()` 文件名是否含 `Reader`
决定默认进阅读器还是批量 GUI。所以不需要第二套构建，只需在载荷里把 exe 改名。

```powershell
.\mini\build.ps1                        # 编 BookCtl.Mini.exe（664 KB）
.\packaging\make-installer.ps1          # 一次出三个 setup.exe
.\packaging\make-installer.ps1 -SkipFull   # 只出 Mini + 阅读器（省 197 MB 重打）
```

`make-installer.ps1` 为阅读器做的事：
1. 建 `dist\release\reader-portable\`；
2. 复制 `mini-portable\BookCtl.Mini.exe` → `BookCtl.Reader.exe`；
3. 写阅读器专用的 `使用说明.txt`（按钮说明 + CLI + 缓存位置）；
4. 打 zip → 追加到 `setup-stub.exe` 尾部 → `本地朗读工厂-阅读器-0.6.0-setup.exe`。

### 9.2 安装器如何区分三个版本

`mini/setup/main.rs::meta_from_self()` 按**自己的文件名**判定，顺序不能颠倒：

| setup 文件名含 | Edition | 安装目录 | 主 exe | prog_id |
|---|---|---|---|---|
| `阅读器` / `Reader` | Reader | `…\本地朗读工厂\Reader` | `BookCtl.Reader.exe` | `LocalReadingFactory.Reader` |
| `Mini` | Mini | `…\本地朗读工厂\Mini` | `BookCtl.Mini.exe` | `LocalReadingFactory.Mini` |
| 其它 | Full | `…\本地朗读工厂\Full` | `BookCtl.App.exe` | `LocalReadingFactory.Full` |

三者的 HKCU 产品项、卸载项、快捷方式、`.epub` 关联互不冲突，可**同时安装**。
`.epub` 关联命令都是 `"<exe>" "%1"`，阅读器版双击 epub 直接进阅读器（按文件名判定，不需要额外参数）。

### 9.3 阅读器包的两个额外约束

- **缓存目录必须是 `…\本地朗读工厂\ReaderCache`**，不能是 `…\Reader`：后者是安装目录，
  卸载会 `remove_dir_all`。验证方法：卸载后 `Get-ChildItem ReaderCache -Filter *.wav` 数量不变。
- **`last.txt` 必须存绝对路径**：桌面/开始菜单快捷方式启动时 cwd 是安装目录，
  存相对路径会导致「双击后没恢复上次的书」。

### 9.4 验收记录（2026-11-24 夜）

| 检查 | 结果 |
|---|---|
| 阅读器 `/S` 安装 | `exit=0`；`BookCtl.Reader.exe` 679,424 B + `使用说明.txt` + `Uninstall-Reader.exe` |
| 注册表 | `HKCU\Software\本地朗读工厂\Reader` 的 `InstallPath/Version/Edition/ExePath` 全对；`DisplayName=本地朗读工厂 阅读器` |
| 快捷方式 | 开始菜单 `本地朗读工厂 阅读器.lnk` + 桌面同名 `.lnk` 均在 |
| 双击启动 | 从安装目录（cwd=安装目录）无参数启动 → 进阅读器并恢复上次书目，后台已备 7 段 |
| 卸载 | `dir / reg_product / reg_uninstall / lnk_start / lnk_desktop / epub_progid` 全 False；`ReaderCache` 3 个 WAV 保留 |
| 三包体积 | Mini 761,141 B / 阅读器 720,628 B / Full 206,943,055 B |

### 9.5 阅读器发布检查清单

- [ ] `reader --dry-run` 的 `prefetch_window` 跨到了下一章（`window_end` > 当前章段数）
- [ ] `--headless --count 5` 5 段全 `ready`，第二次 `cache_hits=5`
- [ ] 相对路径与绝对路径两次调用 `cache_dir` 相同（缺陷 #28）
- [ ] `--play-test` 返回 `playback_available=true`（无声卡环境可只检查该项）
- [ ] MCP `tools/list` 是 9 个工具，且 `tools/call` 的输出只有一行 JSON（缺陷 #27）
- [ ] 阅读器 setup.exe 安装 → 卸载 → 缓存保留 → 重装，四步全绿

---

## 10. 发布到 GitHub Releases（源码与发行版分离）

> 2026-11-25 用户拍板：**所有源码与历史版本推到 git 仓库；已经构建好的安装文件推到 Release，
> Release 就是直接下载即用的发行版。** 因此 `dist/` 继续被 `.gitignore` 排除，不把二进制塞进仓库。

### 10.1 两个通道，别混

| 通道 | 放什么 | 怎么推 |
|---|---|---|
| git 仓库 | 源码、脚本、文档、测试资产；**历史版本**用 tag 固定 | `git push origin main` + `git push origin --tags` |
| GitHub Releases | 三个 `setup.exe` + 两个便携 zip（用户双击即用） | `packaging\publish-release.ps1` 走 REST API |

历史版本 tag：`v0.3.0`（初始化全量源码）→ `v0.4.0`（拖放/分发）→ `v0.5.0`（Mini）→
`v0.6.0`（阅读器）。`git checkout v0.5.0` 即可拿到该版本完整源码；GitHub 也会为每个 tag
自动生成 `Source code (zip/tar.gz)`。

### 10.2 认证：SSH key ≠ API token

- 本机 SSH 密钥已配好（`ssh -T git@github.com` → `Hi tartan01!`），**只能推代码和 tag**。
- 创建 Release、上传 asset 必须调 `api.github.com` / `uploads.github.com`，需要一个
  **GitHub PAT**（fine-grained token 亦可，需 `Contents: read & write`）。
- 脚本从 `$env:GITHUB_TOKEN` 读；**不要把 token 写进仓库、日志或本文档**。

### 10.3 一键发布

```powershell
$env:GITHUB_TOKEN = '<PAT>'
.\packaging\publish-release.ps1 -DryRun        # 先离线核对清单与体积
.\packaging\publish-release.ps1                # 查/建 v0.6.0 Release 并上传 5 个资产
.\packaging\publish-release.ps1 -SkipLarge     # 只想快发小包时跳过 197MB 的 Full
```

脚本读 `packaging\release-notes-v0.6.0.md` 当 Release 正文；参数 `-Tag / -Title / -Repo / -NotesFile`
可改。已存在的同名资产会**先 DELETE 再上传**，所以失败重跑是安全的。

### 10.4 脚本为什么这么写（工程坑）

| 坑 | 处理 |
|---|---|
| Windows PowerShell 5.1 把无 BOM 的 `.ps1` 按 ANSI 解码，中文字符串乱码、语法直接报错 | 脚本存成 **UTF-8 带 BOM**（§4 文件编码约定） |
| 中文文件名传给 `curl.exe` 作 `--data-binary @路径` 可能编码错 | 先复制成 `$env:TEMP\...\assetN.bin` 的纯 ASCII 临时名 |
| query 里的中文资产名 | `[uri]::EscapeDataString($name)` 编码后再拼 URL |
| 197 MB 的 Full 包 | 用 `curl.exe --data-binary @file` 流式上传，`--max-time 1800`；不要走 `Invoke-RestMethod` 的内存字节数组 |
| PowerShell 里 `curl` 是 `Invoke-WebRequest` 别名 | 一律写 `curl.exe` |

### 10.5 发布检查清单

- [ ] `mini\build.ps1` + `packaging\make-installer.ps1` 已经重跑，五个发行文件齐全
- [ ] `publish-release.ps1 -DryRun` 列出的体积与 `release-notes-v0.6.0.md` 的 SHA-256 一致
- [ ] Release 正文写明「双击即用 / 免 .NET / 免管理员」与三个版本的差别
- [ ] 上传后逐个核对 `browser_download_url` 可下载，并比对 SHA-256
- [ ] `git status` 干净；四个 tag 已 `git push origin --tags`
- [ ] 仓库里没有 `.exe` / `.zip` / `.wav` / 任何 token 被提交
