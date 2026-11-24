namespace BookCtl.Core;

public enum JobStatus { Queued, Running, Paused, InputRequired, Cancelled, Completed, Failed }

public static class JobStatuses
{
    public static string S(JobStatus s) => s.ToString().ToLowerInvariant();

    public static bool IsTerminal(string s) =>
        s is "cancelled" or "completed" or "failed";

    public static string DescribePhase(string? phase) => phase switch
    {
        null or "queued" => "排队中，等待 GPU 空闲",
        "S1_INGEST" => "正在解析 EPUB 结构",
        "S2_SPLIT" => "正在按目录切分章节",
        "S3_NORMALIZE" => "正在做文本前端处理（TN/分句）",
        "S4_SYNTH" => "正在调用显卡进行 TTS 合成",
        "S5_VERIFY" => "正在回验（ASR 比对）",
        "S6_MASTER" => "正在母带处理（响度/M4B）",
        "COMPLETED" => "已完成",
        _ => phase,
    };
}

public sealed class GpuStatus
{
    public bool Available { get; set; }
    public string Name { get; set; } = "";
    public int UtilPct { get; set; }
    public long MemUsedMb { get; set; }
    public long MemTotalMb { get; set; }
    public string Mode { get; set; } = "mock";

    public string Display => Available
        ? $"{Name} · {UtilPct}% · {MemUsedMb}/{MemTotalMb} MB ({Mode})"
        : $"未检测到 NVIDIA GPU（{Mode} 模式）";

    public static GpuStatus Offline(string mode = "mock") => new() { Available = false, Mode = mode };
}

public sealed class BookMeta
{
    public string Title { get; set; } = "";
    public string Creator { get; set; } = "";
    public string Language { get; set; } = "zh";
    public string InputPath { get; set; } = "";
    public string InputSha256 { get; set; } = "";
    public string EpubVersion { get; set; } = "";
    public bool DrmProtected { get; set; }
    public string DrmDetail { get; set; } = "";
    public string BookId { get; set; } = "";
    public string? CoverPath { get; set; }
}

public sealed class Chapter
{
    public string Id { get; set; } = "";
    public int Index { get; set; }
    public int Level { get; set; } = 1;
    public string Title { get; set; } = "";
    public string Href { get; set; } = "";
    public string? Anchor { get; set; }
    public string Kind { get; set; } = "body";
    public string TextRaw { get; set; } = "";
    public int CharCount { get; set; }
    public string Status { get; set; } = "pending";
    public string? AudioPath { get; set; }
    public string? Sha256 { get; set; }
    public double Seconds { get; set; }
    public double? Cer { get; set; }
    public int Attempts { get; set; }
}

public sealed class Segment
{
    public string ChunkId { get; set; } = "";
    public string ChapterId { get; set; } = "";
    public string TextRaw { get; set; } = "";
    public string TextNorm { get; set; } = "";
    public string? WavPath { get; set; }
    public double Seconds { get; set; }
    public int Attempts { get; set; }
    public string? Status { get; set; }
}

public sealed class SynthesisProfile
{
    public string Engine { get; set; } = "mock";
    public string Model { get; set; } = "mock-1";
    public string Voice { get; set; } = "zh-default";
    public double Speed { get; set; } = 1.0;
    public int SampleRate { get; set; } = 24000;
    public int Seed { get; set; } = 42;
    public string ConfigHash { get; set; } = "";
    public string Hash { get; set; } = "";

    // 设备选择不参与 Hash：同一本书换卡/换 CPU 不应改变音频确定性判定
    public string Device { get; set; } = "cpu";
    public int GpuIndex { get; set; } = -1;
    public int CpuThreads { get; set; } = 0;
    public bool CpuOffload { get; set; }

    public static SynthesisProfile From(BookCtlConfig cfg, string engineModel, DevicePlan? plan = null)
    {
        var p = new SynthesisProfile
        {
            Engine = cfg.EngineMode,
            Model = engineModel,
            Voice = cfg.Voice,
            Speed = cfg.Speed,
            SampleRate = cfg.SampleRate,
            Seed = cfg.Seed,
            ConfigHash = cfg.Hash(),
        };
        if (plan is not null)
        {
            p.Device = plan.Device;
            p.GpuIndex = plan.GpuIndex;
            p.CpuThreads = plan.CpuThreads;
            p.CpuOffload = plan.CpuOffload;
        }
        p.Hash = "sha256:" + HashUtil.Sha(Jx.S(new { p.Engine, p.Model, p.Voice, p.Speed, p.SampleRate, p.Seed, p.ConfigHash }));
        return p;
    }
}

public sealed class JobRecord
{
    public string JobId { get; set; } = "";
    public string? IdempotencyKey { get; set; }
    public string InputPath { get; set; } = "";
    public string? BookId { get; set; }
    public string? BookTitle { get; set; }
    public string Status { get; set; } = "queued";
    public string? Phase { get; set; }
    public long Revision { get; set; } = 1;
    public string? ConfigHash { get; set; }
    public string? ProfileHash { get; set; }
    public string? ArgsHash { get; set; }
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
    public DateTime UpdatedAt { get; set; } = DateTime.UtcNow;
    public double Progress { get; set; }
    public int ChaptersDone { get; set; }
    public int ChaptersTotal { get; set; }
    public int CharsDone { get; set; }
    public int CharsTotal { get; set; }
    public double AudioSeconds { get; set; }
    public double GpuSeconds { get; set; }
    public string? ErrorCode { get; set; }
    public string? ErrorMessage { get; set; }
    public string? ResultPath { get; set; }
    public bool CancelRequested { get; set; }
    public string? Engine { get; set; }
    public string? Voice { get; set; }

    public bool IsTerminal => JobStatuses.IsTerminal(Status);
}

public sealed class JobEvent
{
    public long Seq { get; set; }
    public string JobId { get; set; } = "";
    public string Ts { get; set; } = "";
    public string Level { get; set; } = "info";
    public string Stage { get; set; } = "";
    public string Code { get; set; } = "";
    public string Message { get; set; } = "";
    public string? DataJson { get; set; }
}
