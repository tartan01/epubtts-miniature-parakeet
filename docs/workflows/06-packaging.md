# 06 · 封装、便携版 / 安装版与代码签名

> 脚本：`packaging/build-portable.ps1`、`bookctl.iss`、`sign.ps1`、
> **`packaging/make-installer.ps1` + `mini/build.ps1`（v0.5 Mini）**；说明 `packaging/README.md`。
> 架构定义：`architecture-epub2audiobook.md §B.6` + **附录 C（Mini）**、README §9 / **§10**。

> **v0.5 起有两条产品线**，两个一键 `setup.exe` 共用同一套自研 Rust 安装器 stub：
> `本地朗读工厂-Mini-0.5.0-setup.exe`（0.7 MB）与 `本地朗读工厂-Full-0.3.0-setup.exe`（197.4 MB）。
> 详见本文件 **§8**。Inno Setup（§3）仍保留作为主线备选方案，本机未安装。

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
.\packaging\make-installer.ps1        # → 两个 setup.exe（Full 复用 dist\*-win-x64.zip）
.\packaging\make-installer.ps1 -SkipFull   # 只出 Mini
```

`mini/.cargo/config.toml` 指定 `rust-lld` + `link-self-contained`，本机无 MSVC / Windows SDK
也能链接，产物不依赖 VC++ Redistributable。`mini/_probe/probe.rs` 是换机验证链接的最小探针。

### 8.3 产物

| 文件 | 体积 | 说明 |
|---|---|---|
| `dist\release\本地朗读工厂-Mini-0.5.0-setup.exe` | **722,886 B** | 拷到另一台电脑测试用这个 |
| `dist\release\本地朗读工厂-Full-0.3.0-setup.exe` | **206,942,031 B** | 重量级主线（复用既有便携包 zip） |
| `dist\release\本地朗读工厂-Mini-0.5.0-portable.zip` | ≈345 KB | 免安装便携版 |
| `dist\release\setup-stub.exe` | 369,664 B | 未附加 payload 的裸 stub |

> `dist/release/` 被 `.gitignore` 排除，**不入库**；跨机测试只拷两个 setup.exe。

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
- [ ] 两个 setup.exe 都已重新生成（改过 `mini/setup/main.rs` 必须重打）
- [ ] 安装 / 卸载闭环实测，卸载保留用户数据
- [ ] 便携包 `使用说明.txt` 含 `chcp 65001` 提示（缺陷 #25）
- [ ] `git status` 里没有 `.rustup/` / `.cargo/` / `mini/target/` / `dist/release/` / 任何 exe
