# 05 · TTS 引擎层、sidecar 协议与模型安装

> 实现：`src/BookCtl.Core/Engine.cs`（C# provider 侧）、`engine/tts_server.py`（Python sidecar）。
> 选型理由与否决项见 `docs/decisions/ADR.md` 与架构 MD §4.1。

## 1. 三档引擎

| 模式 | 用途 | 依赖 | 输出 |
|---|---|---|---|
| `mock` | **校样引擎**：无 GPU / 无模型时验收链路、UI、契约 | 零依赖（纯 C# 或标准库 Python） | 占位信号，时长按每字 10 ms 模拟（不是真语音） |
| `sherpa` | **默认真语音**：`sherpa-onnx` 中文 VITS，纯 CPU | `sherpa-onnx 2.0.3` + 约 132 MB 模型 | 真实中文语音，16 kHz，187 音色 |
| `cosyvoice3` | GPU 高质量适配点（**未接真实权重**） | ≥6 GB 显存 + 模型权重 | 目前返回 503 + 指引 |

C# 侧 `TtsProviderFactory` 支持别名：`sherpa` / `sherpa-onnx` / `onnx` / `piper` / `sidecar` / `python` / `mock`。
`Describe(...)` 给出三档能力说明。

### 选型否决记录（不要重新捡回来）

- **VibeVoice**：强制插入可听 AI 声明 + 音频水印 → 有声书不可接受。
- **XTTS / Coqui**：CPML 非商用许可 + Coqui 主体已注销 → 法律死路。
- **EbookLib**：AGPL，会污染整个项目 → 自写 zip + XML 解析。
- **CosyVoice 3**：代码 + 权重双 Apache-2.0、中文 CER 0.81%，是**理想默认引擎**，
  但需 ≥6 GB 显存；本机 GTX 1050 2GB 跑不了，故降为 P1 适配点。

## 2. sidecar 启动

```bash
cd "C:/Users/romanovsky/WorkBuddy/2026-11-23-13-38-07"
./engine/.venv/Scripts/python.exe -m venv engine/.venv                 # 首次
./engine/.venv/Scripts/python.exe -m pip install sherpa-onnx==2.0.3     # 首次
./engine/.venv/Scripts/python.exe engine/tts_server.py \
    --mode sherpa --port 8765 --device cpu [--model-dir <dir>]
```

`--mode` 支持 `mock` / `sherpa` / `cosyvoice3`。
`discover_sherpa_dir(...)` 自动按以下顺序找含 `*.onnx` + `tokens.txt` 的目录：
`--model-dir` → `engine/../sources` → `engine/sources` → `%LOCALAPPDATA%\bookctl\sources`。

`ensure_sherpa(...)` 关键配置：
```python
sherpa_onnx.OfflineTtsConfig(
    model=sherpa_onnx.OfflineTtsModelConfig(
        vits=sherpa_onnx.OfflineTtsVitsModelConfig(
            model=onnx, lexicon=..., tokens=tokens, dict_dir=...,
            noise_scale=0.667, noise_scale_w=0.8, length_scale=1.0,
        ),
        num_threads=n_threads, provider="cpu", debug=False,
    ),
    rule_fsts=",".join(fsts) if fsts else "",
    max_num_sentences=1,
)
```

## 3. HTTP 接口

### `GET /health`
```json
{
  "ok": true, "mode": "sherpa", "model": "sherpa-onnx-vits-zh",
  "python": "3.13.14", "device": "cpu", "gpu_index": -1,
  "cpu_threads": 16, "model_dir": "...", "voices": [...]
}
```

### `POST /synthesize`
请求：
```json
{
  "text": "第一章 起风了……",
  "voice": "0",
  "speed": 1.0,
  "seed": 42,
  "sample_rate": 24000,
  "chunk_id": "ch0001-seg0001",
  "device": "cpu",
  "gpu_index": -1,
  "cpu_threads": 16,
  "cpu_offload": false
}
```
响应：
```json
{
  "ok": true,
  "wav_b64": "...",
  "seconds": 8.64,
  "sample_rate": 16000,
  "mode": "sherpa",
  "device": "cpu"
}
```
`wav_b64` 由 `SidecarTtsProvider` 解码后写成分段 WAV；`sample_rate` **以 sidecar 返回值为准**
（sherpa 固定 16 kHz，不要因为请求里写了 24000 就按 24000 处理）。

请求级 `device` / `gpu_index` / `cpu_threads` 会传到 sidecar；
`sherpa` 收到 GPU 请求会**明确报错**，不假装使用显卡。

## 4. 模型安装

```bash
CLI=".../bookctl.exe"
"$CLI" source pull sherpa-vits-zh-fanchen
# 实测：约 26 分 39 秒（国内网络 + gh-proxy.com 镜像回退）
# 结果：%LOCALAPPDATA%\bookctl\sources\sherpa-vits-zh-fanchen
#       ├── vits-zh-hf-fanchen-C.onnx
#       ├── tokens.txt / lexicon.txt / *.fst / dict/
#       └── .installed.json
```

也可手动解压模型到该目录，再写 `.installed.json`。
自定义源见 `config/sources.example.json` 与 `docs/workflows/04-device-and-sources.md`。

## 5. 一键模式的兜底（v0.3）

`MainViewModel.StartOneClick()` 逻辑：
```
已安装 CPU 模型？
 ├ 是 → 尝试探测 sidecar /health
 │      ├ 可达 → EngineMode = sherpa, Device = cpu, Voice = "0"
 │      └ 不可达 → 退回 mock，并提示「语音服务未启动，先用校样引擎跑通流程」
 └ 否 → 退回 mock，提示「可到引擎与模型下载中文模型（约 130MB）」
```
**服务没起不会报错**，这是"用户第一次双击就能跑通"的关键。

## 6. 实测真语音探针

```bash
curl -s -X POST http://127.0.0.1:8765/synthesize \
  -H "Content-Type: application/json" \
  -d '{"text":"这是本地朗读工厂的真实中文语音测试。","voice":"0","device":"cpu"}' \
  | python -c "import sys,json,base64; d=json.load(sys.stdin); print(d['ok'],d['seconds'],d['sample_rate'],len(base64.b64decode(d['wav_b64'])))"
```
基线：`ok=True, seconds=8.64, rate=16000`，波形 `peak=10035, rms=1344.9`（非静音）。
文件 `testdata/sherpa-probe.wav`（本机，未入库）。

## 7. 接真实 GPU 引擎的适配点（P1）

1. `engine/tts_server.py` 的 `ensure_model()` / `synth_real()` 保留详细注释；
2. 加载 CosyVoice 3 权重（≥6 GB 显存机器）；
3. `Device.RequiredVram("cosyvoice3") = 6144` 已就位；
4. `Sources.BuiltInCatalog()` 里 `cosyvoice3` 是 `manual` 类型，需补下载/安装逻辑；
5. 保证 `sample_rate` / `voice` / `seed` 语义与 C# 侧 `SynthesisProfile` 一致；
6. 重新跑 `docs/workflows/07-verification.md` 的整本验收。
