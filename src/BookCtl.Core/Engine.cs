using System.Diagnostics;
using System.Net.Http;
using System.Text;
using System.Text.Json.Serialization;

namespace BookCtl.Core;

public sealed record SynthResult(string WavPath, double Seconds, double SynthSeconds, bool Retried = false);

public interface ITtsProvider
{
    string Name { get; }
    string Model { get; }
    Task<bool> HealthAsync(CancellationToken ct = default);
    Task<SynthResult> SynthesizeAsync(string chunkId, string text, SynthesisProfile profile,
        string outWavPath, CancellationToken ct = default);
}

public static class TtsProviderFactory
{
    public static ITtsProvider Create(BookCtlConfig cfg) => cfg.EngineMode.Trim().ToLowerInvariant() switch
    {
        "sidecar" or "cosyvoice3" or "python" => new SidecarTtsProvider(cfg),
        "sherpa" or "sherpa-onnx" or "onnx" or "piper" => new SidecarTtsProvider(cfg),
        _ => new MockTtsProvider(cfg),
    };

    public static string ModelOf(BookCtlConfig cfg) => cfg.EngineMode.Trim().ToLowerInvariant() switch
    {
        "sidecar" or "cosyvoice3" or "python" => "cosyvoice3",
        "sherpa" or "sherpa-onnx" or "onnx" => "sherpa-onnx-vits-zh",
        "piper" => "piper-zh",
        _ => "mock-1",
    };

    public static object Describe(BookCtlConfig cfg) => new object[]
    {
        new { name = "mock", model = "mock-1", device = "cpu", vram_mb = 0, speed = "即时", license = "内置（占位音）",
              note = "校样引擎：生成可播放占位音频，用于无 GPU / 无模型环境跑通全链路", recommended = cfg.EngineMode == "mock" },
        new { name = "sherpa", model = "sherpa-onnx-vits-zh", device = "cpu/gpu", vram_mb = 1536, speed = "CPU 上 RTF 约 0.1-0.4",
              license = "Apache-2.0（sherpa-onnx）+ 各模型自带许可",
              note = "ONNX 推理，纯 CPU 可实时，也可切 onnxruntime-gpu；中文音质中等，适合 2GB 级显卡或纯 CPU 机器", recommended = cfg.EngineMode is "sherpa" or "sherpa-onnx" },
        new { name = "sidecar", model = "cosyvoice3", device = "gpu", vram_mb = 6144, speed = "RTF 约 0.15-0.30",
              license = "Apache-2.0（代码与权重）",
              note = "经 engine/tts_server.py 调本地显卡上的 CosyVoice 3，需 ≥6GB 显存；2GB 卡请用 hybrid 或 sherpa/CPU", recommended = cfg.EngineMode == "sidecar" },
    };
}

public sealed class MockTtsProvider : ITtsProvider
{
    private readonly BookCtlConfig _cfg;
    public MockTtsProvider(BookCtlConfig cfg) => _cfg = cfg;
    public string Name => "mock";
    public string Model => "mock-1";

    public Task<bool> HealthAsync(CancellationToken ct = default) => Task.FromResult(true);

    public async Task<SynthResult> SynthesizeAsync(string chunkId, string text, SynthesisProfile profile,
        string outWavPath, CancellationToken ct = default)
    {
        var sw = Stopwatch.StartNew();
        // mock 语速：每字 10ms（压缩模拟，真实语速约 3.5 字/秒，manifest 中已注明）
        var seconds = Math.Max(0.4, text.Length / 100.0);
        var dir = Path.GetDirectoryName(outWavPath);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
        Wav.WriteTone(outWavPath, 16000, seconds, seedOffset: Math.Abs(chunkId.GetHashCode()));
        // 模拟 GPU 工作占用，让中控台有实时滚动效果
        await Task.Delay(Math.Min(60, 15 + text.Length / 8), ct);
        return new SynthResult(outWavPath, seconds, sw.Elapsed.TotalSeconds);
    }
}

public sealed class SidecarTtsProvider : ITtsProvider
{
    private readonly BookCtlConfig _cfg;
    private readonly HttpClient _http;

    public SidecarTtsProvider(BookCtlConfig cfg)
    {
        _cfg = cfg;
        _http = new HttpClient { Timeout = TimeSpan.FromMinutes(15) };
    }

    public string Name => "sidecar";
    public string Model => "cosyvoice3";

    public async Task<bool> HealthAsync(CancellationToken ct = default)
    {
        try
        {
            using var resp = await _http.GetAsync(_cfg.SidecarUrl.TrimEnd('/') + "/health", ct);
            return resp.IsSuccessStatusCode;
        }
        catch { return false; }
    }

    public async Task<SynthResult> SynthesizeAsync(string chunkId, string text, SynthesisProfile profile,
        string outWavPath, CancellationToken ct = default)
    {
        var sw = Stopwatch.StartNew();
        var payload = Jx.S(new
        {
            text,
            voice = profile.Voice,
            speed = profile.Speed,
            seed = profile.Seed,
            sample_rate = profile.SampleRate,
            chunk_id = chunkId,
            // 设备选择：sidecar 首次加载模型时按这里决定 torch device / 卸载策略
            device = profile.Device,
            gpu_index = profile.GpuIndex,
            cpu_threads = profile.CpuThreads,
            cpu_offload = profile.CpuOffload,
        });
        using var content = new StringContent(payload, Encoding.UTF8, "application/json");
        using var resp = await _http.PostAsync(_cfg.SidecarUrl.TrimEnd('/') + "/synthesize", content, ct);
        var body = await resp.Content.ReadAsStringAsync(ct);
        if (!resp.IsSuccessStatusCode)
            throw new BookCtlException("TTS_SIDECAR_ERROR",
                $"TTS sidecar 返回 {(int)resp.StatusCode}：{Truncate(body, 300)}",
                retryable: true,
                fix: "确认 engine/tts_server.py 已启动且模型加载成功",
                example: "python engine/tts_server.py --mode cosyvoice3 --port 8765");
        var r = Jx.D<SidecarResp>(body)
            ?? throw new BookCtlException("TTS_SIDECAR_ERROR", "sidecar 响应无法解析", retryable: true);
        if (!r.Ok || string.IsNullOrEmpty(r.WavB64))
            throw new BookCtlException("TTS_SIDECAR_ERROR", "sidecar 合成失败：" + Truncate(r.Error ?? "未知错误", 300), retryable: true);

        var dir = Path.GetDirectoryName(outWavPath);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
        await File.WriteAllBytesAsync(outWavPath, Convert.FromBase64String(r.WavB64), ct);
        var seconds = r.Seconds > 0 ? r.Seconds : Wav.DurationOf(outWavPath);
        return new SynthResult(outWavPath, seconds, sw.Elapsed.TotalSeconds);
    }

    private static string Truncate(string s, int n) => s.Length <= n ? s : s[..n] + "…";

    private sealed record SidecarResp(
        bool Ok,
        [property: JsonPropertyName("wav_b64")] string? WavB64,
        double Seconds,
        string? Error);
}

public static class Wav
{
    public static void WriteTone(string path, int sampleRate, double seconds, int seedOffset = 0)
    {
        var n = (int)(sampleRate * seconds);
        var rnd = new Random(1234 + seedOffset);
        var freq = 165 + rnd.Next(0, 40);
        var phase = 0.0;
        using var fs = new FileStream(path, FileMode.Create, FileAccess.Write);
        using var bw = new BinaryWriter(fs);
        WriteHeader(bw, sampleRate, 1, 16, n * 2);
        for (var i = 0; i < n; i++)
        {
            var t = (double)i / sampleRate;
            // 语调起伏 + 音节节奏，模拟朗读包络
            var syllable = Math.Abs(Math.Sin(t * 4.5 + seedOffset % 7));
            var env = Math.Min(1.0, syllable * 1.6) * 0.16;
            phase += 2 * Math.PI * (freq * (1 + 0.06 * Math.Sin(t * 2.1 + seedOffset % 5))) / sampleRate;
            var amp = env * Math.Sin(phase);
            bw.Write((short)(amp * 32767));
        }
    }

    public static void Concat(string outPath, IReadOnlyList<string> inputs, int gapMs = 0)
    {
        if (inputs.Count == 0) throw new BookCtlException("ARGS_INVALID", "拼接列表为空");
        using var fs = new FileStream(outPath, FileMode.Create, FileAccess.Write);
        using var bw = new BinaryWriter(fs);
        var sampleRate = 16000;
        var gapBytes = 0;
        var total = 0L;
        var buffers = new List<byte[]>();
        foreach (var p in inputs)
        {
            if (!File.Exists(p)) continue;
            var data = File.ReadAllBytes(p);
            var (pcm, rate) = ExtractData(data);
            if (buffers.Count == 0)
            {
                sampleRate = rate;
                gapBytes = (int)(sampleRate * 2 * gapMs / 1000.0);
            }
            buffers.Add(pcm);
            total += pcm.Length;
        }
        if (buffers.Count == 0) throw new BookCtlException("ARGS_INVALID", "没有可拼接的音频段");
        total += gapBytes * Math.Max(0, buffers.Count - 1);
        WriteHeader(bw, sampleRate, 1, 16, (int)total);
        for (var i = 0; i < buffers.Count; i++)
        {
            if (i > 0 && gapBytes > 0) bw.Write(new byte[gapBytes]);
            bw.Write(buffers[i]);
        }
    }

    public static double DurationOf(string path)
    {
        var data = File.ReadAllBytes(path);
        var (pcm, rate) = ExtractData(data);
        return pcm.Length / 2.0 / rate;
    }

    private static (byte[] pcm, int rate) ExtractData(byte[] wav)
    {
        if (wav.Length < 44 || wav[0] != 'R' || wav[1] != 'I') throw new BookCtlException("WAV_INVALID", "非 WAV 文件");
        var rate = BitConverter.ToInt32(wav, 24);
        var i = 12;
        while (i + 8 <= wav.Length)
        {
            var chunkId = Encoding.ASCII.GetString(wav, i, 4);
            var size = BitConverter.ToInt32(wav, i + 4);
            if (chunkId == "data")
                return (wav[(i + 8)..(i + 8 + size)], rate);
            i += 8 + size + (size & 1);
        }
        throw new BookCtlException("WAV_INVALID", "WAV 中未找到 data 块");
    }

    private static void WriteHeader(BinaryWriter bw, int sampleRate, int channels, int bits, int dataLen)
    {
        bw.Write("RIFF"u8);
        bw.Write(36 + dataLen);
        bw.Write("WAVE"u8);
        bw.Write("fmt "u8);
        bw.Write(16);
        bw.Write((short)1);
        bw.Write((short)channels);
        bw.Write(sampleRate);
        bw.Write(sampleRate * channels * bits / 8);
        bw.Write((short)(channels * bits / 8));
        bw.Write((short)bits);
        bw.Write("data"u8);
        bw.Write(dataLen);
    }
}

public sealed class GpuMonitor
{
    private GpuStatus? _cached;
    private DateTime _lastQuery = DateTime.MinValue;
    private readonly string _mode;

    public GpuMonitor(string engineMode = "mock") => _mode = engineMode;

    public GpuStatus Query()
    {
        if (_cached is not null && (DateTime.UtcNow - _lastQuery).TotalSeconds < 1) return _cached;
        _cached = QueryNvidia();
        _lastQuery = DateTime.UtcNow;
        return _cached;
    }

    private GpuStatus QueryNvidia()
    {
        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = "nvidia-smi",
                Arguments = "--query-gpu=name,utilization.gpu,memory.used,memory.total --format=csv,noheader,nounits",
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true,
            };
            using var p = Process.Start(psi);
            if (p is null) return GpuStatus.Offline(_mode);
            var line = p.StandardOutput.ReadLine()?.Trim();
            p.WaitForExit(3000);
            if (p.ExitCode != 0 || string.IsNullOrEmpty(line)) return GpuStatus.Offline(_mode);
            var parts = line.Split(',', StringSplitOptions.TrimEntries);
            if (parts.Length < 4) return GpuStatus.Offline(_mode);
            return new GpuStatus
            {
                Available = true,
                Name = parts[0],
                UtilPct = int.TryParse(parts[1], out var u) ? u : 0,
                MemUsedMb = long.TryParse(parts[2], out var m1) ? m1 : 0,
                MemTotalMb = long.TryParse(parts[3], out var m2) ? m2 : 0,
                Mode = _mode,
            };
        }
        catch
        {
            return GpuStatus.Offline(_mode);
        }
    }
}

public sealed class AudioMaster
{
    private readonly string? _ffmpeg;
    private readonly string? _ffprobe;
    public bool HasFfmpeg => _ffmpeg is not null;

    public AudioMaster(BookCtlConfig cfg)
    {
        _ffmpeg = ResolveFfmpeg(cfg.FfmpegPath);
        _ffprobe = _ffmpeg is null ? null : ResolveSibling(_ffmpeg, "ffprobe.exe");
    }

    private static string? ResolveSibling(string exe, string sibling)
    {
        try
        {
            var p = Path.Combine(Path.GetDirectoryName(exe)!, sibling);
            if (File.Exists(p)) return p;
        }
        catch { }
        return null;
    }

    /// <summary>按真实容器取时长：WAV 读头，其余用 ffprobe；都失败则回退 16kHz 单声道估算。</summary>
    public double ProbeDuration(string path)
    {
        if (!File.Exists(path)) return 0;
        try
        {
            var head = new byte[4];
            using (var fs = File.OpenRead(path)) fs.ReadExactly(head, 0, 4);
            if (head[0] == (byte)'R' && head[1] == (byte)'I' && head[2] == (byte)'F' && head[3] == (byte)'F')
                return Wav.DurationOf(path);
        }
        catch { }

        if (_ffprobe is not null)
        {
            try
            {
                var psi = new ProcessStartInfo
                {
                    FileName = _ffprobe,
                    Arguments = $"-v error -show_entries format=duration -of default=nw=1:nk=1 \"{path}\"",
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                    UseShellExecute = false,
                    CreateNoWindow = true,
                };
                using var p = Process.Start(psi);
                if (p is not null)
                {
                    var line = p.StandardOutput.ReadLine();
                    p.WaitForExit(10000);
                    if (double.TryParse(line?.Trim(), System.Globalization.NumberStyles.Float,
                            System.Globalization.CultureInfo.InvariantCulture, out var sec) && sec > 0)
                        return sec;
                }
            }
            catch { }
        }

        // 最后兜底：AAC 64kbps → 字节数 / (64k/8)
        try { return new FileInfo(path).Length / 8000.0; } catch { return 0; }
    }

    private static string? ResolveFfmpeg(string? explicitPath)
    {
        if (!string.IsNullOrWhiteSpace(explicitPath) && File.Exists(explicitPath)) return explicitPath;
        var pathVar = Environment.GetEnvironmentVariable("PATH") ?? "";
        foreach (var dir in pathVar.Split(';', StringSplitOptions.RemoveEmptyEntries))
        {
            try
            {
                var p = Path.Combine(dir.Trim(), "ffmpeg.exe");
                if (File.Exists(p)) return p;
            }
            catch { }
        }
        foreach (var p in new[]
                 {
                     @"C:\ffmpeg\bin\ffmpeg.exe",
                     Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                         "Microsoft", "WinGet", "Links", "ffmpeg.exe"),
                 })
            if (File.Exists(p)) return p;

        // PATH 在进程启动后不会刷新（典型场景：刚 winget 装完 ffmpeg 就直接开 App），
        // 兜底扫 WinGet 包目录 <LocalAppData>\Microsoft\WinGet\Packages\*\ffmpeg-*\bin\ffmpeg.exe
        try
        {
            var packages = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "Microsoft", "WinGet", "Packages");
            if (Directory.Exists(packages))
            {
                foreach (var dir in Directory.EnumerateDirectories(packages, "*FFmpeg*",
                             new EnumerationOptions { RecurseSubdirectories = true, MaxRecursionDepth = 4 }))
                {
                    var bin = Path.Combine(dir, "bin", "ffmpeg.exe");
                    if (File.Exists(bin)) return bin;
                }
            }
        }
        catch { }
        return null;
    }

    public string MasterChapter(string chapterDir, List<string> segmentWavs, string title, int gapMs = 400)
    {
        var wavPath = Path.Combine(chapterDir, "audio.wav");
        Wav.Concat(wavPath, segmentWavs, gapMs);
        if (_ffmpeg is not null)
        {
            var m4a = Path.Combine(chapterDir, "audio.m4a");
            var ok = Run(_ffmpeg, $"-y -nostdin -i \"{wavPath}\" -c:a aac -b:a 64k \"{m4a}\"");
            if (ok && File.Exists(m4a)) return m4a;
        }
        return wavPath;
    }

    public string? MasterBook(List<Chapter> chapters, string outDir, BookMeta meta)
    {
        if (_ffmpeg is null || chapters.Count == 0) return null;
        var audioFiles = chapters
            .Where(c => c.AudioPath is not null && File.Exists(c.AudioPath))
            .Select(c => c.AudioPath!)
            .ToList();
        if (audioFiles.Count == 0) return null;

        var listPath = Path.Combine(outDir, "concat_list.txt");
        // concat demuxer 里反斜杠是转义符，必须统一成正斜杠；单引号需转义
        File.WriteAllLines(listPath, audioFiles.Select(p =>
            $"file '{p.Replace("\\", "/").Replace("'", "'\\''")}'"), new UTF8Encoding(false));

        var metaPath = Path.Combine(outDir, "book.ffmetadata");
        File.WriteAllText(metaPath, BuildFfmetadata(chapters, meta), new UTF8Encoding(false));

        var m4b = Path.Combine(outDir, "book.m4b");
        var ok = Run(_ffmpeg,
            $"-y -nostdin -f concat -safe 0 -i \"{listPath}\" -i \"{metaPath}\" -map_metadata 1 -c copy \"{m4b}\"");
        return ok && File.Exists(m4b) ? m4b : null;
    }

    private static string BuildFfmetadata(List<Chapter> chapters, BookMeta meta)
    {
        var sb = new StringBuilder();
        sb.AppendLine(";FFMETADATA1");
        sb.AppendLine($"title={Escape(meta.Title)}");
        if (!string.IsNullOrEmpty(meta.Creator)) sb.AppendLine($"artist={Escape(meta.Creator)}");
        var start = 0L;
        foreach (var ch in chapters)
        {
            if (ch.Status != "done" || ch.AudioPath is null) continue;
            var end = start + (long)(ch.Seconds * 1000);
            sb.AppendLine("[CHAPTER]");
            sb.AppendLine("TIMEBASE=1/1000");
            sb.AppendLine($"START={start}");
            sb.AppendLine($"END={end}");
            sb.AppendLine($"title={Escape(ch.Title)}");
            start = end;
        }
        return sb.ToString();

        static string Escape(string s) => s.Replace("\\", "\\\\").Replace("=", "\\=").Replace(";", "\\;").Replace("\n", " ");
    }

    private static bool Run(string exe, string args, int timeoutMs = 10 * 60 * 1000)
    {
        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = exe,
                Arguments = "-hide_banner -loglevel error -nostats " + args,
                RedirectStandardError = true,
                RedirectStandardOutput = true,
                UseShellExecute = false,
                CreateNoWindow = true,
            };
            using var p = Process.Start(psi);
            if (p is null) return false;
            // 必须并发读完两个管道：否则 ffmpeg 的进度/stderr 写满 4KB 缓冲后会永久阻塞（已实测死锁）
            var so = p.StandardOutput.ReadToEndAsync();
            var se = p.StandardError.ReadToEndAsync();
            if (!p.WaitForExit(timeoutMs))
            {
                try { p.Kill(entireProcessTree: true); } catch { }
                return false;
            }
            try { Task.WaitAll(new Task[] { so, se }, 5000); } catch { }
            return p.ExitCode == 0;
        }
        catch { return false; }
    }
}
