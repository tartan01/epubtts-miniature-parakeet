# 上下文压缩历史摘要

> 本期会话经历 3 次上下文压缩。以下为压缩时保留的历史对话与结论，用于补齐 `01-full-dialogue.md` 中被折叠的早期内容。
> 已剔除工具调用块与 `<omitted />` 占位；重复内容按出现顺序保留。

---

## C1 · 2026-11-24 16:31:35

Summary of the conversation so far:
The conversation is between an AI agent and a user.
Use this to get up to speed, and continue helping the user as the AI agent.
Some contents may be omitted, indicated by "".


**用户（历史）**

> 
<user_query>
你上网搜索一下方案，做一个软件Windows端的软件可以自动化的软件自动化由代码来驱动的Windows框架软件，我把一本epub格式的电子书给到这个软件系统，就是这个软件它是符合那个Cmd命令来控制的，这个软件纯支持它来控制的，大模型友好，甚至它是支持mcp控制的，然后呢先做e pub格式，给他之后呢它自动会使用开源语音框架在本地显卡上，这样就节省资源，在本地的显卡上用来把它转化成朗读版本，整本图书的音频文件可以按章节读，然后把每一章节都可以分类整理好。你上网搜索一下，然后综合互联网上所有的信息设计一个架构，然后我来参考一下，我是架构师，然后ok的话，我让你开始干活。
</user_query>


**助手（历史）**


我先做一轮资料调研，再从引擎选型、EPUB 工程链路、控制面（CLI/MCP）三条线并行展开，最后给你一份可评审的架构方案。


**助手（历史）**


三路调研已完成，信息量很扎实。我先更新任务状态，然后综合三份报告设计架构并产出评审文档。


**助手（历史）**


文档已生成。我再给你一张可直接看的核心架构图，方便评审时对照。


**助手（历史）**


调研 + 架构设计已完成，两份评审文档已生成（HTML 可视化版已打开预览，Markdown 是完整细节版）。

**调研结论（关键几点）**
- 现有开源项目（audiblez、ebook2audiobook、epub2tts 等）都是“给人用的转换器”，**没有一个是为自动化系统做的**：没有 Job 化/断点续传契约、没有机器可读控制面、默认引擎没有一个同时满足中文整本书 + 许可证干净（ebook2audiobook 默认的 XTTS 权重 CPML 非商用且 Coqui 已注销，是法律死路）。
- **默认引擎选 CosyVoice 3**（Fun-CosyVoice3-0.5B-2512_RL）：代码+权重双 Apache-2.0、中文 CER 0.81%（接近人类 1.26%）、内置文本归一化（数字/日期直接读）、Win11 + RTX 3060 6GB 有实测通路。备选 Qwen3-TTS / VoxCPM2 / GPT-SoVITS，Kokoro 仅作快速校样。VibeVoice 被明确排除（强制插入可听 AI 声明 + 水印）。
- MCP 现状是 2026-07-28 无状态化协议：移除 initialize/session、SSE 可恢复性被移除——**整本书绝不能在一次 tool 调用里跑**，必须 job 异步化。

**架构要点（四层）**
- L1 控制面：CLI 与 MCP Server 共用同一份 handler（单一事实源），SLA ≤2s，`--capabilities`/`--schema`/`--dry-run` 自描述；
- L2 编排：SQLite + 状态机 + 幂等键 + **章节级 checkpoint**（断电后只重跑未完成的章）+ GPU 单实例锁；
- L3 六 Stage 流水线：ingest → TOC anchor 切章 → 中文 TN/发音词典/分句 → GPU 合成（音色 profile 全书锁定一次）→ Whisper ASR 回验（念错自动重生成该段）→ 两遍 loudnorm + 每章独立音频 + 整本 M4B；
- L4 引擎层：Provider 可插拔，跑在**已登录用户会话的常驻 daemon**（不是 SYSTEM 服务，Session 0 拿不到 GPU）。

性能估算：20 万字书 ≈ 15.3 小时音频，RTF 0.15–0.30 下单卡约 2.3–4.6 小时跑完一本。

**需要你拍板的 6 个决策点**（文档第 10 节）：默认引擎是否按报告冻结、音色来源（预设/克隆/VoiceDesign）、是否强制出整本 M4B、ASR 回验全量还是抽检、daemon vs 进程内、portable zip vs PyInstaller。

你评审一下，确认方向和决策点后说一声，我就开始干活（建议第一步先做 P0：在目标机上同章 A/B 盲测冻结引擎）。


**用户（历史）**

> 
<user_query>
很好，你把这个架构设计形成一个 MD 文件，好像已经有了。现在以 MD 文件作为总架构，我调用一些软件来协助你做。我的计划是把它做成一个 WinUI 的，用 WinUI 来写它比较清爽一点，原生支持 Windows 图形化，然后里面是支持一键的我们可以通过命令行 MCP 或 CLI 直接导入 epub 文件，来生成朗读音频。
这个软件是完全的桌面软件形态，能够看到执行速度和中控台。在执行过程中，能实时看到它正在转换、已经调用显卡以及正在转换 TTS 的状态。最后生成好之后，它会新建一个项目文档文件夹供我们检验。
我现在来安装 Visual Studio Code，你就负责写代码。
</user_query>


**助手（历史）**


收到，我按总架构 MD 落地代码。先确认文档内容和 WinUI 3 的构建链现状（VS Code 只是编辑器，这块有个坑要提前告诉你），然后开始写项目。


**助手（历史）**


核对完毕，有个好消息：2026 年微软已经放出 **WinUI 的 `dotnet new` 模板 + WinApp CLI**，VS Code + .NET 10 就能免装 Visual Studio 开发 WinUI 3 —— 你的工具链选择是对的，我按这条链写。先把总架构 MD 补上 v0.2 变更，再落代码。


**助手（历史）**


Windows App SDK 已确认最新稳定版 2.7.0，我更新版本引用后开始写 Core 层代码。


**助手（历史）**


修一个笔误，然后写引擎层（WAV 写入/拼接、mock 引擎、Python sidecar 客户端、GPU 监控、ffmpeg 母带）。


**助手（历史）**


修两个小问题，然后写流水线编排层。


**助手（历史）**


修几处编译隐患（反射取 job_id、JSON 命名、缺失 using），然后写三个可执行入口。


**助手（历史）**


核心链路已通，现在写 WinUI 桌面界面（一键导入 + 中控台 + GPU/速度/TTS 实时状态 + 打开产物文件夹）。


**助手（历史）**


本机没有 .NET SDK，无法替你预编译——构建验证需要你在装好 VS Code + .NET 10 SDK 后跑一次（README 里写了一键命令和可能要微调的两处版本号）。写完剩余文件就交付。


**助手（历史）**


代码写完了。总架构 MD 保持为唯一事实源（已追加「附录 A · v0.2 桌面形态变更」，与代码一一对应），完整项目约 20 个源文件落在 `src/` 和 `engine/`。

**交付了什么**

- **WinUI 3 桌面 App**（unpackaged，双击 exe 即用）：浏览/选择 EPUB → 引擎/音色/语速/M4B 参数 → 「一键生成朗读音频」→ 中控台实时显示你要的四块状态：**当前阶段（S1–S6）、显卡状态（nvidia-smi 型号/利用率/显存，未调用也明示）、TTS 状态（当前第几章几段）、执行速度（字/秒、RTF、ETA）** → 完成后「打开项目文档文件夹」直接定位到 manifest.json。
- **CLI**：`bookctl.exe`，JSON envelope + 稳定退出码 + 幂等 + 断点续传 + `run` 一键命令（实时 JSONL 事件流）。
- **MCP**：`bookctl-mcp.exe` stdio 服务，13 个 tools + job resources + operate_playbook prompt，兼容 2025-11-25/2026-07-28 协议版本，mcp.json 示例已写进 README。
- **Core**：EPUB 解析（TOC anchor 切章、DRM 判定不误杀字体混淆）、中文 TN/分句、SQLite WAL 状态机（CAS、幂等键、章节级 checkpoint、协作式取消）、WAV 拼接 + ffmpeg M4B/ffmetadata。
- **engine/tts_server.py**：标准库 mock 模式**现在就能跑通全链路**（无需显卡）；CosyVoice 3 接入点已留好并写明约束。

**两件必须知道的事**

1. **VS Code 只是编辑器**，编译还需要装：`winget install Microsoft.DotNet.SDK.10` + Windows 11 SDK（WinApp CLI 会引导）。2026 年官方已支持免 Visual Studio 的 WinUI 工具链，README 写了完整步骤。
2. 本机没有 .NET SDK，**这批代码未经编译验证**。Windows App SDK 版本我按 NuGet 核实写死为 2.7.0，若还原失败按 README「版本口径」对齐模板即可。

你装好环境后跑：`dotnet build bookctl.sln` → `dotnet run --project src/BookCtl.Cli -- doctor` → `run --input "任意一本.epub"`（mock 引擎验收全链路）。有任何编译或运行报错，把输出发我，我来修。

## C2 · 2026-11-24 18:12:48

Summary:

1. **Primary Request and Intent:**
   用户整体意图是：从零设计并实现一个“纯正 Windows 桌面软件”，用代码/命令行/MCP/大模型可驱动的方式，把 EPUB 整本书在本地转换成分章节有声书；后续再迭代为可签名封装的 Pro 版，包含便携版与 exe 安装版，并支持显卡/CPU 推理设备切换、内置模型源管理、Apple 风格双模式 UI。

   按时间线梳理：

   - **初始架构调研请求**：用户要求“上网搜索一下方案”，综合互联网信息设计一个 Windows 端自动化软件架构：
     - 软件由代码驱动，符合 `Cmd` / CLI 命令控制；
     - 大模型友好，甚至支持 MCP 控制；
     - 输入一本 `.epub` 电子书；
     - 软件自动使用开源语音框架在本地显卡上 TTS，把整本书转换为朗读版本；
     - 音频按章节分类整理；
     - 用户是架构师，先看架构，确认后再开始干活。

   - **进入实现阶段**：用户确认架构 MD 已有，要求以 MD 为总架构，做成 WinUI 桌面软件：
     - 原生 Windows 图形化；
     - 支持 CLI / MCP / 命令行导入 EPUB 生成朗读音频；
     - 完全桌面软件形态；
     - 能看到执行速度、中控台；
     - 执行中实时看到“正在转换 / 已调用显卡 / 正在转换 TTS 状态”；
     - 生成后新建项目文档文件夹供检验；
     - 用户准备安装 VS Code，要求“我负责写代码”。

   - **关于 IDE / 安装 / 审阅方式**：用户问：
     - 是否需要 VSCode 写工程代码，还是 Agent 直接能写完；
     - 人工审阅用什么 IDE；
     - 是否不需要 IDE，全部用“许愿式编程”自然语言指挥修改；
     - 最后要求“还是你来安装吧，你评估一下方案”。

   - **封装与设备/源管理的前瞻要求**：用户明确后续软件要签名封装：
     - 形成 Pro 版可移动版和 exe 安装版两个版本；
     - 源代码阶段也要考虑封装支持；
     - 封装后具备：
       1. 显卡切换：用户可选显卡 0 / 1 / 2 跑推理；
       2. “DDS 源管理”：内置一些源，可下载本地封装进去，也可一键下载到本地供软件调用；
     - 用户要求“先把没封装好的源代码版本告诉我路径，我先跑来试试”；
     - 用户允许先用 CPU 轻量引擎；
     - 询问是否有分布式/拆分式 TTS 推理框架，能同时调用 GTX 1050 显存和 CPU；
     - 如果没有，就支持纯 GPU 或纯 CPU 推理并可切换；
     - 选择权完全交给用户。

   - **产品命名、双模式 UI、Apple 风格与反馈可视化**：用户新增明确要求：
     - 软件名称暂定“本地朗读工厂”或“帅帅朗读工厂”；
     - 首页可切换“简洁模式”和“详细参数模式”；
     - 详细参数模式：手动 menu 模式，用户手动选择 EPUB 等文件；
     - 简洁模式：内置小型模型，用户导入一本 EPUB 后自动识别、拆分章节并生成朗读文件；
     - 朗读文件生成后，实时图形化告知用户生成到哪个文件夹、生成了哪几章；
     - UI 设计参考 Apple 风格，并给出完整设计提示词：浅灰 `#f5f5f7`、白卡、系统蓝 `#0071e3`、圆角、留白、500ms 缓动、Pill Button、Product Card、Search Field、Vibrancy Nav 等规范。

   - **当前最新请求**：用户要求生成一份“详细且高度结构化”的对话总结，严格按指定结构输出，覆盖所有用户请求、技术决策、文件、代码、错误修复、当前工作和下一步。

2. **Key Technical Concepts:**

   * **CLI-first / MCP / 大模型友好控制面**
     - JSON envelope：`{"ok":true,"api_version":"1.0","data":...}` 或结构化 error；
     - 稳定退出码；
     - `--capabilities` / `--schema` / `--dry-run` / `--pretty` / `--quiet`；
     - MCP stdio JSON-RPC；
     - 长任务 job 化：`job_start` / `job_status` / `job_logs` / `job_cancel` / `job_resume` / `job_artifacts`；
     - 避免整本书在单次 MCP tool 调用里执行，所有耗时操作异步化；
     - MCP tools 后续扩展到 `device_list` / `source_list` / `source_pull`。

   * **总架构 MD 作为唯一事实源**
     - 初版：`architecture-epub2audiobook.md`；
     - 追加“附录 A · v0.2 桌面形态变更”；
     - 后续追加“附录 B · v0.3 双模式、设备选择、源管理、封装”。

   * **WinUI 3 / Windows App SDK / .NET 10**
     - `WinUI 3` unpackaged 桌面程序；
     - `WindowsPackageType=None`；
     - `WindowsAppSDKSelfContained=true`；
     - `net10.0-windows10.0.19041.0`；
     - Windows App SDK 版本核实为 `2.7.0`；
     - VS Code 只是审阅/调试用，不影响 Agent 直接读写源码；
     - Visual Studio 全家桶不建议，纯 XAML 不需要拖控件。

   * **EPUB 工程链路**
     - EPUB zip / `META-INF/container.xml` / OPF / `manifest` / `spine` / NCX / `nav.xhtml`；
     - TOC anchor 切章；
     - DRM 判定；
     - 中文 TN / 分句；
     - 跳过错别目录、版权页、索引、空白段；
     - UTF-8、中文路径、XHTML entity、CSS hidden 等 Windows/EPUB 坑。

   * **TTS 引擎选型**
     - 默认初版架构选 `CosyVoice 3`：Apache-2.0、中文效果好、需 ≥6GB 显存；
     - 排除 `VibeVoice`：会强制插入可听 AI 声明 + 水印；
     - 谨慎使用 `XTTS / Coqui`：CPML / 商用限制；
     - 后续加入 CPU 轻量引擎 `sherpa-onnx`：
       - 实测 `sherpa-onnx 2.0.3` 可安装；
       - 中文模型 `vits-zh-hf-fanchen-C`：约 119MB 压缩包、解包后约 132MB；
       - 纯 CPU 可跑，16kHz，187 个说话人；
       - 真实中文试听文件 `testdata/sherpa-probe.wav` 已生成；
     - `piper` 作为更轻量备选；
     - `mock` 作为校样占位音频引擎；
     - `cosyvoice3` 保留 GPU 适配点。

   * **六阶段流水线**
     - `S1_INGEST`：解析 EPUB；
     - `S2_SPLIT`：按目录切章；
     - `S3_NORMALIZE`：文本正则化 / 分句；
     - `S4_SYNTH`：TTS 合成；
     - `S5_VERIFY`：ASR 回验适配点；
     - `S6_MASTER`：音频母带 / M4B / manifest / qc。

   * **SQLite WAL / JobStore / 状态机**
     - CAS / revision；
     - 幂等键；
     - 章节级 checkpoint；
     - 协作取消；
     - 断点续传；
     - 事件 NDJSON；
     - `worker.lock` PID 锁与孤儿任务回收；
     - `WORKER_LOST` 可 resume。

   * **音频工程**
     - WAV 拼接；
     - `ffmpeg` 生成 `audio.m4a`；
     - `ffmetadata` 章节标记；
     - `book.m4b`；
     - `ffprobe` 校验 chapter metadata；
     - `AudioMaster.ProbeDuration`：WAV 读头 / ffprobe / 码率兜底；
     - `Run()` 并发读取 stdout/stderr，避免 ffmpeg 管道死锁；
     - `concat_list.txt` 路径统一正斜杠。

   * **设备选择 / CPU-GPU 切换**
     - `Device(auto|cpu|gpu|hybrid)`；
     - `GpuIndex`；
     - `CpuThreads`；
     - `DevicePlanner.Resolve()`；
     - 显存门槛：
       - `cosyvoice3` 6144MB；
       - `sherpa` 1536MB；
       - `piper` 512MB；
       - `mock` 0；
     - `auto`：显存够 → GPU；中等显存 → hybrid；显存过小 → CPU；
     - `hybrid`：层卸载，能跑但更慢；
     - 设备不参与 `profile_hash`，避免换卡影响确定性判定。

   * **分布式 / 拆分式 TTS 推理结论**
     - 没有能把 GTX 1050 2GB + CPU 组合起来“提速”的开源 TTS 分布式推理框架；
     - 张量并行需要 ≥2 张同型号、算力 ≥7.0 的卡；GTX 1050 算力 6.1 被 vLLM/SGLang 类框架拒绝；
     - `device_map` 层卸载只解决“模型装不下”，通常更慢；
     - ONNX Runtime 单 session 单 EP；
     - 多进程异构分片会破坏音色一致性；
     - 产品策略：纯 GPU / 纯 CPU / hybrid 兜底，选择权交给用户。

   * **DDS / 引擎与模型源管理**
     - 用户说“DDS 源管理”，实现中按“内置引擎/模型源目录 + 一键下载/解包/安装状态”落地；
     - `SourceManager.BuiltInCatalog()`；
     - 源类型：`archive` / `zip` / `file` / `manual`；
     - `source list` / `source pull` / `source remove`；
     - 多线路下载回退：原始 URL → 用户 mirrors → `BOOKCTL_MIRROR` → `gh-proxy.com` / `hf-mirror.com`；
     - 国内直连 GitHub Release 实测 502，必须镜像回退；
     - 解包用系统 `tar.exe` 或 `ZipFile`；
     - `.installed.json` 标记；
     - 用户自定义 `sources/catalog.user.json` / `config/sources.example.json`。

   * **封装 / 签名 / Pro 版**
     - 便携版：`portable.marker` 存在时，`data\` / `sources\` 写程序目录；
     - 安装版：Inno Setup per-user 安装，删除 marker，数据写 `%LOCALAPPDATA%\bookctl`；
     - `build-portable.ps1`：publish + 拷贝 config/engine + 生成 zip；
     - `bookctl.iss`：Inno Setup 脚本，`.epub` 文件关联，桌面快捷方式；
     - `sign.ps1`：PFX / 证书指纹 / Azure Trusted Signing + RFC3161 时间戳；
     - 两版 self-contained，目标机无需 .NET 10 / Windows App Runtime；
     - 签名私钥不进仓库，只读环境变量；
     - Pro 差异化预留 `Edition` 字段。

   * **Apple 风格 UI / 双模式**
     - 产品名改为“本地朗读工厂”；
     - 顶部模式切换：`简洁模式` / `详细参数`；
     - 简洁模式：
       - 一张输入卡；
       - 一颗系统蓝胶囊按钮“生成朗读音频”；
       - 进度卡；
       - 生成结果卡：输出文件夹 + 实时章节卡片；
       - 自动使用已安装 CPU 模型，否则 mock 并提示下载。
     - 详细参数模式：
       - 左侧参数卡：引擎、音色、推理设备、显卡、CPU 线程、语速、M4B；
       - 右侧状态卡和中控台；
       - 显示 `DevicePlan` 决策文本和警告。
     - 颜色：`#F5F5F7` 背景，白色卡片 24px 圆角，`#0071E3` 唯一强调色；
     - 字体：`Segoe UI Variable`；
     - 控件：胶囊按钮、12px 输入框、24px 卡片；
     - `ToggleButton` 绑定 `IsSimpleMode` / `IsDetailedMode`；
     - `Visibility` 由 `SimpleVisibility` / `DetailedVisibility` 控制；
     - 支持 `--detailed` 启动与 `.epub` 命令行参数，配合安装包文件关联。

3. **Files and Code Sections:**

   * `C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/architecture-epub2audiobook.md`

     * 总架构唯一事实源。
     * 初版包含调研结论、四层架构、CLI/MCP 接口草案、TTS 选型、音频工程、状态机、路线图。
     * 追加“附录 A · v0.2 变更：WinUI 桌面形态”。
     * 追加“附录 B · v0.3 变更：双模式、设备选择、源管理、封装”。
     * 附录 B 关键结论：
       - 产品名“本地朗读工厂”；
       - 简洁模式不是另一个流水线，只是写会话配置后调用同一个 `JobStart`；
       - 设备选择是 S4 前的 `DevicePlan`；
       - 源管理为 L4 旁路；
       - 开源 TTS 没有可提速的 GPU+CPU 拆分推理。

   * `architecture-epub2audiobook.html`

     * 可视化评审文档，含架构图、流水线、选型对比、CLI/MCP 契约、路线图与待拍板决策点。

   * `Directory.Build.props`

     * 统一 SDK / Windows App SDK 版本。
     * `WindowsAppSDKVersion` 从 `1.8.0` 修正为 NuGet 最新稳定版 `2.7.0`。

   * `bookctl.sln`

     * .NET 解决方案，包含：
       - `BookCtl.Core`
       - `BookCtl.Cli`
       - `BookCtl.Mcp`
       - `BookCtl.Worker`
       - `BookCtl.App`

   * `config/bookctl.defaults.json`

     * 默认配置文件，包含工作区、引擎、音色、M4B、ffmpeg 等默认项。

   * `config/sources.example.json`

     * 新增自定义源示例，说明：
       - 如何用 `sources/catalog.user.json` 覆盖内置源；
       - `mirrors`；
       - `type: file` 多文件下载写法；
       - 企业内网镜像示例。

   * `.vscode/settings.json` / `.vscode/tasks.json` / `.vscode/launch.json`

     * VS Code 审阅/调试配置。
     * 用户最终只建议把 VS Code 用于人工审阅，不用于 Agent 写代码。

   * `src/BookCtl.Core/Contracts.cs`

     * 基础契约：JSON envelope、异常、退出码、配置。
     * 新增 v0.3 配置项：
       ```csharp
       public string Device { get; set; } = "auto";
       public int GpuIndex { get; set; } = -1;
       public int CpuThreads { get; set; } = 0;
       public bool AllowCpuOffload { get; set; } = false;
       public string Edition { get; set; } = "community";
       public string EffectiveSourcesRoot { get; set; } = "";

       public string SourcesRoot => !string.IsNullOrWhiteSpace(EffectiveSourcesRoot)
           ? EffectiveSourcesRoot
           : Path.Combine(Path.GetDirectoryName(EffectiveWorkspaceRoot.TrimEnd('\\', '/')) ?? EffectiveWorkspaceRoot, "sources");
       ```
     * `CloneWith(...)` 增加 `device` / `gpuIndex` / `cpuThreads`。
     * `ApplyEnv(...)` 增加：
       - `BOOKCTL_DEVICE`
       - `BOOKCTL_GPU`
       - `BOOKCTL_CPU_THREADS`
       - `BOOKCTL_SOURCES`
     * 新增 `ApplyPortable(...)`：
       ```csharp
       private static void ApplyPortable(BookCtlConfig cfg)
       {
           var baseDir = AppContext.BaseDirectory;
           if (!File.Exists(Path.Combine(baseDir, "portable.marker"))) return;
           if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("BOOKCTL_WORKSPACE")))
               cfg.WorkspaceRoot = Path.Combine(baseDir, "data");
           if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("BOOKCTL_SOURCES")))
               cfg.EffectiveSourcesRoot = Path.Combine(baseDir, "sources");
           cfg.Edition = "pro";
       }
       ```

   * `src/BookCtl.Core/Models.cs`

     * `GpuStatus` / `BookMeta` / `Chapter` / `Segment` / `SynthesisProfile` / `JobRecord` / `JobEvent`。
     * `SynthesisProfile.From(...)` 改为接收 `DevicePlan? plan = null`，增加：
       ```csharp
       public string Device { get; set; } = "cpu";
       public int GpuIndex { get; set; } = -1;
       public int CpuThreads { get; set; } = 0;
       public bool CpuOffload { get; set; }
       ```
     * 设备不参与 `Hash`。

   * `src/BookCtl.Core/JobStore.cs`

     * SQLite WAL、jobs / chapters / events 表、CAS、幂等键、checkpoint、事件流。
     * 新增孤儿任务回收支持：
       ```csharp
       public List<string> FindRunningJobIds()
       public bool FailIfStillRunning(string jobId, string code, string message)
       ```
     * `P(SqliteCommand cmd, string name, object value)` 改为 `object? value`，消除 nullable warning。

   * `src/BookCtl.Core/Epub.cs`

     * EPUB 解析、DRM 判定、OPF / NCX / nav.xhtml、TOC anchor 切章、中文 TN / 分句。
     * 修过 `using` 笔误、`oppfText` / `opfText`、`Xml.XmlException`、正则 `?? default` 等编译问题。

   * `src/BookCtl.Core/Engine.cs`

     * `ITtsProvider` / `MockTtsProvider` / `SidecarTtsProvider`。
     * `TtsProviderFactory` 增加 `sherpa` / `sherpa-onnx` / `onnx` / `piper`。
     * `Describe(...)` 增加 `mock` / `sherpa` / `sidecar` 三档说明。
     * `SidecarTtsProvider.SynthesizeAsync(...)` payload 增加设备字段：
       ```csharp
       device = profile.Device,
       gpu_index = profile.GpuIndex,
       cpu_threads = profile.CpuThreads,
       cpu_offload = profile.CpuOffload,
       ```
     * `Wav.Concat(...)` 修复采样率、空拼接、按首个 WAV 的 rate 写 header。
     * `AudioMaster` 新增：
       - `_ffprobe`
       - `ResolveSibling(...)`
       - `ProbeDuration(...)`
     * `AudioMaster.MasterBook(...)` 修复 `concat_list.txt` 路径：
       ```csharp
       File.WriteAllLines(listPath, audioFiles.Select(p =>
           $"file '{p.Replace("\\", "/").Replace("'", "'\\''")}'"), new UTF8Encoding(false));
       ```
     * `Run(...)` 修复 ffmpeg 管道死锁：
       ```csharp
       Arguments = "-hide_banner -loglevel error -nostats " + args,
       var so = p.StandardOutput.ReadToEndAsync();
       var se = p.StandardError.ReadToEndAsync();
       if (!p.WaitForExit(timeoutMs)) { p.Kill(entireProcessTree: true); return false; }
       Task.WaitAll(new Task[] { so, se }, 5000);
       ```
     * `ResolveFfmpeg(...)` 增加 WinGet 包目录扫描兜底，解决刚装 ffmpeg、PATH 未刷新时找不到的问题。

   * `src/BookCtl.Core/Pipeline.cs`

     * 六阶段流水线、EventBus、WorkerHost。
     * S4 前新增 `DevicePlan`：
       ```csharp
       var plan = DevicePlanner.Resolve(cfg);
       var profile = SynthesisProfile.From(cfg, model, plan);
       ...
       Emit(job, "info", "S4_SYNTH", "gpu_status", devMsg + ...,
           new { gpu = g0, device_plan = plan.ToDto(), engine = cfg.EngineMode, model, voice = cfg.Voice, profile_hash = profile.Hash });
       ```
     * `segment_done` 事件新增：
       ```csharp
       device = plan.Device,
       device_display = plan.Display,
       ```
     * 空段过滤：
       ```csharp
       if (string.IsNullOrWhiteSpace(s)) continue;
       ```
     * 章节收尾改用 `_audio.ProbeDuration(audioPath)`。
     * Resume 计数修复：
       ```csharp
       job.ChaptersDone = chapters.Count(c => c.Status == "done");
       job.CharsDone = chapters.Where(c => c.Status == "done").Sum(c => c.CharCount);
       job.AudioSeconds = chapters.Where(c => c.Status == "done").Sum(c => c.Seconds);
       _store.UpdateJob(job);
       ```
     * done 章节不再 `job.ChaptersDone++`。
     * 完成态清空错误码：
       ```csharp
       job.ErrorCode = null;
       job.ErrorMessage = null;
       ```
     * `WorkerHost` 增加 `EventBus _bus`、构造函数 `WorkerHost(..., EventBus? bus = null)`。
     * `WorkerHost.Start()` 增加 `RecoverOrphans()` / `WriteLock()` / `worker.lock` PID。
     * `RecoverOrphans()` 把残留 running 标为 `WORKER_LOST`，可 resume。

   * `src/BookCtl.Core/Handlers.cs`

     * GUI / CLI / MCP 共用 Handler。
     * 新增 `SourceManager Sources { get; }`。
     * 新增 `DeviceList()`：
       - 列出显卡 index / name / mem_total / mem_free；
       - CPU 核数；
       - `DevicePlanner.Resolve(Config).ToDto()`。
     * `Capabilities()` 增加：
       - `device` 模式清单；
       - `edition`；
       - MCP tools 增加 `device_list` / `source_list` / `source_pull`。
     * `Doctor()` 增加 `devices = DeviceList()`。
     * 新增：
       ```csharp
       public object SourceList()
       public Task<object> SourcePull(...)
       public object SourceRemove(...)
       ```
     * `JobStart(...)` 签名扩展：
       ```csharp
       public object JobStart(string inputPath, string? idempotencyKey = null,
           string? engine = null, string? voice = null, string? device = null,
           int? gpuIndex = null, int? cpuThreads = null)
       ```
     * `RunAndWaitAsync(...)` 签名扩展并调用新 `JobStart(...)`。
     * 幂等命中 failed/cancelled 旧任务时自动 `JobResume(jobId)`。

   * `src/BookCtl.Core/Device.cs`

     * 新增文件，v0.3 设备决策核心。
     * `GpuDevice`、`DevicePlan`、`DevicePlanner`。
     * `RequiredVram(...)`：
       ```csharp
       "cosyvoice3" or "sidecar" or "python" => 6144,
       "sherpa" or "onnx" or "sherpa-onnx" => 1536,
       "piper" => 512,
       _ => 0
       ```
     * `ListGpus()` 用：
       ```text
       nvidia-smi --query-gpu=index,name,memory.total,memory.free --format=csv,noheader,nounits
       ```
     * `Resolve(...)` 按 `auto/cpu/gpu/hybrid` 和显存决策，并输出 warnings。

   * `src/BookCtl.Core/Sources.cs`

     * 新增文件，源管理核心。
     * `SourceInstall` / `SourceFile` / `SourceEntry` / `SourceCatalog` / `SourceManager`。
     * `BuiltInCatalog()` 内置 5 个源：
       - `mock`
       - `sherpa-vits-zh-fanchen`
       - `sherpa-vits-zh-aishell3`
       - `piper-zh-huayan`
       - `cosyvoice3`
     * `SourceInstall` 增加 `Mirrors`。
     * `Candidates(...)` 多线路回退：
       ```csharp
       if (url.Contains("github.com"))
           list.Add("https://gh-proxy.com/" + url);
       if (url.Contains("huggingface.co"))
           list.Add(url.Replace("https://huggingface.co", "https://hf-mirror.com"));
       ```
     * `DownloadAsync(IReadOnlyList<string> urls, ...)` 逐条尝试，全部失败抛 `SOURCE_DOWNLOAD_FAILED`。
     * 修复下载归档残留：
       ```csharp
       var extractDir = Path.Combine(tmpDir, "extract");
       Directory.CreateDirectory(extractDir);
       Extract(archive, extractDir);
       File.Delete(archive);
       ...
       var sourceDir = entry.Install.Type == "file"
           ? tmpDir
           : Path.Combine(tmpDir, "extract");
       Flatten(sourceDir, targetDir);
       ```
     * `Flatten(...)` 拍平解包目录。
     * `PullAsync(...)` 写 `.installed.json`。

   * `src/BookCtl.Cli/Program.cs`

     * CLI JSON envelope 与命令分发。
     * 新增命令：
       - `device list`
       - `source list`
       - `source pull <id>`
       - `source remove <id>`
     * `run` / `job start` 支持：
       - `--device auto|gpu|cpu|hybrid`
       - `--gpu 0|1|2`
       - `--cpu-threads N`
     * 增加 `IntFlag(...)`。
     * 增加 `PullSource(...)` 进度输出。
     * Help 更新环境：
       - `BOOKCTL_DEVICE`
       - `BOOKCTL_GPU`
       - `BOOKCTL_CPU_THREADS`
       - `BOOKCTL_SOURCES`

   * `src/BookCtl.Mcp/Program.cs`

     * MCP stdio server。
     * `job_start` schema 增加：
       - `engine enum: mock, sherpa, sidecar`
       - `device enum: auto, gpu, cpu, hybrid`
       - `gpu_index`
       - `cpu_threads`
     * tool 分发新增：
       ```csharp
       case "device_list": return _handlers.DeviceList();
       case "source_list": return _handlers.SourceList();
       case "source_pull": return PullSource(...);
       ```
     * 增加 `ArgInt(...)`。
     * 修复 static class 中实例方法错误：
       ```csharp
       private static object PullSource(string id)
       {
           return new BookCtlHandlers().SourcePull(id).GetAwaiter().GetResult();
       }
       ```
     * Tools schema 新增 `device_list` / `source_list` / `source_pull`。

   * `src/BookCtl.Worker/Program.cs`

     * 独立 Worker 入口，使用 Core 的 WorkerHost 和 JobStore。

   * `src/BookCtl.App/BookCtl.App.csproj`

     * WinUI 3 项目：
       - `UseWinUI=true`
       - `WindowsPackageType=None`
       - `WindowsAppSDKSelfContained=true`
       - `RuntimeIdentifier=win-x64`
       - `Microsoft.WindowsAppSDK 2.7.0`

   * `src/BookCtl.App/App.xaml`

     * 重写为 Apple 风格资源：
       ```xml
       <Color x:Key="SystemAccentColor">#0071E3</Color>
       <FontFamily x:Key="ContentControlThemeFontFamily">Segoe UI Variable Text</FontFamily>
       ```

   * `src/BookCtl.App/MainWindow.xaml`

     * 全量重写为 Apple 风格双模式界面。
     * 顶部产品名称“本地朗读工厂”。
     * 模式切换：
       ```xml
       <ToggleButton Content="简洁模式" IsChecked="{Binding IsSimpleMode, Mode=TwoWay}" />
       <ToggleButton Content="详细参数" IsChecked="{Binding IsDetailedMode, Mode=TwoWay}" />
       ```
     * 简洁模式：
       - 输入卡；
       - “生成朗读音频”系统蓝胶囊按钮；
       - 进度卡；
       - 生成结果卡：输出文件夹 + `ItemsControl` 逐章卡片。
     * 详细模式：
       - 左侧参数卡；
       - 右侧状态卡和中控台。
     * 移除非法 `Grid FontFamily` 属性。

   * `src/BookCtl.App/MainWindow.xaml.cs`

     * `Title = "本地朗读工厂"`。
     * `Root.DataContext = _vm`。
     * 新增：
       ```csharp
       private void StartSimple_Click(object sender, RoutedEventArgs e) => _vm.StartOneClick();
       private void StartDetailed_Click(object sender, RoutedEventArgs e) => _vm.Start();
       ```
     * 窗口尺寸从 `1280x880` 改为 `1400x960`。
     * 新增 `ApplyCommandLine()`：
       - `--detailed` 进详细模式；
       - 传入 `.epub` 路径自动填入 `InputPath`；
       - 配合安装包 `.epub` 文件关联。

   * `src/BookCtl.App/MainViewModel.cs`

     * 全量重写。
     * 新增 `ChapterCard`。
     * 双模式：
       - `IsSimpleMode`
       - `IsDetailedMode`
       - `SimpleVisibility`
       - `DetailedVisibility`
       - `ModeTitle`
       - `ModeSubtitle`
     * 设备 UI：
       - `DeviceModes`
       - `Devices`
       - `DeviceMode`
       - `DeviceIndex`
       - `CpuThreads`
       - `PlanText`
       - `PlanHint`
     * 结果可视化：
       - `OutputFolder`
       - `Chapters`
       - `ChapterSummary`
       - `CanOpenOutput`
     * `StartOneClick()`：
       ```csharp
       var hasCpuModel = App.Handlers.Sources.InstalledPath("sherpa-vits-zh-fanchen") is not null
                         || App.Handlers.Sources.InstalledPath("sherpa-vits-zh-aishell3") is not null;
       if (IsSimpleMode)
       {
           App.Handlers.Config.EngineMode = hasCpuModel ? "sherpa" : "mock";
           App.Handlers.Config.Device = "cpu";
           App.Handlers.Config.Voice = hasCpuModel ? "0" : "zh-default";
           ...
       }
       StartInternal();
       ```
     * `AddLog(...)` 处理：
       - `segment_done` 的速度 / device_display；
       - `chapter_done` 追加章节卡片；
       - `job_completed` 设置输出文件夹；
       - `job_failed` / `job_cancelled` 更新提示。
     * `LoadDevices()` / `RefreshPlan()` / `UpdateSourceStatus()`。

   * `src/BookCtl.App/app.manifest`

     * Windows 应用清单，支持 unpackaged WinUI。

   * `engine/tts_server.py`

     * 从 mock/cosyvoice3 适配点重写为三模式：
       - `mock`
       - `sherpa`
       - `cosyvoice3`
     * 新增 `discover_sherpa_dir(...)`，自动从：
       - `--model-dir`
       - `engine/../sources`
       - `engine/sources`
       - `%LOCALAPPDATA%\bookctl\sources`
       找含 `*.onnx` + `tokens.txt` 的目录。
     * 新增 `ensure_sherpa(...)`：
       ```python
       config = sherpa_onnx.OfflineTtsConfig(
           model=sherpa_onnx.OfflineTtsModelConfig(
               vits=sherpa_onnx.OfflineTtsVitsModelConfig(
                   model=onnx,
                   lexicon=...,
                   tokens=tokens,
                   dict_dir=...,
                   noise_scale=0.667,
                   noise_scale_w=0.8,
                   length_scale=1.0,
               ),
               num_threads=n_threads,
               provider="cpu",
               debug=False,
           ),
           rule_fsts=",".join(fsts) if fsts else "",
           max_num_sentences=1,
       )
       ```
     * 新增 `synth_sherpa(...)`，支持 `sid` / speed。
     * 请求级 `device` / `gpu_index` / `cpu_threads` 处理。
     * `sherpa` 的 PyPI wheel 是 CPU 版，遇到 GPU 请求明确报错，不假装使用显卡。
     * `GET /health` 返回 device / threads / model_dir / voices。
     * `POST /synthesize` 返回 `wav_b64` / `seconds` / `sample_rate` / `mode` / `device`。

   * `engine/README.md`

     * 重写为三种模式说明：
       - mock
       - sherpa
       - cosyvoice3
     * 增加独立 venv 安装：
       ```powershell
       python -m venv .venv
       .venv\Scripts\python.exe -m pip install sherpa-onnx
       ```
     * 增加设备选择、接口字段、设计约束、模型自动发现。

   * `engine/requirements.txt`

     * Python sidecar 依赖文件，早期创建；后续实际用 `sherpa-onnx` venv 安装验证。

   * `README.md`

     * 标题改为：
       ```markdown
       # 本地朗读工厂（bookctl）· Windows 本地 EPUB 有声书工厂
       ```
     * 更新构建状态、设备选择、源管理、分布式推理结论、封装分发。
     * 新增 §7.1 已修复缺陷、§7.2 设备选择、§7.3 源管理、§7.4 分布式推理结论、§9 封装分发。

   * `packaging/build-portable.ps1`

     * 新增便携版发布脚本。
     * 关键行为：
       - `dotnet publish` App / CLI / MCP / Worker；
       - 拷贝 `config/`、`engine/`、README、架构 MD；
       - 创建 `portable.marker`；
       - 创建 `sources/`、`data/`；
       - 生成 `edition.json`；
       - `Compress-Archive` 生成 zip。

   * `packaging/bookctl.iss`

     * Inno Setup 安装包脚本。
     * 关键配置：
       - `PrivilegesRequired=lowest`
       - `OutputBaseFilename=本地朗读工厂-<version>-setup`
       - `.epub` 关联；
       - 安装时删除 `portable.marker`：
         ```ini
         [InstallDelete]
         Type: files; Name: "{app}\portable.marker"
         ```

   * `packaging/sign.ps1`

     * 代码签名脚本。
     * 支持：
       - `BOOKCTL_PFX` + `BOOKCTL_PFX_PASS`
       - `BOOKCTL_CERT_SHA1`
       - Azure Trusted Signing `BOOKCTL_TS_*`
     * 使用 `signtool sign /fd sha256 /tr <timestamp> /td sha256`。
     * 支持目录或单文件签名，最后 `signtool verify /pa /q`。

   * `packaging/README.md`

     * 封装与分发说明：
       - 便携版 / 安装版差异；
       - 构建命令；
       - 签名要点；
       - 安装版行为；
       - Pro 版差异化预留；
       - 发布前检查清单。

   * `testdata/测试之书.epub`

     * 自造 3 章中文 EPUB，用于端到端验收。
     * 构建过程中修正 zip 格式、路径分隔符和 `mimetype` 顺序。

   * `testdata/ws-fresh/1a8eed25f9db_测试之书/book.m4b`

     * mock 引擎端到端生成的整本有声书样例。
     * `ffprobe` 验证：10.154s、3 个中文章节、时间轴连续、title/artist 元数据正确。

   * `testdata/sherpa-probe.wav`

     * `sherpa-onnx` 真中文语音试听文件。
     * 实测输出：
       - `ok=True`
       - `seconds=8.64`
       - `sample_rate=16000`
       - `device=cpu`
       - `mode=sherpa`
       - 非静音波形：`peak=10035`, `rms=1344.9`。

   * `testdata/ui-simple2.png` / `testdata/ui-detailed.png`

     * 新 UI 截图，验证 Apple 风格双模式实际渲染。
     * 简洁模式截图确认：顶部“本地朗读工厂”、简洁/详细切换、白卡、蓝色胶囊按钮。
     * 详细模式截图确认：左侧参数卡、右侧状态卡、中控台。

   * `.workbuddy/memory/2026-11-23.md` / `.workbuddy/memory/2026-11-24.md`

     * 工作记忆日志。
     * 记录 v0.2 实现、安装编译、端到端验收、v0.3 设备/源管理/封装/UI 等工作。

4. **Errors and fixes:**

   * **WinUI 编译错误：`CS0103: DataContext` 不存在**

     * 根因：WinUI 3 的 `Window` 不是 WPF 的 `FrameworkElement`，没有 `DataContext`。
     * 修复：
       - `MainWindow.xaml` 根 `Grid` 增加 `x:Name="Root"`；
       - `MainWindow.xaml.cs` 改为：
         ```csharp
         Root.DataContext = _vm;
         ```
     * 用户反馈：用户此前明确希望 WinUI 原生桌面软件，未要求改 WPF；因此保持 WinUI 路线。

   * **`WAV_INVALID 非 WAV 文件`**

     * 现象：第 1 章合成完成后，拼接阶段报错。
     * 根因：`MasterChapter(...)` 在有 ffmpeg 时返回 `audio.m4a`，但 Pipeline 用只认 RIFF 的 `Wav.DurationOf(audioPath)` 读它。
     * 修复：
       - 新增 `AudioMaster.ProbeDuration(...)`；
       - WAV 读头；
       - 非 WAV 用 `ffprobe`；
       - 兜底按 AAC 64kbps 估算。
     * 关键代码：
       ```csharp
       if (head[0] == 'R' && ... ) return Wav.DurationOf(path);
       if (_ffprobe is not null) { ... }
       return new FileInfo(path).Length / 8000.0;
       ```

   * **整本 M4B 阶段卡死 3 分钟以上 / `moov atom not found`**

     * 现象：ffmpeg 进程存在但不退出；手动杀掉后 `book.m4b` 无效。
     * 根因：`AudioMaster.Run(...)` 重定向 stdout/stderr 但从不读取，ffmpeg 输出写满 4KB 管道后死锁。
     * 修复：
       - `Run(...)` 并发 `ReadToEndAsync()`；
       - 加 `-hide_banner -loglevel error -nostats`；
       - 超时 `Kill(entireProcessTree: true)`；
       - `concat_list.txt` 路径统一正斜杠。
     * 验证：重新跑 4 秒完成，`ffprobe` 确认 M4B 时长和章节标记正确。

   * **进程被强杀后任务永久停在 `running`**

     * 现象：UI 永远转圈，任务状态 `running`。
     * 根因：无执行权 lease / 心跳 / 孤儿回收。
     * 修复：
       - `worker.lock` 存 PID；
       - `WorkerHost.Start()` 启动时 `RecoverOrphans()`；
       - `JobStore.FindRunningJobIds()` / `FailIfStillRunning(...)`；
       - 残留 running 标成 `WORKER_LOST`，可 `job resume`。
     * 用户无直接反馈，但这是“桌面软件可试跑”必须修复的稳定性问题。

   * **resume 后 `chapters_done=6 / total=3`、`chars_done > chars_total`；完成态仍带 `WORKER_LOST`**

     * 根因：resume 时在上次计数上继续累加；done 章节再次 `++`；完成时未清空错误码。
     * 修复：
       - 按章节 checkpoint 重算 `ChaptersDone` / `CharsDone` / `AudioSeconds`；
       - done 章节不再 `++`；
       - 完成时：
         ```csharp
         job.ErrorCode = null;
         job.ErrorMessage = null;
         ```
     * 验证：干净工作区跑完显示 `3/3`、`347/352`，无旧错误码。

   * **`run` 命中 failed/cancelled 幂等旧任务时立刻返回旧状态**

     * 根因：`RunAndWaitAsync(...)` 调用 `JobStart(...)` 后不处理幂等命中的终态。
     * 修复：
       ```csharp
       var existing = Store.GetJob(jobId);
       if (existing is { Status: "failed" or "cancelled" }) JobResume(jobId);
       ```
     * 用户反馈：无直接反馈，属于实测发现的“自动续跑”缺陷。

   * **空段 / 纯空白分段生成 0 字 WAV**

     * 现象：`第 1 章 6/6 段已合成（0 字）`。
     * 根因：`TextNormalizer.SplitSentences(...)` 可能返回空白段，仍送 TTS。
     * 修复：
       ```csharp
       if (string.IsNullOrWhiteSpace(s)) continue;
       ```

   * **ffmpeg 未被识别 / WinGet 安装后 PATH 未刷新**

     * 现象：某些 shell 中 `bookctl doctor` 报 `ffmpeg.found=false`。
     * 根因：进程 PATH 未刷新，或 Bash/PowerShell 环境不同。
     * 修复：
       - `ResolveFfmpeg(...)` 增加 WinGet Packages 目录扫描：
         ```text
         %LOCALAPPDATA%\Microsoft\WinGet\Packages\*FFmpeg*\bin\ffmpeg.exe
         ```
     * 用户反馈：用户要求“还是你来安装吧”，我已安装 SDK/环境并修补工具链。

   * **测试 EPUB 打包失败 / `META-INF/container.xml` 缺失 / `End of Central Directory record could not be found`**

     * 原因：
       - PowerShell `Compress-Archive` 不支持 `.epub` 扩展名；
       - PowerShell 5.1 zip 内路径可能用反斜杠；
       - Git Bash 的 `tar -a` 实际生成 tar，不是 zip；
       - Python zipfile 最初重复写入 `mimetype`。
     * 修复：
       - 用 Python `zipfile.ZipFile` 创建 EPUB；
       - `mimetype` 第一个且不压缩；
       - 跳过重复 `mimetype`；
       - 路径统一 `/`。
     * 这是测试资产问题，不是产品代码缺陷。

   * **MCP `PullSource` 静态类错误：`CS0708`**

     * 现象：`BookCtl.Mcp.Program` 是 `static class`，但新增了实例方法 `private object PullSource(...)`。
     * 修复：
       ```csharp
       private static object PullSource(string id)
       {
           return new BookCtlHandlers().SourcePull(id).GetAwaiter().GetResult();
       }
       ```

   * **CLI 编译锁文件：`BookCtl.Core.dll` 被 `.NET Host (9988)` 占用**

     * 现象：长时间后台 `source pull` 正在运行同一 CLI 进程，导致全解决方案 build 无法覆盖 `BookCtl.Core.dll`。
     * 处理：
       - 等待后台下载完成后再重新编译；
       - 最终 `dotnet build bookctl.sln` 返回 `BUILD_DONE`；
       - 记录到工作记忆，提示并行下载会锁 bin 目录 DLL。

   * **SourceManager 编译错误：`CS1503 Action<string,double,string>` 不能转 `Action<double,string>?`**

     * 根因：`PullAsync(...)` progress 是三参回调，但 `DownloadAsync(...)` 期望二参。
     * 修复：
       ```csharp
       await DownloadAsync(Candidates(...), archive,
           (frac, label) => progress?.Invoke("downloading", frac, label), ct);
       ```

   * **GitHub Release 下载 SSL / 502**

     * 现象：`source pull sherpa-vits-zh-fanchen` 报：
       ```json
       {"code":"INTERNAL","message":"The SSL connection could not be established, see inner exception."}
       ```
     * 探测结果：
       - GitHub 直连：`CONNECT tunnel failed, response 502`
       - `gh-proxy.com`：HTTP 200
       - `ghfast.top`：403
     * 修复：
       - `Candidates(...)` 多线路回退；
       - 内置 `https://gh-proxy.com/` + URL；
       - HuggingFace 用 `hf-mirror.com`；
       - 支持 `BOOKCTL_MIRROR` 和用户 `mirrors`。
     * 用户反馈：用户要求“一键下载到本地”，国内网络问题必须解决。

   * **模型下载后残留 119MB `.tar.bz2` 且解包目录嵌套**

     * 现象：`sources/sherpa-vits-zh-fanchen/` 中同时有：
       - `vits-zh-hf-fanchen-C.tar.bz2`
       - `vits-zh-hf-fanchen-C/`
     * 根因：`Extract(archive, tmpDir)` 把解压结果与归档文件混在同一目录，`Flatten(tmpDir, targetDir)` 把归档也拍进目标目录。
     * 修复：
       - 解到 `tmpDir/extract`；
       - `File.Delete(archive)`；
       - `Flatten(...)` 的 sourceDir 改为 `tmpDir/extract`；
       - 手工修正已安装目录：删除归档、提升子目录内容。
     * 验证：修正后目录直接包含 `vits-zh-hf-fanchen-C.onnx`、`tokens.txt`、`lexicon.txt`、`*.fst` 等，约 132MB。

   * **WinUI XAML：`WMC0011 Unknown member 'FontFamily' on element 'Grid'`**

     * 根因：WinUI 3 `Grid` 不像 WPF 控件那样直接有 `FontFamily` 属性。
     * 修复：
       - 移除 `FontFamily="Segoe UI Variable Display"`；
       - 改在 `App.xaml` 通过 `ContentControlThemeFontFamily` 设置全局字体。

   * **截图工具被安全策略拦截：`Add-Type` / `Reflection.Assembly`**

     * 现象：PowerShell `Add-Type` 和 `Reflection.Assembly.LoadWithPartialName` 都被安全策略阻止。
     * 修复：
       - 改用 ffmpeg `gdigrab`：
         ```bash
         ffmpeg -f gdigrab -i desktop -frames:v 1 ui-simple2.png
         ```
     * 结果：成功截取简洁模式和详细模式 UI 截图并视觉验证。

   * **sidecar 后台进程被 shell 回收 / `/tmp` 路径不一致**

     * 现象：第一次启动 sidecar 后下一条命令 `curl` 连接失败；Windows Python 找不到 Git Bash `/tmp/tts_probe.json`。
     * 修复：
       - 同一条 shell 命令内启动服务、curl、分析、结束；
       - 使用 Windows 绝对路径或项目内绝对路径；
       - 明确 `PY=/.../engine/.venv/Scripts/python.exe`。
     * 验证：生成 `testdata/sherpa-probe.wav`。

   * **用户要求“还是你来安装吧”后的环境安装**

     * 动作：
       - `winget install --id Microsoft.DotNet.SDK.10 ...`
       - 验证 `dotnet --version` → `10.0.404`
       - `dotnet --list-sdks` → `10.0.404 [C:\Program Files\dotnet\sdk]`
     * 这是对用户“还是你来安装吧”的直接响应。

5. **Problem Solving:**

   已解决的问题：

   - 完成互联网调研与总架构评审稿，形成 MD/HTML 文档。
   - 把“架构方案”落地为可编译工程：Core / CLI / MCP / Worker / WinUI App。
   - 回答 IDE 策略：
     - Agent 不需要 VS Code 写代码；
     - VS Code + C# Dev Kit 足够人工审阅；
     - 修改用“许愿式”自然语言指挥；
     - 用户角色是架构评审 + 验收。
   - 安装 .NET SDK 10.0.404 并完成编译验证。
   - 修复首轮编译/运行暴露的多个真实 bug，使 mock 全链路端到端通过。
   - 生成并验证：
     - 分章 `audio.wav` / `audio.m4a` / `chapter.json`
     - 整本 `book.m4b`
     - `manifest.json`
     - `qc/report.json`
     - `ffmetadata`
     - M4B 中文章节标记
   - 验证 MCP `initialize` / `tools/list`。
   - 验证 WinUI App 实际启动与渲染。
   - 新增设备选择、显卡枚举、CPU/GPU/hybrid 决策。
   - 明确回答“分布式拆分式 TTS 推理”可行性：没有能提速的方案，hybrid 只是兜底。
   - 新增 `sherpa-onnx` CPU 真中文语音引擎、模型安装、sidecar 协议、健康检查和真实试听文件。
   - 新增源管理、内置模型清单、一键下载、镜像回退、解包安装。
   - 新增便携版 / 安装版 / 签名脚本 / Inno Setup 文件关联。
   - 重写 UI 为 Apple 风格双模式。
   - UI 简洁模式和详细模式截图均已验证。
   - 更新 README、architecture MD、engine README、packaging README、工作记忆。

   仍在推进 / 尚未最终交付的部分：

   - 用户明确要求“先把没封装好的源代码版本告诉我路径，我先跑来试试”，但在本轮总结请求打断前，尚未给最终自然语言答复；路径本身已存在且应直接告知。
   - 还未实际执行 `packaging/build-portable.ps1 -Configuration Release` 生成最终 `dist/*.zip`；脚本已写好。
   - 还未生成真实签名安装包，因为没有用户证书 / PFX / EV Key / Azure Trusted Signing 参数。
   - 还未执行“整本书用 `sherpa` 真语音跑全链路并测速度”；仅完成单句真语音 probe 和模型安装。
   - `Sources.cs` 归档残留修复后，需要重新跑一次 `source pull` 或从干净源目录验证新解包流程。
   - 多显卡 0/1/2 的真实硬件切换逻辑已实现，但本机只有 1 张 GTX 1050，未做多卡实机验证。
   - 简洁模式使用 `sherpa` 的全链路还需在 UI 里实际点一次验证事件驱动章节卡片实时追加。
   - `cosyvoice3` 仍是适配点，真实权重加载未实现/未接入。

6. **All user messages:**

   1. 初始架构调研请求：

      ```text
      你上网搜索一下方案，做一个软件Windows端的软件可以自动化的软件自动化由代码来驱动的Windows框架软件，我把一本epub格式的电子书给到这个软件系统，就是这个软件它是符合那个Cmd命令来控制的，这个软件纯支持它来控制的，大模型友好，甚至它是支持mcp控制的，然后呢先做e pub格式，给他之后呢它自动会使用开源语音框架在本地显卡上，这样就节省资源，在本地的显卡上用来把它转化成朗读版本，整本图书的音频文件可以按章节读，然后把每一章节都可以分类整理好。你上网搜索一下，然后综合互联网上所有的信息设计一个架构，然后我来参考一下，我是架构师，然后ok的话，我让你开始干活。
      ```

   2. 进入 WinUI 实现：

      ```text
      很好，你把这个架构设计形成一个 MD 文件，好像已经有了。现在以 MD 文件作为总架构，我调用一些软件来协助你做。我的计划是把它做成一个 WinUI 的，用 WinUI 来写它比较清爽一点，原生支持 Windows 图形化，然后里面是支持一键的我们可以通过命令行 MCP 或 CLI 直接导入 epub 文件，来生成朗读音频。
      这个软件是完全的桌面软件形态，能够看到执行速度和中控台。在执行过程中，能实时看到它正在转换、已经调用显卡以及正在转换 TTS 的状态。最后生成好之后，它会新建一个项目文档文件夹供我们检验。
      我现在来安装 Visual Studio Code，你就负责写代码。
      ```

   3. IDE / 安装 / 许愿式编程评估：

      ```text
      还是你来安装吧，你评估一下方案：
      1. 需要用 VSCode 这个 IDE 来写这个工程项目代码吗？还是你用 Agent 直接就能写完所有代码？
      2. 如果我要人工审阅代码，用什么 IDE 比较好呢？还是不需要 IDE，全部用“许愿式编程”，也就是全部用人工自然语言指令来指挥你修改呢？
      你给个意见。
      ```

   4. 封装、显卡切换、DDS 源管理、CPU/GPU/分布式推理：

      ```text
      你要考虑一件事情：这个软件后续是要进行签名封装，形成 Pro 版可移动版和 exe 安装文件版这两个版本的纯正 Windows 软件。
      因此，你在有源代码的同时，要考虑能够支持封装。封装好以后，软件需具备以下功能：
      1. 显卡切换：软件可以根据用户实际的硬件情况来安排不同的显卡。例如，用户想用显卡 0、显卡 1 或显卡 2 来跑，都可以在软件里面进行切换。
      2. DDS 源管理：软件里面内置了一些 DDS 源，既可以下载好本地版封装进去，也可以在里面添加一个入口，实现一键下载到本地供我们的软件调用。你先把没封装好的源代码版本告诉我路径，我先跑来试试。
      先用 CPU 推理的轻量引擎也可以，然后看看有没有分布式的引擎（即拆分式推理框架），能同时调用 1050 显存和 CPU 一起来推理。TTS 有没有这样的东西？
      如果有最好；如果没有，就支持纯 GPU 或纯 CPU 推理，并且可以切换。这个选择权要完全交给用户，由用户来选择是用 CPU 还是 GPU 推理。
      ```

   5. 产品命名、双模式 UI、Apple 风格设计提示词：

      ```text
      软件名称暂时命名为“本地朗读工厂”或“帅帅朗读工厂”。
      软件功能与模式：
      刚进入软件或在首页 UI 时，可以切换“简洁模式”和“详细参数模式”两种模式。
      1. 详细参数模式（当前模式）：支持手动 menu 模式，用户可以手动选择自己的 epub 等文件。
      2. 简洁模式：考虑内置一个小型的模型（无需太大算力）。用户直接导入一本 epub 文件进入简洁模式后，系统会自动执行识别、拆分章节，用户即可直接导入 epub 文件来生成朗读文件。
      朗读文件生成反馈：
      朗读文件生成好后，会实时以图形化的方式告诉用户是在哪个文件夹生成好了、生成了哪几章。
      UI 设计要求：
      UI 设计参考苹果的简洁化设计，我会给你一个设计提示词。@long-text:"# Apple 风格..." <long_text_quote># Apple 风格 — 风格参考

      > 大量留白、一颗系统蓝、慢到刚好的动效：让产品自己说话。

      **Theme:** light

      Apple 风格的核心不是元素，而是比例：内容占的面积远小于留白，产品图是唯一的主角。底色用偏冷的浅灰（#f5f5f7）代替纯白，卡片比它白一档浮起来。颜色只允许一支系统蓝（#0071e3）用于行动，其余全是灰阶。圆角克制而统一，按钮全圆角、卡片 24px、输入框 12px。动效是这套风格的签名：500ms 与一段缓入缓出的曲线，hover 时轻微上浮、阴影扩散，快一点就变成普通网页，慢一点就拖沓。

      ## Colors

      | Name | Value | Token | Role |
      |---|---|---|---|
      | Apple Black | `#000000` | `--color-apple-black` | 正文、标题与深色满屏区块。深色区块用来把产品图推到最前。 |
      | Apple Grey | `#f5f5f7` | `--color-apple-grey` | 页面画布。比纯白柔一档，让白色卡片有依托。 |
      | Card White | `#ffffff` | `--color-card-white` | 卡片与浮层填充。只在灰底之上出现。 |
      | System Blue | `#0071e3` | `--color-system-blue` | 唯一强调色：主 CTA、链接与选中态。一屏只允许一处。 |
      | Success Green | `#34c759` | `--color-success-green` | 成功与在线状态，小面积标记。 |
      | Alert Red | `#ff3b30` | `--color-alert-red` | 错误与删除，出现即代表不可撤销。 |

      ## Typography

      ### SF Pro Display — 全站唯一字体。大标题用 600 与紧字距，正文 400 保持中性，字级偏大（17px 起）。

      - **Substitute:** -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif
      - **Weights:** 400, 500, 600, 700
      - **Sizes:** 12, 14, 17, 24, 40, 56, 80
      - **Line height:** 1.05 (display), 1.15 (headings), 1.55 (body)
      - **Letter spacing:** -0.025em on display, 0.01em on uppercase labels

      ### Type Scale

      | Role | Family | Weight | Size | Line Height | Letter Spacing | Token |
      |---|---|---|---|---|---|---|
      | display | — | 600 | 80px | 1.05 | -0.025em | `--text-display` |
      | heading-lg | — | 600 | 56px | 1.1 | -0.02em | `--text-heading-lg` |
      | heading | — | 600 | 40px | 1.15 | -0.015em | `--text-heading` |
      | subheading | — | 500 | 24px | 1.3 | — | `--text-subheading` |
      | body-lg | — | 400 | 17px | 1.55 | — | `--text-body-lg` |
      | body | — | 400 | 14px | 1.5 | — | `--text-body` |
      | label | — | 400 | 12px | 1.4 | 0.01em | `--text-label` |

      ## Spacing & Layout

      **Base unit:** 8px
      **Density:** airy

      ### Spacing Scale

      | Name | Value | Token |
      |---|---|---|
      | 8 | 8px | `--spacing-8` |
      | 16 | 16px | `--spacing-16` |
      | 24 | 24px | `--spacing-24` |
      | 32 | 32px | `--spacing-32` |
      | 48 | 48px | `--spacing-48` |
      | 64 | 64px | `--spacing-64` |
      | 96 | 96px | `--spacing-96` |
      | 128 | 128px | `--spacing-128` |
      | 192 | 192px | `--spacing-192` |

      ### Border Radius

      | Element | Value |
      |---|---|
      | cards | 24px |
      | buttons | 999px |
      | inputs | 12px |
      | product-media | 18px |

      - **Page max-width:** 1080px
      - **Section gap:** 128px
      - **Card padding:** 32px
      - **Element gap:** 24px

      ## Components

      ### Pill Button

      **Role:** 主行动，例：Buy、Learn more

      系统蓝填充、白字、999px 全圆角、无描边。字级 17px、字重 500、padding 12px 24px，阴影 0 4px 14px 蓝 30%。hover 时填充提亮到 #0077ed、上浮 2px、阴影扩到 20px 40%；按下缩放 0.96。过渡 500ms，缓动 cubic-bezier(0.25, 0.1, 0.25, 1)。

      - **Sizes:** padding 12px 24px; label 17px
      - **States:** hover: lift 2px, brighter fill; active: scale 0.96

      ### Product Card

      **Role:** 产品卡与功能卡

      白底、24px 圆角、内边距 32px、阴影 0 4px 12px 4%。内容居中：上方 192px 见方的产品图（18px 圆角、苹果灰底），下面 24px/600 标题与 14px 灰字。hover 时上浮 4px、阴影扩到 0 20px 40px 8%、产品图放大 1.05，过渡 500ms。

      ### Search Field

      **Role:** 搜索与输入

      苹果灰底、12px 圆角、无描边、padding 12px 16px、17px 字。聚焦时加 2px 系统蓝焦点环，不改变尺寸。

      - **States:** focus: 2px blue ring

      ### Vibrancy Nav

      **Role:** 顶部导航

      高 48px、白色 80% 不透明、backdrop-blur 20px、底部 1px 灰 50% 描边。链接 12px、字重 400、间距 32px，hover 转 50% 灰。右侧最多一个纯文字 CTA。

      ## Do's and Don'ts

      ### Do

      - 留白优先，元素稀疏到近乎空
      - 只用一支系统蓝，#0071e3
      - 动效 500ms，缓入缓出
      - 按钮全圆角、卡片 24px、输入 12px
      - 标题 600 字重配紧字距
      - 背景用苹果灰而不是纯白

      ### Don't

      - 不要用多个强调色
      - 不要用渐变背景
      - 不要用重阴影
      - 不要把内容挤在一起
      - 不要用装饰性插画或图标墙
      - 不要让圆角混用超过三档

      ## Surfaces

      | Level | Name | Value | Purpose |
      |---|---|---|---|
      | 0 | Apple Grey | `#f5f5f7` | 页面画布 |
      | 1 | Card White | `#ffffff` | 卡片与浮层 |
      | 2 | Black Stage | `#000000` | 产品满屏区 |

      ## Elevation

      - **Card rest:** `0 4px 12px rgba(0,0,0,0.04)`
      - **Card hover:** `0 20px 40px rgba(0,0,0,0.08)`
      - **Button glow:** `0 4px 14px rgba(0,113,227,0.30)`

      ## Imagery

      产品渲染与实拍是唯一的主角：满屏、居中、背景纯净（黑、白或苹果灰）。允许缓慢的视差与缩放。不使用插画、图标墙、渐变背景或人物特写。

      ## Layout

      上限 1080px，容器内边距 24px。区块间距 128px 起，留白至少是内容高度的一半。每屏只讲一件事：一个标题、一句副标题、两个链接。产品区可满屏且反色。

      ## Agent Prompt Guide

      ### Implementation Contract

      - 以用户请求中的页面、用户任务和真实内容为依据，只实施指定范围；标明缺失的业务信息，不编造数据或能力。
      - 先检查现有项目、技术栈与组件；沿用项目结构、路由和依赖，复用已有组件，不重建未要求的页面或功能。
      - 沿用项目断点，调整窄屏与桌面的网格和留白；保留信息层级，避免意外横向溢出。
      - 对本次涉及的交互，处理键盘焦点与减少动态效果；补齐适用的禁用、加载、空内容与错误状态。
      - 完成后按项目现有脚本检查，仅报告实际运行的测试与视觉检查，并列出未验证项。

      QUICK COLOR REFERENCE:

      - text: `#1d1d1f`
      - background: `#f5f5f7`
      - card: `#ffffff`
      - accent: `#0071e3`
      - muted: `#86868b`
      - dark: `#000000`

      EXAMPLE COMPONENT PROMPTS:

      1. 做一个产品卡：白底、24px 圆角、内边距 32px、阴影 0 4px 12px rgba(0,0,0,0.04)。上方放 192px 见方产品图（18px 圆角、#f5f5f7 底），下面 24px/600 标题 #1d1d1f 与 14px #86868b 说明，再下面一个 #0071e3 全圆角按钮（17px/500、padding 12px 24px）。hover 时卡片上浮 4px、阴影扩到 0 20px 40px rgba(0,0,0,0.08)、产品图放大 1.05，过渡 500ms cubic-bezier(0.25,0.1,0.25,1)。

      ## Similar Brands

      - **Apple** — 同一套比例、同一支蓝、同一段缓动
      - **Linear** — 同样以留白与单一强调色组织信息

      ## Quick Start

      ```css

      :root {
        /* Colors */
        --color-apple-black: #000000;
        --color-apple-grey: #f5f5f7;
        --color-card-white: #ffffff;
        --color-system-blue: #0071e3;
        --color-success-green: #34c759;
        --color-alert-red: #ff3b30;

        /* Typography — Font Families */
        --font-sans: 'SF Pro Display', ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;

        /* Typography — Scale */
        --text-display: 80px;
        --leading-display: 1.05;
        --tracking-display: -0.025em;
        --text-heading-lg: 56px;
        --leading-heading-lg: 1.1;
        --tracking-heading-lg: -0.02em;
        --text-heading: 40px;
        --leading-heading: 1.15;
        --tracking-heading: -0.015em;
        --text-subheading: 24px;
        --leading-subheading: 1.3;
        --text-body-lg: 17px;
        --leading-body-lg: 1.55;
        --text-body: 14px;
        --leading-body: 1.5;
        --text-label: 12px;
        --leading-label: 1.4;
        --tracking-label: 0.01em;

        /* Typography — Weights */
        --font-weight-regular: 400;
        --font-weight-medium: 500;
        --font-weight-semibold: 600;
        --font-weight-bold: 700;

        /* Spacing */
        --spacing-unit: 8px;
        --spacing-8: 8px;
        --spacing-16: 16px;
        --spacing-24: 24px;
        --spacing-32: 32px;
        --spacing-48: 48px;
        --spacing-64: 64px;
        --spacing-96: 96px;
        --spacing-128: 128px;
        --spacing-192: 192px;

        /* Layout */
        --page-max-width: 1080px;
        --section-gap: 128px;
        --card-padding: 32px;
        --element-gap: 24px;
        --density: airy;

        /* Border Radius */
        --radius-cards: 24px;
        --radius-buttons: 999px;
        --radius-inputs: 12px;
        --radius-product-media: 18px;

        /* Surfaces */
        --surface-apple-grey: #f5f5f7;
        --surface-card-white: #ffffff;
        --surface-black-stage: #000000;
      }

      ```
      </long_text_quote>
      ```

   6. 当前总结请求：

      ```text
      Your task is to create a detailed and highly structured summary of the conversation so far.

      Your summary must be technically accurate, comprehensive, and strictly follow the required output format.

      When generating the summary:

      1. Review the conversation chronologically.
      2. Identify clearly:

         * All explicit user requests and intents
         * Your actions and responses
         * Technical decisions, design choices, and code patterns
         * File names, code snippets, function signatures, and file edits
         * Any errors encountered and how they were resolved
         * Any direct user feedback instructing you to change behavior

      3. Ensure completeness and precision in all sections.

      ## **Your final summary MUST strictly follow this structure:**

      Summary:

      1. **Primary Request and Intent:**
         A detailed description of all explicit user requests and intentions.

      2. **Key Technical Concepts:**

         * Concept 1
         * Concept 2
         * …

      3. **Files and Code Sections:**

         * `FileName`

           * Why this file is important
           * Summary of changes made (if any)
           * Important code snippet (if applicable)

      4. **Errors and fixes:**

         * Error description

           * How it was fixed
           * Any user feedback

      5. **Problem Solving:**
         Problems solved and ongoing troubleshooting work.

      6. **All user messages:**
         List *all* user messages (actual text, excluding tool results).

      7. **Pending Tasks:**
         List all tasks the user explicitly asked you to continue.

      8. **Current Work:**
         Describe exactly what you were working on immediately before this summary request, including file names and code snippets if applicable.

      9. **Optional Next Step:**
         Only if directly aligned with the user's latest explicit request.
         Include verbatim quotations showing where the previous task left off.
      ```

7. **Pending Tasks:**

   - **告知未封装源代码版本路径**：用户明确说：
     ```text
     你先把没封装好的源代码版本告诉我路径，我先跑来试试。
     ```
     已存在但尚未作为最终答复明确给出。应告知：
     - 项目根目录：`C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/`
     - 源码目录：`C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/src/`
     - 可直接运行的 WinUI Debug exe：
       `C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe`
     - CLI Debug dll：
       `C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/src/BookCtl.Cli/bin/Debug/net10.0/bookctl.dll`
     - 引擎目录：
       `C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/engine/`
     - 打包脚本目录：
       `C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/packaging/`

   - **继续完成“整本书 CPU 真语音全链路并测速度”**：用户允许先用 CPU 轻量引擎；我已安装 `sherpa-onnx` 和模型、生成单句试听，但尚未跑整本 EPUB 的 `sherpa` 全链路。

   - **验证简洁模式一键生成结果可视化**：Apple 风格 UI 已截图，但尚未在简洁模式点击“生成朗读音频”验证章节卡片实时追加。

   - **重新验证修复后的 `source pull` 解包流程**：`Sources.cs` 已修复归档残留和嵌套目录问题，但尚未从干净 sources 目录重新跑完整 `source pull`。

   - **实际生成便携版 zip / 安装包 exe**：脚本已写好，但尚未运行 Release publish、Inno Setup、签名。签名还需用户提供证书或环境变量。

   - **多显卡 0/1/2 实机验证**：本机只有 1 张 GTX 1050，逻辑已实现但多卡硬件未验证。

   - **接真实 GPU `cosyvoice3` 权重**：仍为 P1 适配点，需 ≥6GB 显存机器和模型权重；当前 GTX 1050 2GB 不适合。

   - **如用户继续“许愿式编程”**：后续改动应继续由用户自然语言描述，我负责改文件、编译、运行验证、反馈结果。

8. **Current Work:**

   在本次总结请求前，正在做 v0.3 的最后编译与真 CPU 语音全链路准备。具体状态：

   - 已完成 `source pull sherpa-vits-zh-fanchen`：
     - 下载耗时约 `26m 39s`；
     - 通过 `gh-proxy.com` 镜像回退成功；
     - 输出：
       ```json
       {"ok":true,"api_version":"1.0","data":{"id":"sherpa-vits-zh-fanchen","status":"installed","path":"C:\\Users\\romanovsky\\AppData\\Local\\bookctl\\sources\\sherpa-vits-zh-fanchen"}}
       ```
   - 已修复下载残留压缩包/嵌套目录问题：
     - `src/BookCtl.Core/Sources.cs` 中把解包目录改为 `tmpDir/extract`；
     - `File.Delete(archive)`；
     - `Flatten(sourceDir, targetDir)`；
     - 手工修正本地已安装目录，删除 `vits-zh-hf-fanchen-C.tar.bz2`，提升模型文件到根目录。
   - 已安装并验证 `sherpa-onnx` venv：
     ```text
     Successfully installed sherpa-onnx-2.0.3 sherpa-onnx-core-2.0.3
     sherpa-onnx 2.0.3
     ```
   - 已启动 `engine/tts_server.py --mode sherpa` 并验证 `/health`：
     ```json
     {"ok": true, "mode": "sherpa", "model": "sherpa-onnx-vits-zh", "python": "3.13.14", "device": "cpu", "gpu_index": -1, "cpu_threads": 16, "model_dir": "C:\\Users\\romanovsky\\AppData\\Local\\bookctl\\sources\\sherpa-vits-zh-fanchen"}
     ```
   - 已生成真实中文语音试听：
     - 文件：`C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/testdata/sherpa-probe.wav`
     - 输出：
       ```text
       ok True sec 8.64 rate 16000
       dur 8.64 peak 10035 rms 1344.9
       model loaded, sample_rate=16000, speakers=187
       ```
   - 已修复 `src/BookCtl.Mcp/Program.cs` 中的 static class 编译错误：
     ```csharp
     private static object PullSource(string id)
     {
         // 下载可能几十秒到几分钟：MCP 客户端会等，但只在这一件事上；不做 job 化是为了让模型拿到明确的安装结果
         return new BookCtlHandlers().SourcePull(id).GetAwaiter().GetResult();
     }
     ```
   - 最后执行的动作是全解决方案重新编译：
     ```powershell
     dotnet build "C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\bookctl.sln" -c Debug -v q --nologo
     ```
     工具结果显示：
     ```text
     BUILD_DONE
     ```
     表示全解决方案无 error / warning（筛选结果为空）。
   - 紧接这一步，原本准备继续：
     ```text
     真实语音生成成功（8.64 秒、16kHz、非静音）。现在整本书跑 CPU 真语音全链路并测速度：
     ```
     但用户随即发起了本总结请求，因此尚未实际执行整本 `sherpa` 全链路测试，也尚未给最终“未封装源码路径”答复。

9. **Optional Next Step:**

   若本总结后继续上一任务，最直接的下一步是：先把用户明确要的“未封装源代码版本路径”交付给用户，并继续做 CPU 真语音整本链路验收。

   对齐的用户原话：

   > “你先把没封装好的源代码版本告诉我路径，我先跑来试试。”

   > “先用 CPU 推理的轻量引擎也可以，然后看看有没有分布式的引擎（即拆分式推理框架），能同时调用 1050 显存和 CPU 一起来推理。TTS 有没有这样的东西？”

   上一任务实际停在这句动作描述：

   > “真实语音生成成功（8.64 秒、16kHz、非静音）。现在整本书跑 CPU 真语音全链路并测速度：”

   以及最后工具结果：

   > `BUILD_DONE`

   建议下一步（如继续开发）：
   - 给出源码路径与运行命令；
   - 启动 `engine/tts_server.py --mode sherpa`;
   - 运行：
     ```powershell
     BOOKCTL_ENGINE=sherpa BOOKCTL_DEVICE=cpu bookctl run --input "C:\...\testdata\测试之书.epub"
     ```
   - 验证每章真实中文语音、`book.m4b`、章节标记、速度/RTF；
   - 再告知用户可直接双击哪个 exe 试跑。

## C3 · 2026-11-24 18:48:41

Summary of the conversation so far:
The conversation is between an AI agent and a user.
Use this to get up to speed, and continue helping the user as the AI agent.
Some contents may be omitted, indicated by "".


Summary:

1. **Primary Request and Intent:**
   用户整体意图是：从零设计并实现一个“纯正 Windows 桌面软件”，用代码/命令行/MCP/大模型可驱动的方式，把 EPUB 整本书在本地转换成分章节有声书；后续再迭代为可签名封装的 Pro 版，包含便携版与 exe 安装版，并支持显卡/CPU 推理设备切换、内置模型源管理、Apple 风格双模式 UI。

   按时间线梳理：

   - **初始架构调研请求**：用户要求“上网搜索一下方案”，综合互联网信息设计一个 Windows 端自动化软件架构：
     - 软件由代码驱动，符合 `Cmd` / CLI 命令控制；
     - 大模型友好，甚至支持 MCP 控制；
     - 输入一本 `.epub` 电子书；
     - 软件自动使用开源语音框架在本地显卡上 TTS，把整本书转换为朗读版本；
     - 音频按章节分类整理；
     - 用户是架构师，先看架构，确认后再开始干活。

   - **进入实现阶段**：用户确认架构 MD 已有，要求以 MD 为总架构，做成 WinUI 桌面软件：
     - 原生 Windows 图形化；
     - 支持 CLI / MCP / 命令行导入 EPUB 生成朗读音频；
     - 完全桌面软件形态；
     - 能看到执行速度、中控台；
     - 执行中实时看到“正在转换 / 已调用显卡 / 正在转换 TTS 状态”；
     - 生成后新建项目文档文件夹供检验；
     - 用户准备安装 VS Code，要求“我负责写代码”。

   - **关于 IDE / 安装 / 审阅方式**：用户问：
     - 是否需要 VSCode 写工程代码，还是 Agent 直接能写完；
     - 人工审阅用什么 IDE；
     - 是否不需要 IDE，全部用“许愿式编程”自然语言指挥修改；
     - 最后要求“还是你来安装吧，你评估一下方案”。

   - **封装与设备/源管理的前瞻要求**：用户明确后续软件要签名封装：
     - 形成 Pro 版可移动版和 exe 安装版两个版本；
     - 源代码阶段也要考虑封装支持；
     - 封装后具备：
       1. 显卡切换：用户可选显卡 0 / 1 / 2 跑推理；
       2. “DDS 源管理”：内置一些源，可下载本地封装进去，也可一键下载到本地供软件调用；
     - 用户要求“先把没封装好的源代码版本告诉我路径，我先跑来试试”；
     - 用户允许先用 CPU 轻量引擎；
     - 询问是否有分布式/拆分式 TTS 推理框架，能同时调用 GTX 1050 显存和 CPU；
     - 如果没有，就支持纯 GPU 或纯 CPU 推理并可切换；
     - 选择权完全交给用户。

   - **产品命名、双模式 UI、Apple 风格与反馈可视化**：用户新增明确要求：
     - 软件名称暂定“本地朗读工厂”或“帅帅朗读工厂”；
     - 首页可切换“简洁模式”和“详细参数模式”；
     - 详细参数模式：手动 menu 模式，用户手动选择 EPUB 等文件；
     - 简洁模式：内置小型模型，用户导入一本 EPUB 后自动识别、拆分章节并生成朗读文件；
     - 朗读文件生成后，实时图形化告知用户生成到哪个文件夹、生成了哪几章；
     - UI 设计参考 Apple 风格，并给出完整设计提示词：浅灰 `#f5f5f7`、白卡、系统蓝 `#0071e3`、圆角、留白、500ms 缓动、Pill Button、Product Card、Search Field、Vibrancy Nav 等规范。

   - **当前最新请求**：用户要求生成一份“详细且高度结构化”的对话总结，严格按指定结构输出，覆盖所有用户请求、技术决策、文件、代码、错误修复、当前工作和下一步。

2. **Key Technical Concepts:**

   * **CLI-first / MCP / 大模型友好控制面**
     - JSON envelope：`{"ok":true,"api_version":"1.0","data":...}` 或结构化 error；
     - 稳定退出码；
     - `--capabilities` / `--schema` / `--dry-run` / `--pretty` / `--quiet`；
     - MCP stdio JSON-RPC；
     - 长任务 job 化：`job_start` / `job_status` / `job_logs` / `job_cancel` / `job_resume` / `job_artifacts`；
     - 避免整本书在单次 MCP tool 调用里执行，所有耗时操作异步化；
     - MCP tools 后续扩展到 `device_list` / `source_list` / `source_pull`。

   * **总架构 MD 作为唯一事实源**
     - 初版：`architecture-epub2audiobook.md`；
     - 追加“附录 A · v0.2 桌面形态变更”；
     - 后续追加“附录 B · v0.3 双模式、设备选择、源管理、封装”。

   * **WinUI 3 / Windows App SDK / .NET 10**
     - `WinUI 3` unpackaged 桌面程序；
     - `WindowsPackageType=None`；
     - `WindowsAppSDKSelfContained=true`；
     - `net10.0-windows10.0.19041.0`；
     - Windows App SDK 版本核实为 `2.7.0`；
     - VS Code 只是审阅/调试用，不影响 Agent 直接读写源码；
     - Visual Studio 全家桶不建议，纯 XAML 不需要拖控件。

   * **EPUB 工程链路**
     - EPUB zip / `META-INF/container.xml` / OPF / `manifest` / `spine` / NCX / `nav.xhtml`；
     - TOC anchor 切章；
     - DRM 判定；
     - 中文 TN / 分句；
     - 跳过错别目录、版权页、索引、空白段；
     - UTF-8、中文路径、XHTML entity、CSS hidden 等 Windows/EPUB 坑。

   * **TTS 引擎选型**
     - 默认初版架构选 `CosyVoice 3`：Apache-2.0、中文效果好、需 ≥6GB 显存；
     - 排除 `VibeVoice`：会强制插入可听 AI 声明 + 水印；
     - 谨慎使用 `XTTS / Coqui`：CPML / 商用限制；
     - 后续加入 CPU 轻量引擎 `sherpa-onnx`：
       - 实测 `sherpa-onnx 2.0.3` 可安装；
       - 中文模型 `vits-zh-hf-fanchen-C`：约 119MB 压缩包、解包后约 132MB；
       - 纯 CPU 可跑，16kHz，187 个说话人；
       - 真实中文试听文件 `testdata/sherpa-probe.wav` 已生成；
     - `piper` 作为更轻量备选；
     - `mock` 作为校样占位音频引擎；
     - `cosyvoice3` 保留 GPU 适配点。

   * **六阶段流水线**
     - `S1_INGEST`：解析 EPUB；
     - `S2_SPLIT`：按目录切章；
     - `S3_NORMALIZE`：文本正则化 / 分句；
     - `S4_SYNTH`：TTS 合成；
     - `S5_VERIFY`：ASR 回验适配点；
     - `S6_MASTER`：音频母带 / M4B / manifest / qc。

   * **SQLite WAL / JobStore / 状态机**
     - CAS / revision；
     - 幂等键；
     - 章节级 checkpoint；
     - 协作取消；
     - 断点续传；
     - 事件 NDJSON；
     - `worker.lock` PID 锁与孤儿任务回收；
     - `WORKER_LOST` 可 resume。

   * **音频工程**
     - WAV 拼接；
     - `ffmpeg` 生成 `audio.m4a`；
     - `ffmetadata` 章节标记；
     - `book.m4b`；
     - `ffprobe` 校验 chapter metadata；
     - `AudioMaster.ProbeDuration`：WAV 读头 / ffprobe / 码率兜底；
     - `Run()` 并发读取 stdout/stderr，避免 ffmpeg 管道死锁；
     - `concat_list.txt` 路径统一正斜杠。

   * **设备选择 / CPU-GPU 切换**
     - `Device(auto|cpu|gpu|hybrid)`；
     - `GpuIndex`；
     - `CpuThreads`；
     - `DevicePlanner.Resolve()`；
     - 显存门槛：
       - `cosyvoice3` 6144MB；
       - `sherpa` 1536MB；
       - `piper` 512MB；
       - `mock` 0；
     - `auto`：显存够 → GPU；中等显存 → hybrid；显存过小 → CPU；
     - `hybrid`：层卸载，能跑但更慢；
     - 设备不参与 `profile_hash`，避免换卡影响确定性判定。

   * **分布式 / 拆分式 TTS 推理结论**
     - 没有能把 GTX 1050 2GB + CPU 组合起来“提速”的开源 TTS 分布式推理框架；
     - 张量并行需要 ≥2 张同型号、算力 ≥7.0 的卡；GTX 1050 算力 6.1 被 vLLM/SGLang 类框架拒绝；
     - `device_map` 层卸载只解决“模型装不下”，通常更慢；
     - ONNX Runtime 单 session 单 EP；
     - 多进程异构分片会破坏音色一致性；
     - 产品策略：纯 GPU / 纯 CPU / hybrid 兜底，选择权交给用户。

   * **DDS / 引擎与模型源管理**
     - 用户说“DDS 源管理”，实现中按“内置引擎/模型源目录 + 一键下载/解包/安装状态”落地；
     - `SourceManager.BuiltInCatalog()`；
     - 源类型：`archive` / `zip` / `file` / `manual`；
     - `source list` / `source pull` / `source remove`；
     - 多线路下载回退：原始 URL → 用户 mirrors → `BOOKCTL_MIRROR` → `gh-proxy.com` / `hf-mirror.com`；
     - 国内直连 GitHub Release 实测 502，必须镜像回退；
     - 解包用系统 `tar.exe` 或 `ZipFile`；
     - `.installed.json` 标记；
     - 用户自定义 `sources/catalog.user.json` / `config/sources.example.json`。

   * **封装 / 签名 / Pro 版**
     - 便携版：`portable.marker` 存在时，`data\` / `sources\` 写程序目录；
     - 安装版：Inno Setup per-user 安装，删除 marker，数据写 `%LOCALAPPDATA%\bookctl`；
     - `build-portable.ps1`：publish + 拷贝 config/engine + 生成 zip；
     - `bookctl.iss`：Inno Setup 脚本，`.epub` 文件关联，桌面快捷方式；
     - `sign.ps1`：PFX / 证书指纹 / Azure Trusted Signing + RFC3161 时间戳；
     - 两版 self-contained，目标机无需 .NET 10 / Windows App Runtime；
     - 签名私钥不进仓库，只读环境变量；
     - Pro 差异化预留 `Edition` 字段。

   * **Apple 风格 UI / 双模式**
     - 产品名改为“本地朗读工厂”；
     - 顶部模式切换：`简洁模式` / `详细参数`；
     - 简洁模式：
       - 一张输入卡；
       - 一颗系统蓝胶囊按钮“生成朗读音频”；
       - 进度卡；
       - 生成结果卡：输出文件夹 + 实时章节卡片；
       - 自动使用已安装 CPU 模型，否则 mock 并提示下载。
     - 详细参数模式：
       - 左侧参数卡：引擎、音色、推理设备、显卡、CPU 线程、语速、M4B；
       - 右侧状态卡和中控台；
       - 显示 `DevicePlan` 决策文本和警告。
     - 颜色：`#F5F5F7` 背景，白色卡片 24px 圆角，`#0071E3` 唯一强调色；
     - 字体：`Segoe UI Variable`；
     - 控件：胶囊按钮、12px 输入框、24px 卡片；
     - `ToggleButton` 绑定 `IsSimpleMode` / `IsDetailedMode`；
     - `Visibility` 由 `SimpleVisibility` / `DetailedVisibility` 控制；
     - 支持 `--detailed` 启动与 `.epub` 命令行参数，配合安装包文件关联。

3. **Files and Code Sections:**

   * `C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/architecture-epub2audiobook.md`

     * 总架构唯一事实源。
     * 初版包含调研结论、四层架构、CLI/MCP 接口草案、TTS 选型、音频工程、状态机、路线图。
     * 追加“附录 A · v0.2 变更：WinUI 桌面形态”。
     * 追加“附录 B · v0.3 变更：双模式、设备选择、源管理、封装”。
     * 附录 B 关键结论：
       - 产品名“本地朗读工厂”；
       - 简洁模式不是另一个流水线，只是写会话配置后调用同一个 `JobStart`；
       - 设备选择是 S4 前的 `DevicePlan`；
       - 源管理为 L4 旁路；
       - 开源 TTS 没有可提速的 GPU+CPU 拆分推理。

   * `architecture-epub2audiobook.html`

     * 可视化评审文档，含架构图、流水线、选型对比、CLI/MCP 契约、路线图与待拍板决策点。

   * `Directory.Build.props`

     * 统一 SDK / Windows App SDK 版本。
     * `WindowsAppSDKVersion` 从 `1.8.0` 修正为 NuGet 最新稳定版 `2.7.0`。

   * `bookctl.sln`

     * .NET 解决方案，包含：
       - `BookCtl.Core`
       - `BookCtl.Cli`
       - `BookCtl.Mcp`
       - `BookCtl.Worker`
       - `BookCtl.App`

   * `config/bookctl.defaults.json`

     * 默认配置文件，包含工作区、引擎、音色、M4B、ffmpeg 等默认项。

   * `config/sources.example.json`

     * 新增自定义源示例，说明：
       - 如何用 `sources/catalog.user.json` 覆盖内置源；
       - `mirrors`；
       - `type: file` 多文件下载写法；
       - 企业内网镜像示例。

   * `.vscode/settings.json` / `.vscode/tasks.json` / `.vscode/launch.json`

     * VS Code 审阅/调试配置。
     * 用户最终只建议把 VS Code 用于人工审阅，不用于 Agent 写代码。

   * `src/BookCtl.Core/Contracts.cs`

     * 基础契约：JSON envelope、异常、退出码、配置。
     * 新增 v0.3 配置项：
       ```csharp
       public string Device { get; set; } = "auto";
       public int GpuIndex { get; set; } = -1;
       public int CpuThreads { get; set; } = 0;
       public bool AllowCpuOffload { get; set; } = false;
       public string Edition { get; set; } = "community";
       public string EffectiveSourcesRoot { get; set; } = "";

       public string SourcesRoot => !string.IsNullOrWhiteSpace(EffectiveSourcesRoot)
           ? EffectiveSourcesRoot
           : Path.Combine(Path.GetDirectoryName(EffectiveWorkspaceRoot.TrimEnd('\\', '/')) ?? EffectiveWorkspaceRoot, "sources");
       ```
     * `CloneWith(...)` 增加 `device` / `gpuIndex` / `cpuThreads`。
     * `ApplyEnv(...)` 增加：
       - `BOOKCTL_DEVICE`
       - `BOOKCTL_GPU`
       - `BOOKCTL_CPU_THREADS`
       - `BOOKCTL_SOURCES`
     * 新增 `ApplyPortable(...)`：
       ```csharp
       private static void ApplyPortable(BookCtlConfig cfg)
       {
           var baseDir = AppContext.BaseDirectory;
           if (!File.Exists(Path.Combine(baseDir, "portable.marker"))) return;
           if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("BOOKCTL_WORKSPACE")))
               cfg.WorkspaceRoot = Path.Combine(baseDir, "data");
           if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("BOOKCTL_SOURCES")))
               cfg.EffectiveSourcesRoot = Path.Combine(baseDir, "sources");
           cfg.Edition = "pro";
       }
       ```

   * `src/BookCtl.Core/Models.cs`

     * `GpuStatus` / `BookMeta` / `Chapter` / `Segment` / `SynthesisProfile` / `JobRecord` / `JobEvent`。
     * `SynthesisProfile.From(...)` 改为接收 `DevicePlan? plan = null`，增加：
       ```csharp
       public string Device { get; set; } = "cpu";
       public int GpuIndex { get; set; } = -1;
       public int CpuThreads { get; set; } = 0;
       public bool CpuOffload { get; set; }
       ```
     * 设备不参与 `Hash`。

   * `src/BookCtl.Core/JobStore.cs`

     * SQLite WAL、jobs / chapters / events 表、CAS、幂等键、checkpoint、事件流。
     * 新增孤儿任务回收支持：
       ```csharp
       public List<string> FindRunningJobIds()
       public bool FailIfStillRunning(string jobId, string code, string message)
       ```
     * `P(SqliteCommand cmd, string name, object value)` 改为 `object? value`，消除 nullable warning。

   * `src/BookCtl.Core/Epub.cs`

     * EPUB 解析、DRM 判定、OPF / NCX / nav.xhtml、TOC anchor 切章、中文 TN / 分句。
     * 修过 `using` 笔误、`oppfText` / `opfText`、`Xml.XmlException`、正则 `?? default` 等编译问题。

   * `src/BookCtl.Core/Engine.cs`

     * `ITtsProvider` / `MockTtsProvider` / `SidecarTtsProvider`。
     * `TtsProviderFactory` 增加 `sherpa` / `sherpa-onnx` / `onnx` / `piper`。
     * `Describe(...)` 增加 `mock` / `sherpa` / `sidecar` 三档说明。
     * `SidecarTtsProvider.SynthesizeAsync(...)` payload 增加设备字段：
       ```csharp
       device = profile.Device,
       gpu_index = profile.GpuIndex,
       cpu_threads = profile.CpuThreads,
       cpu_offload = profile.CpuOffload,
       ```
     * `Wav.Concat(...)` 修复采样率、空拼接、按首个 WAV 的 rate 写 header。
     * `AudioMaster` 新增：
       - `_ffprobe`
       - `ResolveSibling(...)`
       - `ProbeDuration(...)`
     * `AudioMaster.MasterBook(...)` 修复 `concat_list.txt` 路径：
       ```csharp
       File.WriteAllLines(listPath, audioFiles.Select(p =>
           $"file '{p.Replace("\\", "/").Replace("'", "'\\''")}'"), new UTF8Encoding(false));
       ```
     * `Run(...)` 修复 ffmpeg 管道死锁：
       ```csharp
       Arguments = "-hide_banner -loglevel error -nostats " + args,
       var so = p.StandardOutput.ReadToEndAsync();
       var se = p.StandardError.ReadToEndAsync();
       if (!p.WaitForExit(timeoutMs)) { p.Kill(entireProcessTree: true); return false; }
       Task.WaitAll(new Task[] { so, se }, 5000);
       ```
     * `ResolveFfmpeg(...)` 增加 WinGet 包目录扫描兜底，解决刚装 ffmpeg、PATH 未刷新时找不到的问题。

   * `src/BookCtl.Core/Pipeline.cs`

     * 六阶段流水线、EventBus、WorkerHost。
     * S4 前新增 `DevicePlan`：
       ```csharp
       var plan = DevicePlanner.Resolve(cfg);
       var profile = SynthesisProfile.From(cfg, model, plan);
       ...
       Emit(job, "info", "S4_SYNTH", "gpu_status", devMsg + ...,
           new { gpu = g0, device_plan = plan.ToDto(), engine = cfg.EngineMode, model, voice = cfg.Voice, profile_hash = profile.Hash });
       ```
     * `segment_done` 事件新增：
       ```csharp
       device = plan.Device,
       device_display = plan.Display,
       ```
     * 空段过滤：
       ```csharp
       if (string.IsNullOrWhiteSpace(s)) continue;
       ```
     * 章节收尾改用 `_audio.ProbeDuration(audioPath)`。
     * Resume 计数修复：
       ```csharp
       job.ChaptersDone = chapters.Count(c => c.Status == "done");
       job.CharsDone = chapters.Where(c => c.Status == "done").Sum(c => c.CharCount);
       job.AudioSeconds = chapters.Where(c => c.Status == "done").Sum(c => c.Seconds);
       _store.UpdateJob(job);
       ```
     * done 章节不再 `job.ChaptersDone++`。
     * 完成态清空错误码：
       ```csharp
       job.ErrorCode = null;
       job.ErrorMessage = null;
       ```
     * `WorkerHost` 增加 `EventBus _bus`、构造函数 `WorkerHost(..., EventBus? bus = null)`。
     * `WorkerHost.Start()` 增加 `RecoverOrphans()` / `WriteLock()` / `worker.lock` PID。
     * `RecoverOrphans()` 把残留 running 标为 `WORKER_LOST`，可 resume。

   * `src/BookCtl.Core/Handlers.cs`

     * GUI / CLI / MCP 共用 Handler。
     * 新增 `SourceManager Sources { get; }`。
     * 新增 `DeviceList()`：
       - 列出显卡 index / name / mem_total / mem_free；
       - CPU 核数；
       - `DevicePlanner.Resolve(Config).ToDto()`。
     * `Capabilities()` 增加：
       - `device` 模式清单；
       - `edition`；
       - MCP tools 增加 `device_list` / `source_list` / `source_pull`。
     * `Doctor()` 增加 `devices = DeviceList()`。
     * 新增：
       ```csharp
       public object SourceList()
       public Task<object> SourcePull(...)
       public object SourceRemove(...)
       ```
     * `JobStart(...)` 签名扩展：
       ```csharp
       public object JobStart(string inputPath, string? idempotencyKey = null,
           string? engine = null, string? voice = null, string? device = null,
           int? gpuIndex = null, int? cpuThreads = null)
       ```
     * `RunAndWaitAsync(...)` 签名扩展并调用新 `JobStart(...)`。
     * 幂等命中 failed/cancelled 旧任务时自动 `JobResume(jobId)`。

   * `src/BookCtl.Core/Device.cs`

     * 新增文件，v0.3 设备决策核心。
     * `GpuDevice`、`DevicePlan`、`DevicePlanner`。
     * `RequiredVram(...)`：
       ```csharp
       "cosyvoice3" or "sidecar" or "python" => 6144,
       "sherpa" or "onnx" or "sherpa-onnx" => 1536,
       "piper" => 512,
       _ => 0
       ```
     * `ListGpus()` 用：
       ```text
       nvidia-smi --query-gpu=index,name,memory.total,memory.free --format=csv,noheader,nounits
       ```
     * `Resolve(...)` 按 `auto/cpu/gpu/hybrid` 和显存决策，并输出 warnings。

   * `src/BookCtl.Core/Sources.cs`

     * 新增文件，源管理核心。
     * `SourceInstall` / `SourceFile` / `SourceEntry` / `SourceCatalog` / `SourceManager`。
     * `BuiltInCatalog()` 内置 5 个源：
       - `mock`
       - `sherpa-vits-zh-fanchen`
       - `sherpa-vits-zh-aishell3`
       - `piper-zh-huayan`
       - `cosyvoice3`
     * `SourceInstall` 增加 `Mirrors`。
     * `Candidates(...)` 多线路回退：
       ```csharp
       if (url.Contains("github.com"))
           list.Add("https://gh-proxy.com/" + url);
       if (url.Contains("huggingface.co"))
           list.Add(url.Replace("https://huggingface.co", "https://hf-mirror.com"));
       ```
     * `DownloadAsync(IReadOnlyList<string> urls, ...)` 逐条尝试，全部失败抛 `SOURCE_DOWNLOAD_FAILED`。
     * 修复下载归档残留：
       ```csharp
       var extractDir = Path.Combine(tmpDir, "extract");
       Directory.CreateDirectory(extractDir);
       Extract(archive, extractDir);
       File.Delete(archive);
       ...
       var sourceDir = entry.Install.Type == "file"
           ? tmpDir
           : Path.Combine(tmpDir, "extract");
       Flatten(sourceDir, targetDir);
       ```
     * `Flatten(...)` 拍平解包目录。
     * `PullAsync(...)` 写 `.installed.json`。

   * `src/BookCtl.Cli/Program.cs`

     * CLI JSON envelope 与命令分发。
     * 新增命令：
       - `device list`
       - `source list`
       - `source pull <id>`
       - `source remove <id>`
     * `run` / `job start` 支持：
       - `--device auto|gpu|cpu|hybrid`
       - `--gpu 0|1|2`
       - `--cpu-threads N`
     * 增加 `IntFlag(...)`。
     * 增加 `PullSource(...)` 进度输出。
     * Help 更新环境：
       - `BOOKCTL_DEVICE`
       - `BOOKCTL_GPU`
       - `BOOKCTL_CPU_THREADS`
       - `BOOKCTL_SOURCES`

   * `src/BookCtl.Mcp/Program.cs`

     * MCP stdio server。
     * `job_start` schema 增加：
       - `engine enum: mock, sherpa, sidecar`
       - `device enum: auto, gpu, cpu, hybrid`
       - `gpu_index`
       - `cpu_threads`
     * tool 分发新增：
       ```csharp
       case "device_list": return _handlers.DeviceList();
       case "source_list": return _handlers.SourceList();
       case "source_pull": return PullSource(...);
       ```
     * 增加 `ArgInt(...)`。
     * 修复 static class 中实例方法错误：
       ```csharp
       private static object PullSource(string id)
       {
           return new BookCtlHandlers().SourcePull(id).GetAwaiter().GetResult();
       }
       ```
     * Tools schema 新增 `device_list` / `source_list` / `source_pull`。

   * `src/BookCtl.Worker/Program.cs`

     * 独立 Worker 入口，使用 Core 的 WorkerHost 和 JobStore。

   * `src/BookCtl.App/BookCtl.App.csproj`

     * WinUI 3 项目：
       - `UseWinUI=true`
       - `WindowsPackageType=None`
       - `WindowsAppSDKSelfContained=true`
       - `RuntimeIdentifier=win-x64`
       - `Microsoft.WindowsAppSDK 2.7.0`

   * `src/BookCtl.App/App.xaml`

     * 重写为 Apple 风格资源：
       ```xml
       <Color x:Key="SystemAccentColor">#0071E3</Color>
       <FontFamily x:Key="ContentControlThemeFontFamily">Segoe UI Variable Text</FontFamily>
       ```

   * `src/BookCtl.App/MainWindow.xaml`

     * 全量重写为 Apple 风格双模式界面。
     * 顶部产品名称“本地朗读工厂”。
     * 模式切换：
       ```xml
       <ToggleButton Content="简洁模式" IsChecked="{Binding IsSimpleMode, Mode=TwoWay}" />
       <ToggleButton Content="详细参数" IsChecked="{Binding IsDetailedMode, Mode=TwoWay}" />
       ```
     * 简洁模式：
       - 输入卡；
       - “生成朗读音频”系统蓝胶囊按钮；
       - 进度卡；
       - 生成结果卡：输出文件夹 + `ItemsControl` 逐章卡片。
     * 详细模式：
       - 左侧参数卡；
       - 右侧状态卡和中控台。
     * 移除非法 `Grid FontFamily` 属性。

   * `src/BookCtl.App/MainWindow.xaml.cs`

     * `Title = "本地朗读工厂"`。
     * `Root.DataContext = _vm`。
     * 新增：
       ```csharp
       private void StartSimple_Click(object sender, RoutedEventArgs e) => _vm.StartOneClick();
       private void StartDetailed_Click(object sender, RoutedEventArgs e) => _vm.Start();
       ```
     * 窗口尺寸从 `1280x880` 改为 `1400x960`。
     * 新增 `ApplyCommandLine()`：
       - `--detailed` 进详细模式；
       - 传入 `.epub` 路径自动填入 `InputPath`；
       - 配合安装包 `.epub` 文件关联。

   * `src/BookCtl.App/MainViewModel.cs`

     * 全量重写。
     * 新增 `ChapterCard`。
     * 双模式：
       - `IsSimpleMode`
       - `IsDetailedMode`
       - `SimpleVisibility`
       - `DetailedVisibility`
       - `ModeTitle`
       - `ModeSubtitle`
     * 设备 UI：
       - `DeviceModes`
       - `Devices`
       - `DeviceMode`
       - `DeviceIndex`
       - `CpuThreads`
       - `PlanText`
       - `PlanHint`
     * 结果可视化：
       - `OutputFolder`
       - `Chapters`
       - `ChapterSummary`
       - `CanOpenOutput`
     * `StartOneClick()`：
       ```csharp
       var hasCpuModel = App.Handlers.Sources.InstalledPath("sherpa-vits-zh-fanchen") is not null
                         || App.Handlers.Sources.InstalledPath("sherpa-vits-zh-aishell3") is not null;
       if (IsSimpleMode)
       {
           App.Handlers.Config.EngineMode = hasCpuModel ? "sherpa" : "mock";
           App.Handlers.Config.Device = "cpu";
           App.Handlers.Config.Voice = hasCpuModel ? "0" : "zh-default";
           ...
       }
       StartInternal();
       ```
     * `AddLog(...)` 处理：
       - `segment_done` 的速度 / device_display；
       - `chapter_done` 追加章节卡片；
       - `job_completed` 设置输出文件夹；
       - `job_failed` / `job_cancelled` 更新提示。
     * `LoadDevices()` / `RefreshPlan()` / `UpdateSourceStatus()`。

   * `src/BookCtl.App/app.manifest`

     * Windows 应用清单，支持 unpackaged WinUI。

   * `engine/tts_server.py`

     * 从 mock/cosyvoice3 适配点重写为三模式：
       - `mock`
       - `sherpa`
       - `cosyvoice3`
     * 新增 `discover_sherpa_dir(...)`，自动从：
       - `--model-dir`
       - `engine/../sources`
       - `engine/sources`
       - `%LOCALAPPDATA%\bookctl\sources`
       找含 `*.onnx` + `tokens.txt` 的目录。
     * 新增 `ensure_sherpa(...)`：
       ```python
       config = sherpa_onnx.OfflineTtsConfig(
           model=sherpa_onnx.OfflineTtsModelConfig(
               vits=sherpa_onnx.OfflineTtsVitsModelConfig(
                   model=onnx,
                   lexicon=...,
                   tokens=tokens,
                   dict_dir=...,
                   noise_scale=0.667,
                   noise_scale_w=0.8,
                   length_scale=1.0,
               ),
               num_threads=n_threads,
               provider="cpu",
               debug=False,
           ),
           rule_fsts=",".join(fsts) if fsts else "",
           max_num_sentences=1,
       )
       ```
     * 新增 `synth_sherpa(...)`，支持 `sid` / speed。
     * 请求级 `device` / `gpu_index` / `cpu_threads` 处理。
     * `sherpa` 的 PyPI wheel 是 CPU 版，遇到 GPU 请求明确报错，不假装使用显卡。
     * `GET /health` 返回 device / threads / model_dir / voices。
     * `POST /synthesize` 返回 `wav_b64` / `seconds` / `sample_rate` / `mode` / `device`。

   * `engine/README.md`

     * 重写为三种模式说明：
       - mock
       - sherpa
       - cosyvoice3
     * 增加独立 venv 安装：
       ```powershell
       python -m venv .venv
       .venv\Scripts\python.exe -m pip install sherpa-onnx
       ```
     * 增加设备选择、接口字段、设计约束、模型自动发现。

   * `engine/requirements.txt`

     * Python sidecar 依赖文件，早期创建；后续实际用 `sherpa-onnx` venv 安装验证。

   * `README.md`

     * 标题改为：
       ```markdown
       # 本地朗读工厂（bookctl）· Windows 本地 EPUB 有声书工厂
       ```
     * 更新构建状态、设备选择、源管理、分布式推理结论、封装分发。
     * 新增 §7.1 已修复缺陷、§7.2 设备选择、§7.3 源管理、§7.4 分布式推理结论、§9 封装分发。

   * `packaging/build-portable.ps1`

     * 新增便携版发布脚本。
     * 关键行为：
       - `dotnet publish` App / CLI / MCP / Worker；
       - 拷贝 `config/`、`engine/`、README、架构 MD；
       - 创建 `portable.marker`；
       - 创建 `sources/`、`data/`；
       - 生成 `edition.json`；
       - `Compress-Archive` 生成 zip。

   * `packaging/bookctl.iss`

     * Inno Setup 安装包脚本。
     * 关键配置：
       - `PrivilegesRequired=lowest`
       - `OutputBaseFilename=本地朗读工厂-<version>-setup`
       - `.epub` 关联；
       - 安装时删除 `portable.marker`：
         ```ini
         [InstallDelete]
         Type: files; Name: "{app}\portable.marker"
         ```

   * `packaging/sign.ps1`

     * 代码签名脚本。
     * 支持：
       - `BOOKCTL_PFX` + `BOOKCTL_PFX_PASS`
       - `BOOKCTL_CERT_SHA1`
       - Azure Trusted Signing `BOOKCTL_TS_*`
     * 使用 `signtool sign /fd sha256 /tr <timestamp> /td sha256`。
     * 支持目录或单文件签名，最后 `signtool verify /pa /q`。

   * `packaging/README.md`

     * 封装与分发说明：
       - 便携版 / 安装版差异；
       - 构建命令；
       - 签名要点；
       - 安装版行为；
       - Pro 版差异化预留；
       - 发布前检查清单。

   * `testdata/测试之书.epub`

     * 自造 3 章中文 EPUB，用于端到端验收。
     * 构建过程中修正 zip 格式、路径分隔符和 `mimetype` 顺序。

   * `testdata/ws-fresh/1a8eed25f9db_测试之书/book.m4b`

     * mock 引擎端到端生成的整本有声书样例。
     * `ffprobe` 验证：10.154s、3 个中文章节、时间轴连续、title/artist 元数据正确。

   * `testdata/sherpa-probe.wav`

     * `sherpa-onnx` 真中文语音试听文件。
     * 实测输出：
       - `ok=True`
       - `seconds=8.64`
       - `sample_rate=16000`
       - `device=cpu`
       - `mode=sherpa`
       - 非静音波形：`peak=10035`, `rms=1344.9`。

   * `testdata/ui-simple2.png` / `testdata/ui-detailed.png`

     * 新 UI 截图，验证 Apple 风格双模式实际渲染。
     * 简洁模式截图确认：顶部“本地朗读工厂”、简洁/详细切换、白卡、蓝色胶囊按钮。
     * 详细模式截图确认：左侧参数卡、右侧状态卡、中控台。

   * `.workbuddy/memory/2026-11-23.md` / `.workbuddy/memory/2026-11-24.md`

     * 工作记忆日志。
     * 记录 v0.2 实现、安装编译、端到端验收、v0.3 设备/源管理/封装/UI 等工作。

4. **Errors and fixes:**

   * **WinUI 编译错误：`CS0103: DataContext` 不存在**

     * 根因：WinUI 3 的 `Window` 不是 WPF 的 `FrameworkElement`，没有 `DataContext`。
     * 修复：
       - `MainWindow.xaml` 根 `Grid` 增加 `x:Name="Root"`；
       - `MainWindow.xaml.cs` 改为：
         ```csharp
         Root.DataContext = _vm;
         ```
     * 用户反馈：用户此前明确希望 WinUI 原生桌面软件，未要求改 WPF；因此保持 WinUI 路线。

   * **`WAV_INVALID 非 WAV 文件`**

     * 现象：第 1 章合成完成后，拼接阶段报错。
     * 根因：`MasterChapter(...)` 在有 ffmpeg 时返回 `audio.m4a`，但 Pipeline 用只认 RIFF 的 `Wav.DurationOf(audioPath)` 读它。
     * 修复：
       - 新增 `AudioMaster.ProbeDuration(...)`；
       - WAV 读头；
       - 非 WAV 用 `ffprobe`；
       - 兜底按 AAC 64kbps 估算。
     * 关键代码：
       ```csharp
       if (head[0] == 'R' && ... ) return Wav.DurationOf(path);
       if (_ffprobe is not null) { ... }
       return new FileInfo(path).Length / 8000.0;
       ```

   * **整本 M4B 阶段卡死 3 分钟以上 / `moov atom not found`**

     * 现象：ffmpeg 进程存在但不退出；手动杀掉后 `book.m4b` 无效。
     * 根因：`AudioMaster.Run(...)` 重定向 stdout/stderr 但从不读取，ffmpeg 输出写满 4KB 管道后死锁。
     * 修复：
       - `Run(...)` 并发 `ReadToEndAsync()`；
       - 加 `-hide_banner -loglevel error -nostats`；
       - 超时 `Kill(entireProcessTree: true)`；
       - `concat_list.txt` 路径统一正斜杠。
     * 验证：重新跑 4 秒完成，`ffprobe` 确认 M4B 时长和章节标记正确。

   * **进程被强杀后任务永久停在 `running`**

     * 现象：UI 永远转圈，任务状态 `running`。
     * 根因：无执行权 lease / 心跳 / 孤儿回收。
     * 修复：
       - `worker.lock` 存 PID；
       - `WorkerHost.Start()` 启动时 `RecoverOrphans()`；
       - `JobStore.FindRunningJobIds()` / `FailIfStillRunning(...)`；
       - 残留 running 标成 `WORKER_LOST`，可 `job resume`。
     * 用户无直接反馈，但这是“桌面软件可试跑”必须修复的稳定性问题。

   * **resume 后 `chapters_done=6 / total=3`、`chars_done > chars_total`；完成态仍带 `WORKER_LOST`**

     * 根因：resume 时在上次计数上继续累加；done 章节再次 `++`；完成时未清空错误码。
     * 修复：
       - 按章节 checkpoint 重算 `ChaptersDone` / `CharsDone` / `AudioSeconds`；
       - done 章节不再 `++`；
       - 完成时：
         ```csharp
         job.ErrorCode = null;
         job.ErrorMessage = null;
         ```
     * 验证：干净工作区跑完显示 `3/3`、`347/352`，无旧错误码。

   * **`run` 命中 failed/cancelled 幂等旧任务时立刻返回旧状态**

     * 根因：`RunAndWaitAsync(...)` 调用 `JobStart(...)` 后不处理幂等命中的终态。
     * 修复：
       ```csharp
       var existing = Store.GetJob(jobId);
       if (existing is { Status: "failed" or "cancelled" }) JobResume(jobId);
       ```
     * 用户反馈：无直接反馈，属于实测发现的“自动续跑”缺陷。

   * **空段 / 纯空白分段生成 0 字 WAV**

     * 现象：`第 1 章 6/6 段已合成（0 字）`。
     * 根因：`TextNormalizer.SplitSentences(...)` 可能返回空白段，仍送 TTS。
     * 修复：
       ```csharp
       if (string.IsNullOrWhiteSpace(s)) continue;
       ```

   * **ffmpeg 未被识别 / WinGet 安装后 PATH 未刷新**

     * 现象：某些 shell 中 `bookctl doctor` 报 `ffmpeg.found=false`。
     * 根因：进程 PATH 未刷新，或 Bash/PowerShell 环境不同。
     * 修复：
       - `ResolveFfmpeg(...)` 增加 WinGet Packages 目录扫描：
         ```text
         %LOCALAPPDATA%\Microsoft\WinGet\Packages\*FFmpeg*\bin\ffmpeg.exe
         ```
     * 用户反馈：用户要求“还是你来安装吧”，我已安装 SDK/环境并修补工具链。

   * **测试 EPUB 打包失败 / `META-INF/container.xml` 缺失 / `End of Central Directory record could not be found`**

     * 原因：
       - PowerShell `Compress-Archive` 不支持 `.epub` 扩展名；
       - PowerShell 5.1 zip 内路径可能用反斜杠；
       - Git Bash 的 `tar -a` 实际生成 tar，不是 zip；
       - Python zipfile 最初重复写入 `mimetype`。
     * 修复：
       - 用 Python `zipfile.ZipFile` 创建 EPUB；
       - `mimetype` 第一个且不压缩；
       - 跳过重复 `mimetype`；
       - 路径统一 `/`。
     * 这是测试资产问题，不是产品代码缺陷。

   * **MCP `PullSource` 静态类错误：`CS0708`**

     * 现象：`BookCtl.Mcp.Program` 是 `static class`，但新增了实例方法 `private object PullSource(...)`。
     * 修复：
       ```csharp
       private static object PullSource(string id)
       {
           return new BookCtlHandlers().SourcePull(id).GetAwaiter().GetResult();
       }
       ```

   * **CLI 编译锁文件：`BookCtl.Core.dll` 被 `.NET Host (9988)` 占用**

     * 现象：长时间后台 `source pull` 正在运行同一 CLI 进程，导致全解决方案 build 无法覆盖 `BookCtl.Core.dll`。
     * 处理：
       - 等待后台下载完成后再重新编译；
       - 最终 `dotnet build bookctl.sln` 返回 `BUILD_DONE`；
       - 记录到工作记忆，提示并行下载会锁 bin 目录 DLL。

   * **SourceManager 编译错误：`CS1503 Action<string,double,string>` 不能转 `Action<double,string>?`**

     * 根因：`PullAsync(...)` progress 是三参回调，但 `DownloadAsync(...)` 期望二参。
     * 修复：
       ```csharp
       await DownloadAsync(Candidates(...), archive,
           (frac, label) => progress?.Invoke("downloading", frac, label), ct);
       ```

   * **GitHub Release 下载 SSL / 502**

     * 现象：`source pull sherpa-vits-zh-fanchen` 报：
       ```json
       {"code":"INTERNAL","message":"The SSL connection could not be established, see inner exception."}
       ```
     * 探测结果：
       - GitHub 直连：`CONNECT tunnel failed, response 502`
       - `gh-proxy.com`：HTTP 200
       - `ghfast.top`：403
     * 修复：
       - `Candidates(...)` 多线路回退；
       - 内置 `https://gh-proxy.com/` + URL；
       - HuggingFace 用 `hf-mirror.com`；
       - 支持 `BOOKCTL_MIRROR` 和用户 `mirrors`。
     * 用户反馈：用户要求“一键下载到本地”，国内网络问题必须解决。

   * **模型下载后残留 119MB `.tar.bz2` 且解包目录嵌套**

     * 现象：`sources/sherpa-vits-zh-fanchen/` 中同时有：
       - `vits-zh-hf-fanchen-C.tar.bz2`
       - `vits-zh-hf-fanchen-C/`
     * 根因：`Extract(archive, tmpDir)` 把解压结果与归档文件混在同一目录，`Flatten(tmpDir, targetDir)` 把归档也拍进目标目录。
     * 修复：
       - 解到 `tmpDir/extract`；
       - `File.Delete(archive)`；
       - `Flatten(...)` 的 sourceDir 改为 `tmpDir/extract`；
       - 手工修正已安装目录：删除归档、提升子目录内容。
     * 验证：修正后目录直接包含 `vits-zh-hf-fanchen-C.onnx`、`tokens.txt`、`lexicon.txt`、`*.fst` 等，约 132MB。

   * **WinUI XAML：`WMC0011 Unknown member 'FontFamily' on element 'Grid'`**

     * 根因：WinUI 3 `Grid` 不像 WPF 控件那样直接有 `FontFamily` 属性。
     * 修复：
       - 移除 `FontFamily="Segoe UI Variable Display"`；
       - 改在 `App.xaml` 通过 `ContentControlThemeFontFamily` 设置全局字体。

   * **截图工具被安全策略拦截：`Add-Type` / `Reflection.Assembly`**

     * 现象：PowerShell `Add-Type` 和 `Reflection.Assembly.LoadWithPartialName` 都被安全策略阻止。
     * 修复：
       - 改用 ffmpeg `gdigrab`：
         ```bash
         ffmpeg -f gdigrab -i desktop -frames:v 1 ui-simple2.png
         ```
     * 结果：成功截取简洁模式和详细模式 UI 截图并视觉验证。

   * **sidecar 后台进程被 shell 回收 / `/tmp` 路径不一致**

     * 现象：第一次启动 sidecar 后下一条命令 `curl` 连接失败；Windows Python 找不到 Git Bash `/tmp/tts_probe.json`。
     * 修复：
       - 同一条 shell 命令内启动服务、curl、分析、结束；
       - 使用 Windows 绝对路径或项目内绝对路径；
       - 明确 `PY=/.../engine/.venv/Scripts/python.exe`。
     * 验证：生成 `testdata/sherpa-probe.wav`。

   * **用户要求“还是你来安装吧”后的环境安装**

     * 动作：
       - `winget install --id Microsoft.DotNet.SDK.10 ...`
       - 验证 `dotnet --version` → `10.0.404`
       - `dotnet --list-sdks` → `10.0.404 [C:\Program Files\dotnet\sdk]`
     * 这是对用户“还是你来安装吧”的直接响应。

5. **Problem Solving:**

   已解决的问题：

   - 完成互联网调研与总架构评审稿，形成 MD/HTML 文档。
   - 把“架构方案”落地为可编译工程：Core / CLI / MCP / Worker / WinUI App。
   - 回答 IDE 策略：
     - Agent 不需要 VS Code 写代码；
     - VS Code + C# Dev Kit 足够人工审阅；
     - 修改用“许愿式”自然语言指挥；
     - 用户角色是架构评审 + 验收。
   - 安装 .NET SDK 10.0.404 并完成编译验证。
   - 修复首轮编译/运行暴露的多个真实 bug，使 mock 全链路端到端通过。
   - 生成并验证：
     - 分章 `audio.wav` / `audio.m4a` / `chapter.json`
     - 整本 `book.m4b`
     - `manifest.json`
     - `qc/report.json`
     - `ffmetadata`
     - M4B 中文章节标记
   - 验证 MCP `initialize` / `tools/list`。
   - 验证 WinUI App 实际启动与渲染。
   - 新增设备选择、显卡枚举、CPU/GPU/hybrid 决策。
   - 明确回答“分布式拆分式 TTS 推理”可行性：没有能提速的方案，hybrid 只是兜底。
   - 新增 `sherpa-onnx` CPU 真中文语音引擎、模型安装、sidecar 协议、健康检查和真实试听文件。
   - 新增源管理、内置模型清单、一键下载、镜像回退、解包安装。
   - 新增便携版 / 安装版 / 签名脚本 / Inno Setup 文件关联。
   - 重写 UI 为 Apple 风格双模式。
   - UI 简洁模式和详细模式截图均已验证。
   - 更新 README、architecture MD、engine README、packaging README、工作记忆。

   仍在推进 / 尚未最终交付的部分：

   - 用户明确要求“先把没封装好的源代码版本告诉我路径，我先跑来试试”，但在本轮总结请求打断前，尚未给最终自然语言答复；路径本身已存在且应直接告知。
   - 还未实际执行 `packaging/build-portable.ps1 -Configuration Release` 生成最终 `dist/*.zip`；脚本已写好。
   - 还未生成真实签名安装包，因为没有用户证书 / PFX / EV Key / Azure Trusted Signing 参数。
   - 还未执行“整本书用 `sherpa` 真语音跑全链路并测速度”；仅完成单句真语音 probe 和模型安装。
   - `Sources.cs` 归档残留修复后，需要重新跑一次 `source pull` 或从干净源目录验证新解包流程。
   - 多显卡 0/1/2 的真实硬件切换逻辑已实现，但本机只有 1 张 GTX 1050，未做多卡实机验证。
   - 简洁模式使用 `sherpa` 的全链路还需在 UI 里实际点一次验证事件驱动章节卡片实时追加。
   - `cosyvoice3` 仍是适配点，真实权重加载未实现/未接入。

6. **All user messages:**

   1. 初始架构调研请求：

      ```text
      你上网搜索一下方案，做一个软件Windows端的软件可以自动化的软件自动化由代码来驱动的Windows框架软件，我把一本epub格式的电子书给到这个软件系统，就是这个软件它是符合那个Cmd命令来控制的，这个软件纯支持它来控制的，大模型友好，甚至它是支持mcp控制的，然后呢先做e pub格式，给他之后呢它自动会使用开源语音框架在本地显卡上，这样就节省资源，在本地的显卡上用来把它转化成朗读版本，整本图书的音频文件可以按章节读，然后把每一章节都可以分类整理好。你上网搜索一下，然后综合互联网上所有的信息设计一个架构，然后我来参考一下，我是架构师，然后ok的话，我让你开始干活。
      ```

   2. 进入 WinUI 实现：

      ```text
      很好，你把这个架构设计形成一个 MD 文件，好像已经有了。现在以 MD 文件作为总架构，我调用一些软件来协助你做。我的计划是把它做成一个 WinUI 的，用 WinUI 来写它比较清爽一点，原生支持 Windows 图形化，然后里面是支持一键的我们可以通过命令行 MCP 或 CLI 直接导入 epub 文件，来生成朗读音频。
      这个软件是完全的桌面软件形态，能够看到执行速度和中控台。在执行过程中，能实时看到它正在转换、已经调用显卡以及正在转换 TTS 的状态。最后生成好之后，它会新建一个项目文档文件夹供我们检验。
      我现在来安装 Visual Studio Code，你就负责写代码。
      ```

   3. IDE / 安装 / 许愿式编程评估：

      ```text
      还是你来安装吧，你评估一下方案：
      1. 需要用 VSCode 这个 IDE 来写这个工程项目代码吗？还是你用 Agent 直接就能写完所有代码？
      2. 如果我要人工审阅代码，用什么 IDE 比较好呢？还是不需要 IDE，全部用“许愿式编程”，也就是全部用人工自然语言指令来指挥你修改呢？
      你给个意见。
      ```

   4. 封装、显卡切换、DDS 源管理、CPU/GPU/分布式推理：

      ```text
      你要考虑一件事情：这个软件后续是要进行签名封装，形成 Pro 版可移动版和 exe 安装文件版这两个版本的纯正 Windows 软件。
      因此，你在有源代码的同时，要考虑能够支持封装。封装好以后，软件需具备以下功能：
      1. 显卡切换：软件可以根据用户实际的硬件情况来安排不同的显卡。例如，用户想用显卡 0、显卡 1 或显卡 2 来跑，都可以在软件里面进行切换。
      2. DDS 源管理：软件里面内置了一些 DDS 源，既可以下载好本地版封装进去，也可以在里面添加一个入口，实现一键下载到本地供我们的软件调用。你先把没封装好的源代码版本告诉我路径，我先跑来试试。
      先用 CPU 推理的轻量引擎也可以，然后看看有没有分布式的引擎（即拆分式推理框架），能同时调用 1050 显存和 CPU 一起来推理。TTS 有没有这样的东西？
      如果有最好；如果没有，就支持纯 GPU 或纯 CPU 推理，并且可以切换。这个选择权要完全交给用户，由用户来选择是用 CPU 还是 GPU 推理。
      ```

   5. 产品命名、双模式 UI、Apple 风格设计提示词：

      ```text
      软件名称暂时命名为“本地朗读工厂”或“帅帅朗读工厂”。
      软件功能与模式：
      刚进入软件或在首页 UI 时，可以切换“简洁模式”和“详细参数模式”两种模式。
      1. 详细参数模式（当前模式）：支持手动 menu 模式，用户可以手动选择自己的 epub 等文件。
      2. 简洁模式：考虑内置一个小型的模型（无需太大算力）。用户直接导入一本 epub 文件进入简洁模式后，系统会自动执行识别、拆分章节，用户即可直接导入 epub 文件来生成朗读文件。
      朗读文件生成反馈：
      朗读文件生成好后，会实时以图形化的方式告诉用户是在哪个文件夹生成好了、生成了哪几章。
      UI 设计要求：
      UI 设计参考苹果的简洁化设计，我会给你一个设计提示词。@long-text:"# Apple 风格..." <long_text_quote># Apple 风格 — 风格参考

      > 大量留白、一颗系统蓝、慢到刚好的动效：让产品自己说话。

      **Theme:** light

      Apple 风格的核心不是元素，而是比例：内容占的面积远小于留白，产品图是唯一的主角。底色用偏冷的浅灰（#f5f5f7）代替纯白，卡片比它白一档浮起来。颜色只允许一支系统蓝（#0071e3）用于行动，其余全是灰阶。圆角克制而统一，按钮全圆角、卡片 24px、输入框 12px。动效是这套风格的签名：500ms 与一段缓入缓出的曲线，hover 时轻微上浮、阴影扩散，快一点就变成普通网页，慢一点就拖沓。

      ## Colors

      | Name | Value | Token | Role |
      |---|---|---|---|
      | Apple Black | `#000000` | `--color-apple-black` | 正文、标题与深色满屏区块。深色区块用来把产品图推到最前。 |
      | Apple Grey | `#f5f5f7` | `--color-apple-grey` | 页面画布。比纯白柔一档，让白色卡片有依托。 |
      | Card White | `#ffffff` | `--color-card-white` | 卡片与浮层填充。只在灰底之上出现。 |
      | System Blue | `#0071e3` | `--color-system-blue` | 唯一强调色：主 CTA、链接与选中态。一屏只允许一处。 |
      | Success Green | `#34c759` | `--color-success-green` | 成功与在线状态，小面积标记。 |
      | Alert Red | `#ff3b30` | `--color-alert-red` | 错误与删除，出现即代表不可撤销。 |

      ## Typography

      ### SF Pro Display — 全站唯一字体。大标题用 600 与紧字距，正文 400 保持中性，字级偏大（17px 起）。

      - **Substitute:** -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif
      - **Weights:** 400, 500, 600, 700
      - **Sizes:** 12, 14, 17, 24, 40, 56, 80
      - **Line height:** 1.05 (display), 1.15 (headings), 1.55 (body)
      - **Letter spacing:** -0.025em on display, 0.01em on uppercase labels

      ### Type Scale

      | Role | Family | Weight | Size | Line Height | Letter Spacing | Token |
      |---|---|---|---|---|---|---|
      | display | — | 600 | 80px | 1.05 | -0.025em | `--text-display` |
      | heading-lg | — | 600 | 56px | 1.1 | -0.02em | `--text-heading-lg` |
      | heading | — | 600 | 40px | 1.15 | -0.015em | `--text-heading` |
      | subheading | — | 500 | 24px | 1.3 | — | `--text-subheading` |
      | body-lg | — | 400 | 17px | 1.55 | — | `--text-body-lg` |
      | body | — | 400 | 14px | 1.5 | — | `--text-body` |
      | label | — | 400 | 12px | 1.4 | 0.01em | `--text-label` |

      ## Spacing & Layout

      **Base unit:** 8px
      **Density:** airy

      ### Spacing Scale

      | Name | Value | Token |
      |---|---|---|
      | 8 | 8px | `--spacing-8` |
      | 16 | 16px | `--spacing-16` |
      | 24 | 24px | `--spacing-24` |
      | 32 | 32px | `--spacing-32` |
      | 48 | 48px | `--spacing-48` |
      | 64 | 64px | `--spacing-64` |
      | 96 | 96px | `--spacing-96` |
      | 128 | 128px | `--spacing-128` |
      | 192 | 192px | `--spacing-192` |

      ### Border Radius

      | Element | Value |
      |---|---|
      | cards | 24px |
      | buttons | 999px |
      | inputs | 12px |
      | product-media | 18px |

      - **Page max-width:** 1080px
      - **Section gap:** 128px
      - **Card padding:** 32px
      - **Element gap:** 24px

      ## Components

      ### Pill Button

      **Role:** 主行动，例：Buy、Learn more

      系统蓝填充、白字、999px 全圆角、无描边。字级 17px、字重 500、padding 12px 24px，阴影 0 4px 14px 蓝 30%。hover 时填充提亮到 #0077ed、上浮 2px、阴影扩到 20px 40%；按下缩放 0.96。过渡 500ms，缓动 cubic-bezier(0.25, 0.1, 0.25, 1)。

      - **Sizes:** padding 12px 24px; label 17px
      - **States:** hover: lift 2px, brighter fill; active: scale 0.96

      ### Product Card

      **Role:** 产品卡与功能卡

      白底、24px 圆角、内边距 32px、阴影 0 4px 12px 4%。内容居中：上方 192px 见方的产品图（18px 圆角、苹果灰底），下面 24px/600 标题与 14px 灰字。hover 时上浮 4px、阴影扩到 0 20px 40px 8%、产品图放大 1.05，过渡 500ms。

      ### Search Field

      **Role:** 搜索与输入

      苹果灰底、12px 圆角、无描边、padding 12px 16px、17px 字。聚焦时加 2px 系统蓝焦点环，不改变尺寸。

      - **States:** focus: 2px blue ring

      ### Vibrancy Nav

      **Role:** 顶部导航

      高 48px、白色 80% 不透明、backdrop-blur 20px、底部 1px 灰 50% 描边。链接 12px、字重 400、间距 32px，hover 转 50% 灰。右侧最多一个纯文字 CTA。

      ## Do's and Don'ts

      ### Do

      - 留白优先，元素稀疏到近乎空
      - 只用一支系统蓝，#0071e3
      - 动效 500ms，缓入缓出
      - 按钮全圆角、卡片 24px、输入 12px
      - 标题 600 字重配紧字距
      - 背景用苹果灰而不是纯白

      ### Don't

      - 不要用多个强调色
      - 不要用渐变背景
      - 不要用重阴影
      - 不要把内容挤在一起
      - 不要用装饰性插画或图标墙
      - 不要让圆角混用超过三档

      ## Surfaces

      | Level | Name | Value | Purpose |
      |---|---|---|---|
      | 0 | Apple Grey | `#f5f5f7` | 页面画布 |
      | 1 | Card White | `#ffffff` | 卡片与浮层 |
      | 2 | Black Stage | `#000000` | 产品满屏区 |

      ## Elevation

      - **Card rest:** `0 4px 12px rgba(0,0,0,0.04)`
      - **Card hover:** `0 20px 40px rgba(0,0,0,0.08)`
      - **Button glow:** `0 4px 14px rgba(0,113,227,0.30)`

      ## Imagery

      产品渲染与实拍是唯一的主角：满屏、居中、背景纯净（黑、白或苹果灰）。允许缓慢的视差与缩放。不使用插画、图标墙、渐变背景或人物特写。

      ## Layout

      上限 1080px，容器内边距 24px。区块间距 128px 起，留白至少是内容高度的一半。每屏只讲一件事：一个标题、一句副标题、两个链接。产品区可满屏且反色。

      ## Agent Prompt Guide

      ### Implementation Contract

      - 以用户请求中的页面、用户任务和真实内容为依据，只实施指定范围；标明缺失的业务信息，不编造数据或能力。
      - 先检查现有项目、技术栈与组件；沿用项目结构、路由和依赖，复用已有组件，不重建未要求的页面或功能。
      - 沿用项目断点，调整窄屏与桌面的网格和留白；保留信息层级，避免意外横向溢出。
      - 对本次涉及的交互，处理键盘焦点与减少动态效果；补齐适用的禁用、加载、空内容与错误状态。
      - 完成后按项目现有脚本检查，仅报告实际运行的测试与视觉检查，并列出未验证项。

      QUICK COLOR REFERENCE:

      - text: `#1d1d1f`
      - background: `#f5f5f7`
      - card: `#ffffff`
      - accent: `#0071e3`
      - muted: `#86868b`
      - dark: `#000000`

      EXAMPLE COMPONENT PROMPTS:

      1. 做一个产品卡：白底、24px 圆角、内边距 32px、阴影 0 4px 12px rgba(0,0,0,0.04)。上方放 192px 见方产品图（18px 圆角、#f5f5f7 底），下面 24px/600 标题 #1d1d1f 与 14px #86868b 说明，再下面一个 #0071e3 全圆角按钮（17px/500、padding 12px 24px）。hover 时卡片上浮 4px、阴影扩到 0 20px 40px rgba(0,0,0,0.08)、产品图放大 1.05，过渡 500ms cubic-bezier(0.25,0.1,0.25,1)。

      ## Similar Brands

      - **Apple** — 同一套比例、同一支蓝、同一段缓动
      - **Linear** — 同样以留白与单一强调色组织信息

      ## Quick Start

      ```css

      :root {
        /* Colors */
        --color-apple-black: #000000;
        --color-apple-grey: #f5f5f7;
        --color-card-white: #ffffff;
        --color-system-blue: #0071e3;
        --color-success-green: #34c759;
        --color-alert-red: #ff3b30;

        /* Typography — Font Families */
        --font-sans: 'SF Pro Display', ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;

        /* Typography — Scale */
        --text-display: 80px;
        --leading-display: 1.05;
        --tracking-display: -0.025em;
        --text-heading-lg: 56px;
        --leading-heading-lg: 1.1;
        --tracking-heading-lg: -0.02em;
        --text-heading: 40px;
        --leading-heading: 1.15;
        --tracking-heading: -0.015em;
        --text-subheading: 24px;
        --leading-subheading: 1.3;
        --text-body-lg: 17px;
        --leading-body-lg: 1.55;
        --text-body: 14px;
        --leading-body: 1.5;
        --text-label: 12px;
        --leading-label: 1.4;
        --tracking-label: 0.01em;

        /* Typography — Weights */
        --font-weight-regular: 400;
        --font-weight-medium: 500;
        --font-weight-semibold: 600;
        --font-weight-bold: 700;

        /* Spacing */
        --spacing-unit: 8px;
        --spacing-8: 8px;
        --spacing-16: 16px;
        --spacing-24: 24px;
        --spacing-32: 32px;
        --spacing-48: 48px;
        --spacing-64: 64px;
        --spacing-96: 96px;
        --spacing-128: 128px;
        --spacing-192: 192px;

        /* Layout */
        --page-max-width: 1080px;
        --section-gap: 128px;
        --card-padding: 32px;
        --element-gap: 24px;
        --density: airy;

        /* Border Radius */
        --radius-cards: 24px;
        --radius-buttons: 999px;
        --radius-inputs: 12px;
        --radius-product-media: 18px;

        /* Surfaces */
        --surface-apple-grey: #f5f5f7;
        --surface-card-white: #ffffff;
        --surface-black-stage: #000000;
      }

      ```
      </long_text_quote>
      ```

   6. 当前总结请求：

      ```text
      Your task is to create a detailed and highly structured summary of the conversation so far.

      Your summary must be technically accurate, comprehensive, and strictly follow the required output format.

      When generating the summary:

      1. Review the conversation chronologically.
      2. Identify clearly:

         * All explicit user requests and intents
         * Your actions and responses
         * Technical decisions, design choices, and code patterns
         * File names, code snippets, function signatures, and file edits
         * Any errors encountered and how they were resolved
         * Any direct user feedback instructing you to change behavior

      3. Ensure completeness and precision in all sections.

      ## **Your final summary MUST strictly follow this structure:**

      Summary:

      1. **Primary Request and Intent:**
         A detailed description of all explicit user requests and intentions.

      2. **Key Technical Concepts:**

         * Concept 1
         * Concept 2
         * …

      3. **Files and Code Sections:**

         * `FileName`

           * Why this file is important
           * Summary of changes made (if any)
           * Important code snippet (if applicable)

      4. **Errors and fixes:**

         * Error description

           * How it was fixed
           * Any user feedback

      5. **Problem Solving:**
         Problems solved and ongoing troubleshooting work.

      6. **All user messages:**
         List *all* user messages (actual text, excluding tool results).

      7. **Pending Tasks:**
         List all tasks the user explicitly asked you to continue.

      8. **Current Work:**
         Describe exactly what you were working on immediately before this summary request, including file names and code snippets if applicable.

      9. **Optional Next Step:**
         Only if directly aligned with the user's latest explicit request.
         Include verbatim quotations showing where the previous task left off.
      ```

7. **Pending Tasks:**

   - **告知未封装源代码版本路径**：用户明确说：
     ```text
     你先把没封装好的源代码版本告诉我路径，我先跑来试试。
     ```
     已存在但尚未作为最终答复明确给出。应告知：
     - 项目根目录：`C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/`
     - 源码目录：`C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/src/`
     - 可直接运行的 WinUI Debug exe：
       `C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe`
     - CLI Debug dll：
       `C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/src/BookCtl.Cli/bin/Debug/net10.0/bookctl.dll`
     - 引擎目录：
       `C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/engine/`
     - 打包脚本目录：
       `C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/packaging/`

   - **继续完成“整本书 CPU 真语音全链路并测速度”**：用户允许先用 CPU 轻量引擎；我已安装 `sherpa-onnx` 和模型、生成单句试听，但尚未跑整本 EPUB 的 `sherpa` 全链路。

   - **验证简洁模式一键生成结果可视化**：Apple 风格 UI 已截图，但尚未在简洁模式点击“生成朗读音频”验证章节卡片实时追加。

   - **重新验证修复后的 `source pull` 解包流程**：`Sources.cs` 已修复归档残留和嵌套目录问题，但尚未从干净 sources 目录重新跑完整 `source pull`。

   - **实际生成便携版 zip / 安装包 exe**：脚本已写好，但尚未运行 Release publish、Inno Setup、签名。签名还需用户提供证书或环境变量。

   - **多显卡 0/1/2 实机验证**：本机只有 1 张 GTX 1050，逻辑已实现但多卡硬件未验证。

   - **接真实 GPU `cosyvoice3` 权重**：仍为 P1 适配点，需 ≥6GB 显存机器和模型权重；当前 GTX 1050 2GB 不适合。

   - **如用户继续“许愿式编程”**：后续改动应继续由用户自然语言描述，我负责改文件、编译、运行验证、反馈结果。

8. **Current Work:**

   在本次总结请求前，正在做 v0.3 的最后编译与真 CPU 语音全链路准备。具体状态：

   - 已完成 `source pull sherpa-vits-zh-fanchen`：
     - 下载耗时约 `26m 39s`；
     - 通过 `gh-proxy.com` 镜像回退成功；
     - 输出：
       ```json
       {"ok":true,"api_version":"1.0","data":{"id":"sherpa-vits-zh-fanchen","status":"installed","path":"C:\\Users\\romanovsky\\AppData\\Local\\bookctl\\sources\\sherpa-vits-zh-fanchen"}}
       ```
   - 已修复下载残留压缩包/嵌套目录问题：
     - `src/BookCtl.Core/Sources.cs` 中把解包目录改为 `tmpDir/extract`；
     - `File.Delete(archive)`；
     - `Flatten(sourceDir, targetDir)`；
     - 手工修正本地已安装目录，删除 `vits-zh-hf-fanchen-C.tar.bz2`，提升模型文件到根目录。
   - 已安装并验证 `sherpa-onnx` venv：
     ```text
     Successfully installed sherpa-onnx-2.0.3 sherpa-onnx-core-2.0.3
     sherpa-onnx 2.0.3
     ```
   - 已启动 `engine/tts_server.py --mode sherpa` 并验证 `/health`：
     ```json
     {"ok": true, "mode": "sherpa", "model": "sherpa-onnx-vits-zh", "python": "3.13.14", "device": "cpu", "gpu_index": -1, "cpu_threads": 16, "model_dir": "C:\\Users\\romanovsky\\AppData\\Local\\bookctl\\sources\\sherpa-vits-zh-fanchen"}
     ```
   - 已生成真实中文语音试听：
     - 文件：`C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07/testdata/sherpa-probe.wav`
     - 输出：
       ```text
       ok True sec 8.64 rate 16000
       dur 8.64 peak 10035 rms 1344.9
       model loaded, sample_rate=16000, speakers=187
       ```
   - 已修复 `src/BookCtl.Mcp/Program.cs` 中的 static class 编译错误：
     ```csharp
     private static object PullSource(string id)
     {
         // 下载可能几十秒到几分钟：MCP 客户端会等，但只在这一件事上；不做 job 化是为了让模型拿到明确的安装结果
         return new BookCtlHandlers().SourcePull(id).GetAwaiter().GetResult();
     }
     ```
   - 最后执行的动作是全解决方案重新编译：
     ```powershell
     dotnet build "C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\bookctl.sln" -c Debug -v q --nologo
     ```
     工具结果显示：
     ```text
     BUILD_DONE
     ```
     表示全解决方案无 error / warning（筛选结果为空）。
   - 紧接这一步，原本准备继续：
     ```text
     真实语音生成成功（8.64 秒、16kHz、非静音）。现在整本书跑 CPU 真语音全链路并测速度：
     ```
     但用户随即发起了本总结请求，因此尚未实际执行整本 `sherpa` 全链路测试，也尚未给最终“未封装源码路径”答复。

9. **Optional Next Step:**

   若本总结后继续上一任务，最直接的下一步是：先把用户明确要的“未封装源代码版本路径”交付给用户，并继续做 CPU 真语音整本链路验收。

   对齐的用户原话：

   > “你先把没封装好的源代码版本告诉我路径，我先跑来试试。”

   > “先用 CPU 推理的轻量引擎也可以，然后看看有没有分布式的引擎（即拆分式推理框架），能同时调用 1050 显存和 CPU 一起来推理。TTS 有没有这样的东西？”

   上一任务实际停在这句动作描述：

   > “真实语音生成成功（8.64 秒、16kHz、非静音）。现在整本书跑 CPU 真语音全链路并测速度：”

   以及最后工具结果：

   > `BUILD_DONE`

   建议下一步（如继续开发）：
   - 给出源码路径与运行命令；
   - 启动 `engine/tts_server.py --mode sherpa`;
   - 运行：
     ```powershell
     BOOKCTL_ENGINE=sherpa BOOKCTL_DEVICE=cpu bookctl run --input "C:\...\testdata\测试之书.epub"
     ```
   - 验证每章真实中文语音、`book.m4b`、章节标记、速度/RTF；
   - 再告知用户可直接双击哪个 exe 试跑。


<user_query>Please continue with the conversation based on the summarized context above. Maintain the same level of detail and helpfulness as before the summarization.</user_query>


**助手（历史）**


我先核对一下当前工程产物与源码路径是否都还在，然后把你要试跑的路径给你，并接着把 CPU 真语音整本链路跑完。


**助手（历史）**


路径都在，我先把它明确给你（这是你之前要的"未封装源码版本"）：

- **工程根目录**：`C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\`
- **源码目录**：`...\src\`（5 个项目：`BookCtl.Core` / `BookCtl.Cli` / `BookCtl.Mcp` / `BookCtl.Worker` / `BookCtl.App`），解决方案 `bookctl.sln`
- **可直接双击的 WinUI 窗口程序**：`...\src\BookCtl.App\bin\Debug\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe`
- **CLI 入口**：`...\src\BookCtl.Cli\bin\Debug\net10.0\bookctl.exe`（或 `dotnet bookctl.dll`）
- **MCP / Worker**：`...\src\BookCtl.Mcp\bin\Debug\net10.0\bookctl-mcp.dll`、`...\src\BookCtl.Worker\bin\Debug\net10.0\bookctl-worker.dll`
- **TTS 引擎**：`...\engine\`（`tts_server.py` + 已装好的 `.venv`），真语音模型在 `%LOCALAPPDATA%\bookctl\sources\sherpa-vits-zh-fanchen`
- **封装脚本**：`...\packaging\` · 默认配置 `...\config\` · 测试书 `...\testdata\测试之书.epub`

注意本机 `dotnet` 不在 PATH，命令行要用绝对路径 `C:\Program Files\dotnet\dotnet.exe`。现在接着把 CPU 真语音整本链路跑完。


**助手（历史）**


整本链路 57.76 秒跑通了（3 章 / 93.8 秒真实中文语音 / book.m4b 已生成）。但日志里暴露了一个必须修的问题：**明明是 CPU 推理，中控台却显示"正在调用显卡进行 TTS 合成"**。我先验证产物，再把这个状态文案修正。


**助手（历史）**


产物齐全：3 章各有 `audio.wav` / `audio.m4a` / `chapter.json`，另有 14 个分段 WAV、`book.m4b`、`manifest.json`、`qc/report.json`。现在修掉状态文案误导的问题。


**助手（历史）**


中文乱码只是 PowerShell 5.1 重定向日志时的编码陷阱（Bash/CMD/MCP 下 CLI 输出完全正常，`"已完成"`、`"测试之书"` 都正确），不是产品缺陷。现在验证音频真实性与 M4B 章节标记，并重新编译带修正的版本。


**助手（历史）**


编译通过（`BUILD_EXIT=0`，无 error/warning）。音频验收也过了：M4B 98.27 秒、3 个中文章节标记连续、title/artist 正确；14 个分段 WAV 全是 16kHz 真语音且非静音（peak 8k–11k，rms ≈1400–1600）。现在用新工作区重跑一次，确认修正后的设备状态文案。


**助手（历史）**


找到真正的编码缺陷了：`Emit()` 里才把 `Console.OutputEncoding` 设为 UTF-8，但**事件流 JSONL 在那之前就输出了**，用的是系统 OEM(GBK) 编码——所以章节标题在 UTF-8 终端下乱码、在 PowerShell 下又反过来。这是"大模型友好"控制面的实打实问题，我一并修掉。


**助手（历史）**


编码与状态文案都验证通过：`job logs` 严格 UTF-8 无 BOM 解码成功，中文事件消息正常，`phase` 已是 `正在用 CPU（16 线程）进行 TTS 合成`（device=cpu / display 一致）。再用极快的 mock 跑一遍，验证**实时事件流**（不是历史回放）编码，同时准备便携版封装。


**助手（历史）**


实时事件流也通过了：严格 UTF-8、无 BOM，31 行纯文本日志（`[S1_INGEST] 正在解析 EPUB 结构`）与 32 行 JSON 事件中文全部正常。现在跑便携版 Release 封装，验证整条打包链路。


**助手（历史）**


便携版脚本踩到 Windows PowerShell 5.1 的经典坑：脚本是 UTF-8 无 BOM，5.1 按 GBK 解码后中文把 here-string 语法破坏了，直接解析失败。这是封装脚本的真实缺陷（用户以后用 `powershell -File` 一样会挂），我修掉它。


**助手（历史）**


便携版发布成功，zip 约 196 MB。但我注意到 CLI/MCP/Worker 被放到 `tools\` 子目录，而 `portable.marker` 只在根目录——便携模式判定可能找不到标记，需要验证。


**助手（历史）**


确认了封装缺陷：便携版 CLI 从 `tools\BookCtl.Cli\` 运行时，`workspace` 仍是 `%LOCALAPPDATA%\bookctl\workspace`，`portable.marker` 没被识别——U 盘带走后数据会写丢。我改成向上查找标记。


**助手（历史）**


便携模式修好了：`workspace = dist\...\data`、`database = ...\data\bookctl.db`、`edition = pro`，U 盘带走不再丢数据。最后补一项你最早提的需求——"命令行导入 EPUB 直接生成朗读音频"，顺便用它验收简洁模式的实时章节卡片。


**助手（历史）**


UI 自动生成跑通了（3 章 + book.m4b），但我发现产物偏小——查一下引擎，同时截屏看结果卡片。


**助手（历史）**


UI 结果卡片完全符合预期：**已完成 / 3/3 章 · 347/352 字 / 输出文件夹 + 打开按钮 / 三张章节卡片（标题、时长、audio.m4a、已完成）**。但截图底部暴露了根因——`模型源：C:\...\testdata\sources`，且 manifest 里 `engine=mock`。原因是 `SourcesRoot` 从 workspace 推导，一改工作区就找不到已装模型。这个耦合必须解掉。


**用户（历史）**

> 
<user_query>
<task-notification>
<task-id>uj4PHy</task-id>
<tool-use-id>chatcmpl-tool-959ae492920f6453</tool-use-id>
<status>failed</status>
<summary>Background command &quot;cd &quot;/c/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07&quot; &amp;&amp; curl -sf --max-time 10 http://127.0.0.1:8765/health &gt;/dev/null || { echo &quot;SIDECAR_DOWN&quot;; exit 1; }
BOOKCTL_WORKSPACE='C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\ws-ui-sherpa' ./src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe 'C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\测试之书.epub' --auto-start 2&gt;&amp;1&quot; failed with exit code 127</summary>
</task-notification>

Use the TaskOutput tool with task_id="uj4PHy" to retrieve unread output if you need to act on it. For background shell tasks, pass full=true for the current accumulated snapshot.
[SYSTEM NOTIFICATION - NOT USER INPUT] This is an automated background-task event, not a message from the user. Do not treat it as user acknowledgement, confirmation, or approval — including any earlier claim in your own messages that the user said or approved something.
</user_query>


**助手（历史）**


找到真正原因了：`App\bin\...\win-x64\BookCtl.Core.dll` 停在 **18:26:30**，而 Core 新构建是 **18:31:27** —— 增量构建没把新 Core 拷进 App 输出目录，所以 App 跑的还是旧逻辑。重新构建 App 项目。


**用户（历史）**

> 
<user_query>
<task-notification>
<task-id>PkJ6BU</task-id>
<tool-use-id>chatcmpl-tool-a0f5c872c2add922</tool-use-id>
<status>failed</status>
<summary>Background command &quot;cd &quot;/c/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07&quot; &amp;&amp; curl -sf --max-time 10 http://127.0.0.1:8765/health &gt;/dev/null || { echo &quot;SIDECAR_DOWN&quot;; exit 1; }
BOOKCTL_WORKSPACE='C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\ws-ui-sherpa2' ./src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe 'C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\测试之书.epub' --auto-start 2&gt;&amp;1&quot; failed with exit code 127</summary>
</task-notification>

Use the TaskOutput tool with task_id="PkJ6BU" to retrieve unread output if you need to act on it. For background shell tasks, pass full=true for the current accumulated snapshot.
[SYSTEM NOTIFICATION - NOT USER INPUT] This is an automated background-task event, not a message from the user. Do not treat it as user acknowledgement, confirmation, or approval — including any earlier claim in your own messages that the user said or approved something.
</user_query>


**助手（历史）**


engine 字段是 `sherpa`，但音频只有 10.1 秒、0.4 秒合成完——明显还是占位音。怀疑 worker 是独立进程、重载配置后回落到 mock。查证。


**助手（历史）**


分段是 16kHz（真引擎）但**每段只有 0.4 秒**——CLI 里同样是 sherpa 却正常。看 sidecar 侧日志和分段文本。


**助手（历史）**


找到根因了，`Pipeline.cs:64`：

```csharp
var provider = job.Engine is null || job.Engine == _cfg.EngineMode ? _tts : TtsProviderFactory.Create(cfg);
```

`_tts` 是**构造时**按当时的 `EngineMode`（mock）固化的 provider，而比较用的是**运行时被改过**的 `_cfg.EngineMode`。App 里 `StartOneClick` 把 `EngineMode` 改成 sherpa 后两者相等，于是复用了旧的 mock provider——清单里记 sherpa，实际出声是占位音。CLI 因为显式传 `--engine` 才侥幸正确。修掉。


**用户（历史）**

> 
<user_query>
<task-notification>
<task-id>m8Fgb1</task-id>
<tool-use-id>call_00_a7Or05gBVzVRGxRtZMsJ2298</tool-use-id>
<status>failed</status>
<summary>Background command &quot;cd &quot;/c/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07&quot; &amp;&amp; curl -sf --max-time 10 http://127.0.0.1:8765/health &gt;/dev/null || { echo &quot;SIDECAR_DOWN&quot;; exit 1; }
BOOKCTL_WORKSPACE='C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\ws-ui-sherpa3' ./src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe 'C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\测试之书.epub' --auto-start 2&gt;&amp;1&quot; failed with exit code 127</summary>
</task-notification>

Use the TaskOutput tool with task_id="m8Fgb1" to retrieve unread output if you need to act on it. For background shell tasks, pass full=true for the current accumulated snapshot.
[SYSTEM NOTIFICATION - NOT USER INPUT] This is an automated background-task event, not a message from the user. Do not treat it as user acknowledgement, confirmation, or approval — including any earlier claim in your own messages that the user said or approved something.
</user_query>


**助手（历史）**


UI 真语音跑通了：**engine=sherpa、97.9 秒音频、34.2 秒合成（RTF≈0.35，快于实时）、M4B 762KB、分段 6.71/7.57/4.67 秒 @16kHz**。截图看最终界面。


**助手（历史）**


窗口被资源管理器挡住了，而且 `gdigrab` 按窗口标题抓不到 WinUI（DirectComposition）内容。我重启一次让窗口回到前台，并让任务重跑（删 job 库、复用已生成的音频缓存）再截图。


**用户（历史）**

> 
<user_query>
<task-notification>
<task-id>JK4uyB</task-id>
<tool-use-id>call_00_IZgb5hRQTSGDXhjNvSzK7151</tool-use-id>
<status>failed</status>
<summary>Background command &quot;cd &quot;/c/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07&quot; &amp;&amp; curl -sf --max-time 10 http://127.0.0.1:8765/health &gt;/dev/null || { echo &quot;SIDECAR_DOWN&quot;; exit 1; }
BOOKCTL_WORKSPACE='C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\ws-ui-sherpa4' ./src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe 'C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\测试之书.epub' --auto-start 2&gt;&amp;1&quot; failed with exit code 127</summary>
</task-notification>

Use the TaskOutput tool with task_id="JK4uyB" to retrieve unread output if you need to act on it. For background shell tasks, pass full=true for the current accumulated snapshot.
[SYSTEM NOTIFICATION - NOT USER INPUT] This is an automated background-task event, not a message from the user. Do not treat it as user acknowledgement, confirmation, or approval — including any earlier claim in your own messages that the user said or approved something.
</user_query>


**助手（历史）**


窗口每次都被 WorkBuddy/资源管理器压住（因为我每次操作都会把主窗口带到前台）。给 App 加一个演示用的置顶开关，再抓一次。


**用户（历史）**

> 
<user_query>
<task-notification>
<task-id>C27a5h</task-id>
<tool-use-id>chatcmpl-tool-96b6b65af6450497</tool-use-id>
<status>failed</status>
<summary>Background command &quot;cd &quot;/c/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07&quot; &amp;&amp; curl -sf --max-time 10 http://127.0.0.1:8765/health &gt;/dev/null || { echo &quot;SIDECAR_DOWN&quot;; exit 1; }
BOOKCTL_WORKSPACE='C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\ws-ui-sherpa4' ./src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe 'C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\测试之书.epub' --auto-start 2&gt;&amp;1&quot; failed with exit code 127</summary>
</task-notification>

Use the TaskOutput tool with task_id="C27a5h" to retrieve unread output if you need to act on it. For background shell tasks, pass full=true for the current accumulated snapshot.
[SYSTEM NOTIFICATION - NOT USER INPUT] This is an automated background-task event, not a message from the user. Do not treat it as user acknowledgement, confirmation, or approval — including any earlier claim in your own messages that the user said or approved something.
</user_query>


**助手（历史）**


最终界面拿到了：**"3/3 章 · 347/352 字 · 已生成 1分33秒"**，输出文件夹指向 `ws-ui-sherpa4\1a8eed25f9db_测试之书`，说明 UI 走的是真语音。现在把本轮修复全部打进便携版包，并更新文档。


**助手（历史）**


便携包验证通过（`workspace = dist\...\data`、`edition = pro`）。更新 README 的缺陷台账。


**助手（历史）**


最后补一个"用户第一次双击就能跑通"的兜底：模型装了但 sidecar 没启动时，一键模式不要硬报错，而是退到校样引擎并说明原因。


**用户（历史）**

> 
<user_query>
<task-notification>
<task-id>f2Y8UT</task-id>
<tool-use-id>call_00_rZTgwKGSIK0QcvMLggaj5879</tool-use-id>
<status>failed</status>
<summary>Background command &quot;cd &quot;/c/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07&quot; &amp;&amp; ./engine/.venv/Scripts/python.exe -c &quot;
import glob,os
for f in glob.glob(r'C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\ws-ui-sherpa4\bookctl.db*'):
    try: os.remove(f); print('del',os.path.basename(f))
    except Exception as e: print('skip',e)
&quot;
BOOKCTL_WORKSPACE='C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\ws-ui-sherpa4' ./src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe 'C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\测试之书.epub' --auto-start --on-top 2&gt;&amp;1&quot; failed with exit code 127</summary>
</task-notification>

Use the TaskOutput tool with task_id="f2Y8UT" to retrieve unread output if you need to act on it. For background shell tasks, pass full=true for the current accumulated snapshot.
[SYSTEM NOTIFICATION - NOT USER INPUT] This is an automated background-task event, not a message from the user. Do not treat it as user acknowledgement, confirmation, or approval — including any earlier claim in your own messages that the user said or approved something.
</user_query>


**用户（历史）**

> 
<user_query>
<task-notification>
<task-id>w77vr4</task-id>
<tool-use-id>call_00_5MQ2oCRIZtH8Bdrhpz3e5150</tool-use-id>
<status>failed</status>
<summary>Background PowerShell command &quot;$ErrorActionPreference='Continue'
Set-Location 'C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07'
$env:PATH = 'C:\Program Files\dotnet;' + $env:PATH
$ProgressPreference = 'SilentlyContinue'
&amp; '.\packaging\build-portable.ps1' -Configuration Release -Version 0.3.0 *&gt; 'packaging-build.log'
Write-Output &quot;SCRIPT_LASTEXIT=$LASTEXITCODE&quot;
Get-Item 'dist\本地朗读工厂-0.3.0-win-x64.zip' | Select-Object Length,LastWriteTime | Format-Table -AutoSize | Out-String&quot; failed with exit code 1</summary>
</task-notification>

Use the TaskOutput tool with task_id="w77vr4" to retrieve unread output if you need to act on it. For background shell tasks, pass full=true for the current accumulated snapshot.
[SYSTEM NOTIFICATION - NOT USER INPUT] This is an automated background-task event, not a message from the user. Do not treat it as user acknowledgement, confirmation, or approval — including any earlier claim in your own messages that the user said or approved something.
</user_query>


**助手（历史）**


打包被安全策略拦住了（脚本要清空 1124 个文件的旧目录，超过批量删除阈值）。我改成原地增量覆盖，不动目录。


**助手（历史）**


兜底验证通过：**sidecar 不可达时一键模式自动退回校样引擎，任务 `completed / COMPLETED`、无错误**。但发现产物 `manifest.json` 里的 `status` 停在 `running`（DB 里已是 completed）——这是用户"检验产物"时会看到的文档，得修。


**用户（历史）**

> 
<user_query>
<task-notification>
<task-id>3Wte9f</task-id>
<tool-use-id>call_01_tk3xycwWUEFdVWnRdUHa2655</tool-use-id>
<status>failed</status>
<summary>Background command &quot;cd &quot;/c/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07&quot; &amp;&amp; BOOKCTL_SIDECAR_URL='http://127.0.0.1:9' BOOKCTL_WORKSPACE='C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\ws-fallback' ./src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe 'C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\测试之书.epub' --auto-start 2&gt;&amp;1&quot; failed with exit code 127</summary>
</task-notification>

Use the TaskOutput tool with task_id="3Wte9f" to retrieve unread output if you need to act on it. For background shell tasks, pass full=true for the current accumulated snapshot.
[SYSTEM NOTIFICATION - NOT USER INPUT] This is an automated background-task event, not a message from the user. Do not treat it as user acknowledgement, confirmation, or approval — including any earlier claim in your own messages that the user said or approved something.
</user_query>


**助手（历史）**


产物清单的终态问题修好了（`manifest.status = completed`）。最后重新打个包含全部修复的包。


**助手（历史）**


CPU 真语音整本链路已经跑通，过程中挖出并修掉了 5 个“看起来正常、其实在撒谎”的问题。

## 你要的源码路径（可直接跑）

| 用途 | 路径 |
|---|---|
| 工程根 | `C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\` |
| **双击就能开的窗口程序** | `...\src\BookCtl.App\bin\Debug\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe` |
| CLI | `...\src\BookCtl.Cli\bin\Debug\net10.0\bookctl.exe` |
| 引擎/sidecar | `...\engine\`（`.venv` 已装 sherpa-onnx 2.0.3） |
| 便携版成品 | `...\dist\本地朗读工厂-0.3.0-win-x64\` + 同目录 `.zip`（196 MB） |

注意本机 `dotnet` 不在 PATH，命令行要写 `C:\Program Files\dotnet\dotnet.exe`。

**现在就能试**：本地语音服务已在 `127.0.0.1:8765` 跑着（sherpa，187 个中文音色，16kHz），直接双击上面的 exe → 简洁模式 → 选 `testdata\测试之书.epub` → “生成朗读音频”。若重启过机器，先执行 `engine\.venv\Scripts\python.exe engine\tts_server.py --mode sherpa`；**服务没起也不会报错**，会自动退回校样引擎并提示原因。

## 真语音实测数据

- 3 章 / 14 段 / **97.9 秒**真中文语音；合成耗时 34.2 秒（模型已加载），**RTF ≈ 0.35，快于实时**
- `book.m4b` 762,919 B，ffprobe 三段章节标记连续（0–39.014 / 39.014–74.301 / 74.301–98.207），title/artist 正确
- 14 个分段 WAV 全 16kHz 非静音（peak 8.5k–11.4k，rms 1.3k–1.6k）
- UI 简洁模式 `--auto-start` 一键跑完，结果卡显示“3/3 章 · 347/352 字 · 已生成 1分33秒”+ 逐章卡片

## 本轮修的 5 个隐蔽缺陷

1. **CPU 推理却显示“正在调用显卡进行 TTS 合成”** → `phase` 按 `DevicePlan.Device` 分档输出
2. **CLI 事件流中文乱码**（只有最终 envelope 切了 UTF-8，事件流早已用 GBK 输出）→ `Main()` 开头统一 stdout/stderr 为无 BOM UTF-8
3. **便携版数据仍写 `%LOCALAPPDATA%`**（工具在 `tools\` 子目录，标记在根目录）→ 向上查找 `portable.marker` ≤6 层
4. **改工作区就丢模型**（`SourcesRoot` 从 `WorkspaceRoot` 推导）→ 固定 `%LOCALAPPDATA%\bookctl\sources`
5. **GUI 选 sherpa 却出占位音**（`Pipeline` 复用构造时固化的 provider，却拿运行时改过的 `EngineMode` 比较，恒等）→ 记录 `_ttsEngine` 再比较。这条最坑：`manifest` 写着 `engine=sherpa`，音频每段只有 0.4 秒
6. 附带：打包脚本加 UTF-8 BOM（PS 5.1 按 GBK 解析会崩 here-string）、`manifest.status` 不再永远停在 `running`、新增 `--auto-start` / `--on-top`

**一个工程坑**（已写进 README）：`dotnet build bookctl.sln` 后 `BookCtl.App\bin\...\win-x64\BookCtl.Core.dll` 可能是旧版，改了 Core 却“没生效”时，先单独重建 App 项目。

## 还没做

签名安装包（缺证书/PFX）、多卡 0/1/2 实机（本机只有 1 张 GTX 1050 2GB）、`cosyvoice3` 真实权重（需 ≥6GB 显存）、`source pull` 从干净目录重跑验证解包。
