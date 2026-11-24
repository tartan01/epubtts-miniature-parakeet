# 04 · 设备选择、源管理与「分布式推理」结论

> 实现：`src/BookCtl.Core/Device.cs`、`Sources.cs`、`Contracts.cs`（`ApplyEnv` / `ApplyPortable`）。
> 架构定义：`architecture-epub2audiobook.md §B.3 / §B.4 / §B.5`。

## 1. 设备决策（`DevicePlanner.Resolve`）

配置项：`Device = auto|cpu|gpu|hybrid`、`GpuIndex`（默认 -1）、`CpuThreads`（0 = 自动）、`AllowCpuOffload`。

显卡枚举用：
```
nvidia-smi --query-gpu=index,name,memory.total,memory.free --format=csv,noheader,nounits
```

引擎显存门槛（`RequiredVram`）：

| 引擎 | `RequiredVram`（MB） |
|---|---|
| `cosyvoice3` / `sidecar` / `python` | 6144 |
| `sherpa` / `sherpa-onnx` / `onnx` | 1536 |
| `piper` | 512 |
| `mock` | 0 |

`auto` 决策：
```
显存足够            → GPU（用 GpuIndex 或 0 号卡）
显存中等但能层卸载   → hybrid（GPU + CPU offload，附 warning：能跑但更慢）
显存过小 / 无 NVIDIA → CPU（CpuThreads = 逻辑核数或用户指定）
```

输出 `DevicePlan`：`Device` / `Display`（人类可读）/ `GpuIndex` / `CpuThreads` / `Warnings` / `ToDto()`。
GUI 详细参数模式直接把 `Display` + warnings 显示出来；`S4_SYNTH` 前发 `gpu_status` 事件。

**三条纪律**：
1. 设备**不参与** `SynthesisProfile.Hash`（换卡不应改变确定性判定）。
2. `phase` 文案必须按 `DevicePlan.Device` 分档（曾硬编码"正在调用显卡"，CPU 推理时撒谎 → 缺陷 #6）。
3. `sherpa` 的 PyPI wheel 是 **CPU 版**：收到 GPU 请求要明确报错，不能假装用了显卡。

## 2. 关于「GTX 1050 显存 + CPU 一起推理」的结论（用户明确问过）

**没有能提速的开源 TTS 分布式 / 拆分式推理框架。** 依据：

- 张量并行需要 ≥2 张**同型号、算力 ≥7.0** 的卡；GTX 1050 算力 **6.1**，vLLM / SGLang 类框架直接拒绝。
- `device_map` / 层卸载（hybrid）只解决"模型装不下"，通常比纯 GPU 慢 2–5 倍，不解决提速。
- ONNX Runtime 单 session 单 Execution Provider，不能一张图同时跑 CUDA + CPU EP 做并行。
- 多进程异构分片会破坏**整本书音色一致性**（有声书成败点），得不偿失。

产品对策：提供 **纯 GPU / 纯 CPU / hybrid 兜底** 三档，**选择权完全交给用户**。
`hybrid` 明确标注"能跑但更慢"，不宣称提速。

## 3. 源管理（用户口中的「DDS 源管理」）

落地为「内置引擎/模型源目录 + 一键下载/解包/安装状态」，`SourceManager.BuiltInCatalog()` 内置 5 个源：

| id | 类型 | 说明 |
|---|---|---|
| `mock` | `manual` | 校样占位引擎，零依赖 |
| `sherpa-vits-zh-fanchen` | `archive` | **`vits-zh-hf-fanchen-C`**，约 119 MB 压缩包 / 132 MB 解包，187 音色，16 kHz，纯 CPU 真中文 |
| `sherpa-vits-zh-aishell3` | `archive` | 备选中文多说话人模型 |
| `piper-zh-huayan` | `file` | 更轻量的中文备选 |
| `cosyvoice3` | `manual` | GPU 引擎适配点，需用户自备权重（≥6 GB 显存） |

源类型：`archive`（tar.bz2 等）/ `zip` / `file`（多文件下载）/ `manual`（手动放置）。
用户可在 `sources/catalog.user.json` 覆盖/扩展，格式参考 `config/sources.example.json`。

### 命令

```bash
CLI=".../bookctl.exe"
"$CLI" source list
"$CLI" source pull sherpa-vits-zh-fanchen
"$CLI" source remove sherpa-vits-zh-fanchen
```
MCP：`source_list` / `source_pull`。

### 多线路下载回退（国内必需）

顺序：原始 URL → 用户 `mirrors` → `BOOKCTL_MIRROR` →
`https://gh-proxy.com/` + URL（GitHub）→ `https://hf-mirror.com`（HuggingFace）。
实测国内直连 GitHub Release 返回 **502**，`gh-proxy.com` 返回 200，`ghfast.top` 403。
任一条失败自动换下一条；全失败抛 `SOURCE_DOWNLOAD_FAILED`。

### 解包与安装标记

```
tmpDir/
├── <archive>.tar.bz2      ← 下载到此
└── extract/               ← 解包到独立子目录
```
关键修复：解包目录必须是 `tmpDir/extract`，`Flatten()` 前 `File.Delete(archive)`。
早期把归档和解压结果混在 `tmpDir`，导致 119 MB 的 `.tar.bz2` 被一起拍进安装目录（缺陷见 `docs/defects.md`）。
安装完成后写 `.installed.json`，`SourceManager.InstalledPath(id)` 返回路径。

### 源根目录的固定规则（缺陷 #9）

```csharp
// 旧：从 WorkspaceRoot 的父目录推导 → 一改工作区，已装模型"消失"，简洁模式退回 mock
// 新：固定用户级目录；便携版由 marker 覆盖
SourcesRoot = EffectiveSourcesRoot
           ?? %LOCALAPPDATA%\bookctl\sources
```
便携版：`ApplyPortable()` 把它覆盖为 `<程序目录>\sources`。

## 4. 环境变量

| 变量 | 说明 |
|---|---|
| `BOOKCTL_DEVICE` | `auto` / `cpu` / `gpu` / `hybrid` |
| `BOOKCTL_GPU` | 显卡 index（0/1/2…） |
| `BOOKCTL_CPU_THREADS` | CPU 线程数 |
| `BOOKCTL_SOURCES` | 覆盖源根目录 |
| `BOOKCTL_MIRROR` | 额外下载镜像 |

## 5. 验证命令

```bash
CLI=".../bookctl.exe"
"$CLI" device list --pretty
"$CLI" capabilities --pretty          # 看 device 模式清单与 sources_root
"$CLI" source list --pretty
curl -s http://127.0.0.1:8765/health  # sidecar 报告 device / threads / model_dir / voices
```

本机实测：`device list` → 1 张 `GTX 1050` 2GB / 16 逻辑核；`source list` → 5 条，
`sherpa-vits-zh-fanchen` 已安装。
