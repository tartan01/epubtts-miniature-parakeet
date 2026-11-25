# AGENTS.md · 仓库级 AI 工作约定

> 任何 AI（Claude / Codex / WorkBuddy / Copilot / 其它）在本仓库工作前必须读完本文件。
> 与 `HANDOVER.md` 冲突时，以 `HANDOVER.md` 的项目现状为准；**契约与工程纪律以本文件为准**。

---

## 1. 语言与沟通

- 用户是中文用户，日常用**简体中文**交流；回复用中文，代码注释可用中文或英文（保持文件既有风格）。
- 用户是架构师，偏好"许愿式"自然语言指令。**不要要求用户复述历史背景**，背景都在 `docs/` 里。
- 交付时先给**可验证的结论**（命令输出、文件路径、实测数字），再给建议；不要只写"已完成"。

## 2. 环境（本机快照，详见 `docs/environment.md`）

- OS：Windows 11 / x64；Shell：Git Bash 或 Windows PowerShell 5.1。
- **.NET SDK 10.0.404**，路径 `C:\Program Files\dotnet\dotnet.exe`，**不在 PATH** → 所有 `dotnet` 调用用绝对路径。
- Windows App SDK **2.7.0**（`Directory.Build.props` 统一），目标 `net10.0-windows10.0.19041.0` / `win-x64`。
- Python sidecar 独立 venv：`engine/.venv/Scripts/python.exe`（Python 3.13，`sherpa-onnx 2.0.3`）。
- ffmpeg / ffprobe 来自 WinGet 包目录；若 `PATH` 未刷新，`AudioMaster.ResolveFfmpeg` 有目录扫描兜底。
- 显卡：**1 张 GTX 1050 / 2GB**（算力 6.1），16 逻辑核。别假设有多卡或大显存。
- 网络：**国内直连 GitHub Release 会 502**，下载必须走镜像回退。
- **Rust 工具链（v0.5 mini 专用）**：`stable-x86_64-pc-windows-gnu`，装在工作区 `.rustup/` + `.cargo/`
  （**均不入库**），`rustc` 在 `.cargo/bin/rustc.exe`。本机**没有 MSVC / Windows SDK**，
  靠 `mini/.cargo/config.toml` 的 `rust-lld` + `link-self-contained` 自包含链接。
  换机先跑 `packaging/install-toolchain.cmd`（rsproxy.cn 镜像），再用 `mini/_probe/probe.rs` 验证链接。
- Windows 系统自带 `C:\Windows\System32\curl.exe` / `tar.exe` / `powershell.exe` 是 mini 的运行时依赖，
  **不要**改成第三方 crate 或外部下载工具。

## 3. 构建与验证（每次改动后都要跑）

```bash
ROOT="C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07"
DN="/c/Program Files/dotnet/dotnet.exe"
cd "$ROOT"

# 全解决方案
"$DN" build bookctl.sln -c Debug -v q --nologo

# 改了 BookCtl.Core 之后，必须单独重建 App（否则 App 输出目录里的 Core.dll 可能是旧的）
"$DN" build src/BookCtl.App/BookCtl.App.csproj -c Debug -v q --nologo

# 快速回归（校样引擎，2 秒）
BOOKCTL_WORKSPACE="$ROOT/testdata/ws-regression" \
  ./src/BookCtl.Cli/bin/Debug/net10.0/bookctl.exe \
  run --input "$ROOT/testdata/测试之书.epub" --engine mock
```

**改了 `Core` 却发现行为没变**：先关掉正在运行的 `BookCtl.App.exe`（DLL 被占用），
再单独重建 App，必要时 `-t:Rebuild`。这是本期最常踩的坑。

验收基线（对照 `HANDOVER.md §7`）：3 章 / 347 字 / `book.m4b` / 3 个章节标记 / `manifest.status=completed`。

**改了 `mini/` 之后（v0.5）**：

```powershell
.\mini\build.ps1                      # 编 BookCtl.Mini.exe + setup-stub.exe
.\packaging\make-installer.ps1        # 重打三个 setup.exe（Mini / 阅读器 / Full；-SkipFull 跳过 197MB）
dist\release\mini-portable\BookCtl.Mini.exe capabilities
dist\release\mini-portable\BookCtl.Mini.exe run --input testdata\测试之书.epub --dry-run
dist\release\mini-portable\BookCtl.Mini.exe run --input testdata\测试之书.epub --engine mock -o $env:TEMP\mini-reg
```

Mini 验收基线：`capabilities` 13 命令 / 8 源 / 4 模型；dry-run `3 章 / 362 字`；
委托 Full 返回 `chapters=3 / paragraphs=14 / seconds=10.1 / engine=mock-via-Full`（**不能是 0 或 sherpa-via-Full**）。
改 `mini/build.ps1` 时保持 `Invoke-Rustc()` 的 `ErrorActionPreference = "Continue"` + `Out-Null`：
PS 5.1 会把 rustc 的 stderr warning 当终止错误，且函数 stdout 会污染返回值。

## 4. 绝对不要做的事

- ❌ 不要把 `bin/`、`obj/`、`.venv/`、`dist/`、`*.m4b/m4a/wav/db/db-wal/db-shm`、模型权重（约 132MB）、日志、截图原始大图提交到仓库。`.gitignore` 已配置，改动前先确认 `git status`。
- ❌ 不要把证书、PFX、私钥、`BOOKCTL_PFX_PASS`、Azure Trusted Signing 凭据写进任何文件；签名脚本只读环境变量。
- ❌ **不要改 `packaging/*.ps1` / `*.iss` 的 UTF-8 BOM**。PowerShell 5.1 按 GBK 解析无 BOM 的中文脚本会直接语法崩溃。
- ❌ 不要给 `edition.json` 加 BOM（JSON 解析器会报错），用 `UTF8Encoding(false)` 写。
- ❌ 不要破坏 CLI 的 JSON envelope / 稳定退出码 / MCP tool 名——这是"大模型友好"的唯一稳定面，外部自动化依赖它。
- ❌ 不要在单次 MCP tool 调用里同步跑完整本书；所有耗时操作必须走 `job_*` 异步任务。
- ❌ 不要把设备选择结果（`DevicePlan`）放进 `profile_hash`——换显卡不应影响产物确定性判定。
- ❌ 不要删除 `.workbuddy/` 目录（WorkBuddy 项目数据，非缓存）。
- ❌ 不要把 `Console.OutputEncoding` 的设置挪回 `Emit()` 里——事件流 JSONL 会先输出，导致中文乱码。UTF-8 必须在 `Main()` 开头统一。
- ❌ **不要把 `.rustup/`、`.cargo/`、`mini/target/`、`mini/*.exe`、`dist/release/`、三个 setup.exe 提交入库**
  （`.gitignore` 已排除）；例外只有 `mini/.cargo/config.toml`。
- ❌ 不要在 `mini/` 里引入任何外部 crate（`serde` / `windows` / `windows-sys` 等）——mini 的价值就是零依赖、离线可构建。
- ❌ 不要改 `mini/build.ps1` 的 UTF-8 BOM；也不要把 `rustc` 换回 MSVC 工具链（本机没装）。
- ❌ **不要因为 PowerShell 5.1 显示中文日志乱码就去改 Mini 的输出编码**：字节本身是干净 UTF-8，
  改编码会破坏 CLI/MCP 的 JSON envelope 契约。正确做法是 `chcp 65001`。
- ❌ 不要在 `mini/setup/main.rs` 的 `Uninstall-*.exe` 路径里要求 payload：卸载器是 stub 头部（无 overlay），
  必须支持**无参数双击卸载**。
- ❌ **不要把阅读器缓存目录改回 `…\Reader`**：那是阅读器版的安装目录，卸载会整目录删除（缺陷 #29）。
  缓存必须在 `…\ReaderCache\`。
- ❌ 不要在 `NullReporter::log()` 里 `println!`：MCP stdio 与 CLI 的 stdout 是协议通道，
  任何非协议文本都会破坏 JSON-RPC / envelope（缺陷 #27）。
- ❌ 不要把 `reader` 改成整本合成后再开播，或为“逐字高亮”去订阅 SAPI `SpeakProgress`（ADR-028/029）。
- ❌ 不要为了播放给 `winmm` 加 `#[link]`：用已写好的 `win32::play_wav_sync()`（动态加载，ADR-030）。

## 5. 必须遵守的代码模式

| 场景 | 约定 |
|---|---|
| 新增业务能力 | 加在 `src/BookCtl.Core`，CLI/MCP/Worker/GUI 通过 `BookCtlHandlers` 自动获得 |
| 新增 CLI 命令 | 同步更新 `Program.cs` 的 Help、`--capabilities`、README §4、架构 MD §6.1 |
| 新增 MCP tool | 同步更新 `Tools` schema、分发 switch、`--capabilities` 的 tools 清单；方法必须 `static`（`Program` 是 static class） |
| 新增配置项 | `Contracts.cs` 的 `BookCtlConfig` + `ApplyEnv` + `CloneWith` + `config/bookctl.defaults.json` |
| 新增事件字段 | `Pipeline.cs` 的 `Emit` + `MainViewModel.AddLog` 消费 + 架构 MD §A.3 事件契约 |
| 新增引擎 | `Engine.cs` 的 `TtsProviderFactory` + `Device.RequiredVram` 显存门槛 + `Sources.BuiltInCatalog` + `engine/tts_server.py` |
| 音频拼接 | 用 `AudioMaster.ProbeDuration`（能读 WAV / m4a / ffprobe 兜底），不要用 `Wav.DurationOf` 读 m4a |
| ffmpeg 调用 | 一律走 `AudioMaster.Run()`，它已处理管道死锁与超时 |
| 文件编码 | 源码/文档 UTF-8；`.ps1`/`.iss` 带 BOM；JSON 产物无 BOM；`.cmd` 用 CRLF |
| Mini CLI 新命令 | 同步 `main.rs` 的 `cli()` 分发 + `help` 文本 + `capabilities_data().commands` + README §10.2 |
| Mini MCP 新 tool | 同步 `mcp_tool_defs()` + `mcp_call()` + `capabilities_data().mcp.tools` |
| Mini 新下载源 | 加进 `model.rs::sources()` + `effective_urls()` 的模板映射，保持 `probe_sources()` 自动测速可用 |
| 安装器改动 | 改 `mini/setup/main.rs` 后必须重跑 `mini\build.ps1` + `packaging\make-installer.ps1`，并实测安装/卸载闭环 |
| 阅读器新命令 | 同步 `reader.rs::handle_cli()` + `main.rs::cli()` 的 `reader` 分支 + `help` + `capabilities_data().commands` + README §11 |
| 阅读器状态变更 | 只改 `ReaderState` 字段 + 置 `ui_dirty = true`；**绝不在持锁时调 `play_wav_sync` 或弹 MessageBox**（与 `ui.rs` 的 WM_TIMER 快照同一约定） |
| 预取策略改动 | 改 `ReaderState::window_end()`；必须保持“至少伸进下一章前 2 段”，否则跨章会等生成 |
| 新增安装形态 | `meta_from_self()` 按文件名判定 Edition，优先级 Reader → Mini → Full；`make-installer.ps1` 同步加一个载荷块 |
| 发版 | 源码/历史版本只走 `git push` + tags；**安装包绝不入库**，统一用 `packaging\publish-release.ps1` 传到 GitHub Releases；改了发行文件要同步更新 `release-notes-v0.6.0.md` 的 SHA-256 |
| 写 PowerShell 脚本 | `.ps1` 必须 **UTF-8 带 BOM**（Windows PowerShell 5.1 按 ANSI 读无 BOM 文件会中文乱码 / 语法报错，见 §4 文件编码）；读中文 `name` 参数用 `[uri]::EscapeDataString`，大文件先复制到 ASCII 临时名再交给 `curl.exe` |

## 6. 架构变更流程

1. 先改 `architecture-epub2audiobook.md`（追加附录，保留历史版本，不要覆写 v0.1/附录 A/附录 B）。
2. 再改代码，编译 + 回归。
3. 同步 `README.md`、`docs/workflows/` 对应文件、`docs/decisions/ADR.md`（新增一条 ADR）。
4. 修 bug 追加到 `docs/defects.md` 与 `README.md §7.1`。
5. 更新 `docs/roadmap.md` 的状态。
6. 提交信息用中文，格式为 `<类型>: <一句话>`（类型：`feat` / `fix` / `docs` / `refactor` / `chore`）。

## 7. 文档同步清单（提交前自检）

- [ ] `docs/environment.md` 的路径/版本是否仍准确
- [ ] `HANDOVER.md §7` 实测基线是否变化
- [ ] `docs/defects.md` 是否记录了本次修复
- [ ] `docs/roadmap.md` 待办状态是否更新
- [ ] `.gitignore` 是否覆盖了新产生的大文件类型
- [ ] `git status --short` 里没有 `bin/obj/.venv/dist/模型/密钥`

## 8. 本仓库的来源

本项目由用户在 2026-11-23 ~ 2026-11-24 与 Agent 对话中从零构建，
完整对话、工作记忆、动作台账都在 `docs/conversation/` 与 `docs/worklog/`。
需要追溯"为什么这么设计"时，优先查 `docs/conversation/02-context-summaries.md` 与 `docs/decisions/ADR.md`。
