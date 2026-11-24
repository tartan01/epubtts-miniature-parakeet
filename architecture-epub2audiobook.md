# Windows 本地 EPUB → 分章节有声书自动化框架 · 架构设计（评审稿 v0.1）

> 工作代号 **bookctl**（CLI 可执行名，可改）｜调研基准 2026-11-23｜面向架构师评审，尚未开工实现
> 目标：把一本 `.epub` 交给你，软件在本地显卡上用开源 TTS 全自动转成「按章节分类整理好的整本朗读音频」；全程可由代码/Cmd 驱动、大模型友好、并原生支持 MCP 控制。

---

## 0. 一句话结论与差异化定位

调研了现有开源方案后，结论很明确：**没有一个是为"自动化系统"做的，都是为人做的**。

| 现有项目 | 输入→输出 | TTS 后端 | 致命缺口（相对本需求） |
|---|---|---|---|
| `audiblez` v4 (MIT) | EPUB → 每章 WAV → M4B | Kokoro-82M | 中文只是 partial、无音色克隆、>400 token 赶速；无 job/API/MCP，交互选章节 |
| `ebook2audiobook` (Apache-2.0) | EPUB/MOBI → M4B | 默认 XTTS-v2 | **XTTS 权重 CPML 明确非商用**且 Coqui 已倒闭；Gradio `0.0.0.0` 无鉴权；无状态机/断点续传契约 |
| `epub2tts` / `-kokoro` | EPUB → M4B | XTTS / Kokoro | 章节预处理**全手工**；Windows 上 DeepSpeed 难装；无机器可读控制面 |
| `Audiobook-Creator` | EPUB → M4B | Kokoro / 云端 | 靠 Playwright 爬站点；无离线 GPU 引擎抽象 |
| `Pandrator` | EPUB → 有声书 | XTTS / 多后端 | 重 GUI + 一键安装器，定位个人用户而非无人值守系统 |

**共同缺口 = 你的机会**：
① 默认后端没有一个同时满足「中文整本书 + 2026 级自然度 + 许可证干净可商用」；
② 没有一个把「一本书」建模成**可持久化、可中断、可查询、可取消、可幂等重入的 Job**；
③ 没有一个把控制面做成 **CLI 契约 = MCP 契约 = 同一份 handler** 的大模型友好接口。

所以本框架不是"再造一个 TTS GUI"，而是：**在成熟开源引擎之上，补一层工程化控制面与可复现流水线**。TTS 引擎是可插拔的 Provider，不绑定某个模型。

---

## 1. 需求拆解与设计原则

| 原始诉求 | 工程化翻译 |
|---|---|
| Windows 端软件、自动化、由代码驱动 | 无 GUI 主路径；`exe` 单入口；所有能力都有命令与退出码；无交互提示 |
| 符合 Cmd 命令控制、纯支持它来控制 | CLI-first：stdout 只出机器数据，stderr 只出日志；CMD / PowerShell 5.1 均可跑中文路径 |
| 大模型友好 | 自描述（`--capabilities` / `--schema`）、可验证 `--dry-run`、原子操作、确定性输出、可自我修复的结构化错误、幂等 |
| 支持 MCP 控制 | MCP Server 是 CLI 的薄适配层，不是第二套实现；stdio 为主 + 可选 Streamable HTTP |
| EPUB 输入 | 结构化解析 TOC/spine/anchor，按章节切分并分类 |
| 开源语音框架 + 本地显卡 | 完全离线推理，GPU 单实例锁 + 显存预算，无任何云端依赖 |
| 整本朗读、按章节分类整理 | 每章独立音频文件 + 目录/编号 + 章节清单；额外产出整本 M4B（带章节标记） |

**六条硬原则（架构约束，不可后补）**
1. **单一事实源**：CLI 与 MCP 共用同一组纯函数 handler；handler 不 print、不 `sys.exit`、不认识 JSON/text。
2. **异步 Job 化**：任何 >2s 的操作只做「提交 / 查询 / 取消」，立即返回 `job_id`。MCP 2026-07-28 无状态化已移除 SSE 重投，长连接跑整本书断流即全损。
3. **章节 = 幂等单元**：每章独立 checkpoint，失败只重跑一章；取消在章节边界生效。
4. **产物落盘、接口只给 URI**：音频不进 stdout、不进 tool content，只给 `resource_link` + sha256 + 时长。
5. **确定性**：同一 `(输入 sha256, synthesis profile hash, config_version)` → 逐字节一致产物；synthesis profile 全书只算一次并锁定音色，绝不逐段重新克隆。
6. **零隐式交互**：非 TTY 自动 `--no-input`；缺参数立即结构化报错并给出可复制的正确命令。

---

## 2. 总体架构（四层 + 两个横切）

```
┌──────────────────────────────────────────────────────────────────────┐
│ L1 控制面  Control Plane（薄、秒级响应、无状态）                        │
│  ┌────────────────────────┐        ┌──────────────────────────────┐  │
│  │ CLI  bookctl.exe       │        │ MCP Server (mcp SDK 2.x)     │  │
│  │ Typer / JSON envelope  │        │ stdio 主 / Streamable HTTP   │  │
│  │ --capabilities --schema│        │ tools + resources + prompts  │  │
│  └───────────┬────────────┘        └──────────────┬───────────────┘  │
│              └──────────────┬─────────────────────┘                  │
│                             ▼  同一份 Handler（纯数据 I/O）            │
├──────────────────────────────────────────────────────────────────────┤
│ L2 编排层  Job Manager（唯一有状态的组件）                              │
│  SQLite WAL · ULID job_id · 幂等键 · revision CAS · 状态机              │
│  章节级 checkpoint · 协作式取消 · JSONL 事件日志 · GPU 单实例锁          │
├──────────────────────────────────────────────────────────────────────┤
│ L3 流水线层  Pipeline（6 Stage DAG，可单独调用、可补偿）                 │
│  S1 ingest → S2 split → S3 normalize → S4 synth → S5 verify → S6 master │
├──────────────────────────────────────────────────────────────────────┤
│ L4 引擎层  Provider（可插拔，进程隔离）                                 │
│  TTS: CosyVoice3(默认) | Qwen3-TTS | VoxCPM2 | GPT-SoVITS | Kokoro(校样)│
│  ASR: faster-whisper / FunASR（回验）  ·  Audio: ffmpeg + ffmetadata    │
├──────────────────────────────────────────────────────────────────────┤
│ 横切 A 存储：workspace/<book-id>/  产物 + manifest.json + checkpoint    │
│ 横切 B 可观测：NDJSON 事件流 / heartbeat.json / doctor / GPU 预算       │
└──────────────────────────────────────────────────────────────────────┘
```

**进程形态（关键决策）**：`bookctl` 是控制面短命进程；真正干活的是**常驻 Worker**（已登录用户会话中的 daemon，非 SYSTEM 服务——Session 0 拿不到交互式 GPU）。CLI/MCP 只与 Worker 通过 SQLite + 文件通信。类比 Ollama：CLI 是遥控器，daemon 扛重活。好处是客户端断开、MCP host 重启、模型崩溃都不影响任务推进，重连后凭 `job_id` 继续。

---

## 3. 数据流水线（Stage DAG）

```
.epub (zip)
  │
  ▼ S1 INGEST ── 校验 zip / META-INF/container.xml / OPF；DRM 判定（见 3.1）
  │              → book.json：title/creator/language/cover/input_sha256/spine[]
  ▼ S2 SPLIT ─── 解析 nav.xhtml(EPUB3) 或 NCX(EPUB2) + OPF spine
  │              按 TOC anchor 二次切分（一个 xhtml 多章节是中文 EPUB 常态）
  │              → chapters[]：{id,index,level,title,href,anchor,src_start,src_end,kind}
  │              kind ∈ frontmatter|toc|body|appendix|footnote|bibliography|cover
  ▼ S3 NORMALIZE ─ HTML → 语义块（heading/p/quote/list/table/code/figure/footnote）
  │              剔除 frontmatter/toc/版权页/脚注标记/代码块/URL（可配置保留策略）
  │              WeTextProcessing 中文 TN（数字/日期/货币/度量/缩写）+ ITN 备用
  │              发音词典 + 多音字拼音注入（专名表，用户可编辑 pronunciation.toml）
  │              语义分句 20–80 字，绝不切在数字/引号/音素中间
  │              → segments[]：{chunk_id,chapter_id,text_raw,text_norm,hash,prev_ctx,next_ctx}
  ▼ S4 SYNTH ──── 锁定 synthesis_profile（voice/clone id、model_version、speed、
  │              sample_rate、seed、词典版本、全部采样参数 → profile_hash）
  │              Provider 常驻显存 + 章节级队列；逐段 WAV；段级内容寻址缓存
  │              失败段：retry（换 seed / 提高 repetition_penalty）→ 隔离不阻塞全书
  │              → segments/*.wav + segments.json（时长、sha256、profile_hash）
  ▼ S5 VERIFY ─── faster-whisper 转写 → 与 text_norm 做 ITN 归一化后比对 CER
  │              超阈值（默认 5%）自动重生成该段，最多 N 次；仍失败标 degraded 并写清单
  │              抽检：静音检测、时长异常、削波、爆音
  ▼ S6 MASTER ─── 段落间 12ms fade + 段落停顿 → 章内拼接
  │              ffmpeg 两遍 loudnorm（EBU R128，-16 LUFS，显式 -ar 48000）
  │              单遍 loudnorm 会 riding gain 导致章节开头爆音，必须两遍
  │              章末插静音 → 每章独立 .m4a/.mp3（按编号+标题分类整理）
  │              合并 + FFMETADATA（media_type=2, CHAPTER 起止）+ 封面/作者
  │              → book.m4b（Apple Books / VLC / 播客客户端可识别章节）
  ▼
manifest.json（含每章 sha256/时长/状态/锚点/引擎版本/config_hash）
```

### 3.1 解析层要点（最容易低估的地方）

- **不要用 EbookLib 做语义切分**：它只做对象级读写，且是 **AGPL-3.0**（商用产品有传染风险）。建议自写 `zipfile + lxml` 轻量解析器：容器 → OPF → nav/NCX → spine → anchor 切片。代码量约 300–500 行，换来零许可风险与完全可控。
- **DRM 误判**：`META-INF/encryption.xml` 只含字体混淆算法时**不是 DRM**，不能据此拒绝正版书；只有 `rights.xml` / 内容加密才是真 DRM → 明确结构化报错 `DRM_PROTECTED`，不尝试破解。
- **目录与 spine 不一致**：以 TOC 定义章节边界，以 spine 保证阅读顺序；TOC 缺失时退化到 `<h1>~<h3>` 启发式，并在 manifest 里标 `split_strategy: heuristic` 供人工复核。
- **保留两份文本**：`text_raw` 与 `text_norm`，附字符偏移映射，便于"念错一个字 → 定位原文 → 只重生成该段"。

### 3.2 音色一致性（整本书成败点）

全书只在 S4 启动时计算一次 speaker prompt（参考音频 / VoiceDesign 描述 / 指令控制），生成 `profile_hash` 写入 manifest，**所有段落复用**；禁止逐段随机 seed、逐段重新克隆。中文社区实践：固定全局风格向量 + 段落短句化，可显著抑制 500 字后的语调漂移。参考音频需校验 SNR、时长、转写匹配度（带噪 prompt 会直接把底噪传染给 15 小时成品）。

---

## 4. 技术选型（含否决理由）

### 4.1 TTS 引擎

| 决策位 | 选择 | 关键依据 | 代价 |
|---|---|---|---|
| **默认主选** | **CosyVoice 3 / Fun-CosyVoice3-0.5B-2512（优先 `_RL`）** | 代码+权重双 Apache-2.0；中文 test-zh CER 1.21%→**0.81%(RL)**（人类 1.26%）；**内置自训练 TN，数字/日期/符号直接读**，中文整本书最省前端；拼音修正多音字；Win11 + RTX 3060 **6GB** 有实测通路；vLLM 0.11.x / TensorRT-LLM 4× / FastAPI + gRPC 服务化齐全 | 依赖链重（Python 锁 3.10、git submodules、sox、`openai-whisper` 安装顺序坑）；官方服务化推荐 16GB（纯推理 6–8GB） |
| 备选 A | **Qwen3-TTS-12Hz-1.7B / 0.6B** | 长文本硬指标最强：官方 10 分钟 WER 2.36%，第三方 21 分钟/1.2 万字无吞音重复、音色相似度 92.3%±1.7%；Apache-2.0；**VoiceDesign 免参考音频** | RTF ≈1.26 @3090（慢于实时），20 万字书 GPU 时间约 19h；Windows 部署资料少 |
| 备选 B | **VoxCPM2** | 双 Apache-2.0、48kHz、免参考音频、8GB 显存、RTF 0.30→0.13；6.25Hz patch 天生适合长序列 | 官方承认长文本会漂移，必须分块；Windows Triton 需 `optimize=False`；无原生 vLLM |
| 备选 C | **GPT-SoVITS V4** | 显存 4–8GB、**RTF 0.014 @4090**（吞吐之王）、MIT、Windows 一键包、微调后音色最稳 | 需 1 分钟干净干音 + ASR 标注校对；标注错全崩；24kHz 需超分 |
| **降级/校样** | **Kokoro-82M** | Apache-2.0、310MB、CPU 可跑、RTF 极低 | **仅用于快速试听/流水线冒烟测试/无 GPU 兜底**，不作为交付引擎（中文 partial、>400 token 赶速、无克隆） |

**明确排除（请写进 ADR，避免后期有人"顺手换回来"）**：

| 引擎 | 否决理由 |
|---|---|
| Coqui XTTS-v2 | **CPML 1.0 明确非商用**，且 Coqui 公司 2024-01 倒闭、coqui.ai/cpml 已 404，**再无主体可签发商用许可** —— 法律死路（ebook2audiobook 默认就是它） |
| F5-TTS 官方权重 | 权重 **CC-BY-NC-4.0**（Emilia 数据），代码 MIT 救不了 |
| 微软 VibeVoice | 技术上最诱人（90 分钟不漂移），但**每个文件强制插入可听的 "This segment was generated by AI" + 水印**，模型卡 research-only → 有声书直接不可用 |
| Fish Speech S2-Pro / Drama 3 | S2-Pro 权重研究非商用；Drama 3 需 ≥48GB FP8 且仅 Linux/WSL 文档化；社区许可禁托管 |
| ChatTTS / Bark / WhisperSpeech | AGPL+CC-BY-NC 双限 / 架构性 13 秒上限且幻觉多 / **发布版仅英语不支持中文** |
| IndexTTS-2.5 | 条件商用许可 + Windows DeepSpeed/CUDA 12.8+/triton-windows 坑 + **分段拼接处韵律不连贯**。中文小说场景可留作后续实验 Provider，但不进 v1 默认 |

> ⚠️ 数据口径说明：CER/WER/RTF/显存多来自厂商自报或单一第三方实测，硬件与精度不同不可横向相减。Phase 0 必须在目标机上做一次「同一章、同一 profile」的 A/B 盲测再冻结引擎。

### 4.2 其余组件

| 层 | 选择 | 理由 / 注意 |
|---|---|---|
| EPUB 解析 | 自写 `zipfile + lxml`（参考 epub2text 的 nav-anchor 思路） | 避开 EbookLib AGPL；完全掌控 anchor 切分 |
| 中文 TN | WeTextProcessing（WFST/Pynini） + 自建发音词典 | Windows 需 `conda install -c conda-forge pynini`；ITN 反用于 ASR 回验比对 |
| 脏文本分句 | wtpsplit 2.x + 规则兜底 | 语义边界优先于字数 |
| ASR 回验 | faster-whisper（CTranslate2） | CPU/GPU 均可，显存与 TTS 错峰调度 |
| 音频 | ffmpeg 6+/7 + ffmetadata | 两遍 loudnorm、M4B 章节标记、`media_type=2` |
| CLI | Typer 0.29 + Rich（help 用） | 类型注解即 schema；管道中禁用颜色/动画 |
| MCP | 官方 `mcp==2.3.x`（pin） | 对齐 2026-07-28 无状态协议；FastMCP 4 仅当需要 background tasks 时再引入 |
| 存储 | SQLite WAL + 原子 `os.replace` 写文件 | 对 Windows 文件锁更友好；checkpoint 不半成品 |
| 打包 | uv + portable zip（v1）→ PyInstaller `--onefile --noconsole` / MCPB（v2） | GUI 客户端读不到 shell PATH，`mcp.json` 里 `command` 必须绝对路径 |

---

## 5. 产物契约（"分类整理好"的落地定义）

```
C:/ProgramData/bookctl/workspace/<book-id>/        # book-id = sha256(epub)[:12]
├── manifest.json              # 全书总账：元数据 + 每章状态 + profile_hash + config_hash
├── book.json                  # S1 产物：OPF 元数据、spine、DRM 判定
├── chapters.json              # S2 产物：章节树（id/level/title/anchor/kind）
├── segments.json              # S3+S4：段落清单与音频映射
├── pronunciation.toml         # 可编辑：专名→拼音/读法（进 profile_hash）
├── audio/
│   ├── chapters/
│   │   ├── 0001_序章/          audio.m4a  chapter.json
│   │   ├── 0002_第一章_xxx/    audio.m4a  chapter.json
│   │   └── ...
│   └── book.m4b               # 整书（带 CHAPTER 标记、封面、作者）
├── book.ffmetadata            # 可复核的章节元数据文本
├── cover.jpg
├── checkpoints/               # 章节级断点 + profile.json
├── logs/job.ndjson            # 单调递增 cursor 的事件流
└── qc/report.json             # ASR 回验 / 响度 / 静音抽检报告
```

`manifest.json` 核心字段（这就是给大模型读的"地图"）：
```json
{
  "api_version": "1.0",
  "book_id": "9f2c1ab4e7d3",
  "input": {"path": "C:/books/xxx.epub", "sha256": "sha256:...", "epub_version": "3.0"},
  "engine": {"name": "cosyvoice3", "model": "Fun-CosyVoice3-0.5B-2512_RL", "version": "..."},
  "synthesis_profile_hash": "sha256:...",
  "config_hash": "sha256:...",
  "status": "completed",
  "totals": {"chapters": 37, "audio_seconds": 55213, "gpu_seconds": 16420},
  "chapters": [
    {"id": "ch-0002", "index": 2, "title": "第一章 xxx", "kind": "body",
     "audio": "audio/chapters/0002_第一章_xxx/audio.m4a",
     "sha256": "sha256:...", "seconds": 1842, "status": "completed",
     "qc": {"cer": 0.021, "lufs": -16.1, "degraded_segments": 0},
     "source": {"href": "text/part0004.xhtml", "anchor": "ch1"}}
  ]
}
```

---

## 6. 控制面契约

### 6.1 CLI 命令表（统一 envelope `{"ok":bool,"api_version":"1.0","data":…}`）

| 类别 | 命令 | 说明 |
|---|---|---|
| 自省 | `--capabilities` / `--schema` / `commands` / `version` / `doctor` | 能力清单、导出全命令 JSON Schema、环境自检（编码/GPU/ffmpeg/模型/权限） |
| 探测（原子、只读） | `book inspect --input <path>` | 章节数、编码、DRM、预估时长，**不改状态** |
| | `book chapters --input <path>` | 输出章节树 JSON，供人工/LLM 剔除前言版权页 |
| | `book plan --input <path> --dry-run` | 输出步骤序列、预计产物路径、GPU/耗时估算、`repro_command` |
| 引擎 | `engine list` / `engine test --name --text` | 列出 Provider 与显存需求；单句试听（校样用 Kokoro） |
| 任务 | **`job start --input … --preset zh-default --idempotency-key …`** | <100ms 返回 `job_id` |
| | `job status <id> --wait-ms 0` / `job list --status --cursor` | 状态/进度/checkpoint/错误 |
| | `job logs <id> --since <cursor> --limit 200 --follow` | NDJSON 事件 tail，cursor 单调递增 |
| | `job cancel <id> --reason` / `job resume <id>` | 协作式取消 / 断点续传 |
| | `job artifacts <id>` | 产物 URI + sha256 + 时长清单 |
| 导出 | `book export <id> --format m4b|mp3|m4a --out` | 确定性、可重复执行 |
| 服务 | `serve --mode daemon` / `mcp serve --transport stdio|streamable-http` | 常驻 Worker / MCP 入口 |

**全局 flag**：`--output-format json|jsonl|text`（非 TTY 强制 json）、`--json`、`--quiet`、`--fields`、`--jq`、`--nul`、`--no-input`、`--yes`、`--dry-run`、`--config`、`--log-level`、`--no-color`、`--debug`。

退出码（跨子命令稳定）：`0` 成功（异步为 `queued`）、`1` 运行失败、`2` 参数、`3` not found、`4` 权限、`5` 冲突/非法状态转移、`69|75` 可重试 transient、`130` 取消。

结构化错误（写 stderr，`--json` 下同构）：
```json
{"ok":false,"api_version":"1.0","error":{
  "code":"IDEMPOTENCY_KEY_CONFLICT","message":"该 key 已绑定不同参数",
  "field":"idempotency_key","value":"bk-001",
  "fix":"换用新 key，或查询已存在任务",
  "example":"bookctl job status 01JCE7QK8ZP4M3N5R7T9V2X",
  "retryable":false,"suggestions":["bookctl job list --status running"]}}
```

### 6.2 MCP Tools（薄适配，SLA ≤2s）

| tool | annotations | 输入 → 输出要点 |
|---|---|---|
| `capabilities` | readOnly, idempotent | `{}` → 版本/平台/GPU/models/limits/协议版本/presets |
| `plan` | readOnly, idempotent | 同 `job_start` → dry-run plan JSON |
| `job_start` | idempotent | `input_uri, preset, idempotency_key, options` → `job_id` + `resources{}` + `poll_interval_ms` |
| `job_status` / `job_list` | readOnly | 状态机 + 单调递增 progress |
| `job_logs` | readOnly | `{lines[], nextCursor, truncated}` |
| `job_cancel` / `job_resume` | idempotent | 协作式 |
| `job_artifacts` | readOnly | 返回 `resource_link`，**不嵌入二进制** |
| `book_inspect` / `book_chapters` | readOnly | 只读探测 |
| `engine_list` / `engine_test` | readOnly | 引擎清单与单句试听 |
| `doctor` | readOnly | 自检 |

**Resources**：`job:///{id}/manifest.json`、`result.json`、`log.ndjson`、`checkpoint.json`、`artifacts/{chapter_id}.m4a`（大文件给 `file:///` 让宿主直读）。
**Prompts**：`operate_playbook` —— 把标准编排顺序（capabilities → plan → start → 轮询 → logs → artifacts）直接喂给宿主模型，无需外部文档。

### 6.3 MCP 协议版本策略（2026 现状，容易踩坑）

- 官方最新修订 **2026-07-28**：移除 `initialize`/session、新增 `server/discover`、日志走 stderr（弃用 `logging/setLevel`）、SSE 可恢复性移除、Tasks 变为 Beta 扩展。
- 实现：**双版本兼容**（`2026-07-28` + `2025-11-25`），用 `server/discover` 探测；不按单一错误码回退。
- `structuredContent` 顶层**一律 object**（兼容 2025-11-25 及更早），同时序列化一份到 `TextContent`。
- **显式 `job_*` tools 是唯一稳定面**；官方 Tasks 扩展只做可选增强（Beta，客户端支持矩阵不一）。
- Windows stdio 三大坑：stdout 必须 `buffer.write(utf-8 bytes)+flush`（`chcp 65001` 救不了管道，cp936 会截帧导致"工具列表假缓存"）；日志全部 stderr；`env` 里注入 `PYTHONUTF8=1`、`PYTHONIOENCODING=utf-8`、`APPDATA` 绝对路径。

### 6.4 任务状态机

```
job_start ─► queued ─► running ◄──► paused
                │         │  ▲          │
       cancel ───┤         │  │ resume   ├─► cancel_requested ─► cancelled (终态)
                │         ▼  │          │
                │    input_required ────┘
                │         ├──► completed (终态，result.json 原子落盘)
                │         └──► failed (终态；retryable+checkpoint 经 job_resume 可回 running)
                └─────────────────────► cancelled
```
- 每次转移用 `revision` 做 CAS 乐观锁；转移事件追加 `log.ndjson`。
- `progress.monotonic` 全局单调递增，换 phase 不归零；章节级为主控粒度，句子级只进 JSONL 日志（不逐句发 progress）。
- 幂等键：客户端提供或从 `(command, canonical_args_hash, input_sha256, config_version)` 派生；同 key 同参返回**逐字节相同**输出，同 key 异参 → 退出码 5。
- 取消是协作式，worker 在章节边界检查 `cancel_requested`；超时才用 Windows Job Object 强杀整棵进程树并回收显存。

---

## 7. 质量保障与可观测性

| 机制 | 具体做法 |
|---|---|
| ASR 回验闭环 | 合成 → faster-whisper 转写 → ITN 归一化比对 CER；>5% 自动重生成该段（换 seed / `repetition_penalty`），最多 N 次；仍失败标 `degraded` 并列入 `qc/report.json` |
| 响度与拼接 | ffmpeg **两遍** `loudnorm`（-16 LUFS, `linear=true`, 显式 `-ar 48000`）；12ms fade；章末静音；削波/长静音检测 |
| 内容寻址缓存 | 段级 key = sha256(text_norm + profile_hash + engine_version + params)，重跑不重复烧 GPU |
| 抽检 | 每章 1 段 ASR 全量 + 全书时长异常/静音/爆音扫描；报告可机器读 |
| 可观测 | `log.ndjson`（cursor 单调）、`heartbeat.json`、`doctor --json`、`nvidia-smi` 显存预算告警 |
| GPU 隔离 | 单实例文件锁（含 PID + device），`max_concurrent_jobs=1`，`gpu_memory_budget_mb`；TTS 与 ASR 错峰 |

**必须写进部署手册的 Windows 坑**：`TORCH_COMPILE_DISABLE=1`（<16GB 显存首次合成触发 Triton 内核编译直接 OOM）；CUDA ≥12.8（Blackwell 50 系必须）；PyTorch 不捆绑 triton，锁 `triton-windows 3.4.x`；不用 DeepSpeed；CosyVoice3 锁 Python 3.10、`openai-whisper` 单独装；`activeCodePage=UTF-8` 应用清单 + `longPathAware`；CMD 示例与 PowerShell 5.1 示例各给一套（`&&` 在 PS5.1 不可用）。

---

## 8. 性能与资源估算（20 万字中文书）

| 项 | 估算 |
|---|---|
| 成品音频 | ≈ 15.3 小时（按 200–220 字/分钟） |
| GPU 生成时间 | RTF 0.15 → ≈2.3h；RTF 0.30 → ≈4.6h；RTF 1.26(Qwen3-TTS 1.7B) → ≈19h |
| ASR 回验 | 额外约 0.2–0.3× 音频时长（可错峰或降为抽检） |
| 显存 | CosyVoice3 FP16 6–8GB（3060 6GB 有实测）；4bit 量化最低 ~4GB；服务化推荐 16GB |
| 磁盘 | 中间 WAV 段约 1–3GB + 成品 M4B 约 400–900MB（64k AAC） |
| 单机吞吐 | 8GB 卡：一本/夜；16GB 卡 + vLLM/TensorRT-LLM：可整章并发 |

---

## 9. 风险登记

| # | 风险 | 等级 | 对策 |
|---|---|---|---|
| R1 | 引擎 A/B 数据来自自报，目标机可能不符 | 高 | Phase 0 同章同 profile 盲测冻结；Provider 抽象保证换引擎不改上层 |
| R2 | Windows Triton / CUDA / Python 版本链 | 高 | 禁 torch.compile、锁版本、每引擎独立 venv、`doctor` 一键自检 |
| R3 | 15 小时成品出现音色漂移 / 拼接韵律断裂 | 高 | 音色 profile 锁定 + 20–80 字分句 + ASR 回验 + 两遍 loudnorm；IndexTTS 类拼接短板不进 v1 |
| R4 | EPUB 结构千奇百怪导致误切章 | 中高 | 双策略（TOC anchor / heading 启发式）+ `book chapters` 人工/LLM 复核 + `--include/--exclude` 章节范围 |
| R5 | 许可证（权重 ≠ 代码 ≠ 输出；克隆真人声音权） | 中高 | 默认后端与备选全 Apache-2.0/MIT；维护 `LICENSES.md` 与 ADR；明确排除清单 |
| R6 | MCP 双协议版本与客户端差异 | 中 | `server/discover` 探测 + 显式 job tools 兜底 + 不依赖 progress 渲染 |
| R7 | 中文路径 / 编码 / Session 0 GPU 不可见 | 中 | UTF-8 二进制 stdout、UTF-8 manifest、daemon 跑已登录用户会话 |
| R8 | 书籍版权与 DRM | —— | 明确 `DRM_PROTECTED` 拒绝；仅处理用户有权转换的内容；不做绕过 |

---

## 10. 实施路线图（建议按 Phase 评审放行）

| Phase | 交付 | 验收标准 |
|---|---|---|
| **P0 技术验证**（建议先做） | 单章 A/B：CosyVoice3 / Qwen3-TTS / VoxCPM2 / GPT-SoVITS 在同一台目标机跑同一章；M4B 章节标记可行性；Windows 依赖脚本 | 出一份实测表（RTF/显存/CER/听感盲测），**冻结默认引擎与 profile** |
| **P1 最小闭环** | S1→S2→S4→S6 + CLI `book inspect/chapters` + `job start/status/logs` + 每章 MP3/M4A + manifest.json | 一本 10 章以内的 EPUB 一键出分章音频；断电后 `job resume` 只重跑未完成章 |
| **P2 质量与控制面** | S3 TN/发音词典 + S5 ASR 回验 + 两遍 loudnorm + 整书 M4B + MCP Server + `doctor` + `--capabilities/--schema/--dry-run` | MCP host 能完成「capabilities → plan → start → 轮询 → artifacts」全链路；CMD/PowerShell 中文路径可用 |
| **P3 产品化** | 多 Provider 热切换、批量书库、portable zip / PyInstaller / MCPB 分发、winget、质量报表、Tasks 扩展可选增强 | 无 Python 环境的 Windows 机器解压即用；`mcp.json` 示例三客户端通过 |

**建议 P1 之前锁定的 6 个决策点（需要你拍板）**：
1. 默认引擎是否按本报告选 CosyVoice3（还是先只做 Kokoro 校样链路验证控制面）？
2. 音色来源：预设音色 / 用户提供 10–30s 参考音频克隆 / 纯文字 VoiceDesign？
3. 交付形态：仅分章文件，还是同时强制出整本 M4B？
4. ASR 回验：全量 / 每章抽检 / 默认关闭（速度优先）？
5. Worker 形态：常驻 daemon（推荐）还是 CLI 进程内跑（简单但断连即停）？
6. 分发方式：portable zip + uv（快）还是 PyInstaller 单 exe（免环境）？

---

## 11. 主要参考来源（节选，完整清单见调研原始报告）

- MCP 规范：`https://modelcontextprotocol.io/specification/2026-07-28/changelog`、`/basic/transports/stdio`、`/extensions/tasks/overview`
- Python SDK：`https://github.com/modelcontextprotocol/python-sdk`（2.3.x / MCPServer）
- TTS：`https://github.com/QwenAudio/CosyVoice`、`https://qwen.ai/blog?id=qwen3tts-0115`、`https://github.com/OpenBMB/VoxCPM`、`https://github.com/RVC-Boss/GPT-SoVITS`
- 中文 TN：`https://github.com/wenet-e2e/WeTextProcessing`
- 现成 pipeline：`https://github.com/DrewThomasson/ebook2audiobook`、`https://github.com/santinic/audiblez`、`https://github.com/aedocw/epub2tts`、`https://github.com/oweneldridge/audiobook-creator-tts`
- 长文本工程：`https://smallest.ai/blog/long-form-tts-how-to-generate-consistent-speech-at-scale`
- 许可：`https://localaimaster.com/blog/xtts-coqui-commercial-license`（CPML 非商用且主体已注销）
- CLI 契约：`https://clig.dev/`、ffmpeg `-progress pipe:1 -nostats`、git `--porcelain=v2 -z`

> **免责说明**：本稿中的 CER / WER / RTF / 显存 / 许可证结论来自 2026-11 公开资料，商用前请复核各权重卡 LICENSE 原文并经法务确认；克隆真人声音需另行取得声音权授权。

---

# 附录 A · v0.2 变更：WinUI 桌面形态（2026-11-24，已批准进入实现）

> 本 MD 为**总架构唯一事实源**，以下为对 v0.1 的增补/修订。凡与 v0.1 冲突处，以本节为准。代码目录结构与本节 A.6 一一对应。

## A.1 需求变更

| 项 | v0.1 | v0.2（当前） |
|---|---|---|
| 主入口 | CLI-first、无 GUI 主路径 | **完全桌面软件形态（WinUI 3）**，一键导入 EPUB |
| CLI / MCP | 主入口 | **降为并列一等公民**：可由命令行/MCP 直接导入并生成，与 GUI 共用同一 Handler |
| 可观测 | NDJSON 日志 / 状态查询 | 新增**中控台 UI**：实时显示执行速度、GPU 已调用、正在转换 TTS 的阶段状态 |
| 产物 | workspace 目录 | 生成完成后**新建「项目文档文件夹」并可直接打开供人工检验** |
| 开发工具 | Python / uv / Typer | **.NET 10 + WinUI 3 + VS Code**，C# 全栈；Python 仅作 TTS 引擎 sidecar |

**不变**：四层架构、单一事实源（CLI/MCP/GUI 共用 Handler）、异步 Job 状态机、章节级 checkpoint、确定性产物、Provider 可插拔、MCP 2026-07-28 兼容策略、许可证排除清单。

## A.2 修订后的 L1 控制面

```
WinUI 3 GUI ──┐
bookctl.exe  ──┼──► BookCtlHandlers（纯数据 I/O，唯一实现）
MCP stdio    ──┘            │
                            ▼
                     JobStore (SQLite WAL，唯一状态)
                            ▲
                            │ claim / checkpoint / events
                     WorkerHost（内嵌或独立进程，单实例消费）
```

- **GUI 不再是第二套业务逻辑**：WinUI 只订阅 `EventBus` 事件 + 轮询 `job_status`，不直接碰流水线。
- **Worker 形态调整**（修订 v0.1「必须独立 daemon」）：桌面形态下 **WorkerHost 默认内嵌在 GUI 进程**，`BookCtl.Worker.exe` 作为可选的独立 daemon；两者通过 SQLite 原子领取（`TryClaimQueued` + revision CAS）保证同一 job 只被一个 worker 执行。仍保留原设计的所有优点（断连可续、状态可查、章节级恢复）。SYSTEM 服务仍禁止。

## A.3 实时状态契约（GUI 中控台的数据来源）

所有状态变化统一为 NDJSON `JobEvent`，写 `events` 表并广播：

```json
{"seq":128,"ts":"2026-11-24T15:59:33+08:00","job_id":"01J...","level":"info",
 "stage":"S4_SYNTH","code":"segment_done","message":"第 3 章 42/210 段已合成",
 "data":{"chapter_index":3,"chapter_title":"第一章","segment":42,"segments_total":210,
         "chars_per_sec":312.4,"audio_sec_per_sec":18.7,"rtf":0.053,
         "gpu":{"available":true,"name":"NVIDIA RTX 3060","util_pct":87,
                "mem_used_mb":5210,"mem_total_mb":6144,"mode":"CUDA"},
         "phase":"正在调用显卡进行 TTS 合成"}}
```

GUI 四块状态卡：**① 当前阶段**（S1…S6 / 已完成）**② GPU 状态**（未调用 / 已调用 + 型号/利用率/显存）**③ TTS 状态**（引擎名、当前章节、段进度）**④ 速度**（chars/s、audio s/s、RTF、ETA）。中控台按 `level` 着色，支持自动滚动与清空；所有事件可导出 `logs/job.ndjson`。

## A.4 产物与「项目文档文件夹」

完成后创建 `workspace/<book-id>_<书名清洗>/`，含 `manifest.json`、`chapters.json`、`audio/chapters/NNNN_标题/audio.wav(.m4a)`、`audio/book.m4b`、`logs/job.ndjson`、`qc/report.json`。GUI 完成后弹出「打开项目文件夹」按钮（`explorer.exe /select,`）。ffmpeg 不可用时降级为 WAV 拼接并在 manifest 标 `degraded`，**不影响分章产物交付**。

## A.5 技术栈与可运行性

- .NET 10；WinUI 3 / Windows App SDK（`WindowsPackageType=None` + 自包含，免 MSIX 双击可运行）；VS Code + `dotnet new winui` 模板 + WinApp CLI，无需完整 Visual Studio。
- 依赖最小化：Core 仅 `Microsoft.Data.Sqlite`；GUI 仅 WindowsAppSDK + BuildTools。MCP stdio 用 `System.Text.Json` 手写 JSON-RPC 2.0，避免 SDK 版本锁定风险。
- **双引擎模式**：`mock`（内置，生成可播放 WAV，用于无 GPU 环境跑通全链路与验收 UI）/ `sidecar`（Python `engine/tts_server.py`，对接 CosyVoice 3）。P0 用 mock 验收控制面，P1 接真实引擎。
- 新增 CLI 便捷命令 `bookctl run --input <epub>`：一键提交 + 内嵌 worker + JSONL 实时事件，等价于 GUI 的「一键生成」。

## A.6 代码目录映射

| 目录 | 对应架构层 | 职责 |
|---|---|---|
| `src/BookCtl.Core/Contracts.cs` `Models.cs` | 契约 | DTO / Envelope / 错误码 / 退出码 |
| `src/BookCtl.Core/JobStore.cs` | L2 | SQLite WAL、状态机 CAS、checkpoint、事件流 |
| `src/BookCtl.Core/Handlers.cs` | L1 | GUI/CLI/MCP 共用唯一实现 |
| `src/BookCtl.Core/Epub.cs` | L3 S1–S3 | zip+OPF 解析、TOC anchor 切章、中文 TN、分句 |
| `src/BookCtl.Core/Engine.cs` | L4 | ITtsProvider / mock / sidecar / GPU 监控 / 音频母带 |
| `src/BookCtl.Core/Pipeline.cs` | L3 S4–S6 | 六阶段编排、EventBus、WorkerHost、manifest |
| `src/BookCtl.Cli/` `src/BookCtl.Mcp/` `src/BookCtl.Worker/` | L1 / L2 | 三个可执行入口 |
| `src/BookCtl.App/` | L1 GUI | WinUI 3 主窗口、中控台、状态卡、打开产物 |
| `engine/` | L4 | Python TTS sidecar（CosyVoice 3 适配点） |
| `config/` `.vscode/` `README.md` | 横切 | 配置分层、VS Code 构建/调试任务、构建说明 |

## A.7 实现状态（本轮交付）

已交付：可编译的完整项目骨架 + mock 引擎全链路（EPUB → 分章音频 → 项目文件夹 → manifest）+ GUI 中控台 + CLI/MCP/Worker 三入口。
未在代码里实现、保留为适配点：真实 CosyVoice 3 权重加载（sidecar 中 lazy import）、WeTextProcessing TN（v1 用规则版）、faster-whisper ASR 回验（mock 模式标 `skipped`）、两遍 loudnorm 的精细参数（ffmpeg 存在时执行基础版）、MCP Streamable HTTP 传输。

> 这些适配点均已在 `ITtsProvider` / `TextNormalizer` / `AudioMaster` / MCP transport 处留出接口，接真实引擎时不动上层。

---

# 附录 B · v0.3 变更：双模式、设备选择、源管理、封装（2026-11-24）

## B.1 产品命名与形态

- 产品名：**本地朗读工厂**（备选：帅帅朗读工厂）；内部代号 `bookctl`，二进制名保持不变以兼容 CLI 脚本与 MCP 配置。
- 同一份源码产出两种封装形态：
  - **便携版**：目录内有 `portable.marker` → 数据 `data\`、模型 `sources\` 全部留在程序目录，可整目录拷到 U 盘；
  - **安装版**：Inno Setup，per-user（`PrivilegesRequired=lowest`），安装时删除 marker → 数据走 `%LOCALAPPDATA%\bookctl`。
- 两者均 self-contained（.NET 与 Windows App Runtime 都随包），目标机零依赖。

## B.2 双界面模式（L1 的新分支，不改变下层）

| 模式 | 输入 | 引擎决策 | 呈现 |
|---|---|---|---|
| **简洁模式** | 一本 EPUB | 有本地 CPU 模型 → `sherpa`；否则 `mock` 并提示下载 | 一张主卡 + 进度 + 「生成结果」卡（输出文件夹 + 逐章卡片，实时追加） |
| **详细参数模式** | 一本 EPUB | 用户显式指定引擎/音色/设备/显卡/线程/语速/M4B | 左参数卡 + 右状态卡 + 中控台事件流 |

简洁模式**不是**另一个流水线：它只是把 `device=cpu`、`engine=sherpa|mock`、`voice=0` 写进会话配置后调用同一个 `JobStart`。
保证「两种模式产出的 manifest 与章节结构完全一致」，避免出现两套产物契约。

## B.3 设备选择（新 L4 决策层）

新增 `DevicePlanner`（`src/BookCtl.Core/Device.cs`），在 S4 前解析出 `DevicePlan`：

- 输入：`Device(auto|cpu|gpu|hybrid)`、`GpuIndex`、`CpuThreads`、引擎的显存门槛；
- 输出：`device`、`gpu_index`、`torch_device`、`cpu_offload`、`reason`、`warnings`；
- 事件 `gpu_status` 携带 `device_plan`，`segment_done` 携带 `device/device_display` → GUI 两处状态卡直接消费；
- 设备**不参与** `profile_hash`：换卡不应让"同一本书产物一致"的判定失效。

决策矩阵：`auto`（显存够→GPU / 1/3 以上→hybrid / 更小→CPU）、`gpu`（强制，显存不足仅告警）、`cpu`、`hybrid`（层卸载，明确告知更慢）。

## B.4 引擎与模型源管理（新 L4 旁路）

- 内置目录写进二进制（`SourceManager.BuiltInCatalog()`），文件 `config/sources.json` 与 `sources/catalog.user.json` 可覆盖/追加；
- 支持 4 种安装类型：`archive`（tar/tar.bz2/tar.gz，调系统 `tar.exe`）、`zip`、`file`（多文件直下）、`manual`（大权重需人工放置，如 CosyVoice 3）；
- **多线路回退**：原始 URL → 用户 `mirrors` → `BOOKCTL_MIRROR` → `gh-proxy.com` / `hf-mirror.com`。实测国内直连 GitHub Release 返回 502，缺回退等于功能不可用；
- 安装完成写 `.installed.json`；CLI `source list|pull|remove`，MCP `source_list|source_pull`。

## B.5 关于"分布式 / 拆分式 TTS 推理"的结论

开源 TTS 目前**没有**能把一张小显存卡与 CPU 组合起来提速的方案：

1. 张量并行需 ≥2 张同型号、算力 ≥7.0 的卡（GTX 1050 = 6.1，vLLM/SGLang 直接拒绝）；
2. 流水线并行（`device_map` 层卸载）只解决"装不下"，CPU 内存带宽是瓶颈，通常比纯 GPU 慢 2–5 倍 → 即我们的 `hybrid`；
3. ONNX Runtime 单 session 只用一个 EP，只能设显存上限做部分卸载，session 内不跨设备并行；
4. 多进程"一章一设备"会破坏音色一致性、增加显存争用，收益为负。

因此产品定位为**三档可选**（纯 GPU / 纯 CPU / 混合兜底），选择权交给用户，决策依据在 UI 里明示（`PlanText` + `PlanHint`）。

## B.6 封装与签名

见 `packaging/README.md`：`build-portable.ps1`（发布+整理+zip）、`bookctl.iss`（Inno Setup，含 .epub 关联）、
`sign.ps1`（PFX / 证书指纹 / Azure Trusted Signing，RFC3161 时间戳）。`Edition` 字段预留 Pro 差异化。
