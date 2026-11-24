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

## 待拍板 / 未决

| 编号 | 议题 | 状态 |
|---|---|---|
| D-1 | CosyVoice 3 真实权重接入（需 ≥6GB 显存机器） | P1，见 `docs/roadmap.md` |
| D-2 | 音色来源与整本书音色一致性策略（A/B 盲测） | 未做 |
| D-3 | S5 ASR 回验范围（全量 / 抽样 / 关闭） | 当前 `verify=skipped` |
| D-4 | Worker 形态：内嵌 vs 独立常驻 daemon | 当前内嵌 + 独立入口并存 |
| D-5 | 分发形态最终定稿（是否真做 Pro 功能门禁） | `Edition` 字段已预留 |
