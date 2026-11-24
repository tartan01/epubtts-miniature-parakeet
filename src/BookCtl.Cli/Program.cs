using BookCtl.Core;

namespace BookCtl.Cli;

public static class Program
{
    private static bool _pretty;
    private static bool _quiet;

    public static int Main(string[] args)
    {
        // 统一 stdout/stderr 为无 BOM UTF-8：事件流 JSONL、最终 envelope、纯文本日志同编码。
        // Windows 控制台默认 OEM(936)，若只在 Emit() 里临时切 UTF-8，先输出的事件流会用 GBK，
        // 导致大模型 / CI 读取 JSONL 时中文（书名、章节标题、状态文案）在不同终端下互相乱码。
        var utf8 = new System.Text.UTF8Encoding(false);
        Console.OutputEncoding = utf8;
        Console.InputEncoding = utf8;
        Console.SetError(new System.IO.StreamWriter(Console.OpenStandardError(), utf8) { AutoFlush = true });

        var rest = new List<string>(args);
        _pretty = TakeFlag(ref rest, "--pretty");
        _quiet = TakeFlag(ref rest, "--quiet");

        try
        {
            if (rest.Count == 0 || rest[0] is "help" or "--help" or "-h")
            {
                Help();
                return rest.Count == 0 ? Api.ExitArgs : 0;
            }

            var cmd = rest[0].ToLowerInvariant();
            var sub = new List<string>(rest.Skip(1));

            switch (cmd)
            {
                case "version":
                    return Emit(new { product = Api.Product, version = "0.2.0", api_version = Api.Version, architecture = Api.ProtocolVersion });
                case "capabilities":
                    return Emit(H().Capabilities());
                case "doctor":
                    return Emit(H().Doctor());
                case "engine":
                {
                    var verb = sub.Count > 0 ? sub[0].ToLowerInvariant() : "list";
                    var engineRest = new List<string>(sub.Skip(1));
                    return verb switch
                    {
                        "list" => Emit(H().EngineList()),
                        "test" => Emit(H().EngineTest(Flag(engineRest, "--text") ?? "这是本地朗读引擎的试听样例。")),
                        _ => BadSub("engine", verb, "list", "test"),
                    };
                }
                case "book":
                {
                    var verb = sub.Count > 0 ? sub[0].ToLowerInvariant() : "";
                    var bookRest = new List<string>(sub.Skip(1));
                    return verb switch
                    {
                        "inspect" => Emit(H().Inspect(NeedInput(bookRest))),
                        "chapters" => Emit(H().Chapters(NeedInput(bookRest))),
                        "plan" => Emit(H().Plan(NeedInput(bookRest))),
                        "export" => Emit(H().Export(NeedPos(bookRest, 0, "job_id"),
                            Flag(bookRest, "--format") ?? "m4b", Flag(bookRest, "--out"))),
                        _ => BadSub("book", verb, "inspect", "chapters", "plan", "export"),
                    };
                }
                case "device":
                {
                    var verb = sub.Count > 0 ? sub[0].ToLowerInvariant() : "list";
                    return verb switch
                    {
                        "list" => Emit(H().DeviceList()),
                        _ => BadSub("device", verb, "list"),
                    };
                }
                case "source":
                {
                    var verb = sub.Count > 0 ? sub[0].ToLowerInvariant() : "list";
                    var sRest = new List<string>(sub.Skip(1));
                    return verb switch
                    {
                        "list" => Emit(H().SourceList()),
                        "pull" => Emit(PullSource(H(), NeedPos(sRest, 0, "source_id"))),
                        "remove" or "rm" => Emit(H().SourceRemove(NeedPos(sRest, 0, "source_id"))),
                        _ => BadSub("source", verb, "list", "pull", "remove"),
                    };
                }
                case "job":
                    return Job(sub);
                case "run":
                    return Run(sub);
                default:
                    throw new BookCtlException("ARGS_INVALID", $"未知命令：{cmd}", field: "command", value: cmd,
                        fix: "运行 bookctl 查看全部命令", suggestions: new[] { "bookctl capabilities" });
            }
        }
        catch (BookCtlException e)
        {
            return EmitError(e);
        }
        catch (Exception e)
        {
            return EmitError(new BookCtlException("INTERNAL", e.Message, retryable: false));
        }
    }

    private static int Job(List<string> sub)
    {
        var h = H();
        var verb = sub.Count > 0 ? sub[0].ToLowerInvariant() : "";
        var rest = new List<string>(sub.Skip(1));
        switch (verb)
        {
            case "start":
                return Emit(h.JobStart(NeedInput(rest),
                    Flag(rest, "--idempotency-key"), Flag(rest, "--engine"), Flag(rest, "--voice"),
                    Flag(rest, "--device"), IntFlag(rest, "--gpu"), IntFlag(rest, "--cpu-threads")));
            case "status":
                return Emit(h.JobStatus(NeedPos(rest, 0, "job_id")));
            case "list":
                return Emit(h.JobList(Flag(rest, "--status"), int.Parse(Flag(rest, "--limit") ?? "50")));
            case "cancel":
                return Emit(h.JobCancel(NeedPos(rest, 0, "job_id"), Flag(rest, "--reason")));
            case "resume":
                return Emit(h.JobResume(NeedPos(rest, 0, "job_id")));
            case "artifacts":
                return Emit(h.JobArtifacts(NeedPos(rest, 0, "job_id")));
            case "logs":
                return Logs(h, rest);
            default:
                return BadSub("job", verb, "start", "status", "list", "cancel", "resume", "artifacts", "logs");
        }
    }

    private static int Logs(BookCtlHandlers h, List<string> rest)
    {
        var jobId = NeedPos(rest, 0, "job_id");
        var since = long.Parse(Flag(rest, "--since") ?? "0");
        var limit = int.Parse(Flag(rest, "--limit") ?? "200");
        var follow = TakeFlag(ref rest, "--follow");

        if (!follow) return Emit(h.JobLogs(jobId, since, limit));

        var cursor = since;
        while (true)
        {
            foreach (var e in h.Store.ReadEvents(jobId, cursor, 200))
            {
                cursor = e.Seq;
                Console.WriteLine(Jx.S(BookCtlHandlers.EventDto(e)));
            }
            var job = h.Store.GetJob(jobId);
            if (job is null || job.IsTerminal) return 0;
            Thread.Sleep(500);
        }
    }

    private static int Run(List<string> rest)
    {
        var input = NeedInput(rest);
        var h = H();
        var result = h.RunAndWaitAsync(input, e =>
        {
            if (!_quiet) Console.Error.WriteLine($"[{e.Stage}] {e.Message}");
            Console.Out.Flush();
            Console.WriteLine(Jx.S(BookCtlHandlers.EventDto(e)));
        }, engine: Flag(rest, "--engine"), voice: Flag(rest, "--voice"),
            device: Flag(rest, "--device"), gpuIndex: IntFlag(rest, "--gpu"),
            cpuThreads: IntFlag(rest, "--cpu-threads")).GetAwaiter().GetResult();
        return Emit(result);
    }

    private static object PullSource(BookCtlHandlers h, string id)
    {
        var last = -1;
        var result = h.SourcePull(id, (phase, frac, label) =>
        {
            if (_quiet) return;
            var pct = (int)Math.Round(frac * 100);
            if (pct == last) return;
            last = pct;
            Console.Error.Write($"\r[{phase}] {pct,3}%  {label,-40}");
        }).GetAwaiter().GetResult();
        if (!_quiet) Console.Error.WriteLine();
        return result;
    }

    private static BookCtlHandlers H() => new();

    private static int Emit(object payload)
    {
        Console.WriteLine(_pretty ? Jx.P(Jx.Ok(payload)) : Jx.S(Jx.Ok(payload)));
        return Api.ExitOk;
    }

    private static int EmitError(BookCtlException e)
    {
        Console.Error.WriteLine(_pretty ? Jx.P(Jx.Fail(e)) : Jx.S(Jx.Fail(e)));
        return e.ExitCode;
    }

    private static int BadSub(string cmd, string verb, params string[] verbs) =>
        throw new BookCtlException("ARGS_INVALID", $"未知子命令：{cmd} {verb}", field: "subcommand", value: verb,
            fix: $"可选：{string.Join(" / ", verbs.Select(v => cmd + " " + v))}");

    private static string NeedInput(List<string> rest) => Flag(rest, "--input") ?? throw new BookCtlException(
        "ARGS_MISSING", "缺少 --input <epub 路径>", field: "input",
        fix: "指定一本 .epub 的绝对路径", example: "bookctl run --input \"C:\\books\\示例.epub\"");

    private static string NeedPos(List<string> rest, int i, string name) =>
        i < rest.Count && !rest[i].StartsWith("--") ? rest[i]
            : throw new BookCtlException("ARGS_MISSING", $"缺少位置参数 {name}", field: name,
                example: $"bookctl job status <{name}>");

    private static string? Flag(List<string> rest, string name)
    {
        for (var i = 0; i < rest.Count - 1; i++)
            if (rest[i].Equals(name, StringComparison.Ordinal))
                return rest[i + 1];
        return null;
    }

    private static int? IntFlag(List<string> rest, string name) =>
        int.TryParse(Flag(rest, name), out var v) ? v : null;

    private static bool TakeFlag(ref List<string> rest, string name)
    {
        var taken = rest.Contains(name);
        rest.RemoveAll(x => x.Equals(name, StringComparison.Ordinal));
        return taken;
    }

    private static void Help()
    {
        Console.Error.WriteLine("""
        bookctl — EPUB 本地朗读自动化框架 v0.2（CLI 控制面）

        用法: bookctl <command> [sub] [flags]     默认输出 JSON envelope，--pretty 为缩进

          bookctl capabilities                     能力清单（大模型可读）
          bookctl doctor                           环境自检（GPU/ffmpeg/sidecar/Python）
          bookctl run --input <epub>               一键：入队 + 内嵌 worker + 实时 JSONL 事件
          bookctl book inspect --input <epub>      只读探测（章节/字数/时长估算）
          bookctl book chapters --input <epub>     章节树（含 kind 分类，可人工复核）
          bookctl book plan --input <epub>         dry-run 计划（步骤/产物/耗时估算）
          bookctl engine list | engine test        引擎清单 / 单句试听
          bookctl device list                      显卡/CPU 一览 + 当前引擎的设备决策
          bookctl source list                      引擎与模型源清单（内置 / 可下载 / 已安装）
          bookctl source pull <id>                 一键下载并校验解包到本地 sources/
          bookctl source remove <id>               删除已下载的源
          bookctl job start --input <epub>         入队（幂等，秒级返回 job_id）
          bookctl job status <id>                  状态/进度/速度
          bookctl job list [--status running]      任务列表
          bookctl job logs <id> [--follow]         NDJSON 事件流（cursor 单调递增）
          bookctl job cancel <id> / resume <id>    协作式取消 / 断点续传
          bookctl job artifacts <id>               产物清单（sha256/时长）
          bookctl book export <id> --format m4b    导出

        设备参数（run / job start 均支持）:
          --device auto|gpu|cpu|hybrid             auto=按显存自动决策；hybrid=GPU+CPU 层卸载
          --gpu 0|1|2                              指定显卡序号（nvidia-smi 顺序）
          --cpu-threads N                          CPU 推理线程数

        示例: bookctl run --input "C:\books\示例.epub" --engine sherpa --device cpu
              bookctl run --input "C:\books\示例.epub" --engine sidecar --device gpu --gpu 0

        环境: BOOKCTL_WORKSPACE / BOOKCTL_ENGINE(mock|sherpa|sidecar) / BOOKCTL_SIDECAR_URL /
              BOOKCTL_VOICE / BOOKCTL_DEVICE / BOOKCTL_GPU / BOOKCTL_CPU_THREADS / BOOKCTL_SOURCES
        退出码: 0 ok · 2 参数 · 3 未找到 · 5 冲突 · 69 可重试 · 130 已取消
        """);
    }
}
