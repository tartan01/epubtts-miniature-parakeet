using System.Diagnostics;
using System.Text;

namespace BookCtl.Core;

public sealed class GpuDevice
{
    public int Index { get; set; }
    public string Name { get; set; } = "";
    public long MemTotalMb { get; set; }
    public long MemFreeMb { get; set; }
    public string Display => $"[{Index}] {Name} · {MemTotalMb:N0} MB";
}

/// <summary>一台机器上「这次到底用 CPU 还是 GPU、用哪张卡、要不要把层卸载到 CPU」的最终决议。</summary>
public sealed class DevicePlan
{
    public string Requested { get; set; } = "auto";
    public string Device { get; set; } = "cpu";        // cpu | gpu | hybrid
    public int GpuIndex { get; set; } = -1;
    public string? GpuName { get; set; }
    public long VramMb { get; set; }
    public long RequiredVramMb { get; set; }
    public int CpuThreads { get; set; }
    public bool CpuOffload { get; set; }
    public string Reason { get; set; } = "";
    public List<string> Warnings { get; set; } = new();

    public string Display => Device switch
    {
        "gpu" => $"GPU {GpuIndex}（{GpuName}，{VramMb:N0}/{RequiredVramMb:N0} MB 需求）",
        "hybrid" => $"GPU {GpuIndex} + CPU 卸载（{GpuName}，显存 {VramMb:N0} < 需求 {RequiredVramMb:N0} MB）",
        _ => $"CPU（{CpuThreads} 线程）",
    };

    /// <summary>传给 Python sidecar 的设备字符串（torch 语义）。</summary>
    public string TorchDevice => Device switch
    {
        "gpu" => $"cuda:{Math.Max(0, GpuIndex)}",
        "hybrid" => $"cuda:{Math.Max(0, GpuIndex)}",
        _ => "cpu",
    };

    public object ToDto() => new
    {
        requested = Requested,
        device = Device,
        gpu_index = GpuIndex,
        gpu_name = GpuName,
        vram_mb = VramMb,
        required_vram_mb = RequiredVramMb,
        cpu_threads = CpuThreads,
        cpu_offload = CpuOffload,
        torch_device = TorchDevice,
        display = Display,
        reason = Reason,
        warnings = Warnings,
    };
}

public static class DevicePlanner
{
    /// <summary>各引擎跑满速所需的显存下限（MB）。数字来源：CosyVoice 3 官方建议 6GB+；ONNX/VITS 类小于 2GB。</summary>
    public static long RequiredVram(string engineMode) => engineMode.Trim().ToLowerInvariant() switch
    {
        "cosyvoice3" or "sidecar" or "python" => 6144,
        "sherpa" or "onnx" or "sherpa-onnx" => 1536,
        "piper" => 512,
        _ => 0,
    };

    public static List<GpuDevice> ListGpus()
    {
        var list = new List<GpuDevice>();
        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = "nvidia-smi",
                Arguments = "--query-gpu=index,name,memory.total,memory.free --format=csv,noheader,nounits",
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true,
                StandardOutputEncoding = Encoding.UTF8,
            };
            using var p = Process.Start(psi);
            if (p is null) return list;
            var stdout = p.StandardOutput.ReadToEnd();
            p.StandardError.ReadToEnd();
            p.WaitForExit(5000);
            foreach (var raw in stdout.Split('\n', StringSplitOptions.RemoveEmptyEntries))
            {
                var parts = raw.Split(',', StringSplitOptions.TrimEntries);
                if (parts.Length < 4) continue;
                if (!int.TryParse(parts[0], out var idx)) continue;
                list.Add(new GpuDevice
                {
                    Index = idx,
                    Name = parts[1],
                    MemTotalMb = long.TryParse(parts[2], out var total) ? total : 0,
                    MemFreeMb = long.TryParse(parts[3], out var free) ? free : 0,
                });
            }
        }
        catch { }
        return list;
    }

    public static DevicePlan Resolve(BookCtlConfig cfg)
    {
        var requested = (cfg.Device ?? "auto").Trim().ToLowerInvariant();
        if (requested is not ("auto" or "cpu" or "gpu" or "hybrid")) requested = "auto";

        var required = RequiredVram(cfg.EngineMode);
        var threads = cfg.CpuThreads > 0 ? cfg.CpuThreads : Math.Max(1, Environment.ProcessorCount);

        // mock 引擎不碰硬件：明确按 CPU 记，避免 UI 误报"已调用显卡"
        if (required == 0 && requested != "gpu")
        {
            return new DevicePlan
            {
                Requested = requested, Device = "cpu", CpuThreads = threads, RequiredVramMb = 0,
                Reason = "当前引擎不消耗 GPU（mock 校样 / 纯 CPU 引擎），按 CPU 记录",
            };
        }

        var gpus = ListGpus();
        if (gpus.Count == 0)
        {
            var plan = new DevicePlan
            {
                Requested = requested, Device = "cpu", CpuThreads = threads, RequiredVramMb = required,
                Reason = "未检测到 NVIDIA 显卡，回退到纯 CPU 推理",
            };
            if (requested is "gpu" or "hybrid")
                plan.Warnings.Add("你要求使用显卡，但 nvidia-smi 没发现可用设备：已自动回退 CPU。");
            return plan;
        }

        var gpu = cfg.GpuIndex >= 0
            ? gpus.FirstOrDefault(g => g.Index == cfg.GpuIndex)
            : gpus.OrderByDescending(g => g.MemTotalMb).First();
        if (gpu is null)
        {
            var plan = new DevicePlan
            {
                Requested = requested, Device = "cpu", CpuThreads = threads, RequiredVramMb = required,
                Reason = $"指定的显卡序号 {cfg.GpuIndex} 不存在（本机有 {string.Join("、", gpus.Select(g => g.Index))}）",
            };
            plan.Warnings.Add("已回退 CPU。请用 device list 查看可用显卡序号。");
            return plan;
        }

        var enough = gpu.MemTotalMb >= required;
        var basePlan = new DevicePlan
        {
            Requested = requested,
            GpuIndex = gpu.Index,
            GpuName = gpu.Name,
            VramMb = gpu.MemTotalMb,
            RequiredVramMb = required,
            CpuThreads = threads,
        };

        switch (requested)
        {
            case "cpu":
                basePlan.Device = "cpu";
                basePlan.Reason = "用户显式选择纯 CPU 推理";
                return basePlan;

            case "gpu":
                basePlan.Device = "gpu";
                basePlan.CpuOffload = false;
                basePlan.Reason = $"用户显式选择 GPU {gpu.Index}";
                if (!enough)
                    basePlan.Warnings.Add(
                        $"显存 {gpu.MemTotalMb:N0} MB 低于该引擎建议的 {required:N0} MB：可能 OOM（显存不足）。可改用 hybrid 混合模式，或换 CPU 模式。");
                return basePlan;

            case "hybrid":
                basePlan.Device = "hybrid";
                basePlan.CpuOffload = true;
                basePlan.Reason = "用户显式选择 GPU + CPU 混合（层卸载）";
                if (enough)
                    basePlan.Warnings.Add("显存其实够用，混合模式反而更慢：建议切回纯 GPU。");
                return basePlan;

            default: // auto
                if (enough)
                {
                    basePlan.Device = "gpu";
                    basePlan.Reason = $"自动选择显存最大的显卡 {gpu.Index}（{gpu.MemTotalMb:N0} MB ≥ 需求 {required:N0} MB）";
                }
                else if (gpu.MemTotalMb >= required / 3)
                {
                    basePlan.Device = "hybrid";
                    basePlan.CpuOffload = true;
                    basePlan.Reason = $"显存不足以整模型驻留（{gpu.MemTotalMb:N0} < {required:N0} MB），自动选 GPU + CPU 卸载：能跑但更慢";
                    basePlan.Warnings.Add("想要更快的速度，请换 ≥8GB 显存的显卡，或改用 CPU 专用轻量引擎（sherpa-onnx / piper）。");
                }
                else
                {
                    basePlan.Device = "cpu";
                    basePlan.Reason = $"显存过小（{gpu.MemTotalMb:N0} MB，需求 {required:N0} MB），自动改用纯 CPU 推理";
                    basePlan.Warnings.Add("该显卡不适合跑此引擎；建议改用 CPU 轻量引擎，或换显卡。");
                }
                return basePlan;
        }
    }
}
