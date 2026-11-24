//! EPUB → 语音生成链路。
//!
//! mini 是"轻量入口"：检测到重量级主线（Full）就通过 CLI 契约委托真 TTS 模型；
//! 没装 Full 时用 Windows 自带 SAPI 语音（经系统 PowerShell 的 System.Speech 封装）
//! 逐段合成 WAV 并拼接成分章节有声书；两者都不可用则生成占位音并明确告知。

use crate::model::{detect_full_cli, Config, Progress, Reporter};
use crate::util::{self, J};
use crate::win32;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct Chapter { pub title: String, pub paragraphs: Vec<String> }
#[derive(Clone, Debug)]
pub struct Book { pub title: String, pub chapters: Vec<Chapter>, pub chars: usize }

pub struct Summary {
    pub chapters: usize,
    pub paragraphs: usize,
    pub seconds: f64,
    pub engine: String,
    pub out_dir: String,
    pub wav_bytes: u64,
}

// ---------------- 通用命令 ----------------
fn run_cmd(program: &str, args: &[&str]) -> (i32, String, String) {
    match Command::new(program).args(args).stdout(Stdio::piped())
        .stderr(Stdio::piped()).stdin(Stdio::null()).output() {
        Ok(o) => (o.status.code().unwrap_or(-1),
                  String::from_utf8_lossy(&o.stdout).into_owned(),
                  String::from_utf8_lossy(&o.stderr).into_owned()),
        Err(e) => (-99, String::new(), e.to_string()),
    }
}
fn tar_exe() -> String {
    let p = "C:\\Windows\\System32\\tar.exe";
    if win32::exists(p) { p.into() } else { "tar.exe".into() }
}
fn powershell_exe() -> String {
    let p = "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
    if win32::exists(p) { p.into() } else { "powershell.exe".into() }
}

// ---------------- 极简 XML / HTML 工具 ----------------
fn tags<'a>(s: &'a str, name: &str) -> Vec<&'a str> {
    // 关键：`<item` 不能误匹配 `<itemref`，所以标签名后必须是空白 / > / /
    let mut out = Vec::new();
    let needle = format!("<{}", name);
    let mut i = 0;
    while let Some(p) = s[i..].find(&needle) {
        let st = i + p;
        let nxt = s[st + needle.len()..].chars().next();
        if !matches!(nxt, Some(' ') | Some('>') | Some('/') | Some('\t') | Some('\r') | Some('\n')) {
            i = st + needle.len();
            continue;
        }
        if let Some(e) = s[st..].find('>') {
            out.push(&s[st..st + e + 1]);
            i = st + e + 1;
        } else { break; }
    }
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    for q in ['"', '\''] {
        let pat = format!("{}={}", name, q);
        if let Some(p) = tag.find(&pat) {
            let st = p + pat.len();
            if let Some(e) = tag[st..].find(q) {
                return Some(tag[st..st + e].to_string());
            }
        }
    }
    None
}

fn between(s: &str, open: &str, close: &str) -> Option<String> {
    let a = s.find(open)? + open.len();
    let b = s[a..].find(close)? + a;
    Some(s[a..b].to_string())
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'&' {
            if let Some(semi) = s[i..].find(';') {
                let end = i + semi;
                if end - i <= 10 {
                    let e = &s[i + 1..end];
                    let ch = match e {
                        "nbsp" => Some(' '), "amp" => Some('&'), "lt" => Some('<'),
                        "gt" => Some('>'), "quot" => Some('"'), "apos" | "#39" => Some('\''),
                        "mdash" => Some('—'), "ndash" => Some('–'), "hellip" => Some('…'),
                        _ => {
                            if let Some(h) = e.strip_prefix("#x").or_else(|| e.strip_prefix("#X")) {
                                u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
                            } else if let Some(d) = e.strip_prefix('#') {
                                d.parse::<u32>().ok().and_then(char::from_u32)
                            } else { None }
                        }
                    };
                    if let Some(c) = ch { out.push(c); i = end + 1; continue; }
                }
            }
        }
        if b[i] < 0x80 {
            out.push(b[i] as char);
            i += 1;
        } else {
            let len = if b[i] < 0xE0 { 2 } else if b[i] < 0xF0 { 3 } else { 4 };
            let end = (i + len).min(b.len());
            out.push_str(std::str::from_utf8(&b[i..end]).unwrap_or(""));
            i = end;
        }
    }
    out
}

fn strip_tags(html: &str) -> String {
    let mut s = html.to_string();
    // 丢弃脚本 / 样式整块
    for (o, c) in [("<script", "</script>"), ("<style", "</style>")] {
        loop {
            let lower = s.to_ascii_lowercase();
            let a = match lower.find(o) { Some(x) => x, None => break };
            let b = match lower[a..].find(c) { Some(x) => a + x + c.len(), None => a + o.len() };
            s.replace_range(a..b, " ");
        }
    }
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(e) = s[i..].find('>') {
                let tag = s[i..i + e + 1].to_ascii_lowercase();
                if tag.starts_with("</p") || tag.starts_with("<br") || tag.starts_with("</div")
                    || tag.starts_with("</h") || tag.starts_with("<p") || tag.starts_with("<div") {
                    out.push('\n');
                }
                i += e + 1;
                continue;
            }
        }
        if bytes[i] < 0x80 {
            out.push(bytes[i] as char);
            i += 1;
        } else {
            let len = if bytes[i] < 0xE0 { 2 } else if bytes[i] < 0xF0 { 3 } else { 4 };
            let end = (i + len).min(bytes.len());
            out.push_str(std::str::from_utf8(&bytes[i..end]).unwrap_or(""));
            i = end;
        }
    }
    decode_entities(&out)
}

fn split_sentences(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for line in raw.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() { continue; }
        for ch in line.chars() {
            cur.push(ch);
            if "。！？!?；;".contains(ch) {
                let t = cur.trim().to_string();
                if t.chars().count() >= 2 { out.push(t); }
                cur.clear();
            } else if cur.chars().count() >= 110 {
                let t = cur.trim().to_string();
                if !t.is_empty() { out.push(t); }
                cur.clear();
            }
        }
        let t = cur.trim().to_string();
        if !t.is_empty() { cur = t; cur.push('。'); }
    }
    let t = cur.trim().to_string();
    if t.chars().count() >= 2 { out.push(t); }
    // 合并过短片段，减少 TTS 调用次数
    let mut merged: Vec<String> = Vec::new();
    for s in out {
        if let Some(last) = merged.last_mut() {
            if last.chars().count() < 12 { last.push_str(&s); continue; }
        }
        merged.push(s);
    }
    merged
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes(); let mut out = Vec::new(); let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let h = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("");
            if let Ok(v) = u8::from_str_radix(h, 16) { out.push(v); i += 3; continue; }
        }
        out.push(b[i]); i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn sanitize(s: &str) -> String {
    let bad = ['\\', '/', ':', '*', '?', '"', '<', '>', '|', '\r', '\n'];
    s.chars().map(|c| if bad.contains(&c) { '_' } else { c })
        .take(48).collect::<String>().trim().to_string()
}

// ---------------- EPUB 解析 ----------------
pub fn parse_epub(path: &str, rp: &dyn Reporter) -> Result<Book, String> {
    if !win32::exists(path) { return Err(format!("找不到 EPUB：{}", path)); }
    if !path.to_ascii_lowercase().ends_with(".epub") {
        return Err("文件扩展名不是 .epub".into());
    }
    let tmp = win32::temp_dir().join(format!("bctl_epub_{}_{}",
        std::process::id(), win32::tick_ms()));
    fs::create_dir_all(&tmp).map_err(|e| format!("创建临时目录失败：{}", e))?;
    let (code, _so, se) = run_cmd(&tar_exe(),
        &["-xf", path, "-C", &tmp.to_string_lossy()]);
    if code != 0 { return Err(format!("EPUB 解压失败（tar 退出码 {}）：{}", code, se.trim())); }

    let cont = fs::read_to_string(tmp.join("META-INF/container.xml")).unwrap_or_default();
    let opf_rel = tags(&cont, "rootfile").first()
        .and_then(|t| attr(t, "full-path"))
        .unwrap_or_else(|| {
            // 兜底：递归找第一个 .opf
            find_first(&tmp, "opf").unwrap_or_else(|| "content.opf".into())
        });
    let opf_path = tmp.join(url_decode(&opf_rel));
    let opf = fs::read_to_string(&opf_path)
        .map_err(|e| format!("读取 OPF 失败（{}）：{}", opf_path.display(), e))?;
    let base = opf_path.parent().unwrap_or(&tmp).to_path_buf();

    let title = between(&opf, "<dc:title", "</dc:title>")
        .map(|t| {
            // 去掉可能存在的属性段（如 <dc:title id="t1">标题</dc:title>）
            let body = t.find('>').map(|i| &t[i + 1..]).unwrap_or(&t);
            strip_tags(body).trim().to_string()
        })
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "未命名电子书".into());

    // manifest: id -> (href, media-type)
    let mut manifest: HashMap<String, (String, String)> = HashMap::new();
    for t in tags(&opf, "item") {
        if let (Some(id), Some(href)) = (attr(&t, "id"), attr(&t, "href")) {
            let mt = attr(&t, "media-type").unwrap_or_default();
            manifest.insert(id, (href, mt));
        }
    }
    // spine 顺序
    let spine: Vec<String> = tags(&opf, "itemref").iter()
        .filter_map(|t| attr(t, "idref")).collect();

    // NCX 标题映射：content src -> navLabel text
    let mut titles: HashMap<String, String> = HashMap::new();
    let ncx_href = manifest.values().find(|(_, mt)| mt.contains("ncx"))
        .map(|(h, _)| h.clone());
    if let Some(h) = ncx_href {
        if let Ok(ncx) = fs::read_to_string(base.join(url_decode(&h))) {
            for np in ncx.split("<navPoint").skip(1) {
                let label = between(np, "<text>", "</text>").map(|x| strip_tags(&x));
                let src = tags(np, "content").first().and_then(|t| attr(t, "src"));
                if let (Some(l), Some(s)) = (label, src) {
                    titles.insert(s.split('#').next().unwrap_or("").to_string(), l.trim().to_string());
                }
            }
        }
    }

    let mut chapters = Vec::new();
    let mut chars = 0usize;
    for (idx, idref) in spine.iter().enumerate() {
        let (href, mt) = match manifest.get(idref) { Some(x) => x.clone(), None => continue };
        if mt.contains("ncx") || mt.starts_with("image/") || mt.contains("css") { continue; }
        let file = base.join(url_decode(href.split('#').next().unwrap_or("")));
        let html = match fs::read_to_string(&file) { Ok(x) => x, Err(_) => continue };
        let paras = split_sentences(&strip_tags(&html));
        if paras.is_empty() { continue; }
        chars += paras.iter().map(|p| p.chars().count()).sum::<usize>();
        let key = href.split('#').next().unwrap_or("").to_string();
        let ch_title = titles.get(&key).cloned()
            .or_else(|| between(&html, "<title>", "</title>").map(|t| strip_tags(&t)))
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| format!("第 {} 章", idx + 1));
        chapters.push(Chapter { title: ch_title, paragraphs: paras });
    }
    if chapters.is_empty() {
        return Err("EPUB 里没有可朗读的正文（spine 为空或内容无法解析）".into());
    }
    rp.log(&format!("解析完成：《{}》· {} 章 · {} 字", title, chapters.len(), chars));
    let _ = fs::remove_dir_all(&tmp);
    Ok(Book { title, chapters, chars })
}

fn find_first(dir: &Path, ext: &str) -> Option<String> {
    for e in fs::read_dir(dir).ok()? {
        let p = e.ok()?.path();
        if p.is_dir() {
            if let Some(rel) = find_first(&p, ext) {
                return Some(format!("{}/{}", p.file_name()?.to_string_lossy(), rel));
            }
        } else if p.extension().map(|x| x.to_string_lossy().eq_ignore_ascii_case(ext)).unwrap_or(false) {
            return Some(p.file_name()?.to_string_lossy().into_owned());
        }
    }
    None
}

// ---------------- WAV ----------------
fn write_wav_pcm(path: &Path, channels: u16, rate: u32, bits: u16, data: &[u8]) -> std::io::Result<()> {
    let byte_rate = rate * channels as u32 * (bits as u32 / 8);
    let block = channels * (bits / 8);
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());       // PCM
    h.extend_from_slice(&channels.to_le_bytes());
    h.extend_from_slice(&rate.to_le_bytes());
    h.extend_from_slice(&byte_rate.to_le_bytes());
    h.extend_from_slice(&block.to_le_bytes());
    h.extend_from_slice(&bits.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&(data.len() as u32).to_le_bytes());
    let mut f = fs::File::create(path)?;
    f.write_all(&h)?; f.write_all(data)?;
    Ok(())
}

struct Wav { channels: u16, rate: u32, bits: u16, data: Vec<u8> }
fn read_wav(path: &Path) -> Result<Wav, String> {
    let b = fs::read(path).map_err(|e| e.to_string())?;
    if b.len() < 44 || &b[0..4] != b"RIFF" { return Err("不是有效 WAV".into()); }
    let channels = u16::from_le_bytes([b[22], b[23]]);
    let rate = u32::from_le_bytes([b[24], b[25], b[26], b[27]]);
    let bits = u16::from_le_bytes([b[34], b[35]]);
    let mut off = 12usize;
    let mut data = Vec::new();
    while off + 8 <= b.len() {
        let id = &b[off..off + 4];
        let sz = u32::from_le_bytes([b[off + 4], b[off + 5], b[off + 6], b[off + 7]]) as usize;
        let st = off + 8;
        if id == b"data" {
            let end = (st + sz).min(b.len());
            data.extend_from_slice(&b[st..end]);
            break;
        }
        off = st + sz + (sz % 2);
    }
    Ok(Wav { channels, rate, bits, data })
}

fn concat_wavs(inputs: &[PathBuf], out: &Path) -> Result<(u64, f64), String> {
    let mut base: Option<Wav> = None;
    for p in inputs {
        match read_wav(p) {
            Ok(w) => {
                if let Some(b) = base.as_mut() { b.data.extend_from_slice(&w.data); }
                else { base = Some(w); }
            }
            Err(e) => return Err(format!("读取 {} 失败：{}", p.display(), e)),
        }
    }
    let w = base.ok_or("没有可拼接的音频")?;
    let bytes_per_sec = (w.rate * w.channels as u32 * (w.bits as u32 / 8)) as f64;
    let seconds = if bytes_per_sec > 0.0 { w.data.len() as f64 / bytes_per_sec } else { 0.0 };
    write_wav_pcm(out, w.channels, w.rate, w.bits, &w.data).map_err(|e| e.to_string())?;
    Ok((w.data.len() as u64 + 44, seconds))
}

fn placeholder_wav(path: &Path) -> Result<(), String> {
    // 0.4 秒 16kHz 16bit 单声道低音量正弦，作为"系统语音不可用"时的明确占位
    let rate = 16000u32; let n = 6400usize;
    let mut data = Vec::with_capacity(n * 2);
    for i in 0..n {
        let v = ((i as f64 * 2.0 * std::f64::consts::PI * 220.0 / rate as f64).sin()
                 * 1200.0) as i16;
        data.extend_from_slice(&v.to_le_bytes());
    }
    write_wav_pcm(path, 1, rate, 16, &data).map_err(|e| e.to_string())
}

// ---------------- SAPI（系统语音）----------------
fn synth_script() -> String {
    // 纯 ASCII 内容，避免 PowerShell 5.1 按 ANSI 解析中文脚本时乱码。
    r#"param([string]$TextFile,[string]$OutFile,[int]$Rate)
$ErrorActionPreference='Stop'
try {
  Add-Type -AssemblyName System.Speech
  $text=[IO.File]::ReadAllText($TextFile,[Text.Encoding]::UTF8)
  if ([string]::IsNullOrWhiteSpace($text)) { exit 2 }
  $s=New-Object System.Speech.Synthesis.SpeechSynthesizer
  $s.Rate=$Rate
  $s.SetOutputToWaveFile($OutFile)
  $s.Speak($text)
  $s.Dispose()
  exit 0
} catch { exit 1 }
"#.to_string()
}

fn synth_sapi(text: &str, wav: &Path, cfg: &Config, tmp: &Path,
              ps1: &Path, rp: &dyn Reporter, warned: &mut bool) -> Result<(), String> {
    let txt = tmp.join("seg.txt");
    fs::write(&txt, text.as_bytes()).map_err(|e| e.to_string())?;
    let rate = ((cfg.rate - 1.0) * 5.0).round().clamp(-10.0, 10.0) as i32;
    let rate_s = rate.to_string();
    let (code, _so, _se) = run_cmd(&powershell_exe(), &[
        "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
        &ps1.to_string_lossy(), "-TextFile", &txt.to_string_lossy(),
        "-OutFile", &wav.to_string_lossy(), "-Rate", &rate_s,
    ]);
    let ok = code == 0 && fs::metadata(wav).map(|m| m.len() > 100).unwrap_or(false);
    if ok { return Ok(()); }
    if !*warned {
        rp.log("⚠ Windows 系统语音不可用（缺少 System.Speech 或中文语音包），后续段落改用占位音；");
        rp.log("  安装重量级主线或下载模型后即可生成真人语音。");
        *warned = true;
    }
    placeholder_wav(wav)
}

// ---------------- 委托重量级主线 ----------------

/// 递归统计产物目录字节数。Full 主线的音频在多层子目录里，只统计一层会得到
/// 数据库文件的大小（曾误报 57344 B），必须递归。
fn dir_size_recursive(dir: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { total += dir_size_recursive(&p); }
            else if p.is_file() { total += e.metadata().map(|m| m.len()).unwrap_or(0); }
        }
    }
    total
}

/// 找 Full 主线自己写的 manifest.json（带 totals 字段），优先用事件流给出的
/// result_path，找不到再递归扫输出目录。Mini 自己写的 manifest 没有 totals，
/// 不会误匹配。
fn find_full_manifest(out: &Path, result_path: &str) -> Option<J> {
    let load = |p: &Path| -> Option<J> {
        let s = fs::read_to_string(p).ok()?;
        let j = util::parse(&s).ok()?;
        if j.get("totals").is_some() { Some(j) } else { None }
    };
    if !result_path.is_empty() {
        if let Some(j) = load(&Path::new(result_path).join("manifest.json")) { return Some(j); }
    }
    fn walk(dir: &Path, depth: usize, load: &dyn Fn(&Path) -> Option<J>) -> Option<J> {
        if depth > 5 { return None; }
        for e in fs::read_dir(dir).ok()?.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Some(j) = walk(&p, depth + 1, load) { return Some(j); }
            } else if p.file_name().map(|n| n == "manifest.json").unwrap_or(false) {
                if let Some(j) = load(&p) { return Some(j); }
            }
        }
        None
    }
    walk(out, 0, &load)
}

/// 把 Full 主线 manifest 的 chapters[] 转成 Mini manifest 的章节行格式。
fn full_chapter_rows(m: &J) -> Vec<J> {
    let Some(chs) = m.get("chapters").and_then(J::as_arr) else { return Vec::new(); };
    chs.iter().map(|c| util::obj(vec![
        ("index", util::i(c.int("index"))),
        ("title", util::s(&c.str("title"))),
        ("paragraphs", util::i(0)),
        ("seconds", util::n(c.num("seconds"))),
        ("file", util::s(&c.str("audio"))),
    ])).collect()
}

fn delegate_full(cli: &str, epub: &str, cfg: &Config, out: &Path,
                 rp: &dyn Reporter) -> Result<Summary, String> {
    rp.log("检测到重量级主线（Full），通过 CLI 契约委托真 TTS 模型生成…");
    let threads = cfg.cpu_threads.to_string();
    let gpu = cfg.gpu_index.max(0).to_string();
    let mut args: Vec<String> = vec![
        "run".into(), "--input".into(), epub.into(),
        "--device".into(), cfg.device.clone(),
        "--cpu-threads".into(), threads,
    ];
    if cfg.gpu_index >= 0 { args.push("--gpu".into()); args.push(gpu); }
    // mini 的 engine=auto 时让主线自己决策；显式引擎名透传
    if !cfg.engine.is_empty() && cfg.engine != "auto" && cfg.engine != "sapi" {
        args.push("--engine".into()); args.push(cfg.engine.clone());
    }
    let mut child = Command::new(cli).args(&args)
        .env("BOOKCTL_WORKSPACE", out)
        .stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null())
        .spawn().map_err(|e| format!("启动重量级 CLI 失败：{}", e))?;

    // 边读事件流边汇总真实结果。Full stdout 是「NDJSON 事件行 + 最后一行 JSON 信封」，
    // 两者都按 UTF-8 解析（实测 Full 输出为干净 UTF-8）；不再把整行 JSON 原样塞进日志。
    let mut engine = String::new();
    let mut chapters = 0usize;
    let mut paragraphs = 0usize;   // normalize_done 的 segments（朗读段数）
    let mut seconds = 0.0f64;
    let mut result_path = String::new();
    let mut status = String::new();
    let mut logs = 0usize;

    if let Some(so) = child.stdout.take() {
        for line in BufReader::new(so).lines() {
            let l = match line { Ok(x) => x, Err(_) => continue };
            let t = l.trim();
            if t.is_empty() { continue; }
            let j = match util::parse(t) {
                Ok(x) => x,
                Err(_) => {
                    let short: String = t.chars().take(180).collect();
                    if !short.is_empty() { rp.log(&short); }
                    continue;
                }
            };

            // 最后一行：{"ok":true,...,"data":{...}} —— 提取真实汇总，不打进日志
            if j.get("ok").is_some() {
                if let Some(d) = j.get("data") {
                    if !d.str("engine").is_empty() { engine = d.str("engine"); }
                    let cd = d.int("chapters_done") as usize;
                    if cd > 0 { chapters = cd; }
                    let au = d.num("audio_seconds");
                    if au > 0.0 { seconds = au; }
                    if !d.str("status").is_empty() { status = d.str("status"); }
                    if !d.str("result_path").is_empty() { result_path = d.str("result_path"); }
                }
                continue;
            }

            let stage = j.str("stage");
            let code = j.str("code");
            let message = j.str("message");
            let data = j.get("data");

            match code.as_str() {
                "split_done" => {
                    if let Some(d) = data {
                        if chapters == 0 { chapters = d.int("included") as usize; }
                    }
                }
                "normalize_done" => {
                    if let Some(d) = data { paragraphs = d.int("segments") as usize; }
                }
                "segment_done" => {
                    // 驱动 GUI 进度条（和 Mini 自带 SAPI 路径一致的契约）
                    if let Some(d) = data {
                        let done = d.num("chars_done");
                        let total = d.num("chars_total").max(1.0);
                        rp.progress(&Progress {
                            state: "downloading".into(), source: "TTS 合成".into(),
                            downloaded: done as u64, total: total as u64,
                            pct: ((done * 100.0 / total).min(100.0)) as u32,
                            message: message.clone(), ..Progress::idle()
                        });
                    }
                }
                "job_completed" | "m4b" => {
                    if let Some(d) = data {
                        if !d.str("result_path").is_empty() { result_path = d.str("result_path"); }
                    }
                }
                _ => {}
            }

            // 人类可读日志：带 ASCII 阶段标签 + message；即使宿主终端把中文按
            // GBK 显示，阶段码仍可读。上限 200 行，防止超长书籍刷爆日志框。
            if logs < 200 && !message.is_empty() {
                let short: String = message.chars().take(180).collect();
                let tag = if stage.is_empty() { "Full" } else { &stage };
                rp.log(&format!("[{}] {}", tag, short));
                logs += 1;
            }
        }
    }
    let st = child.wait().map_err(|e| e.to_string())?;
    if !st.success() { return Err(format!("重量级 CLI 退出码 {:?}", st.code())); }

    // 用 Full 自己写的 manifest 覆盖/补齐（totals.audio_seconds 是章节秒数之和，
    // 比信封里的 audio_seconds 更可信；engine.name 是真实引擎，不写死 sherpa）
    let mut chapter_rows: Vec<J> = Vec::new();
    if let Some(m) = find_full_manifest(out, &result_path) {
        if let Some(e) = m.get("engine") {
            if !e.str("name").is_empty() { engine = e.str("name"); }
        }
        if !m.str("status").is_empty() { status = m.str("status"); }
        if let Some(t) = m.get("totals") {
            let c = t.int("chapters") as usize;
            if c > 0 { chapters = c; }
            let s = t.num("audio_seconds");
            if s > 0.0 { seconds = s; }
        }
        chapter_rows = full_chapter_rows(&m);
        if paragraphs == 0 {
            paragraphs = chapter_rows.len(); // 兜底，避免 0 段
        }
    }
    if status.is_empty() { status = "completed".into(); }
    if engine.is_empty() {
        engine = if cfg.engine.is_empty() || cfg.engine == "auto" { "auto".into() }
                 else { cfg.engine.clone() };
    }
    // 关键修复：engine 标签按真实引擎动态生成，不再硬编码 "sherpa-via-Full"
    let label = format!("{}-via-Full", engine);
    let wav_bytes = dir_size_recursive(out);

    write_manifest(out, &chapter_rows, seconds, &label, cfg, chapters, paragraphs)?;
    rp.progress(&Progress {
        state: "done".into(), source: "生成完成".into(), pct: 100,
        message: format!("委托完成：{} 章 / {:.1}s · 引擎 {} → {}",
                         chapters, seconds, engine, out.to_string_lossy()),
        ..Progress::idle()
    });
    let _ = status;
    Ok(Summary {
        chapters, paragraphs, seconds,
        engine: label, out_dir: out.to_string_lossy().into_owned(),
        wav_bytes,
    })
}

// ---------------- 生成主流程 ----------------
pub fn default_out_dir(epub: &str) -> String {
    let name = Path::new(epub).file_stem()
        .map(|x| sanitize(&x.to_string_lossy())).unwrap_or_else(|| "有声书".into());
    format!("{}\\output\\{}", Config::dir(), name)
}

pub fn generate_book(epub: &str, cfg: &Config, rp: &dyn Reporter,
                     outdir: Option<&str>) -> Result<Summary, String> {
    let out_s = outdir.map(|x| x.to_string()).unwrap_or_else(|| default_out_dir(epub));
    let out = PathBuf::from(&out_s);
    win32::ensure_dir(&out_s).map_err(|e| format!("创建输出目录失败：{}", e))?;

    let book = parse_epub(epub, rp)?;
    if let Some(cli) = detect_full_cli(cfg) {
        return delegate_full(&cli, epub, cfg, &out, rp);
    }

    // 轻量路径：系统 SAPI 逐段合成 → 分章拼接
    rp.log("未检测到重量级主线，使用 Windows 自带语音（SAPI）生成；");
    rp.log("如需真人级音质，请在「模型下载」里装好模型并安装 Full 版。");
    let tmp = out.join("_work");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(tmp.join("segs")).map_err(|e| e.to_string())?;
    let ch_dir = out.join("chapters");
    fs::create_dir_all(&ch_dir).map_err(|e| e.to_string())?;
    let ps1 = tmp.join("synth.ps1");
    fs::write(&ps1, synth_script().as_bytes()).map_err(|e| e.to_string())?;

    let mut warned = false;
    let total_paras: usize = book.chapters.iter().map(|c| c.paragraphs.len()).sum();
    let mut done = 0usize;
    let mut chapter_rows: Vec<J> = Vec::new();
    let mut total_seconds = 0.0f64;
    let mut total_bytes = 0u64;

    for (ci, ch) in book.chapters.iter().enumerate() {
        let mut segs = Vec::new();
        for (pi, para) in ch.paragraphs.iter().enumerate() {
            let wav = tmp.join("segs").join(format!("c{:03}_p{:04}.wav", ci, pi));
            synth_sapi(para, &wav, cfg, &tmp, &ps1, rp, &mut warned)?;
            segs.push(wav);
            done += 1;
            if done % 5 == 0 || done == total_paras {
                rp.progress(&Progress {
                    state: "downloading".into(), source: "语音合成".into(),
                    downloaded: done as u64, total: total_paras as u64,
                    pct: (done * 100 / total_paras.max(1)) as u32,
                    message: format!("合成进度 {}/{} · 第 {} 章《{}》", done, total_paras, ci + 1, ch.title),
                    ..Progress::idle()
                });
            }
        }
        let cwav = ch_dir.join(format!("{:02}_{}.wav", ci + 1, sanitize(&ch.title)));
        let (bytes, secs) = concat_wavs(&segs, &cwav)?;
        total_bytes += bytes; total_seconds += secs;
        chapter_rows.push(util::obj(vec![
            ("index", util::i((ci + 1) as i64)),
            ("title", util::s(&ch.title)),
            ("paragraphs", util::i(ch.paragraphs.len() as i64)),
            ("seconds", util::n((secs * 100.0).round() / 100.0)),
            ("file", util::s(&format!("chapters/{}", cwav.file_name().unwrap().to_string_lossy()))),
        ]));
        rp.log(&format!("  ✓ 第 {} 章《{}》 {} 段 / {:.1}s", ci + 1, ch.title, ch.paragraphs.len(), secs));
    }

    // 整本合并
    let all: Vec<PathBuf> = (0..book.chapters.len())
        .map(|ci| ch_dir.join(format!("{:02}_{}.wav", ci + 1, sanitize(&book.chapters[ci].title))))
        .filter(|p| p.is_file()).collect();
    if !all.is_empty() {
        let (b, s) = concat_wavs(&all, &out.join("book.wav"))?;
        total_bytes += b; let _ = s;
    }
    let _ = fs::remove_dir_all(&tmp);

    write_manifest(&out, &chapter_rows, total_seconds,
                   if warned { "sapi-fallback-placeholder" } else { "sapi" },
                   cfg, book.chapters.len(), total_paras)?;
    rp.progress(&Progress {
        state: "done".into(), source: "生成完成".into(), pct: 100,
        message: format!("完成：{} 章 / {:.1} 秒 → {}", book.chapters.len(), total_seconds, out_s),
        ..Progress::idle()
    });
    Ok(Summary {
        chapters: book.chapters.len(), paragraphs: total_paras,
        seconds: total_seconds,
        engine: if warned { "sapi-fallback-placeholder".into() } else { "sapi".into() },
        out_dir: out_s, wav_bytes: total_bytes,
    })
}

fn write_manifest(out: &Path, chapters: &[J], seconds: f64, engine: &str,
                  cfg: &Config, n_ch: usize, n_para: usize) -> Result<(), String> {
    let m = util::obj(vec![
        ("product", util::s("本地朗读工厂 Mini")),
        ("edition", util::s("Mini")),
        ("status", util::s("completed")),
        ("engine", util::s(engine)),
        ("device", util::s(&cfg.device)),
        ("gpu_index", util::i(cfg.gpu_index as i64)),
        ("cpu_threads", util::i(cfg.cpu_threads as i64)),
        ("model_root", util::s(&cfg.model_root)),
        ("chapters", util::arr(chapters.to_vec())),
        ("chapter_count", util::i(n_ch as i64)),
        ("paragraph_count", util::i(n_para as i64)),
        ("total_seconds", util::n((seconds * 100.0).round() / 100.0)),
    ]);
    let mut f = fs::File::create(out.join("manifest.json")).map_err(|e| e.to_string())?;
    f.write_all(util::pretty(&m).as_bytes()).map_err(|e| e.to_string())?;
    let mut t = fs::File::create(out.join("chapters.txt")).map_err(|e| e.to_string())?;
    for c in chapters {
        let _ = writeln!(t, "{} | {} | {}s",
            c.get("index").and_then(J::as_num).unwrap_or(0.0) as i64,
            c.str("title"),
            c.get("seconds").and_then(J::as_num).unwrap_or(0.0));
    }
    Ok(())
}
