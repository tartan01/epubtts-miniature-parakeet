using System.Diagnostics;
using System.IO.Compression;
using System.Net.Http;
using System.Text;
using System.Text.Json.Serialization;

namespace BookCtl.Core;

public sealed class SourceInstall
{
    /// <summary>archive（tar/tar.bz2/tar.gz，用系统 tar.exe 解）/ zip / file（多文件直下）/ manual（需人工放置）。</summary>
    public string Type { get; set; } = "archive";
    public string Url { get; set; } = "";
    public string Sha256 { get; set; } = "";
    public string Target { get; set; } = "";
    public List<SourceFile>? Files { get; set; }

    /// <summary>额外的下载线路，支持 {url} 占位符。留空则用内置的 GitHub / HuggingFace 镜像规则。</summary>
    public List<string>? Mirrors { get; set; }
}

public sealed class SourceFile
{
    public string Name { get; set; } = "";
    public string Url { get; set; } = "";
}

public sealed class SourceEntry
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Engine { get; set; } = "sherpa";
    public string Kind { get; set; } = "model";     // model | engine
    public string Device { get; set; } = "cpu";     // cpu | gpu | cpu/gpu
    public double SizeMb { get; set; }
    public string License { get; set; } = "";
    public string Homepage { get; set; } = "";
    public string Note { get; set; } = "";
    public bool Bundled { get; set; }               // true = 随安装包内置，无需下载
    public string[]? Voices { get; set; }
    public SourceInstall Install { get; set; } = new();
}

public sealed class SourceCatalog
{
    public string Version { get; set; } = "1";
    public List<SourceEntry> Entries { get; set; } = new();
}

/// <summary>
/// 引擎/模型源管理：内置目录 + 用户可追加的源 + 一键下载到本地 + 校验 + 解包。
/// 设计目标（配合封装）：portable 版 SourcesRoot 在程序目录旁，安装版在 %LOCALAPPDATA%，两版共用同一套目录约定。
/// </summary>
public sealed class SourceManager
{
    private readonly BookCtlConfig _cfg;
    private static readonly HttpClient Http = new() { Timeout = TimeSpan.FromMinutes(60) };

    public string Root { get; }

    public SourceManager(BookCtlConfig cfg)
    {
        _cfg = cfg;
        Root = cfg.SourcesRoot;
    }

    /// <summary>内置默认源清单，直接写进二进制：封装时即使漏拷 config/sources.json 也不会缺目录。</summary>
    public static SourceCatalog BuiltInCatalog() => new()
    {
        Version = "1",
        Entries = new List<SourceEntry>
        {
            new()
            {
                Id = "mock", Name = "内置校样引擎（占位音频）", Engine = "mock", Kind = "engine",
                Device = "cpu", SizeMb = 0, License = "随软件",
                Homepage = "", Bundled = true,
                Note = "不联网、不占显存，用于验证流水线与界面。生成的音频是占位音调，不是真人声。",
            },
            new()
            {
                Id = "sherpa-vits-zh-fanchen", Name = "sherpa-onnx · 中文 VITS（Fanchen，多音色）",
                Engine = "sherpa", Kind = "model", Device = "cpu", SizeMb = 130,
                License = "Apache-2.0（sherpa-onnx）；模型卡见 HuggingFace",
                Homepage = "https://github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models",
                Note = "纯 CPU 即可实时（RTF≈0.1–0.4），支持 0/1 多说话人；2GB 显存机器首选。",
                Voices = new[] { "0", "1" },
                Install = new SourceInstall
                {
                    Type = "archive",
                    Url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-zh-hf-fanchen-C.tar.bz2",
                    Target = "sherpa-vits-zh-fanchen",
                },
            },
            new()
            {
                Id = "sherpa-vits-zh-aishell3", Name = "sherpa-onnx · 中文 VITS（AISHELL-3，174 音色）",
                Engine = "sherpa", Kind = "model", Device = "cpu", SizeMb = 60,
                License = "Apache-2.0（模型）/ AISHELL-3 语料许可",
                Homepage = "https://github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models",
                Note = "多说话人（sid 0–173），适合整本书换音色；体积小，CPU 可跑。",
                Install = new SourceInstall
                {
                    Type = "archive",
                    Url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-zh-aishell3.tar.bz2",
                    Target = "sherpa-vits-zh-aishell3",
                },
            },
            new()
            {
                Id = "piper-zh-huayan", Name = "Piper · 中文女声 huayan（medium）",
                Engine = "piper", Kind = "model", Device = "cpu", SizeMb = 65,
                License = "MIT（Piper）/ CC-BY-4.0（音色）",
                Homepage = "https://huggingface.co/rhasspy/piper-voices",
                Note = "极轻量，CPU 飞快但韵律较平；适合做试听/粗剪。",
                Install = new SourceInstall
                {
                    Type = "file",
                    Target = "piper-zh-huayan",
                    Files = new List<SourceFile>
                    {
                        new() { Name = "zh_CN-huayan-medium.onnx", Url = "https://huggingface.co/rhasspy/piper-voices/resolve/main/zh/zh_CN/huayan/medium/zh_CN-huayan-medium.onnx" },
                        new() { Name = "zh_CN-huayan-medium.onnx.json", Url = "https://huggingface.co/rhasspy/piper-voices/resolve/main/zh/zh_CN/huayan/medium/zh_CN-huayan-medium.onnx.json" },
                    },
                },
            },
            new()
            {
                Id = "cosyvoice3", Name = "CosyVoice 3（0.5B，中文最佳）",
                Engine = "sidecar", Kind = "model", Device = "gpu", SizeMb = 5800,
                License = "Apache-2.0（代码与权重）",
                Homepage = "https://github.com/FunAudioLLM/CosyVoice",
                Note = "需要 ≥6GB 显存；权重体积大、需登录 ModelScope/HF，故不提供直链一键下载，按 engine/README 手动放置到 sources/cosyvoice3。",
                Install = new SourceInstall { Type = "manual", Target = "cosyvoice3" },
            },
        },
    };

    public SourceCatalog Catalog()
    {
        var cat = BuiltInCatalog();
        var byId = cat.Entries.ToDictionary(e => e.Id, e => e, StringComparer.OrdinalIgnoreCase);

        foreach (var path in new[] { FindRepoConfig("sources.json"), Path.Combine(Root, "catalog.user.json") })
        {
            if (string.IsNullOrWhiteSpace(path) || !File.Exists(path)) continue;
            try
            {
                var extra = Jx.D<SourceCatalog>(File.ReadAllText(path, Encoding.UTF8));
                if (extra?.Entries is null) continue;
                foreach (var e in extra.Entries)
                {
                    if (string.IsNullOrWhiteSpace(e.Id)) continue;
                    if (e.Install is null) e.Install = new SourceInstall();
                    byId[e.Id] = e;   // 覆盖 = 用户自定义优先
                }
            }
            catch { }
        }
        return new SourceCatalog { Version = cat.Version, Entries = byId.Values.OrderBy(e => e.Bundled ? 0 : 1).ThenBy(e => e.Id).ToList() };
    }

    private static string? FindRepoConfig(string fileName)
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        for (var i = 0; i < 6 && dir is not null; i++)
        {
            var probe = Path.Combine(dir.FullName, "config", fileName);
            if (File.Exists(probe)) return probe;
            dir = dir.Parent;
        }
        return null;
    }

    public string? InstalledPath(string id)
    {
        var dir = Path.Combine(Root, id);
        if (!Directory.Exists(dir)) return null;
        var marker = Path.Combine(dir, ".installed.json");
        return File.Exists(marker) || Directory.EnumerateFileSystemEntries(dir).Any() ? dir : null;
    }

    public List<object> List()
    {
        var list = new List<object>();
        foreach (var e in Catalog().Entries)
        {
            var dir = Path.Combine(Root, e.Id);
            var installed = e.Bundled || Directory.Exists(dir) && Directory.EnumerateFileSystemEntries(dir).Any();
            long bytes = 0;
            if (Directory.Exists(dir))
            {
                try
                {
                    bytes = new DirectoryInfo(dir).EnumerateFiles("*", SearchOption.AllDirectories).Sum(f => f.Length);
                }
                catch { }
            }
            list.Add(new
            {
                id = e.Id,
                name = e.Name,
                engine = e.Engine,
                kind = e.Kind,
                device = e.Device,
                size_mb = e.SizeMb,
                license = e.License,
                homepage = e.Homepage,
                note = e.Note,
                voices = e.Voices,
                bundled = e.Bundled,
                installed,
                installable = e.Install.Type is not "manual",
                install_type = e.Install.Type,
                path = installed && !e.Bundled ? dir : null,
                size_on_disk_mb = Math.Round(bytes / 1048576.0, 1),
                url = e.Install.Url,
            });
        }
        return list;
    }

    public async Task<object> PullAsync(string id, Action<string, double, string>? progress = null, CancellationToken ct = default)
    {
        var entry = Catalog().Entries.FirstOrDefault(e => string.Equals(e.Id, id, StringComparison.OrdinalIgnoreCase))
            ?? throw new BookCtlException("NOT_FOUND", $"源不存在：{id}", field: "id", value: id,
                fix: "先用 bookctl source list 查看可用源", example: "bookctl source list");

        if (entry.Bundled)
            return new { id = entry.Id, status = "bundled", path = (string?)null, note = "内置源无需下载" };

        if (entry.Install.Type == "manual")
            throw new BookCtlException("MANUAL_INSTALL_REQUIRED",
                $"源 {entry.Id} 需要手动下载（体积大 / 需登录）：{entry.Homepage}",
                fix: $"把文件放到 {Path.Combine(Root, entry.Install.Target)} 后重试",
                example: "见 engine/README.md");

        if (string.IsNullOrWhiteSpace(entry.Install.Url) && entry.Install.Files is null)
            throw new BookCtlException("SOURCE_NO_URL", $"源 {entry.Id} 没有配置下载地址",
                fix: "在 config/sources.json 或 sources/catalog.user.json 中补 install.url");

        var targetDir = Path.Combine(Root, string.IsNullOrWhiteSpace(entry.Install.Target) ? entry.Id : entry.Install.Target);
        Directory.CreateDirectory(Root);
        var tmpDir = Path.Combine(Root, ".tmp", entry.Id + "-" + Guid.NewGuid().ToString("N")[..6]);
        Directory.CreateDirectory(tmpDir);
        try
        {
            if (entry.Install.Type == "file" && entry.Install.Files is { Count: > 0 })
            {
                var files = entry.Install.Files;
                for (var i = 0; i < files.Count; i++)
                {
                    var dest = Path.Combine(tmpDir, files[i].Name);
                    await DownloadAsync(Candidates(files[i].Url, entry.Install.Mirrors), dest, (frac, label) =>
                        progress?.Invoke("downloading", (i + frac) / files.Count, $"{label}（{i + 1}/{files.Count}）"), ct);
                }
            }
            else
            {
                var fileName = Path.GetFileName(new Uri(entry.Install.Url).AbsolutePath);
                var archive = Path.Combine(tmpDir, fileName);
                await DownloadAsync(Candidates(entry.Install.Url, entry.Install.Mirrors), archive,
                    (frac, label) => progress?.Invoke("downloading", frac, label), ct);

                if (!string.IsNullOrWhiteSpace(entry.Install.Sha256))
                {
                    var actual = HashUtil.ShaFile(archive);
                    if (!actual.Equals(entry.Install.Sha256, StringComparison.OrdinalIgnoreCase))
                        throw new BookCtlException("SOURCE_HASH_MISMATCH",
                            $"下载文件校验失败：期望 {entry.Install.Sha256}，实际 {actual}",
                            retryable: true, fix: "删除临时文件后重试；如仍失败说明上游文件已更新，请更新 catalog 中的 sha256");
                }

                progress?.Invoke("extracting", 1, "正在解包…");
                // 解到独立子目录：否则下载的归档文件会和模型文件混在一起，拍平后就残留一个 119MB 的压缩包
                var extractDir = Path.Combine(tmpDir, "extract");
                Directory.CreateDirectory(extractDir);
                Extract(archive, extractDir);
                File.Delete(archive);
            }

            if (Directory.Exists(targetDir)) Directory.Delete(targetDir, true);
            Directory.CreateDirectory(Path.GetDirectoryName(targetDir)!);
            var sourceDir = entry.Install.Type == "file"
                ? tmpDir
                : Path.Combine(tmpDir, "extract");
            Flatten(sourceDir, targetDir);
            File.WriteAllText(Path.Combine(targetDir, ".installed.json"), Jx.P(new
            {
                id = entry.Id,
                engine = entry.Engine,
                installed_at = DateTimeOffset.Now,
                size_mb = entry.SizeMb,
                note = entry.Note,
            }), new UTF8Encoding(false));

            progress?.Invoke("done", 1, "安装完成");
            return new { id = entry.Id, status = "installed", path = targetDir };
        }
        finally
        {
            try { if (Directory.Exists(tmpDir)) Directory.Delete(tmpDir, true); } catch { }
        }
    }

    public object Remove(string id)
    {
        var entry = Catalog().Entries.FirstOrDefault(e => string.Equals(e.Id, id, StringComparison.OrdinalIgnoreCase))
            ?? throw new BookCtlException("NOT_FOUND", $"源不存在：{id}", field: "id", value: id);
        if (entry.Bundled)
            throw new BookCtlException("ARGS_INVALID", "内置源不能删除", field: "id", value: id);

        var dir = Path.Combine(Root, entry.Id);
        if (Directory.Exists(dir)) Directory.Delete(dir, true);
        return new { id = entry.Id, status = "removed" };
    }

    private static async Task DownloadAsync(string url, string dest, Action<double, string>? progress, CancellationToken ct)
        => await DownloadAsync(Candidates(url, null), dest, progress, ct);

    /// <summary>线路候选：原始地址 → 用户 mirrors → 内置镜像（GitHub 走 gh-proxy，HF 走 hf-mirror）。
    /// 国内网络直连 github release 常被拦（实测 502），没有回退就等于功能不可用。</summary>
    private static List<string> Candidates(string url, List<string>? extra)
    {
        var list = new List<string> { url };
        if (extra is not null)
            foreach (var m in extra)
                if (!string.IsNullOrWhiteSpace(m)) list.Add(m.Replace("{url}", url));

        var env = Environment.GetEnvironmentVariable("BOOKCTL_MIRROR");
        if (!string.IsNullOrWhiteSpace(env)) list.Add(env.Replace("{url}", url));

        if (url.Contains("github.com"))
            list.Add("https://gh-proxy.com/" + url);
        if (url.Contains("huggingface.co"))
            list.Add(url.Replace("https://huggingface.co", "https://hf-mirror.com"));

        return list.Distinct(StringComparer.OrdinalIgnoreCase).ToList();
    }

    private static async Task DownloadAsync(IReadOnlyList<string> urls, string dest, Action<double, string>? progress, CancellationToken ct)
    {
        var errors = new List<string>();
        foreach (var url in urls)
        {
            try
            {
                await DownloadOneAsync(url, dest, progress, ct);
                return;
            }
            catch (OperationCanceledException) { throw; }
            catch (Exception e)
            {
                errors.Add($"{Host(url)}: {e.Message}");
                progress?.Invoke(0, $"线路不通，切换下一个镜像（{errors.Count}/{urls.Count}）");
            }
        }
        throw new BookCtlException("SOURCE_DOWNLOAD_FAILED",
            "所有下载线路都失败：" + string.Join("；", errors), retryable: true,
            fix: "检查网络或代理；企业内网可用 BOOKCTL_MIRROR 环境变量或在 sources/catalog.user.json 中给该源加 mirrors");
    }

    private static string Host(string url)
    {
        try { return new Uri(url).Host; } catch { return "?"; }
    }

    private static async Task DownloadOneAsync(string url, string dest, Action<double, string>? progress, CancellationToken ct)
    {
        using var resp = await Http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct);
        if (!resp.IsSuccessStatusCode)
            throw new BookCtlException("SOURCE_DOWNLOAD_FAILED",
                $"HTTP {(int)resp.StatusCode}：{url}", retryable: true);

        var total = resp.Content.Headers.ContentLength ?? -1;
        await using var src = await resp.Content.ReadAsStreamAsync(ct);
        await using var dst = File.Create(dest);
        var buf = new byte[81920];
        long read = 0;
        int n;
        var lastReport = 0L;
        while ((n = await src.ReadAsync(buf, ct)) > 0)
        {
            await dst.WriteAsync(buf.AsMemory(0, n), ct);
            read += n;
            if (total > 0 && read - lastReport > 1_048_576)
            {
                lastReport = read;
                progress?.Invoke(read / (double)total, $"{read / 1048576.0:0.0}/{total / 1048576.0:0.0} MB");
            }
        }
        if (total > 0 && read < total)
            throw new BookCtlException("SOURCE_DOWNLOAD_FAILED", $"下载不完整：{read}/{total} 字节", retryable: true);
    }

    private static void Extract(string archive, string destDir)
    {
        var name = archive.ToLowerInvariant();
        if (name.EndsWith(".zip"))
        {
            ZipFile.ExtractToDirectory(archive, destDir, true);
            return;
        }

        // tar / tar.bz2 / tar.gz：Windows 10+ 自带 bsdtar（System32\tar.exe），支持 bz2/gz
        var psi = new ProcessStartInfo
        {
            FileName = "tar",
            Arguments = $"-xf \"{archive}\" -C \"{destDir}\"",
            RedirectStandardError = true,
            RedirectStandardOutput = true,
            UseShellExecute = false,
            CreateNoWindow = true,
        };
        using var p = Process.Start(psi)
            ?? throw new BookCtlException("SOURCE_EXTRACT_FAILED", "无法启动 tar 解包（系统缺少 tar.exe）");
        var err = p.StandardError.ReadToEndAsync();
        p.StandardOutput.ReadToEndAsync();
        p.WaitForExit(10 * 60 * 1000);
        if (p.ExitCode != 0)
            throw new BookCtlException("SOURCE_EXTRACT_FAILED", $"解包失败：{err.Result.Trim()}",
                fix: "确认压缩包完整；或手动解压到 sources 下对应目录");
    }

    /// <summary>很多模型包解出来是单层同名目录；统一拍平，保证 InstalledPath(id) 下直接就是模型文件。</summary>
    private static void Flatten(string tmpDir, string targetDir)
    {
        var entries = Directory.GetFileSystemEntries(tmpDir);
        var src = entries.Length == 1 && Directory.Exists(entries[0]) ? entries[0] : tmpDir;
        Directory.CreateDirectory(targetDir);
        foreach (var f in Directory.GetFiles(src)) File.Copy(f, Path.Combine(targetDir, Path.GetFileName(f)), true);
        foreach (var d in Directory.GetDirectories(src))
        {
            var dn = Path.GetFileName(d);
            var sub = Path.Combine(targetDir, dn);
            if (Directory.Exists(sub)) Directory.Delete(sub, true);
            Directory.Move(d, sub);
        }
    }
}
