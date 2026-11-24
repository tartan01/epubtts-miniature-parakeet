using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace BookCtl.Core;

public static class Api
{
    public const string Product = "bookctl";
    public const string Version = "1.0";
    public const string ProtocolVersion = "v0.2";
    public const string McpProtocol = "2025-11-25";
    public const string McpProtocolLegacy = "2025-06-18";

    public const int ExitOk = 0;
    public const int ExitRuntime = 1;
    public const int ExitArgs = 2;
    public const int ExitNotFound = 3;
    public const int ExitPermission = 4;
    public const int ExitConflict = 5;
    public const int ExitTransient = 69;
    public const int ExitCancel = 130;
}

public static class Jx
{
    public static readonly JsonSerializerOptions Opt = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private static readonly JsonSerializerOptions PrettyOpt = new(Opt) { WriteIndented = true };

    public static string S(object? o) => JsonSerializer.Serialize(o, Opt);
    public static string P(object? o) => JsonSerializer.Serialize(o, PrettyOpt);
    public static T? D<T>(string json) where T : class => JsonSerializer.Deserialize<T>(json, Opt);

    public static object Ok(object? data) => new { ok = true, api_version = Api.Version, data };
    public static object Fail(BookCtlException e) => new
    {
        ok = false,
        api_version = Api.Version,
        error = new
        {
            code = e.Code,
            message = e.Message,
            field = e.Field,
            value = e.Value,
            fix = e.Fix,
            example = e.Example,
            retryable = e.Retryable,
            suggestions = e.Suggestions,
        },
    };
}

public sealed class BookCtlException : Exception
{
    public string Code { get; }
    public string? Field { get; }
    public string? Value { get; }
    public string? Fix { get; }
    public string? Example { get; }
    public bool Retryable { get; }
    public string[]? Suggestions { get; }
    public int ExitCode { get; }

    public BookCtlException(string code, string message,
        string? field = null, string? value = null, string? fix = null, string? example = null,
        bool retryable = false, string[]? suggestions = null, int? exitCode = null)
        : base(message)
    {
        Code = code;
        Field = field;
        Value = value;
        Fix = fix;
        Example = example;
        Retryable = retryable;
        Suggestions = suggestions;
        ExitCode = exitCode ?? DefaultExit(code, retryable);
    }

    private static int DefaultExit(string code, bool retryable) => retryable ? Api.ExitTransient : code switch
    {
        "ARGS_MISSING" or "ARGS_INVALID" => Api.ExitArgs,
        "NOT_FOUND" or "FILE_NOT_FOUND" => Api.ExitNotFound,
        "PERMISSION_DENIED" => Api.ExitPermission,
        "IDEMPOTENCY_KEY_CONFLICT" or "INVALID_STATE_TRANSITION" => Api.ExitConflict,
        "DRM_PROTECTED" => Api.ExitPermission,
        "CANCELLED" => Api.ExitCancel,
        _ => Api.ExitRuntime,
    };
}

public static class HashUtil
{
    public static string Sha(string s) =>
        Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(s))).ToLowerInvariant();

    public static string ShaFile(string path)
    {
        using var fs = File.OpenRead(path);
        return Convert.ToHexString(SHA256.HashData(fs)).ToLowerInvariant();
    }
}

public static class IdGen
{
    private static readonly Random Rnd = new();

    public static string NewJobId() =>
        $"job-{DateTimeOffset.UtcNow.ToUnixTimeMilliseconds():x8}-{Rnd.Next(1 << 30):x6}";

    public static string NewChunkId(int chapterIndex, int segmentIndex) =>
        $"ch{chapterIndex:D4}-seg{segmentIndex:D4}";
}

public static class Paths
{
    public static string SanitizeName(string s)
    {
        if (string.IsNullOrWhiteSpace(s)) return "untitled";
        var invalid = Path.GetInvalidFileNameChars().Append(' ').ToArray();
        var parts = s.Split(invalid, StringSplitOptions.RemoveEmptyEntries);
        var joined = string.Join('_', parts);
        if (joined.Length == 0) joined = "untitled";
        return joined.Length > 48 ? joined[..48] : joined;
    }

    public static string ForBook(BookCtlConfig cfg, string bookId, string title) =>
        Path.Combine(cfg.EffectiveWorkspaceRoot, bookId + "_" + SanitizeName(title));
}

public sealed class BookCtlConfig
{
    public string ConfigVersion { get; set; } = "v0.2";
    public string WorkspaceRoot { get; set; } = "";
    public string EngineMode { get; set; } = "mock";
    public string SidecarUrl { get; set; } = "http://127.0.0.1:8765";
    public string Voice { get; set; } = "zh-default";
    public double Speed { get; set; } = 1.0;
    public int SampleRate { get; set; } = 24000;
    public int Seed { get; set; } = 42;
    public List<string> IncludeKinds { get; set; } = new() { "body", "frontmatter" };
    public bool GenerateM4B { get; set; } = true;
    public bool Verify { get; set; } = false;
    public int MaxConcurrentJobs { get; set; } = 1;
    public string FfmpegPath { get; set; } = "";
    public int CharsPerMinute { get; set; } = 210;

    /// <summary>推理设备：auto / cpu / gpu / hybrid（GPU 显存不足时把部分层卸载到 CPU）。</summary>
    public string Device { get; set; } = "auto";

    /// <summary>使用的显卡序号（nvidia-smi 顺序），-1 = 自动选显存最大的那张。</summary>
    public int GpuIndex { get; set; } = -1;

    /// <summary>CPU 推理线程数，0 = 自动（物理核数）。</summary>
    public int CpuThreads { get; set; } = 0;

    /// <summary>GPU 显存不足时允许把部分层放到 CPU（device_map 自动卸载），通常是"能跑但更慢"。</summary>
    public bool AllowCpuOffload { get; set; } = false;

    /// <summary>发行版本：community / pro。封装时写入，运行时只读。</summary>
    public string Edition { get; set; } = "community";

    public string EffectiveWorkspaceRoot => string.IsNullOrWhiteSpace(WorkspaceRoot)
        ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "bookctl", "workspace")
        : WorkspaceRoot;

    public string DbPath => Path.Combine(EffectiveWorkspaceRoot, "bookctl.db");

    /// <summary>引擎/模型源的落盘根目录（portable 版为程序目录旁的 sources/，安装版为 %LOCALAPPDATA%）。</summary>
    public string EffectiveSourcesRoot { get; set; } = "";

    /// <summary>引擎/模型源根目录：固定跟随安装位置，不随 WorkspaceRoot 漂移（换输出目录不该丢模型）。</summary>
    public string SourcesRoot => !string.IsNullOrWhiteSpace(EffectiveSourcesRoot)
        ? EffectiveSourcesRoot
        : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "bookctl", "sources");

    public BookCtlConfig CloneWith(string? engine = null, string? voice = null, double? speed = null,
        bool? generateM4b = null, string? device = null, int? gpuIndex = null, int? cpuThreads = null)
    {
        var c = (BookCtlConfig)MemberwiseClone();
        if (!string.IsNullOrWhiteSpace(engine)) c.EngineMode = engine;
        if (!string.IsNullOrWhiteSpace(voice)) c.Voice = voice;
        if (speed is > 0.25 and < 4.0) c.Speed = speed.Value;
        if (generateM4b.HasValue) c.GenerateM4B = generateM4b.Value;
        if (!string.IsNullOrWhiteSpace(device)) c.Device = device;
        if (gpuIndex.HasValue) c.GpuIndex = gpuIndex.Value;
        if (cpuThreads.HasValue) c.CpuThreads = cpuThreads.Value;
        return c;
    }

    public static BookCtlConfig Load(string? explicitPath = null)
    {
        var cfg = new BookCtlConfig();
        var file = explicitPath ?? FindDefaultsFile();
        if (file is not null)
        {
            var loaded = Jx.D<BookCtlConfig>(File.ReadAllText(file, Encoding.UTF8));
            if (loaded is not null) cfg = loaded;
        }
        ApplyEnv(cfg);
        ApplyPortable(cfg);
        return cfg;
    }

    /// <summary>
    /// 便携版：程序目录存在 portable.marker 时，数据与模型全部留在程序目录内（U 盘可带走）。
    /// 安装版不创建该文件，走 %LOCALAPPDATA%，避免 Program Files 无写权限。
    /// </summary>
    private static void ApplyPortable(BookCtlConfig cfg)
    {
        try
        {
            // CLI / MCP / Worker 发布在 tools\<proj>\ 子目录，marker 只在程序根目录；
            // 与 FindDefaultsFile 一致逐级向上查找，最多 6 层，避免把上层无关目录误判为便携版。
            var dir = new DirectoryInfo(AppContext.BaseDirectory);
            string? root = null;
            for (var i = 0; i < 6 && dir is not null; i++, dir = dir.Parent)
            {
                if (!File.Exists(Path.Combine(dir.FullName, "portable.marker"))) continue;
                root = dir.FullName;
                break;
            }
            if (root is null) return;
            if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("BOOKCTL_WORKSPACE")))
                cfg.WorkspaceRoot = Path.Combine(root, "data");
            if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("BOOKCTL_SOURCES")))
                cfg.EffectiveSourcesRoot = Path.Combine(root, "sources");
            cfg.Edition = "pro";
        }
        catch { }
    }

    private static string? FindDefaultsFile()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        for (var i = 0; i < 6 && dir is not null; i++)
        {
            var probe = Path.Combine(dir.FullName, "config", "bookctl.defaults.json");
            if (File.Exists(probe)) return probe;
            probe = Path.Combine(dir.FullName, "bookctl.defaults.json");
            if (File.Exists(probe)) return probe;
            dir = dir.Parent;
        }
        return null;
    }

    private static void ApplyEnv(BookCtlConfig cfg)
    {
        var ws = Environment.GetEnvironmentVariable("BOOKCTL_WORKSPACE");
        if (!string.IsNullOrWhiteSpace(ws)) cfg.WorkspaceRoot = ws;
        var engine = Environment.GetEnvironmentVariable("BOOKCTL_ENGINE");
        if (!string.IsNullOrWhiteSpace(engine)) cfg.EngineMode = engine;
        var sidecar = Environment.GetEnvironmentVariable("BOOKCTL_SIDECAR_URL");
        if (!string.IsNullOrWhiteSpace(sidecar)) cfg.SidecarUrl = sidecar;
        var voice = Environment.GetEnvironmentVariable("BOOKCTL_VOICE");
        if (!string.IsNullOrWhiteSpace(voice)) cfg.Voice = voice;
        var ffmpeg = Environment.GetEnvironmentVariable("BOOKCTL_FFMPEG");
        if (!string.IsNullOrWhiteSpace(ffmpeg)) cfg.FfmpegPath = ffmpeg;
        var device = Environment.GetEnvironmentVariable("BOOKCTL_DEVICE");
        if (!string.IsNullOrWhiteSpace(device)) cfg.Device = device;
        var gpu = Environment.GetEnvironmentVariable("BOOKCTL_GPU");
        if (int.TryParse(gpu, out var gi)) cfg.GpuIndex = gi;
        var threads = Environment.GetEnvironmentVariable("BOOKCTL_CPU_THREADS");
        if (int.TryParse(threads, out var th)) cfg.CpuThreads = th;
        var sources = Environment.GetEnvironmentVariable("BOOKCTL_SOURCES");
        if (!string.IsNullOrWhiteSpace(sources)) cfg.EffectiveSourcesRoot = sources;
    }

    public string Hash() => "sha256:" + HashUtil.Sha(Jx.S(new
    {
        config_version = ConfigVersion,
        engine_mode = EngineMode,
        voice = Voice,
        speed = Speed,
        sample_rate = SampleRate,
        seed = Seed,
        include_kinds = IncludeKinds.OrderBy(x => x).ToList(),
        generate_m4b = GenerateM4B,
        verify = Verify,
        chars_per_minute = CharsPerMinute,
    }));
}
