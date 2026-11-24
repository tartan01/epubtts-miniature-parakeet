using System.IO.Compression;
using System.Net;
using System.Text;
using System.Text.RegularExpressions;
using System.Xml;
using System.Xml.Linq;

namespace BookCtl.Core;

public sealed class TocEntry
{
    public string Title { get; set; } = "";
    public string Href { get; set; } = "";
    public string? Anchor { get; set; }
    public int Level { get; set; } = 1;
}

public sealed class SpineDoc
{
    public string Href { get; set; } = "";
    public string Html { get; set; } = "";
}

public sealed class EpubBook
{
    public BookMeta Meta { get; set; } = new();
    public List<TocEntry> Toc { get; set; } = new();
    public List<SpineDoc> Docs { get; set; } = new();
    public string SplitStrategy { get; set; } = "toc";
    public string? CoverHref { get; set; }
}

public sealed record Heading(string Anchor, string Title, int Level, int Offset);

public static class EpubParser
{
    private static readonly XNamespace NsOpf = "http://www.idpf.org/2007/opf";
    private static readonly XNamespace NsDc = "http://purl.org/dc/elements/1.1/";
    private static readonly XNamespace NsCnt = "urn:oasis:names:tc:opendocument:xmlns:container";

    public static EpubBook Parse(string epubPath)
    {
        if (!File.Exists(epubPath))
            throw new BookCtlException("FILE_NOT_FOUND", $"找不到 EPUB 文件：{epubPath}",
                field: "input", value: epubPath, fix: "检查文件路径与扩展名");

        var meta = new BookMeta
        {
            InputPath = Path.GetFullPath(epubPath),
            InputSha256 = "sha256:" + HashUtil.ShaFile(epubPath),
            Title = Path.GetFileNameWithoutExtension(epubPath),
        };
        meta.BookId = meta.InputSha256["sha256:".Length..][..12];

        using var zip = ZipFile.OpenRead(epubPath);

        CheckDrm(zip, meta);

        var container = ReadEntry(zip, "META-INF/container.xml");
        if (container is null)
            throw new BookCtlException("EPUB_INVALID", "缺少 META-INF/container.xml，不是合法 EPUB",
                fix: "用 EPUBCheck 校验来源文件");
        var opfPath = XDocument.Parse(container).Root?.Elements(NsCnt + "rootfiles")
            .Elements(NsCnt + "rootfile").FirstOrDefault()?.Attribute("full-path")?.Value
            ?? throw new BookCtlException("EPUB_INVALID", "container.xml 中未声明 OPF 路径");

        var opfText = ReadEntry(zip, opfPath)
            ?? throw new BookCtlException("EPUB_INVALID", $"OPF 文件缺失：{opfPath}");
        var opf = XDocument.Parse(opfText);
        var opfDir = Path.GetDirectoryName(opfPath.Replace('\\', '/')) ?? "";

        meta.EpubVersion = opf.Root?.Attribute("version")?.Value ?? "2.0";
        var md = opf.Root?.Element(NsOpf + "metadata");
        meta.Title = (md?.Element(NsDc + "title")?.Value ?? meta.Title).Trim();
        meta.Creator = (md?.Element(NsDc + "creator")?.Value ?? "").Trim();
        meta.Language = (md?.Element(NsDc + "language")?.Value ?? "zh").Trim();

        var items = opf.Root?.Element(NsOpf + "manifest")?.Elements(NsOpf + "item")
            .Select(i => new
            {
                Id = i.Attribute("id")?.Value ?? "",
                Href = i.Attribute("href")?.Value ?? "",
                Type = i.Attribute("media-type")?.Value ?? "",
                Props = i.Attribute("properties")?.Value ?? "",
            }).ToList() ?? new();

        var cover = items.FirstOrDefault(i => i.Props.Contains("cover-image"))
            ?? items.FirstOrDefault(i => i.Id == "cover" && i.Type.StartsWith("image/"));
        var coverHref = cover?.Href;

        var spineEl = opf.Root?.Element(NsOpf + "spine");
        var ncxId = spineEl?.Attribute("toc")?.Value;
        var spineIds = spineEl?.Elements(NsOpf + "itemref")
            .Select(ir => ir.Attribute("idref")?.Value ?? "").Where(id => id.Length > 0).ToList() ?? new();

        // spine 顺序的 XHTML 文档
        var docs = new List<SpineDoc>();
        foreach (var id in spineIds)
        {
            var it = items.FirstOrDefault(i => i.Id == id);
            if (it is null || !it.Type.Contains("xhtml") && !it.Type.Contains("html") && !it.Href.EndsWith(".xhtml") && !it.Href.EndsWith(".html"))
                continue;
            var full = ResolveHref(opfDir, it.Href);
            var html = ReadEntry(zip, full);
            if (html is not null) docs.Add(new SpineDoc { Href = full, Html = html });
        }

        // TOC：EPUB3 nav 优先，退化 NCX
        var toc = new List<TocEntry>();
        var navItem = items.FirstOrDefault(i => i.Props.Contains("nav"));
        if (navItem is not null)
            toc = ParseNavToc(ReadEntry(zip, ResolveHref(opfDir, navItem.Href)) ?? "");
        if (toc.Count == 0 && !string.IsNullOrEmpty(ncxId))
        {
            var ncxItem = items.FirstOrDefault(i => i.Id == ncxId)
                ?? items.FirstOrDefault(i => i.Type == "application/x-dtbncx+xml");
            if (ncxItem is not null)
                toc = ParseNcxToc(ReadEntry(zip, ResolveHref(opfDir, ncxItem.Href)) ?? "");
        }

        return new EpubBook
        {
            Meta = meta,
            Toc = toc,
            Docs = docs,
            CoverHref = coverHref,
        };
    }

    private static void CheckDrm(ZipArchive zip, BookMeta meta)
    {
        if (zip.GetEntry("META-INF/rights.xml") is not null)
        {
            meta.DrmProtected = true;
            meta.DrmDetail = "META-INF/rights.xml 存在（内容加密）";
            return;
        }
        var enc = ReadEntry(zip, "META-INF/encryption.xml");
        if (enc is null) return;
        var algos = Regex.Matches(enc, "Algorithm=\"([^\"]+)\"").Select(m => m.Groups[1].Value).ToList();
        if (algos.Count == 0) return;
        var fontMangling = "http://www.idpf.org/2008/embedding";
        if (algos.All(a => a.Equals(fontMangling, StringComparison.OrdinalIgnoreCase))) return; // 只是字体混淆，不是 DRM
        meta.DrmProtected = true;
        meta.DrmDetail = "encryption.xml 声明了内容级加密：" + string.Join(", ", algos);
    }

    private static string ResolveHref(string opfDir, string href)
    {
        var decoded = Uri.UnescapeDataString(href.Replace('\\', '/'));
        var combined = string.IsNullOrEmpty(opfDir) ? decoded : opfDir + "/" + decoded;
        var parts = combined.Split('/');
        var stack = new List<string>();
        foreach (var p in parts)
        {
            if (p == "." || p.Length == 0) continue;
            if (p == "..") { if (stack.Count > 0) stack.RemoveAt(stack.Count - 1); continue; }
            stack.Add(p);
        }
        return string.Join("/", stack);
    }

    private static string? ReadEntry(ZipArchive zip, string name)
    {
        var entry = zip.GetEntry(name) ?? zip.Entries.FirstOrDefault(e =>
            string.Equals(e.FullName, name, StringComparison.OrdinalIgnoreCase));
        if (entry is null) return null;
        using var sr = new StreamReader(entry.Open(), Encoding.UTF8);
        return sr.ReadToEnd();
    }

    private static List<TocEntry> ParseNavToc(string navHtml)
    {
        var result = new List<TocEntry>();
        if (string.IsNullOrEmpty(navHtml)) return result;
        var navMatch = Regex.Match(navHtml, @"(?is)<nav[^>]*\bepub:type\s*=\s*""toc""[^>]*>(.*?)</nav>");
        var scope = navMatch.Success ? navMatch.Groups[1].Value : navHtml;
        var liPattern = new Regex(@"(?is)<li[^>]*>|</li>|<a[^>]*href\s*=\s*""([^""]+)""[^>]*>(.*?)</a>");
        var depth = 0;
        foreach (Match m in liPattern.Matches(scope))
        {
            if (m.Value.StartsWith("<li", StringComparison.OrdinalIgnoreCase)) { depth++; continue; }
            if (m.Value == "</li>") { depth = Math.Max(0, depth - 1); continue; }
            var href = m.Groups[1].Value;
            var title = HtmlText.StripTags(m.Groups[2].Value);
            if (string.IsNullOrWhiteSpace(title) || string.IsNullOrWhiteSpace(href)) continue;
            var (baseHref, anchor) = SplitFragment(href);
            result.Add(new TocEntry { Title = title, Href = baseHref, Anchor = anchor, Level = Math.Max(1, depth) });
        }
        return result;
    }

    private static List<TocEntry> ParseNcxToc(string ncxText)
    {
        var result = new List<TocEntry>();
        if (string.IsNullOrEmpty(ncxText)) return result;
        try
        {
            var ncx = XDocument.Parse(ncxText);
            XNamespace ns = ncx.Root?.Name.Namespace ?? XNamespace.None;
            void Walk(XElement? navPoint, int level)
            {
                if (navPoint is null) return;
                var title = navPoint.Element(ns + "navLabel")?.Element(ns + "text")?.Value.Trim() ?? "";
                var src = navPoint.Element(ns + "content")?.Attribute("src")?.Value ?? "";
                if (title.Length > 0 && src.Length > 0)
                {
                    var (baseHref, anchor) = SplitFragment(src);
                    result.Add(new TocEntry { Title = title, Href = baseHref, Anchor = anchor, Level = level });
                }
                foreach (var child in navPoint.Elements(ns + "navPoint")) Walk(child, level + 1);
            }
            var map = ncx.Root?.Element(ns + "navMap");
            if (map is not null)
                foreach (var np in map.Elements(ns + "navPoint")) Walk(np, 1);
        }
        catch (XmlException) { }
        return result;
    }

    public static (string href, string? anchor) SplitFragment(string href)
    {
        var i = href.IndexOf('#');
        if (i < 0) return (href, null);
        var anchor = href[(i + 1)..];
        return (href[..i], anchor.Length == 0 ? null : anchor);
    }
}

public static class HtmlText
{
    private static readonly Regex TagStrip = new("<[^>]+>", RegexOptions.Compiled);

    public static string Extract(string html)
    {
        var s = Regex.Replace(html, @"(?is)<head[^>]*>.*?</head>", " ");
        s = Regex.Replace(s, @"(?is)<(script|style|svg)[^>]*>.*?</\1>", " ");
        s = Regex.Replace(s, @"(?is)<blockquote[^>]*>", "引用，");
        s = Regex.Replace(s, @"(?i)</(p|div|h[1-6]|li|tr|blockquote|section|article|figcaption|table)>", "\n");
        s = Regex.Replace(s, @"(?i)<br[^>]*/?>", "\n");
        s = TagStrip.Replace(s, "");
        s = WebUtility.HtmlDecode(s);
        s = s.Replace('\r', ' ');
        s = Regex.Replace(s, "[ \t]+", " ");
        s = Regex.Replace(s, " ?\n ?", "\n");
        s = Regex.Replace(s, "\n{2,}", "\n");
        return s.Trim();
    }

    public static string StripTags(string html) =>
        WebUtility.HtmlDecode(TagStrip.Replace(html, "")).Trim();

    public static List<Heading> FindHeadings(string html)
    {
        var list = new List<Heading>();
        var re = new Regex(@"(?is)<h([1-6])[^>]*?(?:\bid\s*=\s*""([^""]*)""[^>]*|[^>])>(.*?)</h\1>");
        foreach (Match m in re.Matches(html))
        {
            var level = int.Parse(m.Groups[1].Value);
            var id = m.Groups[2].Value;
            var title = StripTags(m.Groups[3].Value);
            list.Add(new Heading(id, title, level, m.Index));
        }
        return list;
    }

    public static int FindAnchorOffset(string html, string anchor)
    {
        if (string.IsNullOrEmpty(anchor)) return 0;
        var esc = Regex.Escape(anchor);
        var m = Regex.Match(html, $@"(?i)\bid\s*=\s*""{esc}""");
        if (m.Success) return m.Index;
        m = Regex.Match(html, $@"(?i)<a[^>]*\bname\s*=\s*""{esc}""[^>]*>");
        if (m.Success) return m.Index;
        return 0;
    }
}

public static class ChapterSplitter
{
    public static List<Chapter> Split(EpubBook book)
    {
        var docs = book.Docs;
        if (docs.Count == 0)
            throw new BookCtlException("EPUB_INVALID", "EPUB spine 中没有任何 XHTML 文档",
                fix: "确认这是标准 EPUB 而非改名的 zip");

        var boundaries = BuildBoundaries(book);
        book.SplitStrategy = boundaries.Strategy;

        var chapters = new List<Chapter>();
        for (var i = 0; i < boundaries.Items.Count; i++)
        {
            var b = boundaries.Items[i];
            var next = i + 1 < boundaries.Items.Count ? boundaries.Items[i + 1] : null;
            var html = SliceHtml(docs, b, next);
            var text = HtmlText.Extract(html);
            if (text.Length < 20) continue; // 空壳章（封面/目录页等）跳过
            var idx = chapters.Count + 1;
            chapters.Add(new Chapter
            {
                Id = $"ch-{idx:D4}",
                Index = idx,
                Level = b.Level,
                Title = string.IsNullOrWhiteSpace(b.Title) ? $"第 {idx} 节" : b.Title,
                Href = b.DocIndex < docs.Count ? docs[b.DocIndex].Href : "",
                Anchor = b.Anchor,
                Kind = ClassifyKind(b.Title, idx, chapters.Count == 0),
                TextRaw = text,
                CharCount = text.Length,
            });
        }
        if (chapters.Count == 0)
            throw new BookCtlException("EPUB_NO_TEXT", "切分后未获得任何正文文本",
                fix: "检查 EPUB 是否为图片型电子书（纯扫描件无法朗读）");
        return chapters;
    }

    private sealed record Boundary(int DocIndex, int Offset, string Title, int Level, string? Anchor);

    private sealed record Boundaries(List<Boundary> Items, string Strategy);

    private static Boundaries BuildBoundaries(EpubBook book)
    {
        var docs = book.Docs;
        var hrefIndex = docs.Select((d, i) => (d.Href, i))
            .GroupBy(x => x.Href, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(g => g.Key, g => g.First().i, StringComparer.OrdinalIgnoreCase);

        // 1) TOC 锚点优先
        var items = new List<Boundary>();
        if (book.Toc.Count >= 3)
        {
            foreach (var t in book.Toc)
            {
                if (!hrefIndex.TryGetValue(t.Href, out var di))
                {
                    // 尝试仅按文件名匹配
                    var byName = docs.Select((d, i) => (d, i))
                        .FirstOrDefault(x => x.d.Href.EndsWith(t.Href, StringComparison.OrdinalIgnoreCase) ||
                                             Path.GetFileName(x.d.Href).Equals(Path.GetFileName(t.Href), StringComparison.OrdinalIgnoreCase));
                    if (byName.d is null) continue;
                    di = byName.i;
                }
                var off = HtmlText.FindAnchorOffset(docs[di].Html, t.Anchor ?? "");
                var title = t.Title;
                var anchor = t.Anchor;
                // 找同位置重复的边界（TOC 里同锚点多级）
                if (items.Any(x => x.DocIndex == di && x.Offset == off))
                {
                    if (items[^1].DocIndex == di && items[^1].Offset == off && t.Level > items[^1].Level)
                    {
                        items[^1] = items[^1] with { Title = title, Level = t.Level };
                    }
                    continue;
                }
                items.Add(new Boundary(di, off, title, t.Level, anchor));
            }
        }
        if (items.Count >= 2) return new Boundaries(items, "toc");

        // 2) heading 启发式
        items.Clear();
        for (var di = 0; di < docs.Count; di++)
        {
            foreach (var h in HtmlText.FindHeadings(docs[di].Html).Where(h => h.Level <= 2))
                items.Add(new Boundary(di, h.Offset, h.Title, h.Level, string.IsNullOrEmpty(h.Anchor) ? null : h.Anchor));
        }
        if (items.Count >= 2) return new Boundaries(items, "heuristic-headings");

        // 3) 每文档一"章"
        items = docs.Select((d, di) => new Boundary(di, 0, "", 1, null)).ToList();
        return new Boundaries(items, "per-document");
    }

    private static string SliceHtml(List<SpineDoc> docs, Boundary b, Boundary? next)
    {
        var sb = new StringBuilder();
        if (next is null || next.DocIndex == b.DocIndex)
        {
            var html = docs[b.DocIndex].Html;
            var end = next is null ? html.Length : Math.Min(next.Offset, html.Length);
            if (end > b.Offset) sb.AppendLine(html[b.Offset..end]);
            return sb.ToString();
        }
        var first = docs[b.DocIndex].Html;
        sb.AppendLine(first[b.Offset..]);
        for (var di = b.DocIndex + 1; di < next.DocIndex && di < docs.Count; di++)
            sb.AppendLine(docs[di].Html);
        var last = docs[next.DocIndex].Html;
        if (next.Offset > 0) sb.AppendLine(last[..Math.Min(next.Offset, last.Length)]);
        return sb.ToString();
    }

    public static string ClassifyKind(string title, int position, bool isFirst)
    {
        var t = title?.Trim() ?? "";
        if (Regex.IsMatch(t, @"目录|目次|contents", RegexOptions.IgnoreCase)) return "toc";
        if (Regex.IsMatch(t, @"附录|注\s*释|脚注|尾注|参考文献|索引|词汇表|术语|参考资料|参考书目|Reference|Index|Appendix", RegexOptions.IgnoreCase))
            return "appendix";
        if (Regex.IsMatch(t, @"^序|^前言|^引言|^导言|^导读|^致谢|^献词|^推荐|^译者|^编者|^出版说明|^题献|^献辞|^凡例|prologue|foreword|preface", RegexOptions.IgnoreCase))
            return "frontmatter";
        if (isFirst && position <= 2 && Regex.IsMatch(t, @"版权|出版|书名|封面|献词|题记", RegexOptions.IgnoreCase)) return "frontmatter";
        return "body";
    }
}

public static class TextNormalizer
{
    public static string Normalize(string raw)
    {
        var s = raw;
        s = Regex.Replace(s, @"https?://\S+", "，链接，");
        s = Regex.Replace(s, @"\b[\w.+-]+@[\w-]+\.[\w.]+\b", "，邮箱，");
        s = Regex.Replace(s, @"[Ａ-Ｚａ-ｚ０-９]", m => ((char)(m.Value[0] - 0xFEE0)).ToString());
        s = Regex.Replace(s, @"[·•‧∙・]", "，");
        s = s.Replace("……", "，").Replace("…", "，");
        s = s.Replace("——", "，").Replace("—", "，");
        s = Regex.Replace(s, @"\(\s*\)|（\s*）", "");
        s = Regex.Replace(s, @"[\[\]【】{}｛｝<>]", "，");
        s = Regex.Replace(s, @"[*#^~_`|/\\@&=+]", "");
        s = Regex.Replace(s, @"\s*\n\s*", "，");
        s = Regex.Replace(s, "，{2,}", "，");
        s = Regex.Replace(s, "。，", "。");
        s = Regex.Replace(s, @"([，。；！？、：])\1+", "$1");
        return s.Trim('，', '。', '，', ' ');
    }

    public static List<string> SplitSentences(string norm, int min = 18, int max = 90)
    {
        var result = new List<string>();
        if (string.IsNullOrWhiteSpace(norm)) return result;
        var sentences = Regex.Split(norm, @"(?<=[。！？；])")
            .Select(x => x.Trim())
            .Where(x => x.Length > 0)
            .ToList();

        var buffer = new StringBuilder();
        foreach (var sentence in sentences)
        {
            if (sentence.Length > max)
            {
                FlushBuffer(result, buffer, min);
                foreach (var piece in SplitLong(sentence, max)) result.Add(piece);
                continue;
            }
            if (buffer.Length + sentence.Length > max) FlushBuffer(result, buffer, min);
            buffer.Append(sentence);
            if (buffer.Length >= min) FlushBuffer(result, buffer, min);
        }
        FlushBuffer(result, buffer, 0);
        return result;
    }

    private static void FlushBuffer(List<string> result, StringBuilder buffer, int min)
    {
        if (buffer.Length >= min) { result.Add(buffer.ToString()); buffer.Clear(); }
        else if (buffer.Length > 0 && (result.Count == 0 || result[^1].Length + buffer.Length < 180))
        {
            if (result.Count > 0) result[^1] += buffer.ToString();
            else result.Add(buffer.ToString());
            buffer.Clear();
        }
        else if (buffer.Length > 0)
        {
            result.Add(buffer.ToString());
            buffer.Clear();
        }
    }

    private static IEnumerable<string> SplitLong(string sentence, int max)
    {
        var parts = Regex.Split(sentence, @"(?<=，)").Select(x => x.Trim()).Where(x => x.Length > 0);
        var buffer = new StringBuilder();
        foreach (var part in parts)
        {
            if (buffer.Length + part.Length > max && buffer.Length > 0)
            {
                yield return buffer.ToString();
                buffer.Clear();
            }
            buffer.Append(part);
        }
        if (buffer.Length > 0)
        {
            if (buffer.Length > max * 2)
            {
                for (var i = 0; i < buffer.Length; i += max)
                    yield return buffer.ToString(i, Math.Min(max, buffer.Length - i));
            }
            else yield return buffer.ToString();
        }
    }
}
