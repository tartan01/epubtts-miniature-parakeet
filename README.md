# 本地朗读工厂（bookctl）· Windows 本地 EPUB 有声书工厂

> 🤖 **接手的 AI 请先读 [`HANDOVER.md`](./HANDOVER.md) 和 [`AGENTS.md`](./AGENTS.md)**，
> 再看 [`docs/README.md`](./docs/README.md) 的文档地图。本期（2026-11-23 ~ 24）的完整对话、
> 工作记忆、技术决策、已修缺陷、待办路线图与环境快照全在 `docs/` 下，**不需要追问用户历史背景**。
>
> 📦 **分工（2026-11-25 起）**：**源码 + 全部历史版本在 git 仓库**（`main` + `v0.3.0` / `v0.4.0` /
> `v0.5.0` / `v0.6.0` tags）；**直接下载即用的安装包在 GitHub Releases**
> `https://github.com/tartan01/epubtts-miniature-parakeet/releases`（三个 setup.exe + 便携 zip）。
> `dist/` 被 `.gitignore` 排除，构建产物**不入库**；发版用 `packaging\publish-release.ps1`。

> 产品名：**本地朗读工厂**（备选名：帅帅朗读工厂）；内部代号 `bookctl`，可执行文件保持 `bookctl.exe` / `BookCtl.App.exe`。
> 总架构唯一事实源：[`architecture-epub2audiobook.md`](./architecture-epub2audiobook.md)。
> 本 README 只讲「怎么构建、怎么跑、怎么打包」。代码与架构文档的目录映射见附录 A.6。

一个把 `.epub` 整本书在**本地**（CPU 或显卡）转成「按章节分类整理好的朗读音频」的 Windows 桌面软件：

- **两种界面模式**：*简洁模式*（拖入书 → 一键生成，自动选 CPU 轻量引擎）／*详细参数模式*（引擎/音色/显卡/语速/M4B 全可控）
- **拖放导入**：桌面或资源管理器里的 `.epub` 直接拖进窗口即可，两种模式通用，不用再点「浏览」找路径
- **推理设备可切换**：`auto / 纯 CPU / 纯 GPU / GPU+CPU 混合`，可指定显卡序号（0/1/2），完全交给用户决定
- **引擎与模型源管理**：内置源清单 + 一键下载到本地 + 校验 + 自动镜像回退（国内直连 GitHub 会 502）
- **CLI 一等公民**：`bookctl.exe`，JSON envelope、稳定退出码、幂等、断点续传
- **MCP 原生支持**：`bookctl-mcp.exe`（stdio，16 个 tools，含 `device_list` / `source_pull`）
- **可封装分发**：便携版 zip + Inno Setup 安装包 + signtool 签名脚本（见 `packaging/README.md`）
- **Mini 轻量产品线（v0.5）**：Rust / Win32 单 exe 603 KB，零模型、零 .NET、零 Python；
  内置 8 个下载源自动测速、模型装到 C/D/E 任意盘、检测到重量级主线就委托真 TTS 模型。
  两个版本都有一键 `setup.exe` 安装包，详见 **§10**。


---

## 1. 环境要求（VS Code 路线，无需完整 Visual Studio）

| 组件 | 要求 | 说明 |
|---|---|---|
| .NET SDK | **10.x** | `winget install Microsoft.DotNet.SDK.10`；`dotnet --version` 验证 |
| Windows 11 SDK | 10.0.22621+ | WinUI 3 XAML 编译需要；装 WinApp CLI 时会引导安装 |
| WinApp CLI（可选但推荐） | `winget install Microsoft.WinAppCLI` 或 `dotnet tool install` | 2026 年官方免-VS 工具链：脚手架、运行、打包（https://learn.microsoft.com/windows/apps/dev-tools/winapp-cli） |
| WinUI 模板（可选） | `dotnet new install Microsoft.WindowsAppSDK.WinUI.CSharp.Templates` | 出现版本不匹配时的对照来源 |
| VS Code | 任意近版 | 只是编辑器；F5 调试用本项目 `.vscode/` 配置 |
| ffmpeg | 可选 | 有则产出整本 M4B + AAC 章节；没有则分章 WAV（manifest 标 degraded） |
| Python 3.9+ | 可选 | 仅 sidecar 真实引擎需要 |

**版本口径**：`Directory.Build.props` 中 `WindowsAppSDKVersion` 默认 `2.7.0`（2026-11 NuGet 最新稳定版，已核实存在）。
若还原失败（例如 SDK 落后），执行 `dotnet new install Microsoft.WindowsAppSDK.WinUI.CSharp.Templates` 后
`dotnet new winui -n probe`，把 `probe.csproj` 中两个 PackageReference 的版本抄回 `Directory.Build.props` 即可，其余代码不动。

> **构建状态（2026-11-24 实测）**：本机已安装 .NET SDK 10.0.404，`dotnet build bookctl.sln` **全解决方案 0 error 0 warning 通过**，
> WinUI App 已实际启动并渲染窗口；mock 引擎端到端跑通（3 章 EPUB → 分章 `audio.wav` + `audio.m4a` + 整本 `book.m4b`，含中文章节标记）。
> 实测中发现并已修复 **30 条缺陷/问题**（v0.3 的 12 个运行期缺陷 + v0.4 的 9 条结构/交互/分发问题
> + v0.5 Mini 的 5 条 + v0.6 阅读器的 4 条，含 3 个"清单/文档说 A、实际跑 B"的隐蔽问题），
> 详见 §7.1 与 `docs/defects.md`。

## 2. 构建与一键入口

**只想跑起来**：双击仓库根目录的 **`启动.cmd`** —— 它自动找最新的便携包或 Debug 构建里的
`BookCtl.App.exe` 并启动，也可以把 `.epub` 路径当参数传进去。

**想验证改动没坏**：双击 **`build.cmd`** —— 编译全解决方案 → 单独重建 App → 跑 mock 回归，一条命令闭环。

本机 `dotnet` 不在 PATH，两个脚本都写死了绝对路径 `C:\Program Files\dotnet\dotnet.exe`。

**exe 到底在哪**（`BookCtl.App.exe` 是唯一 GUI 入口，其余三个是命令行工具，不能双击当界面用）：

| 你要的 | 路径 |
|---|---|
| 桌面程序（开发态） | `src\BookCtl.App\bin\Debug\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe` |
| 桌面程序（便携包） | `dist\本地朗读工厂-<版本>-win-x64\BookCtl.App.exe` |
| 命令行 CLI | `src\BookCtl.Cli\bin\Debug\net10.0\bookctl.exe` |
| MCP 服务 | `src\BookCtl.Mcp\bin\Debug\net10.0\bookctl-mcp.exe` |
| 后台 Worker | `src\BookCtl.Worker\bin\Debug\net10.0\bookctl-worker.exe` |

手动构建：

```powershell
dotnet build bookctl.sln -c Debug

# 只构建桌面 App（WinUI 需要 RID，csproj 已内置 win-x64）
dotnet build src/BookCtl.App/BookCtl.App.csproj -c Debug
```

产物：
- `src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe`（双击可运行，unpackaged 自包含）
- `src/BookCtl.Cli/bin/Debug/net10.0/bookctl.exe`
- `src/BookCtl.Mcp/bin/Debug/net10.0/bookctl-mcp.exe`
- `src/BookCtl.Worker/bin/Debug/net10.0/bookctl-worker.exe`

## 3. 快速开始（三分钟验收全链路，mock 引擎、无需显卡）

```powershell
# 1) 自检：工作区 / GPU / ffmpeg / Python
dotnet run --project src/BookCtl.Cli -- doctor

# 2) 一键转一本书（提交 + 内嵌 worker + 实时 JSONL 事件流）
dotnet run --project src/BookCtl.Cli -- run --input "C:\books\示例.epub"

# 3) 查产物
dotnet run --project src/BookCtl.Cli -- job list
```

完成后到工作区（默认 `%LOCALAPPDATA%\bookctl\workspace\<book-id>_<书名>\`）检查：
`manifest.json`、`audio/chapters/0001_序章/audio.wav(.m4a)`、`book.m4b`、`chapters.json`、`qc/report.json`。

**桌面软件**：VS Code 打开本目录按 F5（`.vscode/launch.json` 已配好），或双击仓库根目录的 `启动.cmd`。
界面 = **把 `.epub` 直接拖进窗口**（或点「浏览…」）→ 引擎/音色/语速/M4B → 「一键生成朗读音频」 →
中控台实时看阶段、显卡利用率、TTS 段进度、速度与 ETA → 完成后「打开项目文档文件夹」。
命令行也支持 `BookCtl.App.exe "D:\books\某书.epub" --auto-start`（安装版注册 `.epub` 关联后双击书同效）。

## 4. CLI 速查（详见 `bookctl` 帮助与总架构 §6.1）

```
bookctl capabilities                          # 能力清单（给大模型读）
bookctl doctor                                # 环境自检
bookctl book inspect|chapters|plan --input <epub>
bookctl engine list | engine test --text "..."
bookctl job start --input <epub> [--engine sidecar] [--voice zh-default]
bookctl job status <id> / list / logs <id> --follow / cancel / resume / artifacts
bookctl run --input <epub>                    # 一键（等价 GUI 的一键生成）
```

环境变量：`BOOKCTL_WORKSPACE`（工作区目录）、`BOOKCTL_ENGINE`（mock|sidecar）、
`BOOKCTL_SIDECAR_URL`、`BOOKCTL_VOICE`、`BOOKCTL_FFMPEG`。
配置文件：`config/bookctl.defaults.json`（分层：CLI 参数 > 环境变量 > 配置文件）。

## 5. MCP 接入（Claude Desktop / Cline / 任意 MCP 宿主）

`mcp.json` 示例（注意用 exe 的**绝对路径**）：

```json
{
  "mcpServers": {
    "bookctl": {
      "command": "C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/src/BookCtl.Mcp/bin/Debug/net10.0/bookctl-mcp.exe",
      "env": { "PYTHONUTF8": "1", "BOOKCTL_ENGINE": "mock" }
    }
  }
}
```

可用 tools：`capabilities` `doctor` `plan` `book_inspect` `book_chapters` `job_start` `job_status`
`job_logs` `job_cancel` `job_resume` `job_artifacts` `engine_list` `engine_test`；
resources：`job:///{id}/manifest.json|log.ndjson`；prompt：`operate_playbook`（标准编排手册）。
对话里让宿主模型「先用 capabilities 和 plan，再 job_start，轮询 job_logs」即可全自动转书。

## 6. 接真实显卡引擎（P1，架构文档 §A.7 的适配点）

1. 按 `engine/requirements.txt` 装 CosyVoice 3 依赖与权重（Apache-2.0）
2. `python engine/tts_server.py --mode cosyvoice3 --model-dir <权重目录> --port 8765`
3. `BOOKCTL_ENGINE=sidecar` 或 GUI 下拉框选 sidecar
4. 逐句接入点都在 `engine/tts_server.py` 的 `ensure_model()` / `synth_real()`，注释里写明了约束（全书一次音色、失败明确返回 503、stdout 干净）

## 7. 当前实现边界（诚实清单）

已实现：EPUB zip/OPF/nav/NCX 解析 + TOC anchor 切章 + kind 分类 + DRM 判定（字体混淆不误杀）、
规则版中文 TN + 语义分句、六阶段流水线（S1–S6）、SQLite WAL 状态机（CAS/幂等/断点续传/协作取消）、
mock 引擎全链路、GPU 监控（nvidia-smi）、WAV 拼接 + ffmpeg M4B/ffmetadata、三入口（WinUI/CLI/MCP）、
段级内容寻址缓存、项目文档文件夹 + manifest.json + qc 报告。

留作适配点（上层接口已留好）：CosyVoice 3 真实推理、WeTextProcessing 完整 TN、faster-whisper ASR 回验、
两遍 loudnorm 精细参数（当前 ffmpeg 基础转码）、MCP Streamable HTTP 传输、多书批量队列 UI。

已知 Windows 坑：见总架构 §7（UTF-8 二进制 stdout、Session 0 无 GPU、Triton 编译 OOM、PowerShell 5.1 无 `&&` 等）。

## 7.1 已修复的实测缺陷（2026-11-24 编译 + 端到端实测）

| # | 现象 | 根因 | 修复 |
|---|---|---|---|
| 1 | WinUI 编译失败 `CS0103: DataContext 不存在` | WinUI 3 的 `Window` 不是 `FrameworkElement`，没有 `DataContext`（WPF 才有） | 根 `Grid x:Name="Root"` 承载 DataContext 供子元素继承 |
| 2 | 第 1 章合成完后报 `WAV_INVALID 非 WAV 文件` | `MasterChapter` 有 ffmpeg 时返回 **m4a**，下一行却用只认 RIFF 的 `Wav.DurationOf` 读它 | 新增 `AudioMaster.ProbeDuration`（WAV 读头 / 其余走 ffprobe / 兜底按码率估算） |
| 3 | 整本 M4B 阶段卡死 3 分钟以上 | `ffmpeg` 的 stdout/stderr 被重定向却从不读取，进度输出写满 4KB 管道即死锁 | `Run()` 并发 `ReadToEndAsync` 两个流 + `-hide_banner -loglevel error -nostats` + 超时 kill |
| 4 | 进程被强杀后任务永久停在 `running`，UI 永远转圈 | 无执行权 Lease/心跳，无人回收孤儿任务 | `worker.lock` 存 PID + 启动时校验存活 + `FailIfStillRunning`（CAS）标记 `WORKER_LOST`，可 `job resume` |
| 5 | 恢复后出现 `chapters_done=6 / total=3`、`chars_done > chars_total`；完成态仍挂 `WORKER_LOST` | resume 时在上次计数上继续累加；完成时未清空错误码 | 以章节 checkpoint 重算 `ChaptersDone/CharsDone/AudioSeconds`；完成时清空 `ErrorCode/ErrorMessage` |
| 6 | 中控台写着"正在调用显卡进行 TTS 合成"，实际在跑 CPU | `segment_done` 的 `phase` 是硬编码字符串 | 按 `DevicePlan.Device` 分别输出 GPU / GPU+CPU 层卸载 / CPU（N 线程） |
| 7 | CLI 事件流 JSONL 中文在 UTF-8 终端乱码、在 PowerShell 里又反过来 | 只有 `Emit()` 才切 `Console.OutputEncoding`，先输出的事件流已按 OEM(936) 编码 | `Main()` 开头统一 stdout/stderr 为无 BOM UTF-8（含 `Console.SetError` 重建，保证进度行也一致） |
| 8 | 便携版 CLI/MCP/Worker 数据仍写 `%LOCALAPPDATA%`，U 盘带走会丢 | 三个工具发布在 `tools\<proj>\`，而 `portable.marker` 只在程序根目录 | 从 `BaseDirectory` 逐级向上找 marker（≤6 层），以 marker 所在目录作为便携根 |
| 9 | 换输出目录或改 `BOOKCTL_WORKSPACE` 后已装模型"消失"，简洁模式退回占位音 | `SourcesRoot` 由 `WorkspaceRoot` 的父目录推导，工作区一动源目录跟着漂移 | `SourcesRoot` 固定 `%LOCALAPPDATA%\bookctl\sources`（便携版由 marker 覆盖为程序目录 `sources/`） |
| 10 | GUI 选 sherpa，`manifest` 记 `engine=sherpa`，实际出声仍是 mock 占位音（0.4s/段） | `Pipeline` 复用构造时固化的 `_tts`，却拿运行时可变的 `_cfg.EngineMode` 做比较，改引擎后两者恒等 | 记录 `_ttsEngine`（provider 创建时的引擎名）再比较；不一致时按 `job.Engine` 新建 provider |
| 11 | `build-portable.ps1` 直接语法崩溃 `意外的标记":"` | 脚本 UTF-8 无 BOM，PowerShell 5.1 按 GBK 解码，中文把 here-string 打乱 | 三个打包脚本统一加 UTF-8 BOM；`edition.json` 显式写无 BOM UTF-8；engine 拷贝跳过 `.venv` |
| 12 | 产物 `manifest.json` 的 `status` 永远是 `running`，DB 里其实已 `completed` | manifest 在 job 终态落定**之前**写出 | 先更新 job 终态并存库，再写 `manifest.json` 与 `qc/report.json` |

**2026-11-24 晚（v0.4）追加 9 条**，完整条目见 `docs/defects.md` #13–#21。最重要的一条是 **#20**：
`dotnet publish` 没有复制 WinUI 的 `*.xbf` 与 `BookCtl.App.pri`，导致免安装包双击即崩（`0xC000027B`）。
**此前交付的便携包一直是坏的**（#21，上一轮只验证了路径解析与 CLI，没真正双击 GUI）；本轮已修并逐个实测启动：
`RUNNING / title=[本地朗读工厂] / responding=True`。
其余为结构/交互/分发问题：找不到 exe（#13）、无拖放（#14）、导入逻辑分散（#15）、
免安装包 473 MB 过重（#16）、脚本裸调 `dotnet`（#17）、`.cmd` 编码（#18）、`-Slim` 撞 safe-delete 阈值（#19）。

**2026-11-24 深夜（v0.5 Mini）追加 5 条**，完整条目见 `docs/defects.md` #22–#26：
Rust 工具链/API 拼写（#22）、GUI 日志换行与书名/进度显示（#23）、**委托 Full 返回全假数据**（#24）、
中文日志乱码的定性（#25，宿主解码问题而非程序缺陷）、`Uninstall-*.exe` 双击卸载（#26）。
其中 #24 最关键：`delegate_full()` 原先硬编码 `sherpa-via-Full`、统计恒为 0、把整行 NDJSON 灌进日志；
现已按 Full 的真实 manifest 回填 `chapters/seconds/engine.name`，并按真实引擎生成标签。

附带修复：幂等命中 `failed/cancelled` 旧任务时 `run` 现在会**自动 resume**（原实现会立刻返回旧状态）；
空/纯空白分段不再送 TTS（原会白跑一次推理）；`concat_list.txt` 内路径统一为正斜杠（ffmpeg concat 里 `\` 是转义符）；
ffmpeg 解析增加 WinGet 包目录兜底扫描（刚装完 ffmpeg、PATH 未刷新时也能找到）。

工程性补充两条：
- `BookCtl.App` 新增 `--auto-start`（命令行/文件关联传入 `.epub` 即开始生成）与 `--on-top`（演示、录屏时窗口置顶）。
- 改了 `BookCtl.Core` 后若桌面程序行为"没变"，先单独重建 App 项目：`dotnet build src/BookCtl.App/BookCtl.App.csproj -c Debug`。
  `dotnet build bookctl.sln` 存在构建顺序 + 时间戳增量复制问题，`BookCtl.App\bin\...\win-x64\BookCtl.Core.dll` 可能仍是上一版（本轮的引擎切换 bug 就是被这个现象掩盖了一轮）。

## 7.2 设备选择：CPU / GPU / 混合（v0.3）

```powershell
bookctl device list                                    # 显卡序号、显存、CPU 核数 + 当前决策
bookctl run --input x.epub --engine sherpa --device cpu
bookctl run --input x.epub --engine sidecar --device gpu --gpu 0
bookctl run --input x.epub --engine sidecar --device hybrid    # GPU+CPU 层卸载
bookctl run --input x.epub --cpu-threads 8
```

决策逻辑（`src/BookCtl.Core/Device.cs`）：

| 模式 | 行为 |
|---|---|
| `auto`（默认） | 显存 ≥ 引擎需求 → 纯 GPU；介于 1/3 ~ 需求之间 → GPU+CPU 卸载；更小 → 纯 CPU |
| `gpu` | 强制指定卡；显存不足会**明确警告**（不静默降级） |
| `cpu` | 纯 CPU，线程数可指定 |
| `hybrid` | 把部分层卸载到 CPU：能跑，但**比纯 GPU 慢**；显存够用时反而更慢 |

各引擎显存门槛：`cosyvoice3` 6144MB、`sherpa` 1536MB、`piper` 512MB、`mock` 0。
实测本机 GTX 1050 2GB 跑 `cosyvoice3` 会自动判为 `hybrid` 并提示换卡或换 CPU 引擎。

## 7.3 引擎与模型源管理（v0.3）

```powershell
bookctl source list                     # 内置 / 可下载 / 已安装 / 手动放置
bookctl source pull sherpa-vits-zh-fanchen   # 一键下载 130MB 中文模型并解包
bookctl source remove sherpa-vits-zh-fanchen
```

- 内置目录写进二进制（`Sources.BuiltInCatalog()`），封装漏拷配置文件也不会缺源；
- 用户可在 `sources/catalog.user.json` 追加/覆盖源（企业内网镜像就填这里）；
- 下载支持**多线路回退**：原始 URL → 用户 mirrors → `BOOKCTL_MIRROR` → `gh-proxy.com`（GitHub）/ `hf-mirror.com`（HuggingFace）。
  实测国内直连 GitHub Release 会 502，没有回退就等于功能不可用；
- 解包用系统 `tar.exe`（支持 .tar.bz2/.tar.gz）或 `ZipFile`（.zip），完成后写 `.installed.json` 标记。

## 7.4 引擎矩阵：到底能不能"1050 显存 + CPU 一起推理"？

**结论：没有能提速的"拆分式 TTS 推理"。** 逐条说明：

| 方案 | 是否可行 | 说明 |
|---|---|---|
| 张量并行（多卡切模型） | ❌ | 需要 ≥2 张同型号卡，且 vLLM/SGLang 要求算力 ≥7.0；GTX 1050 是 6.1，直接被拒 |
| 流水线并行（层切分到 CPU+GPU） | ✅ 但更慢 | 即我们的 `hybrid`：`accelerate` 的 `device_map` 卸载。目的是"塞得下"，不是"跑得快"——CPU 内存带宽是瓶颈，通常比纯 GPU 慢 2–5 倍 |
| ONNX Runtime 跨设备切分 | ❌ | ORT 一个 session 只跑一个 EP；只能设 `cuda_mem_limit` 做部分卸载，同一 session 内不并行 |
| 多进程分片（一章一进程，不同设备） | ⚠️ 收益极小 | 单句 TTS 是串行依赖（每段要连贯），拆开只增加显存争用；跨设备混跑还会破坏音色一致性 |
| 云边协同 / 分布式集群 | ❌（本项目定位） | 与"全程本地、离线可用"的产品前提冲突 |

所以产品给用户的是**三档明确选择**：纯 GPU（最快，需 ≥6GB）/ 纯 CPU（最省，sherpa 可实时）/ GPU+CPU 卸载（显存不够时的兜底）。
总架构 §A.8 记了完整调研来源。

## 8. 目录结构

```
bookctl.sln · Directory.Build.props · README.md · HANDOVER.md · AGENTS.md
启动.cmd · build.cmd                     # 唯一直达入口 / 一键构建+回归（见 §2）
architecture-epub2audiobook.md / .html  # 总架构（唯一事实源 + 可视化评审版）
config/bookctl.defaults.json            # 默认配置
.vscode/                                # F5 调试与构建任务
docs/                                   # 交接文档：workflows / decisions / defects / roadmap / conversation / screenshots
src/BookCtl.Core/                       # 契约/模型/SQLite/EPUB/引擎/流水线/共用 Handler
src/BookCtl.Cli/ src/BookCtl.Mcp/ src/BookCtl.Worker/    # 三个命令行入口（共享 Core）
src/BookCtl.App/                        # 唯一 GUI：WinUI 3（App.xaml / MainWindow.xaml / MainViewModel.cs）
engine/tts_server.py + README.md + requirements.txt      # Python TTS sidecar（mock / sherpa / cosyvoice3）
packaging/                              # build-portable.ps1 / make-release.cmd / bookctl.iss / sign.ps1 / README.md
```

仓库顶层只有「源码 / 脚本 / 配置 / 文档」四类东西，其余全被 `.gitignore` 排除：

| 层 | 位置 | 入库 |
|---|---|---|
| 源码 | `src/`、`engine/*.py`、`config/` | ✅ |
| 构建/运行产物 | `src/**/bin`、`src/**/obj`、`engine/.venv`、`testdata/ws-*`、`*.wav/m4a/m4b`、`*.db*` | ❌ |
| 分发包 | `dist/`（标准版 + `-slim` 轻量版）、`*.zip` | ❌ |
| 模型权重 | `sources/`、`*.onnx`、`*.tar.bz2` | ❌ |

> 口径提醒：**仓库很小（约 1.5 MB / 72 个文件）**，大的是工作目录（约 1.7 GB，全是 bin/obj/dist/.venv/音频）。
> 觉得「怎么好几个 G」时先 `git ls-files | wc -l`，不要以为仓库臃肿。

## 9. 封装分发（Pro 版：便携版 + 安装版）

**一条命令出便携版**（安装包需本机先装 Inno Setup 6；未装则只产便携版）：

```powershell
.\packaging\make-release.cmd            # 标准便携版（完全自包含）
.\packaging\make-release.cmd -Slim      # 轻量便携版（CLI 工具需 .NET 10，GUI 仍零依赖）
```

等价的手工步骤：

```powershell
powershell -ExecutionPolicy Bypass -File packaging\build-portable.ps1 -Version 0.3.0 [-Slim]
powershell -ExecutionPolicy Bypass -File packaging\sign.ps1 -Path dist\本地朗读工厂-0.3.0-win-x64
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" packaging\bookctl.iss /DMyVersion=0.3.0
```

| 形态 | 目录 / 压缩包 | 体积（实测 2026-11-24） | 目标机要求 |
|---|---|---|---|
| 标准便携版 | `dist\本地朗读工厂-0.3.0-win-x64\` · `.zip` | **475 MB / 198 MB** | 无（GUI + CLI 全自包含） |
| Slim 轻量版 | `dist\本地朗读工厂-0.3.0-win-x64-slim\` · `.zip` | **200 MB / 78 MB** | GUI 无；CLI/MCP/Worker 需 .NET 10 运行时 |

Slim 省在哪：三个命令行工具改 framework-dependent（省掉 3 份 .NET 运行时 ≈236 MB）+ 删掉本项目不加载的
`onnxruntime.dll` / `DirectML.dll`（41 MB）。多语言 MUI 仅 3.4 MB，评估后不裁（`docs/defects.md` #19）。

- 便携版：程序目录内有 `portable.marker` → 数据写 `data\`、模型写 `sources\`，整目录可拷到 U 盘；
- 安装版：`PrivilegesRequired=lowest` 的 per-user 安装，删除 marker，数据走 `%LOCALAPPDATA%\bookctl`；
- 签名：支持 PFX / 证书指纹（EV USB Key）/ Azure Trusted Signing 三种，全部带 RFC3161 时间戳；
- **发布前必做**：真机双击 `BookCtl.App.exe` 确认能出窗口再发。缺陷 #20 就是"只验证了 CLI、没双击 GUI"，
  导致此前所有免安装包都是坏的（`0xC000027B`），切勿只看 zip 生成成功；
- 细节与发布检查清单见 [`packaging/README.md`](./packaging/README.md)。

---

## 10. Mini 轻量版与「拷到另一台电脑测试」

v0.6 起仓库有**两种源码线、三个安装包**：

| | 重量级主线（Full） | Mini 轻量入口（批量） | 阅读器（边看边读） |
|---|---|---|---|
| 技术 | .NET 10 / WinUI 3 / Python sidecar | Rust / Win32 原生，零外部依赖 | 同一份 Rust 代码，`reader` 模式 |
| 安装包 | `本地朗读工厂-Full-0.3.0-setup.exe` **197.4 MB** | `本地朗读工厂-Mini-0.6.0-setup.exe` **0.7 MB** | `本地朗读工厂-阅读器-0.6.0-setup.exe` **0.7 MB** |
| 形态 | 整本生成有声书 | 整本生成有声书 | 显示正文 + 段落朗读 + 后台预生成 |
| 模型 | 内置源管理 + sherpa/piper/GPU 引擎 | 本体零模型；按需下载到任意盘 |
| 语音 | 真人级音质（sherpa-onnx 等） | 系统 SAPI 兜底；装了 Full 就委托真 TTS 模型 |
| CLI/MCP | 完整 job 化控制面 | 同一套 JSON envelope / MCP stdio 契约 |

### 10.1 三个安装包在哪

**给别人用 / 换一台电脑**：去 GitHub Releases
`https://github.com/tartan01/epubtts-miniature-parakeet/releases` 直接下载 setup.exe，
双击即可，不需要本仓库、不需要源码、不需要 .NET / Python / 管理员。
Release 说明与 SHA-256 见 [`packaging/release-notes-v0.6.0.md`](./packaging/release-notes-v0.6.0.md)。

> Release 上的资产名是 `BookCtl-*.exe`（GitHub 会过滤资产名里的非 ASCII 字符，
> 实测中文名会被剥成 `-Mini-0.6.0-setup.exe`）；与本地 `dist\release\` 下的中文名产物
> **内容完全一致**（SHA-256 相同），下载后直接双击即可、无需改名，页面另有中文 label 说明。

**本地构建出来的产物**（发版前才生成）：

```
dist\release\本地朗读工厂-Mini-0.6.0-setup.exe       ← 批量模式（0.7 MB）
dist\release\本地朗读工厂-阅读器-0.6.0-setup.exe     ← 阅读器，边看边读（0.7 MB）
dist\release\本地朗读工厂-Full-0.3.0-setup.exe       ← 重量级版本（197.4 MB）
dist\release\本地朗读工厂-Mini-0.6.0-portable.zip    ← 免安装便携版（批量）
dist\release\本地朗读工厂-阅读器-0.6.0-portable.zip  ← 免安装便携版（阅读器）
```

> `dist/release/` 被 `.gitignore` 排除，**不会进仓库**；要从源码重建见 §10.3。
> 拷到另一台电脑时只拷需要的 `.exe`，不需要整个仓库。三个版本可**同时安装**互不覆盖。
> 注意：先装 Full，再装阅读器/Mini，这样阅读器能立即检测到 Full 并用「整本导出」出模型音质。
>
> **重新发布**：`$env:GITHUB_TOKEN = '<PAT>'` 后跑 `.\packaging\publish-release.ps1`
> （先 `-DryRun` 核对清单）。脚本会自动查/建 Release、同名资产先删再传、支持重复运行。
> SSH 密钥只能 `git push`；创建 Release / 上传 asset 走 REST API，必须有 PAT。
> 完整流程见 [`docs/workflows/06-packaging.md`](./docs/workflows/06-packaging.md) §10。

### 10.2 另一台电脑验收步骤（干净 Windows 10/11，免管理员）

1. 拷 `本地朗读工厂-阅读器-0.6.0-setup.exe` 和/或 `本地朗读工厂-Mini-0.6.0-setup.exe` 过去，**双击**（或 `setup.exe /S` 静默）。
   安装到 `%LOCALAPPDATA%\本地朗读工厂\Mini`，写 HKCU 注册表、开始菜单/桌面快捷方式、`.epub` 关联。
   **不需要 .NET、不需要 Python、不需要 VC++ 运行库、不需要管理员权限**。
2. 双击桌面「本地朗读工厂 Mini」，把任意 `.epub` 从桌面**直接拖进窗口**。
3. 「运行配置」区选设备（自动 / CPU / GPU / 混合）与显卡序号，或「导入配置」读 `config.json`。
4. 「模型下载管理」区选盘符（C/D/E…），点「自动测速选源」让软件自己挑最快线路，
   再点「安装 / 下载模型」；断网或某个源失败会自动切下一个源并显示进度。
5. 点「打开模型目录」直接定位模型文件夹；点「一键生成朗读」，完成后「打开输出文件夹」。
   > 干净机器只有系统 SAPI 语音；想要真人级音质，再装 `本地朗读工厂-Full-0.3.0-setup.exe`，
   > Mini 会自动检测到并通过 CLI 契约委托 Full 的 TTS 引擎（无需手工配置路径）。
6. 卸载：开始菜单 → 「程序和功能」→ 本地朗读工厂 Mini，或双击安装目录下的
   `Uninstall-Mini.exe`。**会保留已下载的模型与已生成的有声书**。

命令行 / 大模型接入（同一套 JSON envelope）：

```powershell
BookCtl.Mini.exe capabilities        # 能力清单
BookCtl.Mini.exe device list         # 运行环境 / 盘符 / 剩余空间
BookCtl.Mini.exe source list         # 8 个内置源 + 4 个模型
BookCtl.Mini.exe source probe sherpa-vits-zh-fanchen   # 自动测速
BookCtl.Mini.exe source pull sherpa-vits-zh-fanchen --root D:\本地朗读工厂模型
BookCtl.Mini.exe run --input 书.epub --device auto --pretty
BookCtl.Mini.exe mcp                # MCP stdio（9 个 tools）
BookCtl.Mini.exe doctor             # 环境自检

# 阅读器（装的是 BookCtl.Reader.exe 时，双击即进阅读器）
BookCtl.Reader.exe reader --input 书.epub --dry-run          # 只解析，输出章节/预取计划
BookCtl.Reader.exe reader --input 书.epub --headless --count 5   # 无窗口实测预生成
BookCtl.Reader.exe reader status --input 书.epub             # 缓存命中情况
BookCtl.Reader.exe reader --input 书.epub --play-test        # 验证播放链路
```

> **PowerShell 5.1 看中文日志乱码**：`chcp 65001` 或
> `[Console]::OutputEncoding = [Text.Encoding]::UTF8` 即可。字节本身是干净 UTF-8，
> GUI 与 stdout 的 JSON 信封不受影响。

### 10.3 从源码重建（换机开发）

```powershell
.\packaging\install-toolchain.cmd     # 一键装 Rust GNU 工具链（rsproxy 镜像）
.\mini\build.ps1                      # 编 BookCtl.Mini.exe + setup-stub.exe
.\packaging\make-installer.ps1        # 产出三个 setup.exe（Full 复用既有便携包 zip）
```

本机没有 MSVC / Windows SDK，用 `rust-lld` 自包含链接，产物不依赖 VC++ Redistributable。
`mini\_probe\probe.rs` 是换机首次构建时验证 Win32 可链接的最小探针。

### 10.4 目录与唯一事实源

- `mini/` 七个源码模块：`main / win32 / ui / model / audio / reader / util`，另 `mini/setup/main.rs` 是自解压安装器。
- 总架构唯一事实源：`architecture-epub2audiobook.md` **附录 C**（v0.5 Mini）+ **附录 D**（v0.6 阅读器）。
- 交接细节见 [`HANDOVER.md`](./HANDOVER.md) §3 代码地图、`docs/workflows/06-packaging.md`、
  `docs/defects.md` #22–#30。

---

## 11. 阅读器：边看 EPUB 文字，边听朗读（v0.6）

### 11.1 它解决什么问题

批量模式是"点一下、等它生成整本、再听"。但很多时候你想**坐下来读**：
眼睛看得比机器读得快，等整本合成完就失去阅读节奏了。阅读器把这两件事并行起来：

- 中间显示 EPUB 正文，**当前朗读的段落高亮**，朗读推进时自动跟随滚动；
- 后台只合成"当前位置往后 6 段 + 下一章前 2 段"，人看一段要几秒、机器合成一段只要 0.6 秒，
  所以**语音永远跑在阅读进度前面**，不会卡；
- 想看快一点就往下翻，语音立刻从新位置续上；想停下就「暂停」。

### 11.2 界面与操作

左：章节列表；中：正文（当前段蓝色高亮）；右：段落导航（`·` 已备好 / `⋯` 生成中 / `○` 未生成 / `!` 失败）。

| 控件 | 作用 |
|---|---|
| 打开电子书… / 拖入 `.epub` | 换书；也会记住上次打开的书，下次双击直接续 |
| ⏮ 上一段 / ⏭ 下一段 | 逐段跳转，朗读中跳会立即从新位置继续 |
| ▶ 开始朗读 / ⏸ 暂停朗读 | 播放/暂停当前段落 |
| 语速 − / + | 0.5×–2.0×，换语速用独立缓存不混用 |
| 重合成本段 | 删掉该段 WAV 重新合成（生成失败时用） |
| 跟随：开/关 | 关掉后正文不随朗读滚动，方便自己往后翻着看 |
| 整本导出… | 走批量流水线，装了 Full 就能用模型音质导出整本 |
| 预取 2/4/6/10/16 段 | 后台提前生成多少段 |

### 11.3 数据与边界

- 朗读缓存：`%LOCALAPPDATA%\本地朗读工厂\ReaderCache\<hash>\`，按「书 + 语速」分目录，
  **卸载不删除**；上次打开的书记在同目录 `last.txt`。
- 同步粒度是**段落**，不做字级逐字高亮（取舍见 `docs/decisions/ADR.md` ADR-028）。
- 阅读器用**系统语音即时朗读**（SAPI，单段约 0.6 秒）。想要模型音质的整本成品，点「整本导出…」
  或直接用 Mini/Full 的批量模式。
- 没有音频设备的环境里 `--play-test` 会返回 `playback_failed`，属正常；GUI 里同样听不到声音但不影响预生成。

