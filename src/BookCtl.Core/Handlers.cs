using System.Diagnostics;
using System.Text;
using System.Text.Json;

namespace BookCtl.Core;

public sealed class BookCtlHandlers
{
    public BookCtlConfig Config { get; }
    public JobStore Store { get; }
    public EventBus Bus { get; }
    public ITtsProvider Tts { get; }
    public GpuMonitor Gpu { get; }
    public AudioMaster Audio { get; }
    public WorkerHost Worker { get; }
    public SourceManager Sources { get; }

    public BookCtlHandlers(BookCtlConfig? cfg = null, string? configPath = null)
    {
        Config = cfg ?? BookCtlConfig.Load(configPath);
        Directory.CreateDirectory(Config.EffectiveWorkspaceRoot);
        Store = new JobStore(Config.DbPath);
        Bus = new EventBus(Store);
        Tts = TtsProviderFactory.Create(Config);
        Gpu = new GpuMonitor(Config.EngineMode);
        Audio = new AudioMaster(Config);
        Sources = new SourceManager(Config);
        Worker = new WorkerHost(Store, new PipelineRunner(Store, Bus, Config, Tts, Gpu, Audio), Bus);
    }

    public object SourceList() => new
    {
        root = Sources.Root,
        catalog_version = Sources.Catalog().Version,
        entries = Sources.List(),
        note = "bundled=true 已随软件内置；installable=true 可一键下载；manual 需按 engine/README 手动放置",
    };

    public Task<object> SourcePull(string id, Action<string, double, string>? progress = null, CancellationToken ct = default)
        => Sources.PullAsync(id, progress, ct);

    public object SourceRemove(string id) => Sources.Remove(id);

    public object Capabilities() => new
    {
        product = Api.Product,
        api_version = Api.Version,
        architecture_version = Api.ProtocolVersion,
        config_version = Config.ConfigVersion,
        workspace_root = Config.EffectiveWorkspaceRoot,
        engine_mode = Config.EngineMode,
        voice = Config.Voice,
        presets = new[] { "zh-default", "zh-female-warm", "zh-male-calm" },
        commands = new[]
        {
            "bookctl capabilities", "bookctl doctor", "bookctl book inspect --input <epub>",
            "bookctl book chapters --input <epub>", "bookctl book plan --input <epub>",
            "bookctl engine list", "bookctl engine test --text \"...\"",
            "bookctl job start --input <epub> [--idempotency-key k]", "bookctl job status <id>",
            "bookctl job list [--status running]", "bookctl job logs <id> [--since n]",
            "bookctl job cancel <id>", "bookctl job resume <id>", "bookctl job artifacts <id>",
            "bookctl book export <id> --format m4b", "bookctl run --input <epub>",
        },
        mcp = new
        {
            transports = new[] { "stdio" },
            protocol_versions = new[] { "2026-07-28", Api.McpProtocol, Api.McpProtocolLegacy },
            tools = new[] { "capabilities", "doctor", "plan", "book_inspect", "book_chapters",
                "job_start", "job_status", "job_logs", "job_cancel", "job_resume",
                "job_artifacts", "engine_list", "engine_test", "device_list", "source_list", "source_pull" },
        },
        device = new
        {
            modes = new[] { "auto", "gpu", "cpu", "hybrid" },
            current = Config.Device,
            gpu_index = Config.GpuIndex,
            cpu_threads = Config.CpuThreads,
            note = "auto=按显存自动决策；gpu=指定卡；cpu=纯 CPU；hybrid=GPU+CPU 层卸载（能跑但更慢）",
        },
        edition = Config.Edition,
        determinism = "同一 (输入 sha256, profile_hash, config_hash) 产出逐字节一致",
    };

    /// <summary>显卡/CPU 一览 + 当前引擎在该硬件上的决策（供 GUI 下拉框与 MCP 调用）。</summary>
    public object DeviceList()
    {
        var gpus = DevicePlanner.ListGpus();
        var plan = DevicePlanner.Resolve(Config);
        return new
        {
            gpus = gpus.Select(g => new { index = g.Index, name = g.Name, mem_total_mb = g.MemTotalMb, mem_free_mb = g.MemFreeMb, display = g.Display }).ToArray(),
            cpu = new { logical_cores = Environment.ProcessorCount, threads_default = Math.Max(1, Environment.ProcessorCount) },
            requested = plan.Requested,
            plan = plan.ToDto(),
            modes = new[] { "auto", "gpu", "cpu", "hybrid" },
            note = "GpuIndex 用 nvidia-smi 的序号；多卡机器可指定 0/1/2。hybrid 仅在显存不足时才有意义。",
        };
    }

    public object Doctor() => new
    {
        ok = true,
        workspace = new { path = Config.EffectiveWorkspaceRoot, writable = IsWritable(Config.EffectiveWorkspaceRoot) },
        database = new { path = Config.DbPath, exists = File.Exists(Config.DbPath) },
        engine = new
        {
            mode = Config.EngineMode,
            provider = Tts.Name,
            model = Tts.Model,
            sidecar_url = Config.SidecarUrl,
            sidecar_healthy = Config.EngineMode == "mock" ? (bool?)null : Tts.HealthAsync().GetAwaiter().GetResult(),
        },
        ffmpeg = new { found = Audio.HasFfmpeg, note = Audio.HasFfmpeg ? null : "未找到 ffmpeg：分章 WAV 仍可用，M4B 跳过" },
        gpu = Gpu.Query(),
        devices = DeviceList(),
        python = PythonVersion(),
        worker_running = Worker.IsRunning,
        hints = new[]
        {
            Audio.HasFfmpeg ? null : "安装 ffmpeg 并加入 PATH 可生成整本 M4B",
            Config.EngineMode == "mock" ? "当前为 mock 校样引擎：接真实显卡请设 BOOKCTL_ENGINE=sidecar 并启动 engine/tts_server.py" : null,
        }.Where(x => x is not null).ToArray(),
    };

    public object Inspect(string inputPath)
    {
        var book = EpubParser.Parse(RequireInput(inputPath));
        var chapters = ChapterSplitter.Split(book);
        var included = chapters.Count(c => Config.IncludeKinds.Contains(c.Kind));
        var chars = chapters.Where(c => Config.IncludeKinds.Contains(c.Kind)).Sum(c => c.CharCount);
        return new
        {
            input_path = book.Meta.InputPath,
            input_sha256 = book.Meta.InputSha256,
            book_id = book.Meta.BookId,
            title = book.Meta.Title,
            creator = book.Meta.Creator,
            language = book.Meta.Language,
            epub_version = book.Meta.EpubVersion,
            drm_protected = book.Meta.DrmProtected,
            drm_detail = book.Meta.DrmDetail,
            split_strategy = book.SplitStrategy,
            chapters_total = chapters.Count,
            chapters_to_read = included,
            chars_to_read = chars,
            estimated_audio_seconds = Math.Round(chars / (Config.CharsPerMinute / 60.0), 0),
            estimated_gpu_seconds = Math.Round(chars / (Config.CharsPerMinute / 60.0) / 6.0, 0),
            note = "inspect 为只读操作，不创建任务、不改动状态",
        };
    }

    public object Chapters(string inputPath)
    {
        var book = EpubParser.Parse(RequireInput(inputPath));
        var chapters = ChapterSplitter.Split(book);
        return new
        {
            book_id = book.Meta.BookId,
            title = book.Meta.Title,
            split_strategy = book.SplitStrategy,
            chapters = chapters.Select(c => new
            {
                c.Id, c.Index, c.Title, c.Kind, c.Level, c.Href, c.Anchor, c.CharCount,
                included = Config.IncludeKinds.Contains(c.Kind),
            }),
        };
    }

    public object Plan(string inputPath)
    {
        var book = EpubParser.Parse(RequireInput(inputPath));
        var chapters = ChapterSplitter.Split(book);
        var included = chapters.Where(c => Config.IncludeKinds.Contains(c.Kind)).ToList();
        var chars = included.Sum(c => c.CharCount);
        var outDir = Paths.ForBook(Config, book.Meta.BookId, book.Meta.Title);
        return new
        {
            dry_run = true,
            input_path = book.Meta.InputPath,
            output_dir = outDir,
            book_id = book.Meta.BookId,
            engine = new { mode = Config.EngineMode, model = Tts.Model, voice = Config.Voice },
            steps = new object[]
            {
                new { stage = "S1_INGEST", action = "校验 EPUB / 解析 OPF 与元数据", output = "book.json" },
                new { stage = "S2_SPLIT", action = $"按 {book.SplitStrategy} 切章", output = "chapters.json", count = chapters.Count },
                new { stage = "S3_NORMALIZE", action = "中文 TN / 发音词典 / 语义分句", output = "segments.json" },
                new { stage = "S4_SYNTH", action = "本地显卡逐段 TTS 合成（章节级 checkpoint）", output = "audio/chapters/*" },
                new { stage = "S5_VERIFY", action = Config.Verify ? "ASR 回验" : "跳过（verify=false）", output = "qc/report.json" },
                new { stage = "S6_MASTER", action = "两遍响度 / 章节拼接 / M4B 章节标记", output = "book.m4b + manifest.json" },
            },
            estimated = new
            {
                chapters_to_read = included.Count,
                chars = chars,
                audio_seconds = Math.Round(chars / (Config.CharsPerMinute / 60.0), 0),
                gpu_seconds = Math.Round(chars / (Config.CharsPerMinute / 60.0) / 6.0, 0),
            },
            ffmpeg_available = Audio.HasFfmpeg,
            repro_command = $"bookctl run --input \"{book.Meta.InputPath}\"",
        };
    }

    public object JobStart(string inputPath, string? idempotencyKey = null,
        string? engine = null, string? voice = null, string? device = null,
        int? gpuIndex = null, int? cpuThreads = null)
    {
        var full = RequireInput(inputPath);

        // 单机单任务并发：设备选择写在会话配置上，Worker 领取任务时读取（与 GUI 下拉框同一路径）
        if (!string.IsNullOrWhiteSpace(device)) Config.Device = device.Trim().ToLowerInvariant();
        if (gpuIndex.HasValue) Config.GpuIndex = gpuIndex.Value;
        if (cpuThreads.HasValue) Config.CpuThreads = cpuThreads.Value;
        var job = new JobRecord
        {
            JobId = IdGen.NewJobId(),
            IdempotencyKey = string.IsNullOrWhiteSpace(idempotencyKey)
                ? "auto-" + HashUtil.Sha(full + "|" + (engine ?? Config.EngineMode) + "|" + (voice ?? Config.Voice))[..16]
                : idempotencyKey,
            InputPath = full,
            Status = "queued",
            ConfigHash = Config.Hash(),
            Engine = engine ?? Config.EngineMode,
            Voice = voice ?? Config.Voice,
            CreatedAt = DateTime.UtcNow,
            UpdatedAt = DateTime.UtcNow,
        };
        var argsHash = HashUtil.Sha(Jx.S(new
        {
            input = job.InputPath,
            engine = job.Engine,
            voice = job.Voice,
            config_hash = job.ConfigHash,
        }));
        job = Store.InsertJob(job, argsHash);
        Bus.Emit(job.JobId, "info", "queued", "job_queued",
            $"任务已入队：{Path.GetFileName(job.InputPath)}（引擎 {job.Engine}）",
            new { job_id = job.JobId, engine = job.Engine, voice = job.Voice });
        Worker.Start();
        return new
        {
            job_id = job.JobId,
            status = job.Status,
            poll_interval_ms = 1000,
            repro_command = $"bookctl job status {job.JobId}",
            resources = new
            {
                logs = $"job:///{job.JobId}/log.ndjson",
                manifest = $"job:///{job.JobId}/manifest.json",
            },
        };
    }

    public object JobStatus(string jobId)
    {
        var job = RequireJob(jobId);
        return new
        {
            job_id = job.JobId,
            status = job.Status,
            phase = job.Phase,
            phase_text = JobStatuses.DescribePhase(job.Phase),
            progress = Math.Round(job.Progress, 4),
            chapters_done = job.ChaptersDone,
            chapters_total = job.ChaptersTotal,
            chars_done = job.CharsDone,
            chars_total = job.CharsTotal,
            audio_seconds = Math.Round(job.AudioSeconds, 1),
            gpu_seconds = Math.Round(job.GpuSeconds, 1),
            engine = job.Engine,
            voice = job.Voice,
            book_title = job.BookTitle,
            result_path = job.ResultPath,
            error = job.ErrorCode is null ? null : new { code = job.ErrorCode, message = job.ErrorMessage },
            updated_at = job.UpdatedAt,
            revision = job.Revision,
        };
    }

    public object JobList(string? status = null, int limit = 50) => new
    {
        jobs = Store.ListJobs(status, Math.Clamp(limit, 1, 500)).Select(j => new
        {
            j.JobId, j.Status, j.Phase, j.Progress, j.ChaptersDone, j.ChaptersTotal,
            j.BookTitle, j.Engine, j.ResultPath, j.CreatedAt, j.UpdatedAt,
        }),
    };

    public object JobLogs(string jobId, long sinceSeq = 0, int limit = 200)
    {
        RequireJob(jobId);
        var events = Store.ReadEvents(jobId, sinceSeq, Math.Clamp(limit, 1, 2000));
        return new
        {
            job_id = jobId,
            events = events.Select(EventDto),
            next_cursor = events.Count > 0 ? events[^1].Seq : sinceSeq,
            has_more = events.Count == Math.Clamp(limit, 1, 2000),
        };
    }

    public object JobCancel(string jobId, string? reason = null)
    {
        var job = RequireJob(jobId);
        if (job.IsTerminal)
            throw new BookCtlException("INVALID_STATE_TRANSITION", $"任务已处于终态 {job.Status}，无法取消",
                fix: "如需重跑请提交新任务", example: $"bookctl job start --input \"{job.InputPath}\"");
        var active = Worker.TryCancel(jobId);
        Bus.Emit(jobId, "info", "CANCELLED", "cancel_requested",
            active ? "已发出取消指令，将在当前章节边界生效" : "取消请求已记录，等待 worker 在章节边界检查",
            new { reason, active });
        return new { job_id = jobId, cancel_requested = true, worker_active = active };
    }

    public object JobResume(string jobId)
    {
        var job = RequireJob(jobId);
        if (job.Status is not ("failed" or "cancelled"))
            throw new BookCtlException("INVALID_STATE_TRANSITION", $"仅 failed / cancelled 任务可恢复（当前 {job.Status}）",
                fix: "确认状态后重试", example: $"bookctl job status {jobId}");
        Store.ResetForResume(jobId);
        Worker.Start();
        Bus.Emit(jobId, "info", "resumed", "job_resumed", "任务已恢复到 queued，已完成章节将从 checkpoint 跳过");
        return new { job_id = jobId, status = "queued", note = "章节级 checkpoint 保留，只重跑未完成部分" };
    }

    public object JobArtifacts(string jobId)
    {
        var job = RequireJob(jobId);
        if (job.ResultPath is null || !Directory.Exists(job.ResultPath))
            return new { job_id = jobId, status = job.Status, artifacts = Array.Empty<object>(),
                note = "任务尚未产出（" + job.Status + "），完成后将生成项目文档文件夹" };

        var list = new List<object>();
        var manifestPath = Path.Combine(job.ResultPath, "manifest.json");
        if (File.Exists(manifestPath))
        {
            using var doc = JsonDocument.Parse(File.ReadAllText(manifestPath, Encoding.UTF8));
            var root = doc.RootElement;
            if (root.TryGetProperty("chapters", out var chapters))
                foreach (var ch in chapters.EnumerateArray())
                {
                    var audio = ch.TryGetProperty("audio", out var a) && a.ValueKind == JsonValueKind.String ? a.GetString() : null;
                    if (audio is null) continue;
                    list.Add(new
                    {
                        kind = "chapter_audio",
                        title = ch.TryGetProperty("title", out var t) ? t.GetString() : "",
                        path = Path.Combine(job.ResultPath, audio),
                        sha256 = ch.TryGetProperty("sha256", out var s) ? s.GetString() : null,
                        seconds = ch.TryGetProperty("seconds", out var d) ? d.GetDouble() : 0,
                    });
                }
            if (root.TryGetProperty("audio", out var au) &&
                au.TryGetProperty("book_m4b", out var m4b) && m4b.ValueKind == JsonValueKind.String)
                list.Add(new { kind = "book_m4b", title = "整本有声书", path = Path.Combine(job.ResultPath, m4b.GetString()!), sha256 = (string?)null, seconds = 0.0 });
        }
        return new
        {
            job_id = jobId,
            result_path = job.ResultPath,
            manifest_uri = $"job:///{jobId}/manifest.json",
            artifacts = list,
        };
    }

    public object EngineList() => new { current = Config.EngineMode, engines = TtsProviderFactory.Describe(Config) };

    public object EngineTest(string text = "这是本地朗读引擎的试听样例。")
    {
        var dir = Path.Combine(Config.EffectiveWorkspaceRoot, "engine-test");
        Directory.CreateDirectory(dir);
        var path = Path.Combine(dir, "sample-" + DateTime.UtcNow.Ticks + ".wav");
        var profile = SynthesisProfile.From(Config, Tts.Model);
        var res = Tts.SynthesizeAsync("test-0001", text, profile, path).GetAwaiter().GetResult();
        return new
        {
            engine = Tts.Name, model = Tts.Model, voice = Config.Voice,
            wav_path = res.WavPath, seconds = Math.Round(res.Seconds, 2),
            synth_seconds = Math.Round(res.SynthSeconds, 3), gpu = Gpu.Query(),
        };
    }

    public object Export(string jobId, string format = "m4b", string? outDir = null)
    {
        var job = RequireJob(jobId);
        if (job.ResultPath is null) throw new BookCtlException("NOT_FOUND", "任务尚未产出", field: "job_id");
        var dest = outDir ?? Path.Combine(Config.EffectiveWorkspaceRoot, "exports", jobId);
        Directory.CreateDirectory(dest);
        var copied = new List<string>();
        if (format == "m4b")
        {
            var src = Path.Combine(job.ResultPath, "book.m4b");
            if (!File.Exists(src)) throw new BookCtlException("NOT_FOUND", "book.m4b 不存在（可能缺少 ffmpeg）",
                fix: "安装 ffmpeg 后重新 resume 任务，或改用 --format wav");
            var dst = Path.Combine(dest, "book.m4b");
            File.Copy(src, dst, true);
            copied.Add(dst);
        }
        else
        {
            foreach (var f in Directory.GetFiles(Path.Combine(job.ResultPath, "audio", "chapters"), "audio.*", SearchOption.AllDirectories))
            {
                var dst = Path.Combine(dest, Path.GetFileName(Path.GetDirectoryName(f)) + Path.GetExtension(f));
                File.Copy(f, dst, true);
                copied.Add(dst);
            }
        }
        return new { job_id = jobId, format, output_dir = dest, files = copied };
    }

    public async Task<object> RunAndWaitAsync(string inputPath, Action<JobEvent>? onEvent,
        CancellationToken ct = default, string? idempotencyKey = null, string? engine = null,
        string? voice = null, string? device = null, int? gpuIndex = null, int? cpuThreads = null)
    {
        var started = JobStart(inputPath, idempotencyKey, engine, voice, device, gpuIndex, cpuThreads);
        string jobId;
        using (var doc = JsonDocument.Parse(Jx.S(started)))
            jobId = doc.RootElement.GetProperty("job_id").GetString()!;

        // 幂等命中一个 failed / cancelled 的旧任务时，run 的语义是「把这本跑完」，应自动恢复而不是立刻返回
        var existing = Store.GetJob(jobId);
        if (existing is { Status: "failed" or "cancelled" }) JobResume(jobId);

        Worker.Start();
        long cursor = 0;
        while (true)
        {
            ct.ThrowIfCancellationRequested();
            var evs = Store.ReadEvents(jobId, cursor, 500);
            foreach (var e in evs) { cursor = e.Seq; onEvent?.Invoke(e); }
            var job = Store.GetJob(jobId);
            if (job is null || job.IsTerminal) return JobStatus(jobId);
            await Task.Delay(400, ct);
        }
    }

    private JobRecord RequireJob(string jobId)
    {
        if (string.IsNullOrWhiteSpace(jobId))
            throw new BookCtlException("ARGS_MISSING", "缺少 job_id", field: "job_id",
                example: "bookctl job list");
        return Store.GetJob(jobId)
            ?? throw new BookCtlException("NOT_FOUND", $"任务不存在：{jobId}", field: "job_id", value: jobId,
                fix: "先用 job list 查看有效任务", example: "bookctl job list");
    }

    private static string RequireInput(string? inputPath)
    {
        if (string.IsNullOrWhiteSpace(inputPath))
            throw new BookCtlException("ARGS_MISSING", "缺少 --input <epub 路径>", field: "input",
                fix: "指定一本 .epub 的绝对路径（中文路径可直接用）",
                example: "bookctl run --input \"C:\\books\\示例.epub\"");
        var full = Path.GetFullPath(inputPath);
        if (!File.Exists(full))
            throw new BookCtlException("FILE_NOT_FOUND", $"找不到文件：{full}", field: "input", value: full,
                fix: "检查路径与扩展名");
        if (!full.EndsWith(".epub", StringComparison.OrdinalIgnoreCase))
            throw new BookCtlException("ARGS_INVALID", "当前版本仅支持 .epub 输入", field: "input", value: full);
        return full;
    }

    public static object EventDto(JobEvent e) => new
    {
        seq = e.Seq,
        ts = e.Ts,
        level = e.Level,
        stage = e.Stage,
        code = e.Code,
        message = e.Message,
        data = e.DataJson is null ? (object?)null : JsonSerializer.Deserialize<JsonElement>(e.DataJson, Jx.Opt),
    };

    private static bool IsWritable(string dir)
    {
        try
        {
            var p = Path.Combine(dir, ".write-test-" + Guid.NewGuid().ToString("N")[..8]);
            File.WriteAllText(p, "ok");
            File.Delete(p);
            return true;
        }
        catch { return false; }
    }

    private static string? PythonVersion()
    {
        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = "python", Arguments = "--version",
                RedirectStandardOutput = true, RedirectStandardError = true,
                UseShellExecute = false, CreateNoWindow = true,
            };
            using var p = Process.Start(psi);
            if (p is null) return null;
            var s = (p.StandardOutput.ReadLine() ?? p.StandardError.ReadLine())?.Trim();
            p.WaitForExit(3000);
            return s;
        }
        catch { return null; }
    }
}
