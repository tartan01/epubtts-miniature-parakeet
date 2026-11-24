# 03 · CLI / MCP / 事件流控制面契约

> 这是"大模型友好"的**唯一稳定面**。改动前先读 `AGENTS.md §4`。
> 实现：`src/BookCtl.Cli/Program.cs`、`src/BookCtl.Mcp/Program.cs`、`src/BookCtl.Core/Handlers.cs`。

## 1. JSON envelope

成功：
```json
{"ok":true,"api_version":"1.0","data":{ ... }}
```
失败：
```json
{"ok":false,"api_version":"1.0","error":{"code":"BOOK_NOT_FOUND","message":"...","details":{...}}}
```
配合**稳定退出码**（0 成功 / 非 0 按错误类别），便于 CI 与脚本判定，不依赖解析文本。

全局开关：`--capabilities`（机器可读能力清单）、`--schema`、`--dry-run`、`--pretty`、`--quiet`。

## 2. CLI 命令表

| 命令 | 说明 |
|---|---|
| `doctor` | 自检：工作区 / 数据库 / 显卡 / ffmpeg / Python / 引擎；`--pretty` 输出缩进 JSON |
| `capabilities` | 能力清单：版本、edition、命令、引擎、设备模式、MCP tools、workspace/sources 路径 |
| `run --input <epub>` | **一键**：提交 job + 内嵌 worker + 实时 JSONL 事件流 + 最终 envelope |
| `job start --input <epub>` | 只提交任务，返回 `job_id`（异步） |
| `job status <id>` | 状态 / 阶段 / 进度 / 计数 / 错误码 / result_path |
| `job logs <id>` | 事件流回放（`--follow` 可跟随） |
| `job cancel <id>` | 协作取消 |
| `job resume <id>` | 断点续传（`WORKER_LOST` / `failed` / `cancelled` 可恢复） |
| `job artifacts <id>` | 产物清单 |
| `job list` | 全部任务 |
| `device list` | 显卡 index / name / mem_total / mem_free + CPU 核数 + 当前 `DevicePlan` |
| `source list` | 内置源目录 + 安装状态 + 路径 |
| `source pull <id>` | 一键下载 / 解包 / 安装模型（多线路回退，可能耗时数分钟） |
| `source remove <id>` | 移除已安装源 |

`run` / `job start` 常用参数：
```
--engine mock|sherpa|sidecar     --voice <id>          --device auto|cpu|gpu|hybrid
--gpu <0|1|2>                    --cpu-threads <N>     --no-m4b
--idempotency-key <key>          --dry-run             --pretty / --quiet
```

### 输出编码（重要）

`Main()` **开头**统一把 stdout/stderr 设为无 BOM UTF-8：
```csharp
var utf8 = new System.Text.UTF8Encoding(false);
Console.OutputEncoding = utf8;
Console.InputEncoding = utf8;
Console.SetError(new System.IO.StreamWriter(Console.OpenStandardError(), utf8) { AutoFlush = true });
```
原因：Windows 控制台默认 OEM(936)，若只在 `Emit()` 里临时切 UTF-8，
**先输出的事件流 JSONL 已经用 GBK 编码**，导致中文书名/章节标题在不同终端下互相乱码（缺陷 #7）。
`run` 会同时输出纯文本进度行（`[S1_INGEST] 正在解析 EPUB 结构`）与 JSON 事件行，两者编码必须一致。

## 3. MCP（stdio JSON-RPC 2.0）

入口：`src/BookCtl.Mcp/bin/Debug/net10.0/bookctl-mcp.exe`（源码 `BookCtl.Mcp/Program.cs`，手写、无第三方 SDK）。

| 项目 | 约定 |
|---|---|
| 传输 | stdio，UTF-8 二进制写 stdout（Windows 上不能让运行时替你转码） |
| 协议版本 | `initialize` 返回 `2025-11-25`，兼容无状态版 `2026-07-28` |
| 方法 | `initialize` / `tools/list` / `tools/call` / `resources/*` / `prompts/*` |
| 结果 | `tools/call` 带 `structuredContent`，与 CLI envelope 的 `data` 同构 |

tools 清单（以 `--capabilities` 实际返回为准）：
```
capabilities, doctor,
job_start, job_status, job_logs, job_cancel, job_resume, job_artifacts,
device_list, source_list, source_pull
```

`job_start` 参数：
```json
{
  "input_path": "C:\\...\\book.epub",
  "idempotency_key": "optional",
  "engine": "mock|sherpa|sidecar",
  "voice": "0",
  "device": "auto|cpu|gpu|hybrid",
  "gpu_index": -1,
  "cpu_threads": 0
}
```

**设计纪律**：
- 整本书**绝不**在单次 tool 调用里同步执行；耗时操作全部 job 化，SLA ≤2 s。
- 唯一例外是 `source_pull`（为了让模型拿到明确的安装结果），它同步等待下载完成。
- `BookCtl.Mcp.Program` 是 `static class`，新增 helper 必须 `static`：
  ```csharp
  private static object PullSource(string id) =>
      new BookCtlHandlers().SourcePull(id).GetAwaiter().GetResult();
  ```
- 新增 tool 时同步更新：`Tools` schema、分发 `switch`、`--capabilities` 的 tools 清单、README §5、架构 MD §6.2。

宿主配置示例（`mcp.json`）见 `README.md §5`。

## 4. 环境变量（控制面覆盖）

| 变量 | 默认 | 说明 |
|---|---|---|
| `BOOKCTL_WORKSPACE` | `%LOCALAPPDATA%\bookctl\workspace` | 产物 + `bookctl.db` |
| `BOOKCTL_SOURCES` | `%LOCALAPPDATA%\bookctl\sources` | 模型源根（**不再随 workspace 漂移**） |
| `BOOKCTL_ENGINE` | `mock` | 引擎 |
| `BOOKCTL_DEVICE` | `auto` | 设备模式 |
| `BOOKCTL_GPU` | `-1` | 显卡 index |
| `BOOKCTL_CPU_THREADS` | `0`（自动） | CPU 线程 |
| `BOOKCTL_SIDECAR_URL` | `http://127.0.0.1:8765` | TTS sidecar |
| `BOOKCTL_MIRROR` | — | 下载镜像 |

配置优先级：代码默认值 → `config/bookctl.defaults.json` → 环境变量 → 显式 CLI 参数 → 便携版 marker 覆盖。

## 5. 便携模式判定

`Contracts.ApplyPortable(cfg)` 从 `AppContext.BaseDirectory` **逐级向上（≤6 层）** 查找 `portable.marker`，
找到则以 marker 所在目录为便携根：
```
workspace = <root>\data
sources   = <root>\sources
Edition   = "pro"
```
原因：CLI/MCP/Worker 发布在 `<root>\tools\<proj>\` 子目录，marker 只在根目录，
旧实现只在 `BaseDirectory` 找，导致 U 盘带走后数据仍写 `%LOCALAPPDATA%`（缺陷 #8）。

安装版由 Inno Setup 安装时**删除 `portable.marker`**，数据回到 `%LOCALAPPDATA%\bookctl`。

## 6. 验证控制面的命令

```bash
ROOT="C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07"
CLI="$ROOT/src/BookCtl.Cli/bin/Debug/net10.0/bookctl.exe"
"$CLI" capabilities --pretty | head -40
"$CLI" doctor --pretty
"$CLI" device list --pretty
"$CLI" source list --pretty

# MCP 握手（echo 一行 initialize，观察返回）
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"probe","version":"1"}}}' \
  | "$ROOT/src/BookCtl.Mcp/bin/Debug/net10.0/bookctl-mcp.exe"
```
