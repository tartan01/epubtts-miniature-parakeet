# ADR · 技术决策台账

> 每条决策记录：**选了什么 / 为什么 / 否掉了什么 / 影响哪些文件**。
> 新增决策请追加新编号，不要覆写历史。总架构唯一事实源仍是 `architecture-epub2audiobook.md`。

---

## ADR-001 · CLI-first + MCP + 大模型友好的控制面

- **决策**：GUI / CLI / MCP / Worker 四入口共用 `BookCtl.Core.Handlers.BookCtlHandlers`；
  所有输出为 JSON envelope `{"ok":true,"api_version":"1.0","data":...}` + 稳定退出码；
  提供 `--capabilities` / `--schema` / `--dry-run` / `--pretty` / `--quiet`。
- **理由**：用户要求"由代码驱动、符合 Cmd、大模型友好、甚至 MCP 控制"。
  调研发现现有开源转换器（audiblez / ebook2audiobook / epub2tts）都是"给人用的"，
  没有 Job 化 / 断点续传契约 / 机器可读控制面。
- **影响**：`Contracts.cs`、`Cli/Program.cs`、`Mcp/Program.cs`、`Handlers.cs`。

## ADR-002 · 长任务全部 job 化

- **决策**：`job_start / job_status / job_logs / job_cancel / job_resume / job_artifacts`；
  MCP tool SLA ≤2 s，**整本书绝不在单次 tool 调用里同步执行**。
  唯一例外 `source_pull` 同步等待（要让模型拿到明确安装结果）。
- **理由**：MCP 客户端有超时；整本书可能几十分钟。
- **影响**：`JobStore.cs`、`Pipeline.cs`、`Handlers.cs`、`Mcp/Program.cs`。

## ADR-003 · SQLite WAL + 章节级 checkpoint + 幂等

- **决策**：`jobs` / `chapters` / `events` 三表；CAS + `revision` 并发控制；
  同一 `(input sha, profile_hash, config_hash)` 确定性产物；事件 NDJSON 追加写；
  `worker.lock` 存 PID + 启动时孤儿任务回收（`WORKER_LOST` 可 resume）。
- **理由**：桌面程序可能被强杀；长任务必须能续跑；GUI 需要实时事件流。
- **影响**：`JobStore.cs`、`Pipeline.cs`（`RecoverOrphans` / `FailIfStillRunning`）。

## ADR-004 · 六阶段流水线 S1–S6

- **决策**：`INGEST → SPLIT → NORMALIZE → SYNTH → VERIFY → MASTER`；TOC anchor 切章；
  中文 TN / 分句；跳过版权页、索引、空白段；ffmpeg 生成 `book.m4b` + `ffmetadata` 章节标记。
- **理由**："整本图书按章节分类整理好"需要明确的产物契约与可检验的项目文件夹。
- **影响**：`Epub.cs`、`Engine.cs`（`Wav.Concat` / `AudioMaster`）、`Pipeline.cs`。

## ADR-005 · 不用 EbookLib（AGPL）

- **决策**：自写 zip + XML 解析（`container.xml` → OPF → manifest/spine → NCX/nav.xhtml）。
- **理由**：EbookLib 是 AGPL，会污染整个项目许可。
- **影响**：`Epub.cs`。

## ADR-006 · 默认真引擎选 sherpa-onnx CPU（而非 GPU）

- **决策**：`vits-zh-hf-fanchen-C`（187 音色 / 16 kHz / 约 132 MB / Apache 友好）作为默认真语音；
  `mock` 作为零依赖校样引擎；`cosyvoice3` 保留 GPU 适配点。
- **理由**：本机是 **GTX 1050 / 2GB**，而 CosyVoice 3 需 ≥6GB 显存。
  实测 sherpa 整本 97.9 s 音频、合成 34.2 s（RTF≈0.35），完全可用。
- **否决**：VibeVoice（强制可听 AI 声明 + 水印）、XTTS/Coqui（CPML 非商用 + 主体注销）。
- **影响**：`Engine.cs`、`engine/tts_server.py`、`Sources.cs`、`Device.cs`。

## ADR-007 · WinUI 3 unpackaged，不是 WPF / MSIX

- **决策**：`net10.0-windows10.0.19041.0` / `win-x64` / `UseWinUI=true` /
  `WindowsPackageType=None` / `WindowsAppSDKSelfContained=true` / Windows App SDK **2.7.0**。
- **理由**：用户要"纯正 Windows 原生桌面软件"，且 2026 年 `dotnet new` WinUI 模板 + WinApp CLI
  已支持免 Visual Studio 工具链。self-contained 让目标机无需装 Runtime。
- **坑**：WinUI 的 `Window` 没有 `DataContext`（不是 WPF）→ 绑定挂在根 `Grid x:Name="Root"`；
  `Grid` 不能设 `FontFamily` → 在 `App.xaml` 用 `ContentControlThemeFontFamily` 全局设置。
- **影响**：`BookCtl.App.csproj`、`App.xaml`、`MainWindow.xaml*`、`.vscode/`。

## ADR-008 · 设备选择：`auto|cpu|gpu|hybrid`，选择权交给用户

- **决策**：`DevicePlanner.Resolve()` 按引擎显存门槛（cosyvoice3 6144 / sherpa 1536 / piper 512 MB）
  与 `nvidia-smi` 枚举结果决策；`GpuIndex` 支持 0/1/2；`CpuThreads` 可调。
  **设备不参与 `profile_hash`**。
- **理由**：用户明确要求可切换显卡、且"选择权完全交给用户"。
- **影响**：`Device.cs`、`Contracts.cs`、`Models.cs`、`Pipeline.cs`、`MainViewModel.cs`。

## ADR-009 · 不存在可提速的「GPU + CPU 拆分式 TTS 推理」

- **决策**：不做分布式张量并行，只提供纯 GPU / 纯 CPU / hybrid 兜底，并在 UI 标注 hybrid 更慢。
- **理由**：张量并行需 ≥2 张同型号且算力 ≥7.0 的卡（GTX 1050 算力 6.1 被 vLLM/SGLang 拒绝）；
  `device_map` 层卸载只解决"装不下"且慢 2–5 倍；ORT 单 session 单 EP；
  多进程异构分片会破坏整本书音色一致性。
- **影响**：架构 MD §B.5、README §7.4、`docs/workflows/04-device-and-sources.md`。

## ADR-010 · 「DDS 源管理」= 内置引擎/模型源目录 + 一键下载

- **决策**：`SourceManager.BuiltInCatalog()` 5 个源；`archive/zip/file/manual` 四类型；
  `source list|pull|remove`；`.installed.json` 标记；用户可用 `sources/catalog.user.json` 覆盖。
- **理由**：用户既要"内置可封装进本地"，又要"一键下载入口"。
- **关键约束**：国内直连 GitHub Release **502** → 多线路回退
  （原始 → 用户 mirrors → `BOOKCTL_MIRROR` → `gh-proxy.com` / `hf-mirror.com`）。
- **影响**：`Sources.cs`、`Handlers.cs`、`Cli/Program.cs`、`Mcp/Program.cs`、`config/sources.example.json`。

## ADR-011 · 便携版 / 安装版双形态 + 签名

- **决策**：`portable.marker` 存在 → 数据写程序目录 `data/` + `sources/`，`Edition=pro`；
  Inno Setup per-user 安装时删除 marker → 数据写 `%LOCALAPPDATA%\bookctl`；
  `.epub` 文件关联；`sign.ps1` 支持 PFX / 指纹 / Azure Trusted Signing + RFC3161 时间戳。
- **理由**：用户要求"Pro 版可移动版 + exe 安装版两个版本的纯正 Windows 软件"。
- **影响**：`Contracts.ApplyPortable`、`packaging/*`、`edition.json`。

## ADR-012 · Apple 风格双模式 UI

- **决策**：产品名「本地朗读工厂」（备选「帅帅朗读工厂」）；
  顶部切「简洁模式 / 详细参数」；简洁模式 = 选 EPUB + 一颗蓝色胶囊按钮 + 进度卡 + 结果卡（输出文件夹 + 逐章卡片）；
  详细模式 = 参数卡 + 状态卡 + 中控台。
  规范：`#F5F5F7` 画布 / 白卡 24px 圆角 / `#0071E3` 唯一强调色 / 胶囊按钮 / `Segoe UI Variable` / 500ms 缓动。
- **理由**：用户给了完整 Apple 风格设计提示词，并要求生成后图形化告知"生成到哪个文件夹、哪几章"。
- **关键**：简洁模式**不是另一套流水线**，只是写会话配置后调用同一个 `JobStart`。
- **影响**：`App.xaml`、`MainWindow.xaml*`、`MainViewModel.cs`。

## ADR-013 · 一键模式的兜底优先级

- **决策**：已装 CPU 模型 + sidecar 可达 → `sherpa`；否则退回 `mock` 并在界面说明原因，**不硬报错**。
- **理由**："用户第一次双击就能跑通"比"严格使用真引擎"更重要。
- **影响**：`MainViewModel.StartOneClick()`。

## ADR-014 · 输出编码在 `Main()` 开头统一为无 BOM UTF-8

- **决策**：`Console.OutputEncoding` / `InputEncoding` / `Console.SetError(...)` 在入口一次性设置。
- **理由**：早期只在 `Emit()` 里切 UTF-8，导致先输出的事件流 JSONL 用 OEM(936) 编码，
  中文书名/章节标题在不同终端下互相乱码。
- **影响**：`Cli/Program.cs`。

## ADR-015 · `SourcesRoot` 与 `WorkspaceRoot` 解耦

- **决策**：源根固定 `%LOCALAPPDATA%\bookctl\sources`；便携版由 marker 覆盖。
- **理由**：旧实现从工作区父目录推导，一改 `BOOKCTL_WORKSPACE` 已装模型就"消失"，
  简洁模式静默退回占位音。
- **影响**：`Contracts.cs`、`docs/defects.md` #9。

## ADR-016 · `Pipeline` 记录 provider 创建时的引擎名

- **决策**：用 `_ttsEngine`（provider 创建时固化的引擎名）与 `job.Engine` 比较，
  不一致时按 `job.Engine` 新建 provider；不再拿运行时可变的 `_cfg.EngineMode` 比较。
- **理由**：GUI 改引擎后两者恒等，复用了旧的 `mock` provider，
  出现"manifest 写 sherpa、音频却是 0.4 s 占位音"的隐蔽 bug（缺陷 #10）。
- **影响**：`Pipeline.cs`。

## ADR-017 · 产物清单在 job 终态落定之后再写

- **决策**：先更新 job 状态并 `UpdateJob`，再写 `manifest.json` / `qc/report.json`。
- **理由**：早期 manifest 的 `status` 永远是 `running`，与 DB 不一致，用户"检验产物"会被误导。
- **影响**：`Pipeline.cs`、缺陷 #12。

## ADR-018 · 打包脚本 UTF-8 BOM

- **决策**：`packaging/*.ps1` 与 `*.iss` 必须带 UTF-8 BOM；`edition.json` 必须无 BOM。
- **理由**：Windows PowerShell 5.1 按 GBK 解析无 BOM 的中文脚本会破坏 here-string 语法；
  JSON 带 BOM 又会让部分解析器报错。
- **影响**：`packaging/*`、缺陷 #11。

---

## ADR-019 · 拖放导入与统一 ImportEpub 入口

- **决策**：WinUI 根容器开启 `AllowDrop`；所有导入路径（拖放 / 文件选择器 / 命令行 / `.epub` 文件关联）
  统一调用 `MainViewModel.ImportEpub(path)`。
- **理由**：拖放是 Windows 用户对桌面软件的默认预期，选择器多两步；导入校验（扩展名、存在性、绝对路径）
  分散在多处必然出现"命令行能进、拖放不能进"一类不一致。收敛成单一入口后，可测性也更好
  （命令行 `--auto-start` 与拖放走同一段代码）。
- **影响**：`MainWindow.xaml`（根 Grid + 拖放提示层）、`MainWindow.xaml.cs`、`MainViewModel.cs`。
- **反面意见**：拖入后不自动开始生成——避免误拖一个文件就消耗算力；用户明确点击「生成」再执行。

## ADR-020 · 双形态分发与 Slim 发布策略

- **决策**：`build-portable.ps1` 保留默认（完全自包含）形态，新增 `-Slim`：
  ① GUI 仍 self-contained（双击即用、免装运行时）；
  ② CLI/MCP/Worker 改 framework-dependent（需 .NET 10 运行时）；
  ③ 裁掉 `onnxruntime.dll` / `DirectML.dll`（本项目 TTS 走 Python sidecar，主程序不加载）。
- **理由**：默认形态要满足"客户机什么都不装"；但把同一份运行时复制三遍纯属浪费（239 MB）。
  命令行工具面向高级用户/AI 宿主，机器上大概率已有 .NET；用 `tools\PREREQUISITE.txt` 明确告知。
- **放弃项**：多语言 MUI 实测仅 3.4 MB（172 个），裁剪收益远小于宿主 safe-delete 批量删除的代价，
  且需绕过安全策略 —— 明确不做。以后要裁应在 publish 前控制语言白名单。
- **代价**：Slim 版的 CLI 在无 .NET 10 的干净机器上不可用；GUI 不受影响。
- **影响**：`packaging/build-portable.ps1`、`packaging/make-release.cmd`、README §9、缺陷 #16/#19。

---

## ADR-021 · 新增独立 `mini/` 产品线（用户拍板 A 路线）

- **决策**：不推倒重写 .NET 主线，新增 Rust / Win32 原生 `mini/` 作为第二条产品线；
  零外部 crate、零 .NET、零 Python，单 exe 目标 5–10 MB（实测 **603 KB**）。
- **理由**：用户要求「不带模型的轻量化版本、用最高效语言重写、Windows 支持高效简洁语言」；
  同时要保留重量级版本。两条线并存、共用 CLI/MCP 契约语义，风险最低。
- **工具链**：本机无 MSVC / Windows SDK / VC++ Redistributable；装 BuildTools 需提权且数 GB。
  改用 `stable-x86_64-pc-windows-gnu` + `rust-lld` + `link-self-contained=yes`
  （`mini/.cargo/config.toml`），产物拷到另一台干净 Windows 直接跑。
- **否掉**：B（Rust 整体替换主线，要重做 WinUI）、C（仅 .NET framework-dependent + SAPI，仍带运行时）。
- **影响**：`mini/`、`packaging/install-toolchain.cmd`、架构 MD 附录 C、README §10。

## ADR-022 · Mini 的语音路径：系统 SAPI 兜底 + 委托 Full 真 TTS

- **决策**：Mini 自身**不加载任意模型权重**。检测到 Full（注册表 `CliPath` 或邻近 `tools\`）
  就通过 CLI 契约 `bookctl.exe run` 委托真 TTS 模型；未检测到 Full 用 Windows 自带
  `System.Speech.Synthesis.SpeechSynthesizer`（经 PowerShell 脚本）逐段合成 WAV 并拼接分章有声书；
  两者都不可用则生成明确标注的占位音。
- **理由**：Mini 的价值是「零依赖轻量入口」，塞推理引擎就失去意义；
  而用户要求的「自动调用 TTS 模型」通过委托 Full 实现，无需 Mini 自己带权重。
- **代价 / 已知边界**：模型已下载但 Full 未安装时仍走 SAPI，不能直接调权重（见 roadmap R-14）。
- **影响**：`mini/src/audio.rs` 的 `generate_book()` / `delegate_full()` / `synth_sapi()`。

## ADR-023 · 程序与模型分离、选盘安装

- **决策**：程序装 `%LOCALAPPDATA%\本地朗读工厂\<Mini|Full>`；模型根由用户在 GUI 选盘
  （`GetLogicalDriveStringsW` + `GetDiskFreeSpaceExW` 枚举并显示剩余空间），
  `model_root_for(letter)` → `<盘符>:\本地朗读工厂模型`，写进 `config.json`。
- **理由**：重量级 TTS 权重动辄数百 MB，用户要能放到 D/E 盘；换盘/换机只改一个字段。
- **影响**：`mini/src/model.rs`、`mini/src/ui.rs` 的盘符下拉与「打开模型目录」按钮。

## ADR-024 · 内置多源 + 自动测速 + 失败自动切源

- **决策**：内置 8 个源（`github / gh-proxy / ghfast / ghproxy-net / gitmirror / hf-mirror /
  huggingface / custom`）；`probe_sources()` 用系统 `curl.exe` 测速排序，
  GUI「自动测速选源」直接选最快；`install_model()` 按 `effective_urls()` 遍历，
  失败/超时/404 明确提示后自动切下一个源；`Progress` 回调显示字节/速度/ETA/百分比。
- **理由**：用户核心痛点就是「自己去网上找很多镜像一个一个试」，国内直连 GitHub Release 会 502。
- **影响**：`mini/src/model.rs`、GUI 的源/模型/盘符控件与进度条。

## ADR-025 · 自研 overlay 自解压安装器，双 release 都出 setup.exe

- **决策**：单 exe = Rust stub + 尾部 overlay（`<payload zip><len:u64 LE><magic "BCTLPKG1">`）。
  运行时读自身尾部 → 解压到用户目录 → 写 HKCU 产品信息 / `Uninstall` 卸载项 / `.epub` 关联 /
  开始菜单 + 桌面快捷方式。**Mini 与 Full 共用同一套 stub**；Full 载荷复用既有便携包 zip，
  重量级主线源码与产物完整保留。
- **理由**：本机未装 Inno Setup、winget 安装需提权；PE overlay 是 7z SFX / NSIS 的标准做法。
  用 HKCU 而非 HKLM，**不需要管理员权限**。
- **两阶段卸载**：卸载器 stub 位于待删目录内，Windows 不允许删除运行中的 exe；
  先复制自身到 `%TEMP%`，由副本等待后删目录/注册表/两处快捷方式，保留用户模型与有声书。
  `Uninstall-*.exe` 无参数双击也必须能卸载（缺陷 #26）。
- **影响**：`mini/setup/main.rs`、`packaging/make-installer.ps1`；产物 `dist/release/*-setup.exe`。

## ADR-026 · 委托链路按 Full 的真实产物回填，不改 JSON 输出编码

- **决策**：`delegate_full()` 解析 Full 的 NDJSON 事件流，只输出 `[stage] message` 人类可读进度
  并用 `segment_done` 驱动 GUI 进度条；完成后读 Full 自己的 `manifest.json`，
  用 `totals.chapters / totals.audio_seconds / engine.name / chapters[]` 回填 Mini 的
  `Summary` 与 manifest；`engine` 标签按真实引擎动态生成（`mock-via-Full` / `sherpa-via-Full`）。
- **理由**：初版把整行 NDJSON 灌进日志、硬编码 `sherpa-via-Full`、统计恒为 0，
  在 `--engine mock` 时给出错误信息；产物字节数只统计一层目录导致误报 57,344 B。
- **PowerShell 5.1 中文乱码**：实测 Full 与 Mini 的 stdout/stderr **字节均为干净 UTF-8**，
  是宿主用 GBK 解码子进程 stderr 的显示问题。**不改程序输出编码**（会破坏 CLI/MCP 的
  JSON envelope 契约），改为在 `help` 与便携包 `使用说明.txt` 给出 `chcp 65001` 指引。
- **影响**：`mini/src/audio.rs`、`mini/src/main.rs`、`mini/build.ps1`、缺陷 #24/#25。

---

## ADR-027 · 阅读器是 `mini/` 的一个模式，不新建第三条产品线

**背景**：用户要求"在 v0.5 基础上再做一个版本，把它叫做阅读器"——边看 EPUB 文字、边听朗读、
同时后台生成后面章节的语音。

**决策**：阅读器 = 同一个 `BookCtl.Mini.exe` 的 `reader` 模式（`reader` 子命令 / `--reader` /
文件名 `BookCtl.Reader.exe` 自动判定）。复用 `audio::parse_epub`、`Config`、JSON envelope、
安装器 stub，只新增 `mini/src/reader.rs` 一个模块 + 第三个 setup.exe。

**为什么**：如果新建第三条代码线，会出现三份 EPUB 解析、三份 SAPI 调用、三份配置读取，
与 v0.4 刚清理掉的"结构混乱"问题反向。单 exe 多模式让"再做一个版本"的成本只剩一个模块 + 一个包。

**代价**：阅读器与批量模式共享同一个进程体积（664 KB，其中大部分是 Win32 FFI 与 EPUB 解析）；
阅读器无法用 Full 的模型做单段合成（Full CLI 只有整本流水线），见 ADR-028。

---

## ADR-028 · 同步粒度定为段落，不做字级

**备选**：
- 整章合成后才开播 → 用户点播放要等整章（数秒到数十秒），违背"边看边读"。
- 字级同步（SAPI `SpeakProgress` 时间戳 + `waveOut` 定位偏移）→ 视觉最细腻，但要手写
  `waveOut` 回调、处理 WAV 采样率换算，且 `SpeakProgress` 在 PowerShell 封装下不可靠。
- **段落级**：合成单位 = `parse_epub` 拆出的自然段（约 12–110 字），一段一个 WAV。

**决策**：段落级。每段独立 WAV、独立缓存、独立播放；播完自动推进游标。

**为什么**：① 人的阅读节奏天然按段落走，高亮一段足够自然；② 单段合成 0.6 秒，
预取窗口天然成立；③ 保持零外部 crate、零 COM 手写、零 `waveOut` 复杂度。

**代价**：段内不做逐字高亮。若将来要字级，需引入 `SpeakProgress` 或换引擎，另开 ADR。

---

## ADR-029 · 预取窗口：默认 6 段 + 强制伸进下一章前 2 段

**决策**：`window_end = max(cursor + prefetch, next_chapter_first + 2)`，默认 `prefetch = 6`，
GUI 可调 2/4/6/10/16。

**为什么**：用户原话是"边读它同时在生成后面的章节"。只在当前章内预取，读完本章最后一段就会卡住
等下一章；强制把下一章前 2 段纳入窗口，跨章时永远不会断流。窗口有上限（不会退化成整本合成），
所以内存/磁盘占用可控，跳读时也只需丢弃窗口外的排队。

**缓存**：按「规范化绝对路径 + 文件大小 + 语速」做 FNV-1a key，跨会话复用；
跳读用 `gen` 代际号让旧位置的在途合成结果只写缓存、不改当前窗口状态。

---

## ADR-030 · 播放用动态加载 `winmm.dll`，不新增链接依赖

**备选**：
- `#[link(name = "winmm")] extern PlaySoundW` → 需要 `winmm.lib` 在 rust-lld 自包含导入库目录里，
  不确定，且换工具链可能失败。
- 手写 `waveOutOpen/waveOutWrite` → 精确控制，但代码量大且要处理缓冲区复用。
- **`LoadLibraryW("winmm.dll")` + `GetProcAddress("PlaySoundW")`**，只取一个函数指针。

**决策**：动态加载。`SND_FILENAME | SND_NODEFAULT` 在播放线程同步播放（播完返回 → 推进游标），
暂停用 `SND_PURGE` 立即打断。

**为什么**：不扩大导入表、不依赖导入库存在性、链接参数完全不变（`rust-lld` + `link-self-contained`
继续成立）。代价是函数指针需要 `OnceLock` 缓存且不能 `FreeLibrary`（进程生命周期内保持加载）。

**验收**：`reader --play-test` → `status=played`、`play_ms=8601`、`playback_available=true`。

---

## 待拍板 / 未决

| 编号 | 议题 | 状态 |
|---|---|---|
| D-1 | CosyVoice 3 真实权重接入（需 ≥6GB 显存机器） | P1，见 `docs/roadmap.md` |
| D-2 | 音色来源与整本书音色一致性策略（A/B 盲测） | 未做 |
| D-3 | S5 ASR 回验范围（全量 / 抽样 / 关闭） | 当前 `verify=skipped` |
| D-4 | Worker 形态：内嵌 vs 独立常驻 daemon | 当前内嵌 + 独立入口并存 |
| D-5 | 分发形态最终定稿（是否真做 Pro 功能门禁） | `Edition` 字段已预留 |
| D-6 | Mini 是否直接加载本地模型权重（不依赖 Full） | 未做，R-14；当前靠委托 Full |
| D-7 | 代码签名（PFX / EV / Azure Trusted Signing） | 脚本就绪，缺用户证书 |
