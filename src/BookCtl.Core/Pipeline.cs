using System.Diagnostics;
using System.IO.Compression;
using System.Text;

namespace BookCtl.Core;

public sealed class EventBus
{
    private readonly JobStore _store;
    public event Action<JobEvent>? OnEvent;

    public EventBus(JobStore store) => _store = store;

    public JobEvent Emit(string jobId, string level, string stage, string code, string message, object? data = null)
    {
        var e = new JobEvent
        {
            JobId = jobId,
            Ts = DateTimeOffset.Now.ToString("o"),
            Level = level,
            Stage = stage,
            Code = code,
            Message = message,
            DataJson = data is null ? null : Jx.S(data),
        };
        e.Seq = _store.AppendEvent(e);
        OnEvent?.Invoke(e);
        return e;
    }

    public void Log(string message)
    {
        OnEvent?.Invoke(new JobEvent
        {
            Ts = DateTimeOffset.Now.ToString("o"),
            Level = "info",
            Stage = "SYSTEM",
            Code = "log",
            Message = message,
        });
    }
}

public sealed class PipelineRunner
{
    private readonly JobStore _store;
    private readonly EventBus _bus;
    private readonly BookCtlConfig _cfg;
    private readonly ITtsProvider _tts;
    private readonly GpuMonitor _gpu;
    private readonly AudioMaster _audio;
    /// <summary>_tts / _gpu 创建时对应的引擎名；只与它比较才能判断能否复用缓存实例。</summary>
    private readonly string _ttsEngine;

    public PipelineRunner(JobStore store, EventBus bus, BookCtlConfig cfg,
        ITtsProvider tts, GpuMonitor gpu, AudioMaster audio)
    {
        _store = store; _bus = bus; _cfg = cfg; _tts = tts; _gpu = gpu; _audio = audio;
        _ttsEngine = cfg.EngineMode;
    }

    public async Task RunAsync(JobRecord job, CancellationTokenSource cts)
    {
        var ct = cts.Token;
        var cfg = _cfg.CloneWith(job.Engine, job.Voice);
        var model = TtsProviderFactory.ModelOf(cfg);
        // 注意：_tts / _gpu 是构造时按当时的 EngineMode 建好的，必须与"构造时的引擎名"比较。
        // 若拿运行时可变的 _cfg.EngineMode 比，GUI 把 mock 改成 sherpa 后两者相等，
        // 会复用旧的 mock provider —— 清单记 sherpa、实际出占位音（已踩过）。
        var sameEngine = job.Engine is null
            || string.Equals(job.Engine, _ttsEngine, StringComparison.OrdinalIgnoreCase);
        var provider = sameEngine ? _tts : TtsProviderFactory.Create(cfg);
        var gpu = sameEngine ? _gpu : new GpuMonitor(cfg.EngineMode);
        var outDir = "";
        var synthClock = new Stopwatch();

        try
        {
            SetPhase(job, "S1_INGEST");
            Emit(job, "info", "stage_start", "正在解析 EPUB 结构");
            var book = EpubParser.Parse(job.InputPath);
            job.BookId = book.Meta.BookId;
            job.BookTitle = book.Meta.Title;
            outDir = Paths.ForBook(cfg, book.Meta.BookId, book.Meta.Title);
            Directory.CreateDirectory(outDir);
            Directory.CreateDirectory(Path.Combine(outDir, "audio", "chapters"));
            ExtractCover(job.InputPath, book.CoverHref, outDir);
            File.WriteAllText(Path.Combine(outDir, "book.json"), Jx.P(new
            {
                api_version = Api.Version,
                book = book.Meta,
                split_strategy = book.SplitStrategy,
                toc_entries = book.Toc.Count,
            }), new UTF8Encoding(false));
            Bump(job, 0.04, commit: true);

            SetPhase(job, "S2_SPLIT");
            Emit(job, "info", "stage_start", $"正在按目录切分章节（策略：{book.SplitStrategy}）");
            var chapters = ChapterSplitter.Split(book);
            // 断点续传：已有 done 章节的 checkpoint 优先，避免 resume 时被二次切分覆盖
            var prior = _store.GetChapters(job.JobId);
            var priorById = prior.ToDictionary(c => c.Id, c => c);
            foreach (var ch in chapters)
            {
                if (!priorById.TryGetValue(ch.Id, out var old)) continue;
                if (old.Status == "done" && !string.IsNullOrEmpty(old.AudioPath) && File.Exists(old.AudioPath))
                {
                    ch.Status = "done";
                    ch.AudioPath = old.AudioPath;
                    ch.Seconds = old.Seconds;
                    ch.Sha256 = old.Sha256;
                    ch.Cer = old.Cer;
                }
            }
            foreach (var ch in chapters.Where(c => !cfg.IncludeKinds.Contains(c.Kind) && c.Status != "done"))
            {
                ch.Status = "skipped";
                Emit(job, "info", "chapter_skipped", $"跳过 {ch.Kind} 章节「{ch.Title}」");
            }
            _store.InsertChapters(job.JobId, chapters);
            job.ChaptersTotal = chapters.Count(c => c.Status != "skipped");
            job.CharsTotal = chapters.Where(c => c.Status != "skipped").Sum(c => c.CharCount);
            // resume 时必须以章节 checkpoint 为准重算进度，否则会在上次的计数上继续累加（出现 6/3 这种越界）
            job.ChaptersDone = chapters.Count(c => c.Status == "done");
            job.CharsDone = chapters.Where(c => c.Status == "done").Sum(c => c.CharCount);
            job.AudioSeconds = chapters.Where(c => c.Status == "done").Sum(c => c.Seconds);
            _store.UpdateJob(job);
            File.WriteAllText(Path.Combine(outDir, "chapters.json"), Jx.P(new
            {
                api_version = Api.Version,
                split_strategy = book.SplitStrategy,
                chapters = chapters.Select(c => new
                {
                    id = c.Id, index = c.Index, title = c.Title, kind = c.Kind,
                    level = c.Level, href = c.Href, anchor = c.Anchor, chars = c.CharCount,
                }),
            }), new UTF8Encoding(false));
            Bump(job, 0.08, commit: true);
            Emit(job, "info", "S2_SPLIT", "split_done",
                $"共 {chapters.Count} 章，其中朗读 {job.ChaptersTotal} 章、约 {job.CharsTotal:N0} 字",
                new { chapters = chapters.Count, included = job.ChaptersTotal, chars = job.CharsTotal });

            SetPhase(job, "S3_NORMALIZE");
            Emit(job, "info", "stage_start", "正在做文本前端处理（正则 TN / 分句）");
            var segments = new List<Segment>();
            foreach (var ch in chapters.Where(c => c.Status != "skipped"))
            {
                var norm = TextNormalizer.Normalize(ch.TextRaw);
                var i = 0;
                foreach (var s in TextNormalizer.SplitSentences(norm))
                {
                    if (string.IsNullOrWhiteSpace(s)) continue; // 空段不送 TTS：既浪费算力又没有音频
                    i++;
                    segments.Add(new Segment
                    {
                        ChunkId = IdGen.NewChunkId(ch.Index, i),
                        ChapterId = ch.Id,
                        TextRaw = ch.TextRaw,
                        TextNorm = s,
                        Status = "pending",
                    });
                }
            }
            File.WriteAllText(Path.Combine(outDir, "segments.json"), Jx.P(new
            {
                api_version = Api.Version,
                segments = segments.Select(s => new { s.ChunkId, s.ChapterId, text_norm = s.TextNorm, chars = s.TextNorm.Length }),
            }), new UTF8Encoding(false));
            Bump(job, 0.15, commit: true);
            Emit(job, "info", "S3_NORMALIZE", "normalize_done",
                $"前端处理完成：{segments.Count:N0} 个朗读段（20–90 字语义分句）",
                new { segments = segments.Count });

            SetPhase(job, "S4_SYNTH");
            var plan = DevicePlanner.Resolve(cfg);
            var profile = SynthesisProfile.From(cfg, model, plan);
            job.ProfileHash = profile.Hash;
            job.Engine = cfg.EngineMode;
            job.Voice = cfg.Voice;
            _store.UpdateJob(job);
            var g0 = gpu.Query();
            var devMsg = plan.Device switch
            {
                "gpu" => $"使用 GPU {plan.GpuIndex}（{plan.GpuName}）开始本地合成",
                "hybrid" => $"使用 GPU {plan.GpuIndex} + CPU 层卸载开始合成（显存不足，速度会慢）",
                _ => plan.RequiredVramMb == 0
                    ? $"使用 CPU 合成（{plan.CpuThreads} 线程，引擎不消耗 GPU）"
                    : $"使用 CPU 合成（{plan.CpuThreads} 线程）",
            };
            Emit(job, "info", "S4_SYNTH", "gpu_status", devMsg + (plan.Warnings.Count > 0 ? "｜" + plan.Warnings[0] : ""),
                new
                {
                    gpu = g0,
                    device_plan = plan.ToDto(),
                    engine = cfg.EngineMode,
                    model,
                    voice = cfg.Voice,
                    profile_hash = profile.Hash,
                });
            synthClock.Start();

            var segsByChapter = segments.GroupBy(s => s.ChapterId).ToDictionary(g => g.Key, g => g.ToList());
            foreach (var ch in chapters)
            {
                ThrowIfCancelRequested(job);
                ct.ThrowIfCancellationRequested();

                if (ch.Status == "skipped") continue;
                if (ch.Status == "done" && !string.IsNullOrEmpty(ch.AudioPath) && File.Exists(ch.AudioPath))
                    continue; // 已计入上面的 checkpoint 重算，不能再 ++（否则 resume 后 6/3）

                ch.Status = "synthesizing";
                ch.Attempts++;
                _store.UpdateChapter(job.JobId, ch);
                Emit(job, "info", "S4_SYNTH", "chapter_start",
                    $"开始合成第 {ch.Index} 章「{ch.Title}」（{ch.CharCount:N0} 字）",
                    new { chapter_index = ch.Index, chapter_title = ch.Title, chars = ch.CharCount });

                var segs = segsByChapter.GetValueOrDefault(ch.Id) ?? new List<Segment>();
                var wavPaths = new List<string>();
                for (var si = 0; si < segs.Count; si++)
                {
                    ct.ThrowIfCancellationRequested();
                    var seg = segs[si];
                    var wavPath = Path.Combine(outDir, "segments", seg.ChunkId + ".wav");
                    if (File.Exists(wavPath))
                    {
                        seg.Seconds = Wav.DurationOf(wavPath); // 段级内容寻址缓存命中
                    }
                    else
                    {
                        var res = await provider.SynthesizeAsync(seg.ChunkId, seg.TextNorm, profile, wavPath, ct);
                        seg.Seconds = res.Seconds;
                        job.GpuSeconds += res.SynthSeconds;
                    }
                    wavPaths.Add(wavPath);
                    job.CharsDone += seg.TextNorm.Length;
                    job.AudioSeconds += seg.Seconds;
                    job.Progress = Math.Min(0.90, 0.15 + 0.75 * job.CharsDone / Math.Max(1, job.CharsTotal));
                    _store.UpdateJob(job);

                    var elapsed = synthClock.Elapsed.TotalSeconds;
                    var remainChars = Math.Max(0, job.CharsTotal - job.CharsDone);
                    Emit(job, "info", "S4_SYNTH", "segment_done",
                        $"第 {ch.Index} 章 {si + 1}/{segs.Count} 段已合成（{seg.TextNorm.Length} 字）",
                        new
                        {
                            chapter_index = ch.Index,
                            chapter_title = ch.Title,
                            segment = si + 1,
                            segments_total = segs.Count,
                            chars_done = job.CharsDone,
                            chars_total = job.CharsTotal,
                            audio_seconds = Math.Round(job.AudioSeconds, 1),
                            chars_per_sec = Math.Round(job.CharsDone / Math.Max(0.5, elapsed), 1),
                            audio_sec_per_sec = Math.Round(job.AudioSeconds / Math.Max(0.5, elapsed), 1),
                            rtf = Math.Round(job.AudioSeconds / Math.Max(0.05, job.GpuSeconds), 3),
                            gpu = gpu.Query(),
                            device = plan.Device,
                            device_display = plan.Display,
                            eta_seconds = Math.Round(remainChars / Math.Max(1, job.CharsDone / Math.Max(0.5, elapsed)), 0),
                            phase = plan.Device switch
                            {
                                "gpu" => $"正在调用显卡 {plan.GpuIndex}（{plan.GpuName}）进行 TTS 合成",
                                "hybrid" => $"正在用显卡 {plan.GpuIndex} + CPU 层卸载进行 TTS 合成",
                                _ => $"正在用 CPU（{plan.CpuThreads} 线程）进行 TTS 合成",
                            },
                        });
                }

                var chapterDir = Path.Combine(outDir, "audio", "chapters",
                    $"{ch.Index:D4}_{Paths.SanitizeName(ch.Title)}");
                Directory.CreateDirectory(chapterDir);
                var audioPath = _audio.MasterChapter(chapterDir, wavPaths, ch.Title);
                ch.AudioPath = audioPath;
                // MasterChapter 在有 ffmpeg 时返回 m4a，不能再用只认 RIFF 的 Wav.DurationOf
                ch.Seconds = _audio.ProbeDuration(audioPath);
                ch.Status = "done";
                ch.Sha256 = "sha256:" + HashUtil.ShaFile(audioPath);
                _store.UpdateChapter(job.JobId, ch);
                File.WriteAllText(Path.Combine(chapterDir, "chapter.json"), Jx.P(new
                {
                    chapter_id = ch.Id, index = ch.Index, title = ch.Title, kind = ch.Kind,
                    audio = Path.GetFileName(audioPath), seconds = Math.Round(ch.Seconds, 2),
                    sha256 = ch.Sha256, source = new { href = ch.Href, anchor = ch.Anchor },
                }), new UTF8Encoding(false));
                job.ChaptersDone++;
                _store.UpdateJob(job);
                Emit(job, "info", "S4_SYNTH", "chapter_done",
                    $"第 {ch.Index} 章完成：{FormatDuration(ch.Seconds)}，已保存 {Path.GetFileName(audioPath)}",
                    new { chapter_index = ch.Index, chapter_title = ch.Title, seconds = Math.Round(ch.Seconds, 1), audio = audioPath });
            }
            synthClock.Stop();

            SetPhase(job, "S5_VERIFY");
            if (cfg.Verify && cfg.EngineMode != "mock")
            {
                Emit(job, "info", "stage_start", "正在 ASR 回验（faster-whisper 适配点）");
                // 适配点：P2 接入 faster-whisper 回验；当前为占位
                Emit(job, "warn", "S5_VERIFY", "verify_placeholder", "ASR 回验为 P2 适配点，本版本跳过");
            }
            else
            {
                Emit(job, "info", "S5_VERIFY", "verify_skipped", $"ASR 回验未启用（engine={cfg.EngineMode}, verify={cfg.Verify}）");
            }
            Bump(job, 0.93, commit: true);

            SetPhase(job, "S6_MASTER");
            Emit(job, "info", "stage_start", "正在母带处理（章节拼接 / 响度 / M4B）");
            var finalChapters = _store.GetChapters(job.JobId);
            string? m4b = null;
            if (cfg.GenerateM4B)
            {
                m4b = _audio.MasterBook(finalChapters, outDir, book.Meta);
                Emit(job, m4b is null ? "warn" : "info", "S6_MASTER", "m4b",
                    m4b is null
                        ? (_audio.HasFfmpeg
                            ? "ffmpeg 合成整本 M4B 失败（分章音频不受影响，可用 book export 重试）"
                            : "未找到 ffmpeg，跳过整本 M4B（分章音频不受影响，属 degraded 模式）")
                        : $"整本有声书已生成：{Path.GetFileName(m4b)}",
                    new { path = m4b });
            }

            job.Status = "completed";
            job.Phase = "COMPLETED";
            job.Progress = 1.0;
            job.ResultPath = outDir;
            job.ErrorCode = null;      // 恢复后成功完成，不能还挂着上次的 WORKER_LOST
            job.ErrorMessage = null;
            _store.UpdateJob(job);

            // manifest.json 是交给用户"检验产物"的文档，必须反映终态；
            // 放到 job 状态落定之后再写，否则里面永远写着 running。
            var manifest = BuildManifest(job, book, finalChapters, outDir, m4b, profile, model, cfg);
            File.WriteAllText(Path.Combine(outDir, "manifest.json"), Jx.P(manifest), new UTF8Encoding(false));
            Directory.CreateDirectory(Path.Combine(outDir, "qc"));
            File.WriteAllText(Path.Combine(outDir, "qc", "report.json"), Jx.P(new
            {
                job_id = job.JobId,
                verify = "skipped",
                degraded = new { m4b_missing = m4b is null && cfg.GenerateM4B },
                note = cfg.EngineMode == "mock" ? "mock 引擎：音频为占位信号，时长为压缩模拟值（每字 10ms），仅用于链路与 UI 验收" : null,
            }), new UTF8Encoding(false));
            Emit(job, "info", "COMPLETED", "job_completed",
                $"整本书已完成，产物文件夹：{outDir}",
                new
                {
                    result_path = outDir,
                    book_m4b = m4b,
                    chapters = job.ChaptersDone,
                    audio_seconds = Math.Round(job.AudioSeconds, 1),
                    gpu_seconds = Math.Round(job.GpuSeconds, 1),
                });
        }
        catch (OperationCanceledException)
        {
            job.Status = "cancelled";
            job.ErrorCode = "CANCELLED";
            job.ErrorMessage = "用户请求取消";
            _store.UpdateJob(job);
            Emit(job, "info", "CANCELLED", "job_cancelled",
                $"任务已取消（章节级 checkpoint 保留，可 job resume 恢复）：已完成 {job.ChaptersDone}/{job.ChaptersTotal} 章");
        }
        catch (BookCtlException e)
        {
            job.Status = "failed";
            job.ErrorCode = e.Code;
            job.ErrorMessage = e.Message;
            _store.UpdateJob(job);
            Emit(job, "error", "FAILED", "job_failed", $"任务失败：{e.Message}",
                new { code = e.Code, fix = e.Fix, example = e.Example });
        }
        catch (Exception e)
        {
            job.Status = "failed";
            job.ErrorCode = "INTERNAL";
            job.ErrorMessage = e.Message;
            _store.UpdateJob(job);
            Emit(job, "error", "FAILED", "job_failed", $"内部错误：{e.Message}");
        }
        finally
        {
            _store.UpdateJob(job);
        }
    }

    private void ThrowIfCancelRequested(JobRecord job)
    {
        if (_store.IsCancelRequested(job.JobId))
            throw new OperationCanceledException("用户请求取消");
    }

    private void SetPhase(JobRecord job, string phase)
    {
        job.Phase = phase;
        _store.UpdateJob(job);
    }

    private void Bump(JobRecord job, double progress, bool commit)
    {
        if (progress > job.Progress) job.Progress = progress;
        if (commit) _store.UpdateJob(job);
    }

    private JobEvent Emit(JobRecord job, string level, string stage, string code, string message, object? data = null) =>
        _bus.Emit(job.JobId, level, stage, code, message, data);

    private JobEvent Emit(JobRecord job, string level, string code, string message, object? data = null) =>
        _bus.Emit(job.JobId, level, job.Phase ?? "RUN", code, message, data);

    private static object BuildManifest(JobRecord job, EpubBook book, List<Chapter> chapters,
        string outDir, string? m4b, SynthesisProfile profile, string model, BookCtlConfig cfg) => new
    {
        api_version = Api.Version,
        job_id = job.JobId,
        book = new
        {
            id = book.Meta.BookId,
            title = book.Meta.Title,
            creator = book.Meta.Creator,
            language = book.Meta.Language,
            epub_version = book.Meta.EpubVersion,
            input = new { path = book.Meta.InputPath, sha256 = book.Meta.InputSha256 },
        },
        engine = new { name = cfg.EngineMode, model, voice = cfg.Voice, version = "0.2.0" },
        synthesis_profile_hash = profile.Hash,
        config_hash = job.ConfigHash,
        status = job.Status,
        totals = new
        {
            chapters = chapters.Count(c => c.Status == "done"),
            audio_seconds = Math.Round(chapters.Where(c => c.Status == "done").Sum(c => c.Seconds), 1),
            gpu_seconds = Math.Round(job.GpuSeconds, 1),
        },
        audio = new { book_m4b = m4b is null ? null : Relative(outDir, m4b) },
        chapters = chapters
            .Where(c => c.Status != "skipped")
            .Select(c => new
            {
                id = c.Id,
                index = c.Index,
                title = c.Title,
                kind = c.Kind,
                status = c.Status,
                audio = c.AudioPath is null ? null : Relative(outDir, c.AudioPath),
                sha256 = c.Sha256,
                seconds = Math.Round(c.Seconds, 1),
                qc = new { cer = (double?)null, lufs = (double?)null },
                source = new { href = c.Href, anchor = c.Anchor },
            }),
        logs = "logs/job.ndjson",
    };

    private static string? Relative(string root, string path)
    {
        try
        {
            var rel = Path.GetRelativePath(root, path).Replace('\\', '/');
            return rel;
        }
        catch { return path; }
    }

    private static void ExtractCover(string epubPath, string? coverHref, string outDir)
    {
        if (string.IsNullOrWhiteSpace(coverHref)) return;
        try
        {
            using var zip = ZipFile.OpenRead(epubPath);
            var entry = zip.GetEntry(coverHref)
                ?? zip.Entries.FirstOrDefault(e => string.Equals(e.FullName, coverHref, StringComparison.OrdinalIgnoreCase));
            if (entry is null) return;
            var ext = Path.GetExtension(coverHref);
            var outPath = Path.Combine(outDir, string.IsNullOrEmpty(ext) ? "cover.jpg" : "cover" + ext);
            using var src = entry.Open();
            using var dst = File.Create(outPath);
            src.CopyTo(dst);
        }
        catch { }
    }

    public static string FormatDuration(double seconds)
    {
        var t = TimeSpan.FromSeconds(Math.Max(0, seconds));
        return t.TotalHours >= 1 ? $"{(int)t.TotalHours}小时{t.Minutes}分{t.Seconds}秒" : $"{t.Minutes}分{t.Seconds}秒";
    }
}

public sealed class WorkerHost
{
    private readonly JobStore _store;
    private readonly PipelineRunner _runner;
    private readonly EventBus _bus;
    private readonly CancellationTokenSource _cts = new();
    private readonly Dictionary<string, CancellationTokenSource> _active = new();
    private readonly object _gate = new();
    private Task? _loop;

    public bool IsRunning => _loop is not null && !_loop.IsCompleted;

    public WorkerHost(JobStore store, PipelineRunner runner, EventBus? bus = null)
    {
        _store = store;
        _runner = runner;
        _bus = bus ?? new EventBus(store);
    }

    public void Start()
    {
        if (IsRunning) return;
        if (!_recovered)
        {
            _recovered = true;
            RecoverOrphans();
            WriteLock();
        }
        _loop = Task.Run(LoopAsync);
    }

    private bool _recovered;

    /// <summary>
    /// Worker 是长任务执行者，被强杀/断电后 jobs.status 会永久停在 running。
    /// 启动时检查 worker.lock：若持锁进程已死，则把残留 running 任务标记为 WORKER_LOST（可 resume）。
    /// </summary>
    private void RecoverOrphans()
    {
        var dir = Path.GetDirectoryName(_store.DbPath);
        if (string.IsNullOrEmpty(dir)) return;
        var lockPath = Path.Combine(dir, "worker.lock");
        try
        {
            if (File.Exists(lockPath) && int.TryParse(File.ReadAllText(lockPath).Trim(), out var pid)
                && pid != Environment.ProcessId && IsAlive(pid))
                return; // 另一个 Worker 正持有执行权，不要抢
        }
        catch { }

        foreach (var id in _store.FindRunningJobIds())
        {
            if (_store.FailIfStillRunning(id, "WORKER_LOST",
                    "上次运行的 Worker 已异常退出，任务被标记为失败。已完成章节的音频保留，可直接 resume 续跑。"))
            {
                _bus.Emit(id, "warn", "recovered", "job_orphaned",
                    "检测到该任务所属 Worker 已退出，已释放执行权（可 bookctl job resume 续跑）");
            }
        }
    }

    private static bool IsAlive(int pid)
    {
        try { using var p = System.Diagnostics.Process.GetProcessById(pid); return !p.HasExited; }
        catch { return false; }
    }

    private void WriteLock()
    {
        var dir = Path.GetDirectoryName(_store.DbPath);
        if (string.IsNullOrEmpty(dir)) return;
        var lockPath = Path.Combine(dir, "worker.lock");
        try
        {
            File.WriteAllText(lockPath, Environment.ProcessId.ToString());
            AppDomain.CurrentDomain.ProcessExit += (_, _) =>
            {
                try
                {
                    if (File.Exists(lockPath) && File.ReadAllText(lockPath).Trim() == Environment.ProcessId.ToString())
                        File.Delete(lockPath);
                }
                catch { }
            };
        }
        catch { }
    }

    private async Task LoopAsync()
    {
        while (!_cts.IsCancellationRequested)
        {
            JobRecord? job = null;
            try { job = _store.TryClaimQueued(); }
            catch { }

            if (job is null)
            {
                try { await Task.Delay(600, _cts.Token); }
                catch (OperationCanceledException) { break; }
                continue;
            }

            var jcts = new CancellationTokenSource();
            lock (_gate) _active[job.JobId] = jcts;
            try { await _runner.RunAsync(job, jcts); }
            catch (Exception e) { Console.Error.WriteLine("[worker] " + e.Message); }
            finally { lock (_gate) _active.Remove(job.JobId); }
        }
    }

    public bool TryCancel(string jobId)
    {
        _store.RequestCancel(jobId);
        lock (_gate)
            if (_active.TryGetValue(jobId, out var cts))
            {
                cts.Cancel();
                return true;
            }
        return false;
    }

    public async Task StopAsync()
    {
        _cts.Cancel();
        lock (_gate)
            foreach (var c in _active.Values) c.Cancel();
        if (_loop is not null)
        {
            try { await _loop; } catch { }
        }
    }
}
