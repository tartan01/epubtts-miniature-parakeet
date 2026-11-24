#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""bookctl TTS sidecar —— 本地 TTS 适配层（HTTP + JSON，零第三方依赖即可跑 mock）

协议（与 C# SidecarTtsProvider 对齐）：
  GET  /health                 -> {"ok":true,"mode":"mock","model":"mock-1","device":...,"voices":[...]}
  POST /synthesize             <- {"text":..., "voice":..., "speed":..., "seed":..., "sample_rate":24000,
                                   "chunk_id":..., "device":"cpu|gpu|hybrid", "gpu_index":0, "cpu_threads":0}
                              -> {"ok":true,"wav_b64":"...","seconds":1.23,"sample_rate":24000}

运行模式：
  --mode mock      占位音（校样 / 无模型 / 无显卡）
  --mode sherpa    sherpa-onnx 真中文语音，纯 CPU 可实时（pip install sherpa-onnx）
  --mode cosyvoice3 CosyVoice 3 真实引擎适配点（需 ≥6GB 显存）

设备选择（--device 只是默认值，请求里带 device 时以请求为准，首次加载生效）：
  cpu            纯 CPU 推理
  gpu / hybrid   仅对 torch 系引擎（cosyvoice3）有意义；sherpa 的 PyPI wheel 是 CPU 版，
                 要 GPU 需自行编译 onnxruntime-gpu 版本，此时会明确报错而不是假装用了显卡
"""
from __future__ import annotations

import argparse
import base64
import io
import json
import math
import os
import struct
import sys
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

MODE = "mock"
MODEL_DIR = ""
DEVICE = "cpu"
GPU_INDEX = -1
CPU_THREADS = 0

_MODEL = None
_MODEL_KEY = None


def log(*args: object) -> None:
    print("[bookctl-tts]", *args, file=sys.stderr, flush=True)


def wav_bytes(pcm: bytes, rate: int) -> bytes:
    buf = io.BytesIO()
    with wave.open(buf, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(pcm)
    return buf.getvalue()


def floats_to_pcm16(samples, peak: float = 0.95) -> bytes:
    """float32 [-1,1] -> 16bit PCM，带软限幅，避免破音。"""
    if not samples:
        return b""
    mx = max(1e-6, max(abs(s) for s in samples))
    gain = min(1.0, peak / mx) if mx > peak else 1.0
    return struct.pack("<%dh" % len(samples),
                       *[int(max(-1.0, min(1.0, s * gain)) * 32767) for s in samples])


def synth_mock(text: str, rate: int, seed: int, speed: float) -> tuple[bytes, float]:
    """占位语调信号；时长为压缩模拟值（每字 10ms）。"""
    seconds = max(0.4, len(text) / 100.0 / max(0.25, speed))
    n = int(rate * seconds)
    freq = 165.0 + (seed % 40)
    phase = 0.0
    chunks: list[bytes] = []
    for i in range(n):
        t = i / rate
        syllable = abs(math.sin(t * 4.5 + seed % 7))
        env = min(1.0, syllable * 1.6) * 0.16
        phase += 2 * math.pi * (freq * (1 + 0.06 * math.sin(t * 2.1 + seed % 5))) / rate
        chunks.append(struct.pack("<h", int(env * math.sin(phase) * 32767)))
    return wav_bytes(b"".join(chunks), rate), seconds


# ———————————————————————————— sherpa-onnx（纯 CPU 真语音） ————————————————————————————

def discover_sherpa_dir(start: str) -> str:
    """没显式指定 --model-dir 时，在 sources/ 下自动找第一个可用模型目录。"""
    candidates = []
    if start and os.path.isdir(start):
        candidates.append(start)
    here = os.path.dirname(os.path.abspath(__file__))
    for root in (os.path.join(here, "..", "sources"),
                 os.path.join(here, "sources"),
                 os.path.join(os.path.expanduser("~"), "AppData", "Local", "bookctl", "sources")):
        if not os.path.isdir(root):
            continue
        for name in sorted(os.listdir(root)):
            d = os.path.join(root, name)
            if os.path.isdir(d):
                candidates.append(d)
    for d in candidates:
        if not os.path.isdir(d):
            continue
        files = os.listdir(d)
        has_onnx = any(f.endswith(".onnx") for f in files)
        if has_onnx and "tokens.txt" in files:
            return os.path.abspath(d)
    return ""


def ensure_sherpa(model_dir: str, device: str, threads: int):
    """按需加载 sherpa-onnx OfflineTts；设备/线程变化时重建。"""
    global _MODEL, _MODEL_KEY

    if device.startswith("cuda") or device == "gpu":
        raise RuntimeError(
            "sherpa-onnx 的 PyPI wheel 是 CPU 版：无法在 GPU 上推理。"
            "请改用 --device cpu，或换 cosyvoice3 引擎走显卡。"
        )

    d = discover_sherpa_dir(model_dir)
    if not d:
        raise RuntimeError(
            "没有找到 sherpa 模型。先执行：bookctl source pull sherpa-vits-zh-fanchen"
            "（或把模型目录放到 sources/ 下，需含 *.onnx 与 tokens.txt）"
        )

    key = (d, threads)
    if _MODEL is not None and _MODEL_KEY == key:
        return _MODEL

    try:
        import sherpa_onnx
    except ImportError as exc:
        raise RuntimeError(
            "缺少 sherpa-onnx：请在 engine 目录执行 "
            "python -m venv .venv && .venv\\Scripts\\pip install sherpa-onnx"
        ) from exc

    files = os.listdir(d)
    onnx = next((os.path.join(d, f) for f in files if f.endswith(".onnx") and "int8" not in f), None) \
        or next(os.path.join(d, f) for f in files if f.endswith(".onnx"))
    tokens = os.path.join(d, "tokens.txt")
    lexicon = os.path.join(d, "lexicon.txt")
    dict_dir = os.path.join(d, "dict") if os.path.isdir(os.path.join(d, "dict")) else None
    fsts = [os.path.join(d, f) for f in ("phone.fst", "date.fst", "number.fst") if f in files]

    n_threads = threads if threads and threads > 0 else max(1, os.cpu_count() or 4)
    config = sherpa_onnx.OfflineTtsConfig(
        model=sherpa_onnx.OfflineTtsModelConfig(
            vits=sherpa_onnx.OfflineTtsVitsModelConfig(
                model=onnx,
                lexicon=lexicon if os.path.exists(lexicon) else "",
                tokens=tokens,
                dict_dir=dict_dir or "",
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
    log(f"loading sherpa-onnx model: {os.path.basename(onnx)}  threads={n_threads}")
    _MODEL = sherpa_onnx.OfflineTts(config)
    _MODEL_KEY = key
    log(f"model loaded, sample_rate={_MODEL.sample_rate}, speakers={_MODEL.num_speakers}")
    return _MODEL


def synth_sherpa(text: str, voice: str, speed: float, threads: int) -> tuple[bytes, float, int]:
    tts = ensure_sherpa(MODEL_DIR, DEVICE, threads)
    sid = 0
    v = (voice or "").strip().lower()
    if v.isdigit():
        sid = int(v)
    elif v.startswith("sid=") and v[4:].isdigit():
        sid = int(v[4:])
    sid = max(0, min(sid, max(0, getattr(tts, "num_speakers", 1) - 1)))
    audio = tts.generate(text, sid=sid, speed=max(0.5, min(2.0, speed)))
    pcm = floats_to_pcm16(audio.samples)
    rate = audio.sample_rate
    return wav_bytes(pcm, rate), len(audio.samples) / rate, rate


# ———————————————————————————— CosyVoice 3（GPU 适配点） ————————————————————————————

def ensure_cosyvoice():
    """CosyVoice 3 接入点（P1）。

    步骤：Python 3.10 + CUDA>=12.8 + torch/torchaudio + triton-windows，
    下载 Fun-CosyVoice3-0.5B-2512_RL 到 --model-dir，然后：
        from cosyvoice.cli.cosyvoice import CosyVoice
        _MODEL = CosyVoice(MODEL_DIR, fp16=True, load_jit=False, load_trt=False)
    设备：torch.device("cuda:0") / device_map 卸载到 CPU（hybrid）；
    音色：全书只算一次 speaker prompt（profile_hash 锁定），禁止逐段重新克隆。
    """
    raise RuntimeError(
        "cosyvoice3 模式尚未接入权重。请按 engine/README.md 配置模型，"
        "或改用 --mode sherpa（纯 CPU 真语音）/ --mode mock（校样）。"
    )


def synth_cosyvoice(text: str, voice: str, speed: float, seed: int):
    ensure_cosyvoice()
    raise RuntimeError("synth_real 未实现")


class Handler(BaseHTTPRequestHandler):
    server_version = "bookctl-tts/0.3"

    def _send(self, code: int, obj: dict) -> None:
        body = json.dumps(obj, ensure_ascii=False).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        if not self.path.startswith("/health"):
            self._send(404, {"ok": False, "error": "not found"})
            return
        info = {
            "ok": True,
            "mode": MODE,
            "model": "mock-1" if MODE == "mock" else ("sherpa-onnx-vits-zh" if MODE == "sherpa" else "cosyvoice3"),
            "python": sys.version.split()[0],
            "device": DEVICE,
            "gpu_index": GPU_INDEX,
            "cpu_threads": CPU_THREADS or (os.cpu_count() or 1),
            "model_dir": discover_sherpa_dir(MODEL_DIR) if MODE == "sherpa" else (MODEL_DIR or None),
        }
        if MODE == "sherpa" and _MODEL is not None:
            info["sample_rate"] = _MODEL.sample_rate
            info["voices"] = [str(i) for i in range(max(1, getattr(_MODEL, "num_speakers", 1)))]
        self._send(200, info)

    def do_POST(self) -> None:
        global DEVICE, CPU_THREADS
        if not self.path.startswith("/synthesize"):
            self._send(404, {"ok": False, "error": "not found"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            req = json.loads(self.rfile.read(length).decode("utf-8"))
        except Exception as exc:
            self._send(400, {"ok": False, "error": f"bad request: {exc}"})
            return

        text = str(req.get("text", ""))
        voice = str(req.get("voice", "0"))
        speed = float(req.get("speed", 1.0))
        seed = int(req.get("seed", 42))
        chunk = str(req.get("chunk_id", "unknown"))
        rate = int(req.get("sample_rate", 24000))
        if not text:
            self._send(400, {"ok": False, "error": "text is empty"})
            return

        # 请求级设备偏好：首次加载模型时生效（加载后切换设备需要重启 sidecar）
        req_device = str(req.get("device") or DEVICE).lower()
        if req_device in ("gpu", "hybrid"):
            req_device = f"cuda:{max(0, int(req.get('gpu_index', 0)))}"
        threads = int(req.get("cpu_threads") or 0)
        if req_device != DEVICE or (threads and threads != CPU_THREADS):
            log(f"device/threads changed by request: {DEVICE}->{req_device}, threads={threads} (首次加载时生效)")
            DEVICE, CPU_THREADS = req_device, threads

        try:
            if MODE == "mock":
                pcm_wav, seconds = synth_mock(text, rate, seed + sum(map(ord, chunk)) % 997, speed)
                out_rate = rate
            elif MODE == "sherpa":
                pcm_wav, seconds, out_rate = synth_sherpa(text, voice, speed, CPU_THREADS)
            else:
                pcm_wav, seconds = synth_cosyvoice(text, voice, speed, seed)
                out_rate = rate
            self._send(200, {
                "ok": True,
                "wav_b64": base64.b64encode(pcm_wav).decode("ascii"),
                "seconds": seconds,
                "sample_rate": out_rate,
                "mode": MODE,
                "device": DEVICE,
            })
        except Exception as exc:
            log("synth failed", chunk, "->", exc)
            self._send(503, {"ok": False, "error": str(exc)})

    def log_message(self, fmt: str, *args: object) -> None:
        log(fmt % args)


def main() -> None:
    global MODE, MODEL_DIR, DEVICE, GPU_INDEX, CPU_THREADS
    parser = argparse.ArgumentParser(description="bookctl TTS sidecar")
    parser.add_argument("--mode", choices=["mock", "sherpa", "cosyvoice3"], default="mock")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--model-dir", default="")
    parser.add_argument("--device", default="cpu", help="cpu | gpu | cuda:0 | hybrid")
    parser.add_argument("--gpu-index", type=int, default=-1)
    parser.add_argument("--cpu-threads", type=int, default=0)
    args = parser.parse_args()

    MODE, MODEL_DIR = args.mode, args.model_dir
    DEVICE, GPU_INDEX, CPU_THREADS = args.device, args.gpu_index, args.cpu_threads

    if MODE == "sherpa" and args.device.startswith("cuda"):
        log("注意：sherpa-onnx PyPI wheel 为 CPU 版，--device 将回退为 cpu")

    server = ThreadingHTTPServer((args.host, args.port), Handler)
    log(f"serving on http://{args.host}:{args.port}  mode={MODE} device={DEVICE}")
    log("仅监听回环地址；日志走 stderr，不污染 stdout")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
