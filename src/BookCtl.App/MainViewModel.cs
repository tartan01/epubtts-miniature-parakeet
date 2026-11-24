using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.CompilerServices;
using System.Text.Json;
using BookCtl.Core;
using Microsoft.UI.Xaml;

namespace BookCtl.App;

public sealed class LogLine
{
    public string Time { get; init; } = "";
    public string Level { get; init; } = "INFO";
    public string Stage { get; init; } = "";
    public string Text { get; init; } = "";
}

/// <summary>已生成章节的结果卡片（简洁模式实时出现）。</summary>
public sealed class ChapterCard
{
    public string Index { get; init; } = "";
    public string Title { get; init; } = "";
    public string Duration { get; init; } = "";
    public string FileName { get; init; } = "";
    public string Badge { get; init; } = "已完成";
}

public sealed class MainViewModel : INotifyPropertyChanged
{
    public event PropertyChangedEventHandler? PropertyChanged;

    private string? _jobId;
    private string? _resultPath;
    private double _charsPerSec;
    private double _audioSecPerSec;
    private double _rtf;
    private double _etaSeconds;
    private int _gpuUtil;
    private string _gpuName = "未检测";

    public MainViewModel()
    {
        Engines = new ObservableCollection<string> { "sherpa", "mock", "sidecar" };
        Voices = new ObservableCollection<string> { "0", "1", "2", "3", "zh-default" };
        DeviceModes = new ObservableCollection<string> { "自动（按显存决策）", "纯 CPU", "纯 GPU", "GPU + CPU 混合" };
        Devices = new ObservableCollection<string>();

        var cfg = App.Handlers.Config;
        Engine = cfg.EngineMode;
        Voice = cfg.Voice;
        Speed = cfg.Speed.ToString("0.0");
        GenerateM4B = cfg.GenerateM4B;
        DeviceMode = DeviceLabel(cfg.Device);
        CpuThreads = cfg.CpuThreads > 0 ? cfg.CpuThreads.ToString() : "";
        LoadDevices();

        WorkspaceText = $"工作区：{cfg.EffectiveWorkspaceRoot}";
        FooterText = "待命";
        HintText = "选择一本 .epub，点击「生成朗读音频」。";
        PhaseText = "待命";
        GpuText = App.Handlers.Gpu.Query().Display;
        TtsText = $"{Engine} / {Voice}";
        SpeedText = "等待任务";
        ProgressText = "0 / 0 章 · 0 / 0 字";
        EtaText = "";
        Busy = false;
        CanStart = true;
        CanOpenOutput = false;
        OutputFolder = "";
        IsSimpleMode = true;
        UpdateSourceStatus();
    }

    // ——— 视图模式 ———
    private bool _isSimpleMode = true;

    public bool IsSimpleMode
    {
        get => _isSimpleMode;
        set
        {
            if (!Set(ref _isSimpleMode, value)) return;
            OnPropertyChanged(nameof(IsDetailedMode));
            OnPropertyChanged(nameof(SimpleVisibility));
            OnPropertyChanged(nameof(DetailedVisibility));
            OnPropertyChanged(nameof(ModeTitle));
            OnPropertyChanged(nameof(ModeSubtitle));
        }
    }

    public bool IsDetailedMode
    {
        get => !_isSimpleMode;
        set { if (value) IsSimpleMode = false; }
    }

    public Visibility SimpleVisibility => _isSimpleMode ? Visibility.Visible : Visibility.Collapsed;
    public Visibility DetailedVisibility => _isSimpleMode ? Visibility.Collapsed : Visibility.Visible;

    public string ModeTitle => _isSimpleMode ? "拖入一本书，直接变成有声书。" : "所有参数，都在你手里。";
    public string ModeSubtitle => _isSimpleMode
        ? "自动识别目录、切分章节，用本地 CPU 引擎朗读一整本书。"
        : "选择引擎、音色、显卡、语速与输出格式，逐段可控。";

    // ——— 引擎 / 音色 / 设备 ———
    public ObservableCollection<string> Engines { get; }
    public ObservableCollection<string> Voices { get; }
    public ObservableCollection<string> DeviceModes { get; }
    public ObservableCollection<string> Devices { get; }

    private string _inputPath = "";
    public string InputPath
    {
        get => _inputPath;
        set { if (Set(ref _inputPath, value)) OnPropertyChanged(nameof(HasInput)); }
    }

    public bool HasInput => !string.IsNullOrWhiteSpace(_inputPath);

    private string _engine = "mock";
    public string Engine
    {
        get => _engine;
        set
        {
            if (!Set(ref _engine, value)) return;
            App.Handlers.Config.EngineMode = value;
            OnPropertyChanged(nameof(EngineBadge));
            TtsText = $"{Engine} / {Voice}";
            UpdateSourceStatus();
            LoadDevices();
        }
    }

    private string _voice = "zh-default";
    public string Voice
    {
        get => _voice;
        set
        {
            if (!Set(ref _voice, value)) return;
            App.Handlers.Config.Voice = value;
            TtsText = $"{Engine} / {Voice}";
        }
    }

    private string _speed = "1.0";
    public string Speed { get => _speed; set => Set(ref _speed, value); }

    private bool _generateM4B = true;
    public bool GenerateM4B
    {
        get => _generateM4B;
        set { if (Set(ref _generateM4B, value)) App.Handlers.Config.GenerateM4B = value; }
    }

    private string _deviceMode = "自动（按显存决策）";
    public string DeviceMode
    {
        get => _deviceMode;
        set
        {
            if (!Set(ref _deviceMode, value)) return;
            App.Handlers.Config.Device = DeviceKey(value);
            RefreshPlan();
        }
    }

    private int _deviceIndex;
    public int DeviceIndex
    {
        get => _deviceIndex;
        set
        {
            if (!Set(ref _deviceIndex, value)) return;
            App.Handlers.Config.GpuIndex = value > 0 ? value - 1 : -1;
            RefreshPlan();
        }
    }

    private string _cpuThreads = "";
    public string CpuThreads
    {
        get => _cpuThreads;
        set
        {
            if (!Set(ref _cpuThreads, value)) return;
            App.Handlers.Config.CpuThreads = int.TryParse(value, out var n) ? n : 0;
        }
    }

    public string EngineBadge => Engine switch
    {
        "sherpa" => "CPU 引擎 · 本地",
        "sidecar" => "GPU 引擎 · 本地",
        _ => "校样引擎",
    };

    // ——— 运行状态 ———
    private bool _busy;
    public bool Busy { get => _busy; set => Set(ref _busy, value); }

    private bool _canStart = true;
    public bool CanStart { get => _canStart; set => Set(ref _canStart, value); }

    private bool _canOpenOutput;
    public bool CanOpenOutput { get => _canOpenOutput; set => Set(ref _canOpenOutput, value); }

    private string _phaseText = "";
    public string PhaseText { get => _phaseText; set => Set(ref _phaseText, value); }

    private string _gpuText = "";
    public string GpuText { get => _gpuText; set => Set(ref _gpuText, value); }

    private string _ttsText = "";
    public string TtsText { get => _ttsText; set => Set(ref _ttsText, value); }

    private string _speedText = "";
    public string SpeedText { get => _speedText; set => Set(ref _speedText, value); }

    private double _progressValue;
    public double ProgressValue { get => _progressValue; set => Set(ref _progressValue, value); }

    private string _progressText = "";
    public string ProgressText { get => _progressText; set => Set(ref _progressText, value); }

    private string _etaText = "";
    public string EtaText { get => _etaText; set => Set(ref _etaText, value); }

    private string _hintText = "";
    public string HintText { get => _hintText; set => Set(ref _hintText, value); }

    private string _workspaceText = "";
    public string WorkspaceText { get => _workspaceText; set => Set(ref _workspaceText, value); }

    private string _footerText = "";
    public string FooterText { get => _footerText; set => Set(ref _footerText, value); }

    private string _sourceStatus = "";
    public string SourceStatus { get => _sourceStatus; set => Set(ref _sourceStatus, value); }

    private string _outputFolder = "";
    public string OutputFolder { get => _outputFolder; set => Set(ref _outputFolder, value); }

    public ObservableCollection<LogLine> Logs { get; } = new();
    public ObservableCollection<ChapterCard> Chapters { get; } = new();

    public string ChapterSummary => Chapters.Count == 0 ? "尚未生成章节" : $"已生成 {Chapters.Count} 章";

    // ——— 行为 ———

    public void ToggleMode(bool simple) => IsSimpleMode = simple;

    /// <summary>简洁模式：不暴露参数，直接用能跑的最小引擎（有 CPU 模型就用 sherpa，没有就 mock 校样）。</summary>
    public void StartOneClick()
    {
        if (!ValidateInput()) return;
        var hasCpuModel = App.Handlers.Sources.InstalledPath("sherpa-vits-zh-fanchen") is not null
                          || App.Handlers.Sources.InstalledPath("sherpa-vits-zh-aishell3") is not null;
        if (IsSimpleMode)
        {
            // 装了模型 ≠ 服务起来了：sidecar 没起时硬跑会抛 TTS_SIDECAR_ERROR。
            // 一键模式必须"点了就出结果"，所以先探活，不可达就退到校样引擎并把原因说清楚。
            var sidecarOk = hasCpuModel && SidecarAlive(App.Handlers.Config.SidecarUrl);
            App.Handlers.Config.EngineMode = sidecarOk ? "sherpa" : "mock";
            App.Handlers.Config.Device = "cpu";
            App.Handlers.Config.Voice = sidecarOk ? "0" : "zh-default";
            Engine = App.Handlers.Config.EngineMode;
            Voice = App.Handlers.Config.Voice;
            if (!sidecarOk)
                HintText = hasCpuModel
                    ? $"已装中文模型，但语音服务 {App.Handlers.Config.SidecarUrl} 未响应，先用校样引擎跑通流程。启动：python engine/tts_server.py --mode sherpa"
                    : "未检测到本地 CPU 语音模型，先用校样引擎跑通流程。可到「引擎与模型」下载中文模型（约 130MB）。";
        }
        StartInternal();
    }

    /// <summary>2 秒探活：sidecar 不在就退回校样引擎，避免一键模式直接报错。</summary>
    private static bool SidecarAlive(string url)
    {
        try
        {
            using var http = new System.Net.Http.HttpClient { Timeout = TimeSpan.FromSeconds(2) };
            using var r = http.GetAsync(url.TrimEnd('/') + "/health").GetAwaiter().GetResult();
            return r.IsSuccessStatusCode;
        }
        catch { return false; }
    }

    public void Start()
    {
        if (!ValidateInput()) return;
        if (!double.TryParse(Speed, out var sp)) sp = 1.0;
        App.Handlers.Config.Speed = Math.Clamp(sp, 0.5, 2.0);
        App.Handlers.Config.EngineMode = Engine;
        App.Handlers.Config.Voice = Voice;
        App.Handlers.Config.GenerateM4B = GenerateM4B;
        StartInternal();
    }

    private bool ValidateInput()
    {
        if (string.IsNullOrWhiteSpace(InputPath) || !File.Exists(InputPath))
        {
            HintText = "请先选择一个存在的 .epub 文件。";
            return false;
        }
        if (Busy)
        {
            HintText = "已有任务在执行，请等待完成或先取消。";
            return false;
        }
        return true;
    }

    private void StartInternal()
    {
        try
        {
            var started = App.Handlers.JobStart(InputPath);
            using var doc = JsonDocument.Parse(Jx.S(started));
            _jobId = doc.RootElement.GetProperty("job_id").GetString();
            _resultPath = null;
            _charsPerSec = _audioSecPerSec = _rtf = _etaSeconds = 0;
            Busy = true;
            CanStart = false;
            CanOpenOutput = false;
            Logs.Clear();
            Chapters.Clear();
            OnPropertyChanged(nameof(ChapterSummary));
            OutputFolder = "";
            HintText = "已开始。正在识别目录并切分章节，随后逐章生成音频。";
            FooterText = "转换中";
            PhaseText = "排队中";
            ProgressValue = 0;
            AddLocal("INFO", "queued", $"已提交：{Path.GetFileName(InputPath)}（{Engine}）");
        }
        catch (BookCtlException e)
        {
            HintText = $"{e.Code}：{e.Message}。{e.Fix}";
        }
    }

    public void Cancel()
    {
        if (_jobId is null || !Busy) return;
        try
        {
            App.Handlers.JobCancel(_jobId, "用户取消");
            HintText = "已发出取消指令，将在当前章节边界生效；已完成章节保留。";
        }
        catch (BookCtlException e) { HintText = e.Message; }
    }

    public void OpenOutput()
    {
        var dir = _resultPath;
        if (string.IsNullOrEmpty(dir) || !Directory.Exists(dir))
        {
            HintText = "产物文件夹尚未生成。";
            return;
        }
        var target = Path.Combine(dir, "manifest.json");
        var arg = File.Exists(target) ? $"/select,\"{target}\"" : $"\"{dir}\"";
        Process.Start(new ProcessStartInfo { FileName = "explorer.exe", Arguments = arg, UseShellExecute = true });
    }

    public void ClearLogs() => Logs.Clear();

    public void AddLog(JobEvent e)
    {
        Logs.Add(new LogLine
        {
            Time = ParseTime(e.Ts),
            Level = e.Level.ToUpperInvariant(),
            Stage = e.Stage,
            Text = e.Message,
        });
        while (Logs.Count > 800) Logs.RemoveAt(0);

        if (e.Code == "segment_done" && e.DataJson is not null)
        {
            try
            {
                using var d = JsonDocument.Parse(e.DataJson);
                var r = d.RootElement;
                _charsPerSec = GetDouble(r, "chars_per_sec", _charsPerSec);
                _audioSecPerSec = GetDouble(r, "audio_sec_per_sec", _audioSecPerSec);
                _rtf = GetDouble(r, "rtf", _rtf);
                _etaSeconds = GetDouble(r, "eta_seconds", _etaSeconds);
                if (r.TryGetProperty("gpu", out var g) && g.ValueKind == JsonValueKind.Object)
                {
                    _gpuUtil = (int)GetDouble(g, "util_pct", _gpuUtil);
                    var name = g.TryGetProperty("name", out var n) ? n.GetString() : null;
                    if (!string.IsNullOrEmpty(name)) _gpuName = name;
                }
                SpeedText = $"{_charsPerSec:N0} 字/秒 · {_audioSecPerSec:N1}× 实时\nRTF {_rtf:0.000}";
                EtaText = _etaSeconds > 0 ? $"预计剩余 {FormatEta(_etaSeconds)}" : "";
                var dev = r.TryGetProperty("device_display", out var dd) ? dd.GetString() : null;
                GpuText = string.IsNullOrEmpty(dev) ? App.Handlers.Gpu.Query().Display : dev;
            }
            catch { }
        }
        else if (e.Code == "chapter_done" && e.DataJson is not null)
        {
            try
            {
                using var d = JsonDocument.Parse(e.DataJson);
                var r = d.RootElement;
                var idx = (int)GetDouble(r, "chapter_index", Chapters.Count + 1);
                var title = r.TryGetProperty("chapter_title", out var t) ? t.GetString() ?? "" : "";
                var sec = GetDouble(r, "seconds", 0);
                var audio = r.TryGetProperty("audio", out var a) ? a.GetString() ?? "" : "";
                Chapters.Add(new ChapterCard
                {
                    Index = $"第 {idx} 章",
                    Title = title,
                    Duration = FormatEta(sec),
                    FileName = string.IsNullOrEmpty(audio) ? "" : Path.GetFileName(audio),
                });
                OnPropertyChanged(nameof(ChapterSummary));
            }
            catch { }
        }
        else if (e.Code == "job_completed" && e.DataJson is not null)
        {
            try
            {
                using var d = JsonDocument.Parse(e.DataJson);
                if (d.RootElement.TryGetProperty("result_path", out var p))
                {
                    _resultPath = p.GetString();
                    OutputFolder = _resultPath ?? "";
                }
            }
            catch { }
            CanOpenOutput = !string.IsNullOrEmpty(_resultPath);
            HintText = $"完成！已生成 {Chapters.Count} 章音频，输出文件夹见下方。";
            FooterText = "已完成";
        }
        else if (e.Code == "job_failed" || e.Code == "job_cancelled")
        {
            HintText = e.Message;
            FooterText = e.Code == "job_failed" ? "失败（可续跑）" : "已取消";
        }
    }

    public void Refresh()
    {
        if (_jobId is null) return;
        var job = App.Handlers.Store.GetJob(_jobId);
        if (job is null) return;

        PhaseText = JobStatuses.DescribePhase(job.Phase);
        ProgressValue = Math.Round(job.Progress * 100, 1);
        ProgressText = $"{job.ChaptersDone} / {job.ChaptersTotal} 章 · {job.CharsDone:N0} / {job.CharsTotal:N0} 字 · 已生成 {PipelineRunner.FormatDuration(job.AudioSeconds)}";

        if (!job.IsTerminal) return;
        if (!Busy) return;

        Busy = false;
        CanStart = true;
        if (job.Status == "completed")
        {
            _resultPath = job.ResultPath;
            OutputFolder = job.ResultPath ?? OutputFolder;
            CanOpenOutput = !string.IsNullOrEmpty(_resultPath);
            PhaseText = "已完成";
            SpeedText = $"总耗时 {PipelineRunner.FormatDuration(job.GpuSeconds)} · 成品 {PipelineRunner.FormatDuration(job.AudioSeconds)}";
            EtaText = "全部完成";
            FooterText = "已完成";
            if (Chapters.Count == 0) HintText = "转换完成。";
        }
        else
        {
            PhaseText = job.Status == "cancelled" ? "已取消（可续跑）" : "失败（可续跑）";
            HintText = job.ErrorMessage ?? "任务结束";
        }
    }

    // ——— 辅助 ———

    private void LoadDevices()
    {
        Devices.Clear();
        Devices.Add("自动选择（显存最大）");
        try
        {
            foreach (var g in DevicePlanner.ListGpus())
                Devices.Add($"[{g.Index}] {g.Name} · {g.MemTotalMb:N0} MB");
        }
        catch { }
        if (_deviceIndex >= Devices.Count) _deviceIndex = 0;
        OnPropertyChanged(nameof(DeviceIndex));
        RefreshPlan();
    }

    private void RefreshPlan()
    {
        try
        {
            var plan = DevicePlanner.Resolve(App.Handlers.Config);
            PlanText = plan.Display;
            PlanHint = plan.Warnings.Count > 0 ? plan.Warnings[0] : plan.Reason;
        }
        catch { }
    }

    private string _planText = "";
    public string PlanText { get => _planText; set => Set(ref _planText, value); }

    private string _planHint = "";
    public string PlanHint { get => _planHint; set => Set(ref _planHint, value); }

    private void UpdateSourceStatus()
    {
        try
        {
            var root = App.Handlers.Sources.Root;
            var installed = App.Handlers.Sources.List()
                .Select(o => Jx.S(o))
                .Count(s => s.Contains("\"installed\":true"));
            SourceStatus = $"模型源：{installed} 项就绪 · {root}";
        }
        catch { }
    }

    private static string DeviceKey(string label) => label switch
    {
        "纯 CPU" => "cpu",
        "纯 GPU" => "gpu",
        "GPU + CPU 混合" => "hybrid",
        _ => "auto",
    };

    private static string DeviceLabel(string key) => key switch
    {
        "cpu" => "纯 CPU",
        "gpu" => "纯 GPU",
        "hybrid" => "GPU + CPU 混合",
        _ => "自动（按显存决策）",
    };

    private void AddLocal(string level, string stage, string text) => Logs.Add(new LogLine
    {
        Time = DateTime.Now.ToString("HH:mm:ss"), Level = level, Stage = stage, Text = text,
    });

    private static string ParseTime(string ts) =>
        DateTimeOffset.TryParse(ts, out var d) ? d.ToLocalTime().ToString("HH:mm:ss") : DateTime.Now.ToString("HH:mm:ss");

    private static double GetDouble(JsonElement el, string name, double dflt) =>
        el.ValueKind == JsonValueKind.Object && el.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetDouble(out var d) ? d : dflt;

    private static string FormatEta(double seconds)
    {
        var t = TimeSpan.FromSeconds(seconds);
        return t.TotalHours >= 1
            ? $"{(int)t.TotalHours} 小时 {t.Minutes} 分"
            : t.TotalMinutes >= 1 ? $"{t.Minutes} 分 {t.Seconds} 秒" : $"{t.Seconds} 秒";
    }

    private bool Set<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value)) return false;
        field = value;
        OnPropertyChanged(name);
        return true;
    }

    private void OnPropertyChanged(string? name) => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
