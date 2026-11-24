# HANDOVER · 给下一位接手 AI 的总入口

> **读我优先。** 这份文件是「本地朗读工厂（bookctl）」项目在本期（2026-11-23 ~ 2026-11-24）
> 结束时的完整交接。目标是：你不需要追问用户任何历史问题，就能直接继续优化、写代码。
>
> 用户是**架构师**，习惯用中文自然语言「许愿式」下指令，由 Agent 负责读写源码、编译、运行验证、回报结果。
> 请保持同样的协作方式：**先动手验证，再给结论；不要让用户复述历史**。

---

## 0. 一句话现状

一个可编译、可运行、已端到端验收的 **Windows 原生桌面软件 + CLI + MCP** 三合一项目：
输入一本 `.epub`，在本地（CPU 或 GPU）用开源 TTS 把整本书转成**按章节整理**的有声书
（`book.m4b` + 每章 `audio.wav/m4a` + `manifest.json`），带实时中控台、断点续传、设备选择、模型源管理，
并已有可发布的便携版 zip。

版本：主线 **v0.3.0**，Mini 轻量产品线 **v0.5.0**。远端 `main` 已推送到
`git@github.com:tartan01/epubtts-miniature-parakeet.git`。

**2026-11-24 晚增量（v0.4）**：新增 EPUB 拖放导入（两种模式通用）、统一 `ImportEpub` 入口、
根目录 `启动.cmd` / `build.cmd` / `packaging/make-release.cmd` 三个一键脚本、
`build-portable.ps1 -Slim` 轻量分发形态。详见 README §2/§8/§9、ADR-019/020、`docs/defects.md` #13–21。

**2026-11-24 深夜增量（v0.5，本轮）**：新增 **独立 `mini/` 产品线** —— Rust / Win32 原生单 exe
603 KB，零模型、零 .NET、零 Python；内置 8 个下载源自动测速、模型装到任意盘、检测到 Full 就委托真 TTS；
GUI 保留全部按钮并支持拖入 `.epub` / `config.json`；CLI/MCP 契约与主线一致。
**两个版本都封装成一键 `setup.exe`**（Mini 0.7 MB / Full 197.4 MB），安装写 HKCU 注册表、
用户目录、快捷方式、`.epub` 关联，两阶段卸载保留用户数据。
详见 README **§10**、架构 MD **附录 C**、ADR-021~026、`docs/defects.md` #22–#26。

> 用户已明确「等一下会拿另一台电脑传过去测试」：可直接拷 `dist/release/` 下两个 setup.exe，
> 验收步骤见 README §10.2。

---

## 1. 文档地图（先看哪几份）

| 优先级 | 文件 | 内容 |
|---|---|---|
| ★★★ | `HANDOVER.md`（本文件） | 现状、怎么跑、代码地图、坑、下一步 |
| ★★★ | `AGENTS.md` | 仓库级 AI 工作约定：必须遵守的构建/提交/契约规则 |
| ★★★ | `architecture-epub2audiobook.md` | **总架构唯一事实源**（v0.1 + 附录 A v0.2 + 附录 B v0.3 + **附录 C v0.5 mini**）。任何设计变更先改它 |
| ★★★ | `README.md` | 用户向说明、命令速查、实现边界、§7 缺陷台账、**§10 Mini 与跨机测试** |
| ★★ | `docs/workflows/` | 7 份流程文档：构建运行 / 流水线 / 控制面 / 设备与源 / TTS 引擎 / 封装（含 mini）/ 验收 |
| ★★ | `docs/decisions/ADR.md` | 技术决策台账（ADR-001~026，含 mini 的 021~026） |
| ★★ | `docs/defects.md` | 26 条缺陷/问题台账（含 mini 的 #22–#26；现象 → 根因 → 修法 → 验证） |
| ★★ | `docs/roadmap.md` | 未完成事项与建议推进顺序（R-13 轻量产品线已落地；R-14/15/16 待办） |
| ★★ | `docs/environment.md` | 本机环境快照、绝对路径、版本、网络限制（含 Rust 工具链） |
| ★ | `docs/conversation/01-full-dialogue.md` | 本期完整对话（用户 7 轮 + 助手 112 条回复 + 系统事件） |
| ★ | `docs/conversation/02-context-summaries.md` | 上下文压缩的历史摘要（补齐全量背景） |
| ★ | `docs/conversation/03-action-ledger.md` | 493 次工具调用全量台账（改过哪些文件、跑过哪些命令） |
| ★ | `docs/worklog/` | 本期工作记忆日志（按日；`2026-11-24-v05.md` 记 Mini 收尾） |
| ★ | `docs/screenshots/` | UI 实拍（简洁/详细/真语音 + 拖放 + slim + **07~09 Mini 界面与安装版**） |
| ★ | `architecture-epub2audiobook.html` | 架构可视化评审版（浏览器打开） |
| ★ | `engine/README.md`、`packaging/README.md` | 引擎层与封装层说明 |

> 原始 WorkBuddy 会话 jsonl（含工具结果原文）**未入库**，因为它包含宿主运行时上下文。
> 需要时用 `tools/export_session.py <session.jsonl> docs/conversation` 重新导出。

---

## 2. 5 分钟跑起来（验证你拿到的是能跑的代码）

本机 `dotnet` **不在 PATH**，必须用绝对路径 `C:\Program Files\dotnet\dotnet.exe`。
以下命令在 Git Bash 或 PowerShell 均可（中文路径用引号）。

**最快两条路**：先双击 `build.cmd`（编译 + 重建 App + mock 回归一体化），成功后双击 `启动.cmd` 打开界面。

```bash
ROOT="C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07"
cd "$ROOT"

# 1) 构建（0 error / 0 warning）
"/c/Program Files/dotnet/dotnet.exe" build bookctl.sln -c Debug -v q --nologo

# 2) 自检：工作区 / 显卡 / ffmpeg / Python
"./src/BookCtl.Cli/bin/Debug/net10.0/bookctl.exe" doctor --pretty

# 3) 最快全链路（校样引擎，无需模型、无需显卡，约 2 秒）
BOOKCTL_WORKSPACE="$ROOT/testdata/ws-quickcheck" \
  "./src/BookCtl.Cli/bin/Debug/net10.0/bookctl.exe" \
  run --input "$ROOT/testdata/测试之书.epub" --engine mock
```

**真中文语音**（需要 sidecar，模型已装在本机 `%LOCALAPPDATA%\bookctl\sources\sherpa-vits-zh-fanchen`）：

```bash
# 终端 A：启动 TTS sidecar（保持运行）
./engine/.venv/Scripts/python.exe engine/tts_server.py --mode sherpa --port 8765 --device cpu

# 终端 B：整本转换（3 章 / 约 98 秒音频 / 合成约 34 秒，RTF≈0.35）
BOOKCTL_WORKSPACE="$ROOT/testdata/ws-check-sherpa" \
  "./src/BookCtl.Cli/bin/Debug/net10.0/bookctl.exe" \
  run --input "$ROOT/testdata/测试之书.epub" --engine sherpa --device cpu
```

**桌面程序**（WinUI 3 unpackaged，双击即可，无需安装 Windows App Runtime）：

```
src\BookCtl.App\bin\Debug\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe
```

命令行导入 EPUB 并自动开始（配合安装包 `.epub` 文件关联）：

```bash
BookCtl.App.exe "C:\...\testdata\测试之书.epub" --auto-start [--detailed] [--on-top]
```

> `--auto-start` 之后走**简洁模式一键生成**；`--detailed` 进详细参数模式；`--on-top` 仅用于演示截图。
> **sidecar 没启动不会报错**：一键模式自动退回 `mock` 校样引擎并在界面提示原因（v0.3 兜底）。

产物目录（"项目文档文件夹"）：`<workspace>/<sha>_<书名>/`
内含 `book.m4b`、`book.ffmetadata`、`concat_list.txt`、`manifest.json`、`qc/report.json`、
`chapters.json`、`segments.json`、`segments/*.wav`、`audio/chapters/000N_章名/{audio.wav,audio.m4a,chapter.json}`。

---

## 3. 代码地图

```
启动.cmd                       ← 唯一 GUI 入口（自动找便携包 / Debug / Release 的 BookCtl.App.exe）
build.cmd                      ← 一键：全解决方案编译 + 单独重建 App + mock 回归
bookctl.sln
├── src/BookCtl.Core/            ← 全部业务逻辑，GUI/CLI/MCP/Worker 共用
│   ├── Contracts.cs             JSON envelope / 错误码 / 退出码 / BookCtlConfig
│   │                            （含 ApplyEnv / ApplyPortable / SourcesRoot）
│   ├── Models.cs                GpuStatus / BookMeta / Chapter / Segment / SynthesisProfile / JobRecord / JobEvent
│   ├── JobStore.cs              SQLite WAL：jobs / chapters / events，CAS + revision + 幂等 + checkpoint + 事件流
│   ├── Epub.cs                  zip + container.xml + OPF + manifest/spine + NCX/nav.xhtml
│   │                            TOC anchor 切章 / DRM 判定 / 中文 TN / 分句
│   ├── Engine.cs                ITtsProvider / MockTtsProvider / SidecarTtsProvider / TtsProviderFactory
│   │                            Wav.Concat / GpuMonitor(nvidia-smi) / AudioMaster(ffmpeg M4B + ffmetadata)
│   ├── Pipeline.cs              六阶段 S1–S6 + EventBus + DevicePlan + WorkerHost + 孤儿任务回收
│   ├── Handlers.cs              GUI/CLI/MCP 共用的 JobStart/Status/Logs/Cancel/Resume/Capabilities/Doctor/Device/Source
│   ├── Device.cs                GpuDevice / DevicePlan / DevicePlanner.Resolve(auto|cpu|gpu|hybrid)
│   └── Sources.cs               SourceManager：内置目录 / 多线路下载 / 解包拍平 / .installed.json
├── src/BookCtl.Cli/Program.cs   CLI 入口（JSON envelope + 稳定退出码 + 实时 JSONL 事件流）
├── src/BookCtl.Mcp/Program.cs   MCP stdio JSON-RPC 2.0（手写，无外部依赖）
├── src/BookCtl.Worker/Program.cs 独立 Worker 入口（复用 Core.WorkerHost）
├── src/BookCtl.App/             WinUI 3 桌面程序
│   ├── App.xaml                 Apple 风格全局资源（#F5F5F7 / #0071E3 / Segoe UI Variable）
│   ├── MainWindow.xaml          双模式界面（简洁 / 详细参数）+ 结果卡 + 中控台
│   ├── MainWindow.xaml.cs       ApplyCommandLine + 拖放（DragOver/Drop/DragLeave + 全窗口提示层）
│   └── MainViewModel.cs         EventBus 订阅、章节卡片实时追加、设备计划、源状态、一键兜底
├── engine/tts_server.py        Python sidecar：mock / sherpa / cosyvoice3 三模式
├── config/                     bookctl.defaults.json / sources.example.json
├── packaging/                  build-portable.ps1(-Slim) / make-release.cmd / bookctl.iss / sign.ps1 / README.md
│                               + install-toolchain.cmd / make-installer.ps1（mini）
├── testdata/测试之书.epub       自造 3 章中文 EPUB（2.6 KB，验收用）
└── tools/export_session.py     会话转录 → Markdown 交接文档导出器

mini/                           ← v0.5 独立 Rust 产品线（不引用 src/ 任何代码）
├── README.md                   mini 的模块说明 / 构建 / 契约 / 已知边界
├── .cargo/config.toml          rust-lld + link-self-contained（免 MSVC/SDK）
├── Cargo.toml / Cargo.lock     单 crate、零外部依赖
├── build.ps1                   编 BookCtl.Mini.exe + setup-stub.exe
├── _probe/probe.rs             Win32 链接探针（换机先验）
├── src/main.rs                 GUI/CLI/MCP 入口分发 + capabilities/doctor/device/source/config/run
├── src/win32.rs                手写 Win32 FFI（kernel32/user32/comctl32/comdlg32/shell32/gdi32/advapi32）
├── src/ui.rs                   原生窗口、WM_TIMER 快照刷新、拖放、进度条、日志
├── src/model.rs                运行配置 + 8 下载源 + 4 模型 + 测速/下载/切源/选盘
├── src/audio.rs                EPUB→SAPI WAV；delegate_full() 委托 Full CLI
├── src/util.rs                 手写 JSON 解析/序列化 + envelope（无 serde）
└── setup/main.rs               自解压安装器（PE overlay → HKCU → 快捷方式 → 两阶段卸载）
```

**依赖方向**：`Core` 不依赖任何上层；`Cli/Mcp/Worker/App` 全部只引用 `Core` 的 `BookCtlHandlers`。
新增能力优先加在 `Core`，四个入口自动获得。**`mini/` 是故意独立的第二条产品线**：
Rust/std only，不引用 `src/`，只通过 CLI 契约与 Full 主线通信（注册表 `CliPath` → `bookctl.exe run`）。

---

## 4. 控制面契约（改接口前必读）

- 所有 CLI 输出为 JSON envelope：`{"ok":true,"api_version":"1.0","data":...}` 或
  `{"ok":false,"error":{"code":"...","message":"..."}}`，配合稳定退出码。
- `run` 会输出**实时事件流 JSONL** + 纯文本进度行 + 最终 envelope；UTF-8 无 BOM 在 `Main()` 开头统一设置。
- 长任务一律 job 化：`job_start / job_status / job_logs / job_cancel / job_resume / job_artifacts`。
- MCP tools（stdio JSON-RPC）：`capabilities / doctor / job_* / device_list / source_list / source_pull`。
- 事件 `segment_done` 带 `chars / seconds / speed(chars/s) / rtf / device / device_display / eta`；
  `chapter_done` 驱动 GUI 章节卡片；`job_completed` 带 `result_path`。
- 详细命令表见 `docs/workflows/03-control-plane.md` 与架构 MD §6.1/§6.2。

---

## 5. 必须知道的技术决策（详见 `docs/decisions/ADR.md`）

1. **WinUI 3 unpackaged**（`WindowsPackageType=None` + `WindowsAppSDKSelfContained=true`），
   不是 WPF、不是 MSIX。WinUI 的 `Window` **没有 `DataContext`**，绑定挂在根 `Grid x:Name="Root"` 上。
2. **默认真引擎是 `sherpa-onnx` CPU**（`vits-zh-hf-fanchen-C`，187 音色，16 kHz，约 132 MB），
   因为本机只有 **GTX 1050 / 2GB**。`cosyvoice3`（Apache-2.0，中文最好）需 ≥6GB 显存，仅保留适配点。
3. **不存在能提速的「GPU + CPU 拆分式 TTS 推理」**：张量并行要 ≥2 张算力 ≥7.0 的卡（1050 = 6.1 被拒），
   `device_map` 层卸载只解决"装不下"且更慢，ORT 单 session 单 EP。故提供 `auto/cpu/gpu/hybrid` 三档，选择权交给用户。
4. **排除 VibeVoice**（强制可听 AI 声明 + 水印）、**XTTS/Coqui**（CPML 非商用 + 主体注销）。
5. **"DDS 源管理"落地为「内置引擎/模型源目录 + 一键下载/解包/安装状态」**，
   国内直连 GitHub Release 502，必须镜像回退（用户 mirrors → `BOOKCTL_MIRROR` → `gh-proxy.com` / `hf-mirror.com`）。
6. **设备不参与 `profile_hash`**：换显卡不应改变产物确定性判定。
7. **简洁模式不是另一套流水线**：只是写会话配置后调用同一个 `JobStart`。
8. **v0.5 新增独立 `mini/` 产品线**（用户已拍板 A 路线）：Rust + Win32 原生，零外部 crate、
   零 .NET/Python、单 exe 603 KB；用 `rust-lld` 自包含链接避开 MSVC/SDK。与主线**并存不推倒重写**。
9. **Mini 是「轻量入口」而非另一套推理引擎**：内置 8 源自动测速 + 模型装到任意盘；
   检测到 Full 就按 CLI 契约委托真 TTS 模型，没装 Full 用系统 SAPI 兜底。
10. **两个版本都自研 overlay 自解压 `setup.exe`**（不依赖 Inno Setup）：写 HKCU、用户目录、
    快捷方式、`.epub` 关联；卸载两阶段且保留用户模型/有声书。
11. **PowerShell 5.1 中文日志乱码是宿主编解码问题**：实测 stdout/stderr 字节均为干净 UTF-8，
    `chcp 65001` 即可，不要去改程序输出编码（会破坏 JSON envelope 契约）。

---

## 6. 已知坑（踩过，别再踩）

| # | 坑 | 正确做法 |
|---|---|---|
| 1 | `dotnet build bookctl.sln` 后 `App\bin\...\win-x64\BookCtl.Core.dll` 可能仍是旧版 | 改了 `Core` 却"没生效"时，**单独重建 App 项目**（必要时 `-t:Rebuild`），并先关闭正在运行的 `BookCtl.App.exe` 释放 DLL |
| 2 | PowerShell 5.1 把 UTF-8 无 BOM 的 `.ps1` 按 GBK 解析，中文 here-string 直接语法崩溃 | `packaging/*.ps1` 与 `.iss` **必须带 UTF-8 BOM**；`edition.json` 反而要**无 BOM**（用 `WriteAllText` + `UTF8Encoding(false)`） |
| 3 | 打包脚本清空旧目录会被安全策略拦截（>1000 文件批量删除） | 原地增量覆盖发布，再 `Compress-Archive -Force`，不要先删 `dist/` |
| 4 | 长跑的 `source pull` 会锁 `BookCtl.Cli/bin` 的 DLL，导致全解决方案 build 失败 | 等下载完成再编译 |
| 5 | ffmpeg 重定向 stdout/stderr 却不读取 → 管道写满死锁（曾卡 3 分钟） | `AudioMaster.Run()` 并发 `ReadToEndAsync()` + `-hide_banner -loglevel error -nostats` + 超时 Kill |
| 6 | `WinUI` 的 `Grid` 不能设 `FontFamily`（不是 WPF 控件） | 在 `App.xaml` 用 `ContentControlThemeFontFamily` 全局设置 |
| 7 | `BookCtl.Mcp.Program` 是 `static class`，新增方法必须 `static` | 参考 `PullSource` 的写法 |
| 8 | WinUI 内容用 `gdigrab -i title=` 抓到空白（DirectComposition） | 用 `-i desktop` 全屏抓；需要窗口在前台时给 App 传 `--on-top` |
| 9 | 中文路径 / EPUB `mimetype` 顺序 / zip 内反斜杠 | 测试 EPUB 用 Python `zipfile` 生成，`mimetype` 第一个且不压缩、路径统一 `/` |
| 10 | `BOOKCTL_WORKSPACE` 一改，旧实现里模型源跟着漂移 | `SourcesRoot` 已固定为 `%LOCALAPPDATA%\bookctl\sources`（便携版由 marker 覆盖） |

完整 26 条缺陷/问题台账见 `docs/defects.md` 与 `README.md §7.1`（#22–#26 为 v0.5 Mini）。

---

## 7. 本期实测基线（回归测试对照）

| 项目 | 基线值 |
|---|---|
| 全解决方案编译 | `dotnet build bookctl.sln -c Debug` → 0 error / 0 warning |
| 测试书 | `testdata/测试之书.epub`，3 章，352 字（有效 347 字） |
| mock 全链路 | `book.m4b` 10.154 s，3 个中文章节标记 |
| sherpa 全链路 | 3 章 / 14 段 / **97.9~98.2 s** 音频；首轮 57.8 s，模型加载后 **34.2 s（RTF≈0.35）** |
| M4B | 762,919 B；ffprobe 章节 `0–39.014 / 39.014–74.301 / 74.301–98.207`，title/artist 正确 |
| 分段 WAV | 14 个，全 16 kHz 非静音（peak 8.5k–11.4k，rms 1.3k–1.6k） |
| MCP | `initialize` 返回 `2025-11-25`，`tools/list` 13+ tools |
| GPU 枚举 | 1 张 GTX 1050 2GB / 16 逻辑核 |
| 便携版 | `dist/本地朗读工厂-0.3.0-win-x64.zip` ≈196 MB；CLI `workspace=dist\...\data`、`edition=pro` |
| UI | 简洁模式一键完成显示「3/3 章 · 347/352 字 · 已生成 1分33秒」+ 3 张章节卡片 |

**v0.5 Mini 实测基线（对照用）**：

| 项目 | 基线值 |
|---|---|
| `BookCtl.Mini.exe` | 616,960 B（603 KB），零外部 crate / 零 .NET / 零 Python |
| CLI | `capabilities` 13 命令 / 8 源 / 4 模型；`device list` 16 线程 / 1 盘；`doctor`、`source list` 正常 |
| MCP | `initialize` → `2025-06-18`；`tools/list` → 6 tools |
| EPUB dry-run | `《测试之书》· 3 章 · 362 字` |
| SAPI 真语音 | 3 章 / 14 段 / 94.56 s / `wav_bytes=8,340,024` |
| 委托 Full | `chapters=3 / paragraphs=14 / seconds=10.1 / engine=mock-via-Full / wav_bytes≈681,002` |
| setup.exe | Mini 722,886 B；Full 206,942,031 B |
| 安装/卸载 | HKCU + Uninstall + `.epub` 关联 + 两处快捷方式全核验；两阶段卸载后 5 项全 False |

---

## 8. 还没做（接手后从这里开始）

按 `docs/roadmap.md` 推进，简要：

- **P0 · 签名安装包**：`packaging/sign.ps1` + `bookctl.iss` 已写好，缺用户证书（PFX / EV / Azure Trusted Signing 参数）。
- **P0 · `source pull` 从干净目录重跑**：解包残留修复后尚未完整复验。
- **P1 · `cosyvoice3` 真实权重接入**：需 ≥6GB 显存机器（本机 2GB 跑不了）。
- **P1 · 多显卡 0/1/2 实机验证**：逻辑已实现，本机只有 1 张卡。
- **P2 · S5 ASR 回验闭环**（faster-whisper）、M4B 两遍 loudnorm、音色一致性 A/B 盲测。
- **P2 · Pro 版差异化**：`Edition` 字段已预留，尚未做功能门禁。
- **R-13 独立轻量产品线：✅ 已完成（v0.5）**。用户已拍板 A 路线，`mini/` 落地并验收，
  两个 setup.exe 已产出。跨另一台电脑的实测由用户进行（**R-16**，步骤见 README §10.2）。
- **R-14 · Mini 直接调用本地模型权重**：目前 Mini 只在「检测到 Full」时委托 Full CLI；
  模型已下载但 Full 未安装时仍走 SAPI，不能直接加载权重。需先与用户确认是否接受
  Mini 不再是「单文件零依赖」（见 `docs/roadmap.md`）。
- **P3 · Mini GUI 视觉**：Win32 原生控件受系统主题限制，白卡对比与 Apple 风格仍弱于 WinUI 主线；
  功能与契约均已验收，视觉可继续细化。
- **P3 · 跨机验收**：用户准备拿另一台干净 Windows 测试两个 setup.exe；
  重点看「无 .NET / 无 Python / 免管理员」是否真的开箱即用，以及卸载是否保留用户数据。

---

## 9. 给接手 AI 的工作方式建议

1. **先读** `architecture-epub2audiobook.md` 的附录 A/B 与 `AGENTS.md`，再动代码。
2. 用户用中文「许愿式」下指令时：直接改文件 → 编译 → 跑一遍验收 → 用中文回报结果与剩余风险。
3. 任何**接口/契约/架构**变更，同步更新：架构 MD → README → `docs/workflows/` → `docs/decisions/ADR.md`。
4. 任何**修 bug**，追加到 `docs/defects.md` 与 `README.md §7.1`，并写明复现现象与根因。
5. 任何**环境/路径**变化，更新 `docs/environment.md`。
6. 提交前确认 `.gitignore` 生效，不要把 `bin/obj/.venv/dist/模型/密钥/大音频` 推上去。
7. 不要凭记忆改代码：本机绝对路径、版本号、命令都写在 `docs/environment.md`，以它为准。
8. 遇到"改了没生效"，先怀疑第 6 节第 1 条（Core.dll 增量复制）。

---

**本期交接生成时间**：2026-11-24 22:2x GMT+8（v0.5 Mini 收尾后更新）
**目标仓库**：`git@github.com:tartan01/epubtts-miniature-parakeet.git`
