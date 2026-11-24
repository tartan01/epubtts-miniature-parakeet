# 02 · 六阶段流水线、状态机与产物契约

> 实现位置：`src/BookCtl.Core/Pipeline.cs`（`PipelineRunner` / `EventBus` / `WorkerHost`）、
> `Epub.cs`、`Engine.cs`、`JobStore.cs`。架构定义见 `architecture-epub2audiobook.md §3`。

## 1. 六阶段 DAG

| Stage | 名称 | 做什么 | 关键实现 |
|---|---|---|---|
| S1 | `S1_INGEST` | 打开 EPUB（zip），读 `META-INF/container.xml` → OPF → `manifest` / `spine`；判定 DRM（有加密直接拒） | `Epub.ParseAsync` |
| S2 | `S2_SPLIT` | 用 NCX / `nav.xhtml` 的 TOC anchor 切章；跳过错别字目录、版权页、索引、空白段 | `Epub.SplitChapters` |
| S3 | `S3_NORMALIZE` | 中文文本正则化（TN）、分句；**过滤纯空白段**（`IsNullOrWhiteSpace` 直接 continue） | `TextNormalizer` |
| S4 | `S4_SYNTH` | 逐段调 TTS；章节级 checkpoint；实时发出 `segment_done` | `ITtsProvider.SynthesizeAsync` |
| S5 | `S5_VERIFY` | ASR 回验适配点（当前 `verify="skipped"`） | 预留，见 `docs/roadmap.md` |
| S6 | `S6_MASTER` | 章内 WAV 拼接 → `audio.wav` → ffmpeg `audio.m4a` → 整本 `book.m4b` + `ffmetadata` 章节标记 → `manifest.json` / `qc/report.json` | `AudioMaster` |

### EPUB 解析的 Windows 坑（已处理）

- `mimetype` 必须是 zip 第一个条目且不压缩；zip 内路径统一正斜杠 `/`。
- 中文路径 / 中文书名一律 UTF-8；XHTML entity 需解码；CSS `display:none` 的内容要跳过。
- 不用 `EbookLib`（AGPL 会污染），自写 zip + XML 解析。

### S4 之前的 DevicePlan

```csharp
var plan = DevicePlanner.Resolve(cfg);          // auto|cpu|gpu|hybrid + GpuIndex + CpuThreads
var profile = SynthesisProfile.From(cfg, model, plan);
Emit(job, "info", "S4_SYNTH", "gpu_status", ...,
    new { gpu = g0, device_plan = plan.ToDto(), engine = cfg.EngineMode, model,
          voice = cfg.Voice, profile_hash = profile.Hash });
```

**设备不参与 `profile_hash`**：换显卡 / 换设备不应改变产物确定性判定。

### 音频工程要点

- `Wav.Concat(...)`：按首个 WAV 的采样率写 header，处理空拼接。
- `AudioMaster.ProbeDuration(path)`：WAV 读头 → 非 WAV 用 `ffprobe` → 兜底按 AAC 64 kbps 估算。
  **不要用 `Wav.DurationOf` 读 `audio.m4a`**（曾导致 `WAV_INVALID`）。
- `concat_list.txt`：路径统一正斜杠并转义单引号。
- `AudioMaster.Run()`：并发 `ReadToEndAsync()` 读 stdout/stderr，
  参数带 `-hide_banner -loglevel error -nostats`，超时 `Kill(entireProcessTree: true)`。
  **不读管道会让 ffmpeg 写满 4KB 后死锁**（曾卡死 3 分钟）。
- `book.m4b` 用 `ffmetadata` 写章节标记，可用 `ffprobe -show_chapters` 验证。

## 2. 状态机与 JobStore

```
queued → running → completed
              ├→ cancelled
              └→ failed / WORKER_LOST ──(job resume)──> running
```

- **SQLite WAL**：`jobs` / `chapters` / `events` 三表。
- **CAS + revision**：并发更新靠 `WHERE revision = ?`，避免双 worker 抢同一章。
- **幂等键**：同一 `(input sha, profile_hash, config_hash)` 确定性产物；
  `run` 命中 `failed` / `cancelled` 的旧任务会自动 `JobResume`。
- **章节级 checkpoint**：每章 `done` 后落库；resume 时按章节表**重算**
  `ChaptersDone / CharsDone / AudioSeconds`（不在旧计数上累加），done 章节不再 `++`。
- **事件 NDJSON**：`JobEvent` 追加写，`job logs` 可回放；GUI 中控台直接消费 `EventBus`。
- **协作取消**：`job cancel` 置标志，S4 在段边界检查后退出。
- **孤儿任务回收**：`worker.lock` 存 PID；`WorkerHost.Start()` 调 `RecoverOrphans()`，
  把残留 `running` 标为 `WORKER_LOST` 并允许 resume；`FindRunningJobIds()` / `FailIfStillRunning()` 做 CAS。
- **完成态清理**：`ErrorCode = null; ErrorMessage = null;`（否则 resume 成功后仍挂 `WORKER_LOST`）。

## 3. 关键事件契约（GUI 中控台的数据来源）

| 事件 | 关键字段 | 消费方 |
|---|---|---|
| `phase` / `gpu_status` | `phase` 文案、`device`、`device_display`、`device_plan`、`profile_hash` | 状态卡 |
| `segment_done` | `chunk_id`, `chars`, `seconds`, `speed`(字/秒), `rtf`, `eta`, `device`, `device_display` | 进度条 / 速度卡 |
| `chapter_done` | `chapter_index`, `title`, `seconds`, `audio_path` | GUI **追加章节卡片** |
| `job_completed` | `result_path`, `chapters_done`, `chars_done`, `audio_seconds` | 结果卡 / 打开文件夹按钮 |
| `job_failed` / `job_cancelled` | `error_code`, `error_message` | 错误提示 |

> `phase` 文案必须按 `DevicePlan.Device` 分档输出：GPU / GPU+CPU 层卸载 / CPU（N 线程）。
> 早期硬编码"正在调用显卡进行 TTS 合成"在 CPU 推理时是**撒谎**，已修（缺陷 #6）。

## 4. 产物契约（"分类整理好"的落地定义）

```
<workspace>/<sha>_<书名>/
├── book.m4b                       整本有声书（含章节标记 + title/artist 元数据）
├── book.ffmetadata                ffmpeg 章节标记源文件
├── concat_list.txt                ffmpeg concat 列表
├── manifest.json                  ★ 总清单：status / engine / profile / totals / chapters
├── qc/report.json                 质检（verify=skipped / degraded / mock 说明）
├── chapters.json                  章节索引
├── segments.json                  分段文本 + 时长
├── segments/*.wav                 逐段原始 WAV（sherpa 为 16 kHz）
└── audio/chapters/000N_章名/
    ├── audio.wav                  章内拼接 WAV
    ├── audio.m4a                  ffmpeg 压缩后单章音频
    └── chapter.json               单章元数据（标题、字数、时长、状态）
```

**`manifest.json` 的 `status` 必须是 `completed`**：早期在 job 终态落定前就写文件，导致永远是 `running`（缺陷 #12）。
现在顺序为：更新 job 终态 → 存库 → 再写 `manifest.json` / `qc/report.json`。

## 5. 实测基线

| 引擎 | 章节 | 分段 | 音频时长 | 合成耗时 | RTF | M4B 大小 |
|---|---|---|---|---|---|---|
| `mock`（校样） | 3 | — | 10.154 s（模拟值，每字 10 ms） | ~2 s | — | 小 |
| `sherpa`（真语音） | 3 | 14 | 97.9~98.2 s | 首次 57.8 s / 模型已加载 34.2 s | ≈0.35 | 762,919 B |

`sherpa` 分段 WAV 全为 16 kHz 非静音（peak 8.5k–11.4k，rms 1.3k–1.6k）。
`ffprobe` 章节时间轴：`0–39.014 / 39.014–74.301 / 74.301–98.207`。

## 6. 回归验证命令

```bash
ROOT="C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07"
BOOKCTL_WORKSPACE="$ROOT/testdata/ws-regression" \
  "$ROOT/src/BookCtl.Cli/bin/Debug/net10.0/bookctl.exe" \
  run --input "$ROOT/testdata/测试之书.epub" --engine mock

FP="/c/Users/romanovsky/AppData/Local/Microsoft/WinGet/Packages/Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe/ffmpeg-9.1-full_build/bin/ffprobe.exe"
"$FP" -v error -show_entries "format=duration,size:format_tags=title,artist" \
  -show_chapters -of json "$ROOT/testdata/ws-regression/1a8eed25f9db_测试之书/book.m4b"
```
