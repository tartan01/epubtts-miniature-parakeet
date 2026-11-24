using System.Text;
using System.Text.Json;
using BookCtl.Core;

namespace BookCtl.Mcp;

public static class Program
{
    private sealed class RpcException : Exception
    {
        public int Code { get; }
        public RpcException(int code, string message) : base(message) => Code = code;
    }

    private static BookCtlHandlers _handlers = null!;

    public static async Task<int> Main()
    {
        Console.OutputEncoding = Encoding.UTF8;
        _handlers = new BookCtlHandlers();
        _handlers.Worker.Start();
        Console.Error.WriteLine("[bookctl-mcp] stdio server ready, workspace: " + _handlers.Config.EffectiveWorkspaceRoot);

        using var so = Console.OpenStandardOutput();
        using var stdin = Console.OpenStandardInput();
        using var reader = new StreamReader(stdin, Encoding.UTF8);

        string? line;
        while ((line = await reader.ReadLineAsync()) is not null)
        {
            if (string.IsNullOrWhiteSpace(line)) continue;
            JsonDocument doc;
            try { doc = JsonDocument.Parse(line); }
            catch { continue; }

            var root = doc.RootElement;
            if (!root.TryGetProperty("method", out var m) || m.ValueKind != JsonValueKind.String) continue;
            var method = m.GetString()!;
            var hasId = root.TryGetProperty("id", out var idEl);
            if (method.StartsWith("notifications/", StringComparison.Ordinal)) continue;
            if (!hasId) continue;

            object? id = idEl.ValueKind == JsonValueKind.String ? idEl.GetString() : idEl.GetInt64();
            try
            {
                var result = Handle(method, root);
                if (result is not null)
                    Write(so, new { jsonrpc = "2.0", id, result });
            }
            catch (RpcException e)
            {
                Write(so, new { jsonrpc = "2.0", id, error = new { code = e.Code, message = e.Message } });
            }
            catch (BookCtlException e)
            {
                Write(so, new { jsonrpc = "2.0", id, error = new { code = -32000, message = e.Code + ": " + e.Message, data = new { fix = e.Fix, example = e.Example } } });
            }
            catch (Exception e)
            {
                Write(so, new { jsonrpc = "2.0", id, error = new { code = -32603, message = e.Message } });
            }
        }
        return 0;
    }

    private static void Write(Stream so, object payload)
    {
        var bytes = Encoding.UTF8.GetBytes(Jx.S(payload) + "\n");
        so.Write(bytes);
        so.Flush();
    }

    private static object? Handle(string method, JsonElement root)
    {
        switch (method)
        {
            case "initialize":
            {
                var requested = "2025-11-25";
                if (root.TryGetProperty("params", out var ps) &&
                    ps.TryGetProperty("protocolVersion", out var pv) && pv.ValueKind == JsonValueKind.String)
                    requested = pv.GetString()!;
                return new
                {
                    protocolVersion = requested,
                    capabilities = new { tools = new { listChanged = false }, resources = new { subscribe = false }, prompts = new { listChanged = false } },
                    serverInfo = new { name = "bookctl", version = "0.2.0", architecture = Api.ProtocolVersion },
                    instructions = "EPUB → 本地 GPU 分章节有声书。先 capabilities / plan，再 job_start 提交（秒级返回），轮询 job_status、job_logs 跟踪，完成后 job_artifacts 获取产物。",
                };
            }
            case "server/discover":
                return new
                {
                    protocolVersion = "2026-07-28",
                    serverInfo = new { name = "bookctl", version = "0.2.0" },
                    capabilities = new { tools = new { listChanged = false }, resources = new { subscribe = false } },
                };
            case "ping":
                return new { };
            case "tools/list":
                return new { tools = Json(TopToolsJson()) };
            case "prompts/list":
                return new
                {
                    prompts = new object[]
                    {
                        new { name = "operate_playbook", description = "bookctl 标准操作手册：探测→计划→提交→跟踪→取产物" },
                    },
                };
            case "prompts/get":
            {
                var name = S(root, "params", "name") ?? "";
                if (name != "operate_playbook") throw new RpcException(-32602, "未知 prompt: " + name);
                return new
                {
                    description = "bookctl 标准操作手册",
                    messages = new object[]
                    {
                        new { role = "user", content = new { type = "text", text = PlaybookText() } },
                    },
                };
            }
            case "resources/list":
            {
                var list = new List<object>();
                foreach (var j in _handlers.Store.ListJobs(null, 10))
                {
                    if (j.ResultPath is null) continue;
                    list.Add(new { uri = $"job:///{j.JobId}/manifest.json", name = $"manifest of {j.BookTitle ?? j.JobId}", mimeType = "application/json" });
                    list.Add(new { uri = $"job:///{j.JobId}/log.ndjson", name = $"events of {j.JobId}", mimeType = "application/x-ndjson" });
                }
                return new { resources = list };
            }
            case "resources/read":
            {
                var uri = S(root, "params", "uri") ?? throw new RpcException(-32602, "缺少 uri");
                var (jobId, rel) = ParseJobUri(uri);
                var job = _handlers.Store.GetJob(jobId) ?? throw new RpcException(-32602, "未知 job: " + jobId);
                if (job.ResultPath is null) throw new RpcException(-32602, "任务尚未产出");
                var allowed = new[] { "manifest.json", "chapters.json", "book.json", "log.ndjson" };
                if (!allowed.Contains(rel)) throw new RpcException(-32602, "仅允许读取: " + string.Join(", ", allowed));
                var path = Path.GetFullPath(Path.Combine(job.ResultPath, rel));
                if (!path.StartsWith(_handlers.Config.EffectiveWorkspaceRoot, StringComparison.OrdinalIgnoreCase))
                    throw new RpcException(-32602, "路径越界");
                if (rel == "log.ndjson")
                {
                    var sb = new StringBuilder();
                    foreach (var e in _handlers.Store.ReadEvents(jobId, 0, 5000))
                        sb.AppendLine(Jx.S(BookCtlHandlers.EventDto(e)));
                    return new { contents = new object[] { new { uri, mimeType = "application/x-ndjson", text = sb.ToString() } } };
                }
                if (!File.Exists(path)) throw new RpcException(-32602, "文件不存在: " + rel);
                return new { contents = new object[] { new { uri, mimeType = "application/json", text = File.ReadAllText(path, Encoding.UTF8) } } };
            }
            case "tools/call":
            {
                var name = S(root, "params", "name") ?? "";
                JsonElement args = default;
                if (root.TryGetProperty("params", out var p2) && p2.TryGetProperty("arguments", out var a) && a.ValueKind == JsonValueKind.Object)
                    args = a;
                try
                {
                    var payload = CallTool(name, args);
                    return new
                    {
                        content = new object[] { new { type = "text", text = Jx.S(payload) } },
                        structuredContent = payload,
                    };
                }
                catch (BookCtlException e)
                {
                    return new
                    {
                        content = new object[] { new { type = "text", text = Jx.S(Jx.Fail(e)) } },
                        isError = true,
                    };
                }
            }
            default:
                throw new RpcException(-32601, "未知 method: " + method);
        }
    }

    private static object CallTool(string name, JsonElement args)
    {
        var input = ArgS(args, "input") ?? ArgS(args, "input_uri");
        switch (name)
        {
            case "capabilities": return _handlers.Capabilities();
            case "doctor": return _handlers.Doctor();
            case "plan": return _handlers.Plan(input ?? Throw("input"));
            case "book_inspect": return _handlers.Inspect(input ?? Throw("input"));
            case "book_chapters": return _handlers.Chapters(input ?? Throw("input"));
            case "job_start":
                return _handlers.JobStart(input ?? Throw("input"),
                    ArgS(args, "idempotency_key"), ArgS(args, "engine"), ArgS(args, "voice"),
                    ArgS(args, "device"), ArgInt(args, "gpu_index"), ArgInt(args, "cpu_threads"));
            case "job_status": return _handlers.JobStatus(ArgS(args, "job_id") ?? ArgS(args, "id") ?? Throw("job_id"));
            case "job_logs":
                return _handlers.JobLogs(ArgS(args, "job_id") ?? Throw("job_id"),
                    ArgL(args, "since"), (int)ArgL(args, "limit", 200));
            case "job_cancel": return _handlers.JobCancel(ArgS(args, "job_id") ?? Throw("job_id"), ArgS(args, "reason"));
            case "job_resume": return _handlers.JobResume(ArgS(args, "job_id") ?? Throw("job_id"));
            case "job_artifacts": return _handlers.JobArtifacts(ArgS(args, "job_id") ?? Throw("job_id"));
            case "engine_list": return _handlers.EngineList();
            case "engine_test": return _handlers.EngineTest(ArgS(args, "text") ?? "这是本地朗读引擎的试听样例。");
            case "device_list": return _handlers.DeviceList();
            case "source_list": return _handlers.SourceList();
            case "source_pull": return PullSource(ArgS(args, "source_id") ?? ArgS(args, "id") ?? Throw("source_id"));
            default:
                throw new BookCtlException("ARGS_INVALID", $"未知 tool：{name}", fix: "先调用 tools/list 查看可用工具");
        }
    }

    private static string Throw(string field) => throw new BookCtlException("ARGS_MISSING", $"缺少参数 {field}", field: field);

    private static string TopToolsJson() => """
    [
      { "name": "capabilities", "description": "返回 bookctl 能力清单（引擎/命令/协议版本/工作区），无副作用", "annotations": { "readOnlyHint": true, "idempotentHint": true }, "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } },
      { "name": "doctor", "description": "环境自检：工作区可写、ffmpeg、GPU、Python、sidecar 健康度", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } },
      { "name": "plan", "description": "dry-run 转换计划：步骤、产物路径、字数与 GPU 时长估算。不创建任务", "annotations": { "readOnlyHint": true, "idempotentHint": true }, "inputSchema": { "type": "object", "properties": { "input": { "type": "string", "description": "EPUB 绝对路径" } }, "required": [ "input" ] } },
      { "name": "book_inspect", "description": "只读探测 EPUB：标题/章节切分策略/字数/DRM 判定", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object", "properties": { "input": { "type": "string", "description": "EPUB 绝对路径" } }, "required": [ "input" ] } },
      { "name": "book_chapters", "description": "章节树（标题/kind 分类/字数/included 标记）供人工或模型复核", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object", "properties": { "input": { "type": "string" } }, "required": [ "input" ] } },
      { "name": "job_start", "description": "提交整本转换任务。秒级返回 job_id，之后用 job_status/job_logs 轮询。幂等：同 idempotency_key 同参返回同一任务。可用 device 指定用 CPU / GPU / 混合推理", "annotations": { "idempotentHint": true }, "inputSchema": { "type": "object", "properties": { "input": { "type": "string", "description": "EPUB 绝对路径" }, "idempotency_key": { "type": "string" }, "engine": { "type": "string", "enum": [ "mock", "sherpa", "sidecar" ] }, "voice": { "type": "string" }, "device": { "type": "string", "enum": [ "auto", "gpu", "cpu", "hybrid" ], "description": "推理设备，默认 auto（按显存自动决策）" }, "gpu_index": { "type": "integer", "description": "显卡序号，-1 为自动" }, "cpu_threads": { "type": "integer", "description": "CPU 线程数，0 为自动" } }, "required": [ "input" ] } },
      { "name": "job_status", "description": "查询任务状态：阶段、进度、章节数、时长、错误", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object", "properties": { "job_id": { "type": "string" } }, "required": [ "job_id" ] } },
      { "name": "job_logs", "description": "读取任务事件流（NDJSON，cursor 单调递增），用于跟踪实时进度", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object", "properties": { "job_id": { "type": "string" }, "since": { "type": "integer", "default": 0 }, "limit": { "type": "integer", "default": 200 } }, "required": [ "job_id" ] } },
      { "name": "job_cancel", "description": "协作式取消：在当前章节边界生效，checkpoint 保留", "inputSchema": { "type": "object", "properties": { "job_id": { "type": "string" }, "reason": { "type": "string" } }, "required": [ "job_id" ] } },
      { "name": "job_resume", "description": "恢复 failed/cancelled 任务，只重跑未完成章节", "inputSchema": { "type": "object", "properties": { "job_id": { "type": "string" } }, "required": [ "job_id" ] } },
      { "name": "job_artifacts", "description": "完成后获取产物清单：每章音频路径/sha256/时长与整本 M4B", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object", "properties": { "job_id": { "type": "string" } }, "required": [ "job_id" ] } },
      { "name": "engine_list", "description": "列出可用 TTS Provider 与当前默认", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } },
      { "name": "engine_test", "description": "单句试听：生成一个 WAV 样例并返回路径", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } } } },
      { "name": "device_list", "description": "列出本机显卡（序号/显存）+ CPU 核数 + 当前引擎的推理设备决策（GPU / CPU / 混合）", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } },
      { "name": "source_list", "description": "引擎与模型源清单：内置 / 可一键下载 / 已安装 / 手动放置", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false } },
      { "name": "source_pull", "description": "一键下载并校验解包一个模型源到本地（可能耗时数十秒）", "inputSchema": { "type": "object", "properties": { "source_id": { "type": "string", "description": "来源 id，如 sherpa-vits-zh-fanchen" } }, "required": [ "source_id" ] } }
    ]
    """;

    private static string PlaybookText() => """
    bookctl 标准操作流程（严格遵守顺序）：
    1. capabilities —— 获取能力与工作区根目录
    2. book_inspect --input <epub> —— 确认章节结构与 DRM
    3. plan --input <epub> —— dry-run 确认步骤与产物路径
    4. job_start --input <epub> —— 提交（立即返回 job_id，绝不等整本书完成）
    5. 循环 job_status + job_logs --since <cursor> —— 跟踪进度（阶段 S1..S6、GPU 状态、速度）
    6. 终态 completed 后 job_artifacts —— 获取每章音频与整本 M4B 路径
    注意：任何超过 2 秒的操作都必须通过 job 模型异步化；取消用 job_cancel；失败可 job_resume 断点续传。
    """;

    private static (string jobId, string rel) ParseJobUri(string uri)
    {
        var prefix = "job:///";
        if (!uri.StartsWith(prefix, StringComparison.Ordinal))
            throw new RpcException(-32602, "uri 必须形如 job:///{job_id}/{file}");
        var tail = uri[prefix.Length..];
        var slash = tail.IndexOf('/');
        if (slash <= 0) throw new RpcException(-32602, "uri 缺少文件部分");
        return (tail[..slash], tail[(slash + 1)..]);
    }

    private static string? S(JsonElement root, string prop, string child)
    {
        if (root.TryGetProperty(prop, out var p) && p.ValueKind == JsonValueKind.Object &&
            p.TryGetProperty(child, out var c) && c.ValueKind == JsonValueKind.String)
            return c.GetString();
        return null;
    }

    private static string? ArgS(JsonElement args, string name)
    {
        if (args.ValueKind == JsonValueKind.Object && args.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String)
            return v.GetString();
        return null;
    }

    private static long ArgL(JsonElement args, string name, long dflt = 0)
    {
        if (args.ValueKind == JsonValueKind.Object && args.TryGetProperty(name, out var v) &&
            v.ValueKind == JsonValueKind.Number && v.TryGetInt64(out var l))
            return l;
        return dflt;
    }

    private static int? ArgInt(JsonElement args, string name)
    {
        if (args.ValueKind == JsonValueKind.Object && args.TryGetProperty(name, out var v) &&
            v.ValueKind == JsonValueKind.Number && v.TryGetInt32(out var i))
            return i;
        return null;
    }

    private static object PullSource(string id)
    {
        // 下载可能几十秒到几分钟：MCP 客户端会等，但只在这一件事上；不做 job 化是为了让模型拿到明确的安装结果
        return new BookCtlHandlers().SourcePull(id).GetAwaiter().GetResult();
    }

    private static JsonElement Json(string s)
    {
        using var doc = JsonDocument.Parse(s);
        return doc.RootElement.Clone();
    }
}
