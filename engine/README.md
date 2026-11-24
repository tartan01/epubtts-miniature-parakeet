# engine/ —— Python TTS sidecar（本地引擎适配层）

C# 主程序（本地朗读工厂 / bookctl）通过 HTTP 调用本服务做合成，Python 侧只承担「引擎适配」，
不参与任务编排（状态机 / 断点 / 进度全部在 C# JobStore 中）。

## 三种模式

| 模式 | 算力 | 音质 | 适用 |
|---|---|---|---|
| `mock` | 极低（不推理） | 占位音调 | 跑通链路、验收 UI、无模型无网 |
| `sherpa` | **纯 CPU**，16 线程约 RTF 0.1–0.4 | 中文可用（VITS 多说话人） | 2GB 显卡 / 纯 CPU 机器 / 简洁模式默认 |
| `cosyvoice3` | GPU ≥6GB | 目前开源中文最好之一 | 有 8GB+ 显卡的机器 |

## 启动

```powershell
# 1) 装 CPU 引擎（独立 venv，不污染系统 Python）
cd engine
python -m venv .venv
.venv\Scripts\python.exe -m pip install sherpa-onnx

# 2) 下模型（也可用 bookctl source pull sherpa-vits-zh-fanchen 一键下载）
.venv\Scripts\python.exe tts_server.py --mode sherpa --port 8765

# 3) 校样模式（零依赖）
python tts_server.py --mode mock --port 8765

# 4) GPU 模式（P1 适配点，缺权重/依赖会返回 503 + 指引）
python tts_server.py --mode cosyvoice3 --model-dir D:\models\Fun-CosyVoice3-0.5B-2512_RL
```

C# 侧切换：环境变量 `BOOKCTL_ENGINE=sherpa|mock|sidecar`，或 GUI「详细参数」里的引擎下拉框。

## 设备选择

`--device cpu | gpu | cuda:0 | hybrid`，也可由 C# 每次请求携带（`device` / `gpu_index` / `cpu_threads`），
**首次加载模型时生效**（模型已加载后切设备需重启 sidecar）。

- `sherpa`：PyPI 的 wheel 是 **CPU 版 ONNX Runtime**，要 GPU 得自行编译 onnxruntime-gpu；
  传 `--device cuda` 会明确报错，绝不假装用了显卡。
- `cosyvoice3`：`cuda:N` 正常；`hybrid` 走 `device_map` 层卸载——**能跑但更慢**，只用于显存不足时兜底。

## 接口

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/health` | 健康检查（mode / model / python / device / 已加载模型的音色数） |
| POST | `/synthesize` | `{text, voice, speed, seed, sample_rate, chunk_id, device, gpu_index, cpu_threads}` → `{ok, wav_b64, seconds, sample_rate, mode, device}` |

设计约束（对应总架构 §3.2 / A.5）：

1. **音色一致性**：全书只算一次 speaker prompt（由 C# 侧 `profile_hash` 锁定传入），
   禁止逐段重新克隆 / 逐段换 seed；sherpa 用固定 `sid`。
2. **失败要明确**：模型未加载 / 依赖缺失 → 503 + 人话错误信息；C# 侧标记任务 failed，
   可 `job resume` 断点续跑。
3. **stdout 干净**：服务日志全走 stderr，stdout 不输出任何内容。
4. **只监听 127.0.0.1**：不对外暴露。
5. **模型自动发现**：`--model-dir` 为空时，依次找 `engine/../sources/*`、`engine/sources`、
   `%LOCALAPPDATA%\bookctl\sources\*`，选第一个含 `*.onnx` + `tokens.txt` 的目录。
