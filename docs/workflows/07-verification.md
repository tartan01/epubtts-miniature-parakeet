# 07 · 验收流程与实测基线（回归测试手册）

> 每次改动后至少跑 §1（快速回归）；改了引擎 / 流水线 / 封装时跑 §2–§5。
> 所有路径以 `docs/environment.md` 为准。

## 1. 快速回归（必跑，约 10 秒）

```bash
ROOT="C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07"
DN="/c/Program Files/dotnet/dotnet.exe"
CLI="$ROOT/src/BookCtl.Cli/bin/Debug/net10.0/bookctl.exe"
cd "$ROOT"

"$DN" build bookctl.sln -c Debug -v q --nologo
"$CLI" doctor --pretty
BOOKCTL_WORKSPACE="$ROOT/testdata/ws-regression" \
  "$CLI" run --input "$ROOT/testdata/测试之书.epub" --engine mock > /tmp/reg.jsonl 2>&1
```
检查：
- 编译 0 error / 0 warning；
- 输出严格 UTF-8 无 BOM，中文（`测试之书`、`已完成`）正常；
- `manifest.json` 的 `status = completed`；
- `audio/chapters/` 下 3 个章节目录齐全。

## 2. 真语音整本链路（约 1 分钟）

```bash
# 终端 A
cd "$ROOT" && ./engine/.venv/Scripts/python.exe engine/tts_server.py --mode sherpa --port 8765 --device cpu
# 终端 B
curl -sf http://127.0.0.1:8765/health
BOOKCTL_WORKSPACE="$ROOT/testdata/ws-verify-sherpa" \
  "$CLI" run --input "$ROOT/testdata/测试之书.epub" --engine sherpa --device cpu
```

**期望基线**：
| 指标 | 值 |
|---|---|
| 章节 / 分段 | 3 章 / 14 段 |
| 音频总时长 | 97.9 ~ 98.2 秒 |
| 合成耗时 | 首次约 57.8 s（含模型加载）；模型已加载约 34.2 s |
| RTF | ≈ 0.35（快于实时） |
| `book.m4b` | 762,919 B |
| 分段 WAV | 14 个，全 16 kHz，**非静音**（peak 8.5k–11.4k，rms 1.3k–1.6k） |
| `manifest.engine` | `sherpa`（且音频确实是真语音，不是 0.4 s/段的占位音） |

## 3. M4B 与章节标记校验

```bash
FP="/c/Users/romanovsky/AppData/Local/Microsoft/WinGet/Packages/Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe/ffmpeg-9.1-full_build/bin/ffprobe.exe"
"$FP" -v error \
  -show_entries "format=duration,size:format_tags=title,artist" \
  -show_chapters -of json \
  "$ROOT/testdata/ws-verify-sherpa/1a8eed25f9db_测试之书/book.m4b"
```
期望：3 个中文章节，`0–39.014 / 39.014–74.301 / 74.301–98.207`，时间轴连续，
`duration ≈ 98.207`，title/artist 正确。

### 波形非静音校验

```bash
./engine/.venv/Scripts/python.exe -c "
import wave,struct,glob,math,os
root=r'C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07\testdata\ws-verify-sherpa\1a8eed25f9db_测试之书'
fs=sorted(glob.glob(os.path.join(root,'segments','*.wav')))
print('count',len(fs))
for p in fs[:3]:
    with wave.open(p,'rb') as w:
        n=w.getnframes(); rate=w.getframerate(); raw=w.readframes(n)
    s=struct.unpack('<%dh'%n, raw)
    print(os.path.basename(p),'rate',rate,'dur',round(n/rate,2),
          'peak',max(abs(x) for x in s),'rms',round(math.sqrt(sum(x*x for x in s)/n),1))
"
```

## 4. 控制面 / 设备 / 源校验

```bash
"$CLI" capabilities --pretty
"$CLI" device list --pretty
"$CLI" source list --pretty
"$CLI" job list
# MCP 握手
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"probe","version":"1"}}}' \
  | "$ROOT/src/BookCtl.Mcp/bin/Debug/net10.0/bookctl-mcp.exe"
```
期望：`initialize` 返回 `2025-11-25`；`tools/list` ≥13 个 tools；
`device list` 报 1 张 GTX 1050 2GB / 16 逻辑核；`source list` 5 条且 fanchen 已安装。

## 5. GUI 一键模式校验

```bash
BOOKCTL_WORKSPACE="$ROOT/testdata/ws-verify-ui" \
  "$ROOT/src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe" \
  "$ROOT/testdata/测试之书.epub" --auto-start [--on-top]
```
期望：
- 简洁模式自动开始；
- 章节卡片**实时逐张追加**（`chapter_done` 事件驱动）；
- 完成显示「3/3 章 · 347/352 字 · 已生成 1分33秒」+ 输出文件夹 + 打开按钮；
- 三张章节卡片带标题、时长、`audio.m4a`、已完成状态。

截图参考 `docs/screenshots/03-simple-result-sherpa.png`。

### 兜底校验（sidecar 不可达）

```bash
BOOKCTL_SIDECAR_URL='http://127.0.0.1:9' BOOKCTL_WORKSPACE="$ROOT/testdata/ws-verify-fallback" \
  BookCtl.App.exe "$ROOT/testdata/测试之书.epub" --auto-start
```
期望：自动退回 `mock`，任务 `completed / COMPLETED`，无错误码，界面说明原因。

## 6. 便携版校验

```bash
cd "$ROOT/dist/本地朗读工厂-0.3.0-win-x64/tools/BookCtl.Cli"
./bookctl.exe capabilities --pretty
./bookctl.exe doctor --pretty
```
期望：`workspace_root = dist\...\data`、`edition = pro`、`sources_root = dist\...\sources`。
zip ≈196 MB，`portable.marker` 在程序根目录，`engine/` 不含 `.venv`。

## 7. 断点续传 / 孤儿任务校验

```bash
# 启动一个任务后强杀进程，再启动任意入口
"$CLI" job list                       # 残留 running 应被标为 WORKER_LOST
"$CLI" job resume <job_id>
"$CLI" job status <job_id>            # chapters_done 不应超过 total
```
期望：计数按章节 checkpoint 重算（不出现 `6/3`、`chars_done > total`）；
完成态 `ErrorCode/ErrorMessage` 为空；`manifest.status = completed`。

## 8. 验收证据（本机，未入库）

| 文件 | 说明 |
|---|---|
| `testdata/ui-final2.png` → `docs/screenshots/03-simple-result-sherpa.png` | 一键真语音完成界面 |
| `testdata/ws-ui-sherpa4/.../book.m4b` | 真语音整本有声书 |
| `testdata/ws-sherpa/.../manifest.json` | 真语音产物清单 |
| `testdata/sherpa-probe.wav` | 单句真语音试听 |

## 9. Mini 产品线回归（v0.5，改了 `mini/` 必跑）

```powershell
MINI="dist\release\mini-portable\BookCtl.Mini.exe"
$MINI capabilities          # 13 命令 / 8 源 / 4 模型
$MINI device list           # 16 线程 / 盘符 / 剩余空间
$MINI doctor                # full_cli 取决于是否装了 Full
$MINI source list
$MINI run --input testdata\测试之书.epub --dry-run        # 《测试之书》· 3 章 · 362 字
$MINI run --input testdata\测试之书.epub --engine mock -o $env:TEMP\mini-reg
```

`mcp` 验收：

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | "$MINI" mcp
# initialize → protocolVersion 2025-06-18；tools/list → 6 个 tools
```

期望（委托 Full 时）：`chapters=3 / paragraphs=14 / seconds=10.1 /
engine=mock-via-Full / wav_bytes≈681002`；`manifest.json` 含 3 条真实章节；
日志是 `[stage] message` 而非整行 NDJSON。**`engine` 不能是写死的 `sherpa-via-Full`、
统计不能是 0**（缺陷 #24）。

改了 `mini/setup/main.rs` 后还要跑安装/卸载闭环：

```powershell
dist\release\本地朗读工厂-Mini-0.6.0-setup.exe /S
& "$env:LOCALAPPDATA\本地朗读工厂\Mini\Uninstall-Mini.exe" /UNINSTALL /S
# 目录 / reg_product / reg_uninstall / lnk_start / lnk_desktop 全 False
```

> PowerShell 5.1 看中文日志乱码时先 `chcp 65001`（缺陷 #25，宿主解码问题，非程序缺陷）。

## 9.5 阅读器回归（v0.6，改了 `mini/src/reader.rs` 必跑）

```powershell
$R="dist\release\mini-portable\BookCtl.Mini.exe"
& $R reader --input testdata\测试之书.epub --dry-run --pretty
& $R reader --input testdata\测试之书.epub --headless --count 5
& $R reader --input testdata\测试之书.epub --headless --count 5   # 再来一次看命中
& $R reader status --input testdata\测试之书.epub
& $R reader prefetch --input testdata\测试之书.epub --from 3 --count 5
& $R reader --input testdata\测试之书.epub --play-test
```

期望：

| 命令 | 关键断言 |
|---|---|
| `--dry-run` | `chapters=3 / paragraphs=14`；`prefetch_window` 里出现**下一章**的段（跨章预取） |
| `--headless --count 5` | 5 段 `state=ready`、`elapsed_ms` 各 500–800、`wav_bytes>100` |
| 第二次 `--headless` | `cache_hits=5`、`elapsed_ms` 全 0（不重复合成） |
| `status` | `ready`/`total` 与缓存目录一致 |
| `prefetch --from 3` | 返回段 6/7/8 属第 2 章 |
| `--play-test` | `playback_available=true`；有音频设备时 `status=played`、`play_ms>1000` |

GUI 验收（人工 30 秒）：双击阅读器 → 三栏中文正常 → 正文首段蓝色高亮 →
等 3 秒状态栏出现「全书已备 N 段」→ 点「开始朗读」→ 高亮随朗读前移且正文自动滚动 →
点「跟随：关」后正文不再滚动 → 点右侧段落可跳转 → 拖入另一本 `.epub` 能换书。

安装/卸载闭环：

```powershell
dist\release\本地朗读工厂-阅读器-0.6.0-setup.exe /S
& "$env:LOCALAPPDATA\本地朗读工厂\Reader\Uninstall-Reader.exe" /UNINSTALL /S
# 卸载后：目录/注册表/快捷方式/ProgId 全 False，且 ReaderCache 里 WAV 数量不变
```

> MCP 侧要看 `tools/list` 是 **9 个**工具；`tools/call` 的 stdout 必须**只有一行 JSON**
> （`NullReporter` 曾往 stdout 打印解析日志，缺陷 #27）。

## 10. 未覆盖的验证（留给接手者）

- **另一台干净 Windows 跨机验收**（用户进行中，R-16）：Mini 无 .NET / 无 Python / 免管理员、
  拖入 `.epub`、自动测速选源、装到 D 盘、卸载保留数据
- 多显卡 0/1/2 实机切换（本机只有 1 张卡）
- `cosyvoice3` 真实权重（需 ≥6 GB 显存）
- `source pull` 从**干净**目录重跑，验证解包残留修复
- 签名安装包实机安装 + SmartScreen 表现（缺证书）
- S5 ASR 回验闭环、两遍 loudnorm、音色一致性 A/B 盲测
- Mini 直接加载本地模型权重（R-14，当前靠委托 Full）
