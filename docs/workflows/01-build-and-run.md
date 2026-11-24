# 01 · 环境搭建、构建与三种运行方式

## 1. 前置依赖

| 组件 | 版本 / 位置 | 备注 |
|---|---|---|
| .NET SDK | 10.0.404 | `C:\Program Files\dotnet\dotnet.exe`，**不在 PATH**，一律用绝对路径 |
| Windows App SDK | 2.7.0 | 由 `Directory.Build.props` 统一，`WindowsAppSDKSelfContained=true`，目标机无需装 Runtime |
| Python | 3.13（venv） | `engine/.venv/Scripts/python.exe`；`pip install sherpa-onnx==2.0.3` |
| TTS 模型 | `vits-zh-hf-fanchen-C` | `%LOCALAPPDATA%\bookctl\sources\sherpa-vits-zh-fanchen`，约 132 MB，187 音色，16 kHz |
| ffmpeg / ffprobe | WinGet 包 `Gyan.FFmpeg` | `%LOCALAPPDATA%\Microsoft\WinGet\Packages\...\bin\`；`ResolveFfmpeg` 有目录扫描兜底 |
| GPU（可选） | GTX 1050 2GB | 本机仅 1 张；CUDA 路线仅 `cosyvoice3` 适配点需要 |
| 操作系统 | Windows 11 x64 | WinUI 3 unpackaged，最低 `10.0.19041.0` |

不需要 Visual Studio 全家桶；VS Code + C# Dev Kit 仅用于**人工审阅**（XAML 纯文本，无拖拽控件）。
写代码由 Agent 直接读写文件完成。

## 2. 一键构建

```bash
ROOT="C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07"
DN="/c/Program Files/dotnet/dotnet.exe"
cd "$ROOT"
"$DN" build bookctl.sln -c Debug -v q --nologo
```

预期：0 error / 0 warning。

**重要工程坑**：`dotnet build bookctl.sln` 后，`src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.Core.dll`
可能仍是上一版（增量时间戳复制 + 构建顺序）。改了 `Core` 却"没生效"时：

```bash
# 先关闭占用 DLL 的窗口程序
# PowerShell: Get-Process BookCtl.App | Stop-Process -Force
"$DN" build src/BookCtl.App/BookCtl.App.csproj -c Debug -t:Rebuild -v q --nologo
ls -l --time-style=+%H:%M:%S \
  src/BookCtl.Core/bin/Debug/net10.0/BookCtl.Core.dll \
  src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.Core.dll
```
两个时间戳应接近。

## 3. 三种运行方式

### 3.1 桌面 GUI（WinUI 3）

```
src\BookCtl.App\bin\Debug\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe
```

- 双击启动 → 默认**简洁模式**（导入 EPUB → 点「生成朗读音频」→ 实时章节卡片）。
- 顶部可切「详细参数」：引擎 / 音色 / 推理设备 / 显卡 / CPU 线程 / 语速 / M4B 开关 + 中控台。
- 命令行参数：
  | 参数 | 作用 |
  |---|---|
  | `<路径>.epub` | 自动填入输入框（配合安装包 `.epub` 文件关联） |
  | `--auto-start` | 简洁模式自动开始生成 |
  | `--detailed` | 启动即进入详细参数模式 |
  | `--on-top` | 窗口置顶，仅用于演示 / 截图 |

示例：
```bash
BOOKCTL_WORKSPACE="$ROOT/testdata/ws-ui" \
  "$ROOT/src/BookCtl.App/bin/Debug/net10.0-windows10.0.19041.0/win-x64/BookCtl.App.exe" \
  "$ROOT/testdata/测试之书.epub" --auto-start
```

> 简洁模式的兜底：检测到已安装 CPU 模型 → 用 `sherpa`；
> 模型未装 **或 sidecar 不可达** → 自动退回 `mock` 校样引擎并在界面说明原因，不会硬报错。

### 3.2 CLI

```bash
CLI="$ROOT/src/BookCtl.Cli/bin/Debug/net10.0/bookctl.exe"
"$CLI" doctor --pretty                      # 自检：工作区 / 显卡 / ffmpeg / Python
"$CLI" --help                               # 全命令
"$CLI" capabilities --pretty                 # 机器可读能力清单（MCP/自动化用）
"$CLI" run --input "$ROOT/testdata/测试之书.epub" --engine mock
"$CLI" run --input "book.epub" --engine sherpa --device cpu
"$CLI" job list / job status <id> / job logs <id> / job resume <id> / job cancel <id>
"$CLI" device list / source list / source pull <id> / source remove <id>
```

`run` 的常用开关：`--engine mock|sherpa|sidecar`、`--device auto|cpu|gpu|hybrid`、
`--gpu <0|1|2>`、`--cpu-threads N`、`--voice`、`--no-m4b`、`--idempotency-key`、`--dry-run`、`--pretty`、`--quiet`。

### 3.3 MCP（stdio JSON-RPC）

```
src\BookCtl.Mcp\bin\Debug\net10.0\bookctl-mcp.exe
```

宿主 `mcp.json` 示例见 `README.md §5`。协议要点：
- `initialize` 返回 `2025-11-25`，兼容 `2026-07-28` 无状态版本；
- `tools/list` 返回 `capabilities / doctor / job_start / job_status / job_logs / job_cancel /
  job_resume / job_artifacts / device_list / source_list / source_pull` 等；
- `tools/call` 结果带 `structuredContent`；
- **整本书不要在单次 tool 调用里同步执行**，一律 `job_start` 后轮询 `job_status`；
- `source_pull` 是唯一可能耗时几十秒~几分钟的同步 tool（为了让模型拿到明确安装结果），其余 SLA ≤2 s。

## 4. TTS sidecar（真语音必需）

```bash
cd "$ROOT"
./engine/.venv/Scripts/python.exe -m venv engine/.venv          # 首次
./engine/.venv/Scripts/python.exe -m pip install sherpa-onnx    # 首次，实测 2.0.3
./engine/.venv/Scripts/python.exe engine/tts_server.py --mode sherpa --port 8765 --device cpu
curl -s http://127.0.0.1:8765/health
```

`/health` 返回 `mode / device / cpu_threads / model_dir / voices`。
模型自动发现顺序：`--model-dir` → `engine/../sources` → `engine/sources` →
`%LOCALAPPDATA%\bookctl\sources`（找含 `*.onnx` + `tokens.txt` 的目录）。

也可用 CLI 一键装模型：
```bash
"$CLI" source list
"$CLI" source pull sherpa-vits-zh-fanchen
```

## 5. 常见故障排查

| 现象 | 排查 |
|---|---|
| `dotnet` 不是内部或外部命令 | 用绝对路径 `C:\Program Files\dotnet\dotnet.exe` |
| `doctor` 报 `ffmpeg.found=false` | 刚用 WinGet 装完，PATH 未刷新；`ResolveFfmpeg` 会扫 WinGet 目录，重启 shell 即可 |
| 改了 Core 没生效 | 见 §2，单独重建 App |
| build 时 `BookCtl.Core.dll` 被占用 | 有后台 `source pull` 或 `BookCtl.App` 在跑，等它结束或关掉再编译 |
| 中文输出乱码 | 检查是否用了旧版 CLI；UTF-8 统一在 `Main()` 开头（v0.3 已修） |
| 一键生成只有 10 秒音频 | 说明退回 `mock`；检查模型是否装、sidecar 是否在 `8765`、看界面提示 |
| 音频 0.4 秒/段但 manifest 写 sherpa | 旧版 `Pipeline` provider 复用 bug（已修，见 `docs/defects.md` #10） |
| UI 截图空白 | WinUI 用 DirectComposition，`gdigrab -i title=` 抓不到；用 `-i desktop` 全屏 + `--on-top` |

## 6. 环境变量速查

| 变量 | 作用 |
|---|---|
| `BOOKCTL_WORKSPACE` | 工作区（产物 + `bookctl.db`）根目录 |
| `BOOKCTL_SOURCES` | 模型源根目录（覆盖默认 `%LOCALAPPDATA%\bookctl\sources`） |
| `BOOKCTL_ENGINE` | 默认引擎 `mock` / `sherpa` / `sidecar` |
| `BOOKCTL_DEVICE` | `auto` / `cpu` / `gpu` / `hybrid` |
| `BOOKCTL_GPU` | 显卡 index |
| `BOOKCTL_CPU_THREADS` | CPU 线程数（0 = 自动） |
| `BOOKCTL_SIDECAR_URL` | sidecar 地址，默认 `http://127.0.0.1:8765` |
| `BOOKCTL_MIRROR` | 下载镜像回退线路 |
| `BOOKCTL_PFX` / `BOOKCTL_PFX_PASS` / `BOOKCTL_CERT_SHA1` / `BOOKCTL_TS_*` | 仅签名脚本使用，**不要写进文件** |
