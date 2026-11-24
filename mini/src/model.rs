//! 运行配置 / 内置多下载源 / 选盘安装 / 下载管理器。
//!
//! 用户痛点：以前要自己上网找镜像一个一个试。这里把"多个源 + 自动测速选源 +
//! 失败自动切换 + 进度 + 选盘安装"全部内置，界面只需点按钮。

use crate::util::{self, J};
use crate::win32;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const APP_DIR: &str = "本地朗读工厂";
pub const EDITION: &str = "Mini";

// ============================ 运行配置 ============================

#[derive(Clone, Debug)]
pub struct Config {
    pub device: String,          // auto | cpu | gpu | hybrid
    pub gpu_index: i32,
    pub cpu_threads: u32,
    pub voice: String,
    pub rate: f64,
    pub engine: String,          // auto | sapi | sherpa | piper
    pub model_root: String,      // 用户选的盘 / 路径
    pub preferred_source: String,
    pub custom_source: String,
    pub curl_path: String,
    pub full_cli: String,        // 重量级主线 CLI（可选，存在则委托生成）
    pub edition: String,
}

impl Config {
    pub fn defaults() -> Self {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as u32).unwrap_or(4);
        Self {
            device: "auto".into(),
            gpu_index: -1,
            cpu_threads: threads,
            voice: "auto".into(),
            rate: 1.0,
            engine: "auto".into(),
            model_root: String::new(),   // 空 = 首次运行时让用户选盘
            preferred_source: "auto".into(),
            custom_source: String::new(),
            curl_path: String::new(),
            full_cli: String::new(),
            edition: EDITION.into(),
        }
    }

    pub fn dir() -> String { format!("{}\\{}", win32::local_appdata(), APP_DIR) }
    pub fn path() -> String { format!("{}\\config.json", Self::dir()) }

    pub fn load() -> Self {
        let p = Self::path();
        match fs::read_to_string(&p) {
            Ok(t) => match util::parse(&t) {
                Ok(j) => Self::from_j(&j),
                Err(e) => { eprintln!("config.json 解析失败（用默认值）：{}", e); Self::defaults() }
            },
            Err(_) => Self::defaults(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::dir();
        win32::ensure_dir(&dir).map_err(|e| e.to_string())?;
        let mut f = fs::File::create(Self::path()).map_err(|e| e.to_string())?;
        f.write_all(util::pretty(&self.to_j()).as_bytes()).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn import(path: &str) -> Result<Self, String> {
        let t = fs::read_to_string(path).map_err(|e| format!("读取配置失败：{}", e))?;
        let j = util::parse(&t).map_err(|e| format!("配置不是合法 JSON：{}", e))?;
        let mut c = Self::from_j(&j);
        c.edition = EDITION.into();
        c.save()?;
        Ok(c)
    }

    pub fn to_j(&self) -> J {
        util::obj(vec![
            ("edition", util::s(&self.edition)),
            ("device", util::s(&self.device)),
            ("gpu_index", util::i(self.gpu_index as i64)),
            ("cpu_threads", util::i(self.cpu_threads as i64)),
            ("voice", util::s(&self.voice)),
            ("rate", util::n(self.rate)),
            ("engine", util::s(&self.engine)),
            ("model_root", util::s(&self.model_root)),
            ("preferred_source", util::s(&self.preferred_source)),
            ("custom_source", util::s(&self.custom_source)),
            ("curl_path", util::s(&self.curl_path)),
            ("full_cli", util::s(&self.full_cli)),
        ])
    }

    pub fn from_j(j: &J) -> Self {
        let d = Self::defaults();
        Self {
            device: nonempty(j.str("device"), &d.device),
            gpu_index: j.get("gpu_index").and_then(J::as_num).map(|x| x as i32).unwrap_or(d.gpu_index),
            cpu_threads: {
                let v = j.get("cpu_threads").and_then(J::as_num).unwrap_or(0.0) as u32;
                if v == 0 { d.cpu_threads } else { v }
            },
            voice: nonempty(j.str("voice"), &d.voice),
            rate: { let v = j.get("rate").and_then(J::as_num).unwrap_or(0.0); if v <= 0.0 { 1.0 } else { v } },
            engine: nonempty(j.str("engine"), &d.engine),
            model_root: j.str("model_root"),
            preferred_source: nonempty(j.str("preferred_source"), "auto"),
            custom_source: j.str("custom_source"),
            curl_path: j.str("curl_path"),
            full_cli: j.str("full_cli"),
            edition: EDITION.into(),
        }
    }

    pub fn device_display(&self) -> String {
        match self.device.as_str() {
            "cpu" => format!("CPU {} 线程", self.cpu_threads),
            "gpu" => format!("GPU {}", self.gpu_index.max(0)),
            "hybrid" => "GPU+CPU 混合".into(),
            _ => "自动（推荐）".into(),
        }
    }
}
fn nonempty(s: String, d: &str) -> String { if s.trim().is_empty() { d.into() } else { s } }

/// 检测重量级主线是否已安装（注册表 / 同目录），用于"委托真 TTS 模型"生成。
pub fn detect_full_cli(cfg: &Config) -> Option<String> {
    if !cfg.full_cli.is_empty() && win32::exists(&cfg.full_cli) {
        return Some(cfg.full_cli.clone());
    }
    let reg = win32::reg_get_str(win32::HKEY_CURRENT_USER,
        &format!("Software\\{}\\Full", APP_DIR), "CliPath");
    if let Some(p) = reg { if win32::exists(&p) { return Some(p); } }
    // 也检查 mini 旁边是否有 Full 版 tools 目录（同一 release 包里并存的情况）
    let near = win32::exe_dir().join("tools").join("BookCtl.Cli").join("bookctl.exe");
    if near.is_file() { return Some(near.to_string_lossy().into_owned()); }
    None
}

// ============================ 内置多下载源 ============================

#[derive(Clone, Debug)]
pub struct Source {
    pub id: &'static str,
    pub name: &'static str,
    pub note: &'static str,
    pub latency_ms: f64,   // -1 = 未测速；-2 = 不可达
}

#[derive(Clone, Debug)]
pub struct ModelEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub engine: &'static str,
    pub device: &'static str,
    pub size_mb: u64,
    pub target: &'static str,
    pub archive: &'static str,   // tar.bz2 | zip | dir
    pub urls: Vec<(&'static str, &'static str)>,  // (source_id, full_url)
}

const GH_PATH: &str = "k2-fsa/sherpa-onnx/releases/download/tts-models/";
const FANCHEN: &str = "vits-zh-hf-fanchen-C.tar.bz2";
const AISHELL: &str = "vits-zh-aishell3.tar.bz2";

pub fn sources() -> Vec<Source> {
    vec![
        Source { id: "github", name: "GitHub 官方直连", note: "源站；国内可能超时", latency_ms: -1.0 },
        Source { id: "gh-proxy", name: "gh-proxy.com 镜像", note: "GitHub Release 常用代理", latency_ms: -1.0 },
        Source { id: "ghfast", name: "ghfast.top 镜像", note: "备选线路 A", latency_ms: -1.0 },
        Source { id: "ghproxy-net", name: "ghproxy.net 镜像", note: "备选线路 B", latency_ms: -1.0 },
        Source { id: "gitmirror", name: "hub.gitmirror.com 镜像", note: "备选线路 C", latency_ms: -1.0 },
        Source { id: "hf-mirror", name: "hf-mirror.com（HF 模型）", note: "HuggingFace 国内镜像", latency_ms: -1.0 },
        Source { id: "huggingface", name: "HuggingFace 官方", note: "HF 源站；国内可能超时", latency_ms: -1.0 },
        Source { id: "custom", name: "自定义源（配置里填）", note: "用户自己指定的镜像前缀", latency_ms: -1.0 },
    ]
}

pub fn source_name(id: &str) -> String {
    sources().into_iter().find(|s| s.id == id)
        .map(|s| s.name.to_string()).unwrap_or_else(|| id.to_string())
}

fn gh_urls(file: &str) -> Vec<(&'static str, &'static str)> {
    // 用 Box::leak 把拼接出的 URL 变成 &'static str，保持 catalog 结构简洁。
    let base = format!("https://github.com/{}", GH_PATH);
    let official: &'static str = Box::leak(format!("{}{}", base, file).into_boxed_str());
    // gh-proxy 的完整格式是 https://gh-proxy.com/https://github.com/...
    let p1b: &'static str = Box::leak(format!("https://gh-proxy.com/https://github.com/{}{}", GH_PATH, file).into_boxed_str());
    let p2: &'static str = Box::leak(format!("https://ghfast.top/https://github.com/{}{}", GH_PATH, file).into_boxed_str());
    let p3: &'static str = Box::leak(format!("https://ghproxy.net/https://github.com/{}{}", GH_PATH, file).into_boxed_str());
    let p4: &'static str = Box::leak(format!("https://hub.gitmirror.com/https://github.com/{}{}", GH_PATH, file).into_boxed_str());
    vec![
        ("github", official),
        ("gh-proxy", p1b),
        ("ghfast", p2),
        ("ghproxy-net", p3),
        ("gitmirror", p4),
    ]
}

pub fn models() -> Vec<ModelEntry> {
    vec![
        ModelEntry {
            id: "mock", name: "内置校样引擎（不下载，占位音）",
            engine: "mock", device: "cpu", size_mb: 0,
            target: "mock", archive: "dir", urls: vec![],
        },
        ModelEntry {
            id: "sherpa-vits-zh-fanchen",
            name: "sherpa-onnx · 中文 VITS（Fanchen，多音色）",
            engine: "sherpa", device: "cpu", size_mb: 130,
            target: "sherpa-vits-zh-fanchen", archive: "tar.bz2",
            urls: gh_urls(FANCHEN),
        },
        ModelEntry {
            id: "sherpa-vits-zh-aishell3",
            name: "sherpa-onnx · 中文 VITS（AISHELL-3，174 音色）",
            engine: "sherpa", device: "cpu", size_mb: 60,
            target: "sherpa-vits-zh-aishell3", archive: "tar.bz2",
            urls: gh_urls(AISHELL),
        },
        ModelEntry {
            id: "piper-zh-huayan",
            name: "Piper · 中文女声 huayan（medium）",
            engine: "piper", device: "cpu", size_mb: 65,
            target: "piper-zh-huayan", archive: "tar.bz2",
            urls: {
                let base = "rhasspy/piper-voices/resolve/main/zh/zh_CN/huayan/medium/";
                let u1: &'static str = Box::leak(format!("https://huggingface.co/{}zh_CN-huayan-medium.onnx", base).into_boxed_str());
                let u2: &'static str = Box::leak(format!("https://hf-mirror.com/{}zh_CN-huayan-medium.onnx", base).into_boxed_str());
                vec![("huggingface", u1), ("hf-mirror", u2)]
            },
        },
    ]
}

pub fn find_model(id: &str) -> Option<ModelEntry> {
    models().into_iter().find(|m| m.id == id)
}

/// 用户自定义源：配置里填完整前缀，替换官方 github 前缀。
pub fn effective_urls(entry: &ModelEntry, cfg: &Config) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = entry.urls.iter()
        .map(|(sid, url)| (sid.to_string(), url.to_string())).collect();
    if !cfg.custom_source.trim().is_empty() {
        let prefix = cfg.custom_source.trim().trim_end_matches('/');
        if let Some((_, first)) = entry.urls.first() {
            // 取第一个 URL 的路径部分，拼到自定义前缀上
            if let Some(pos) = first.find("://") {
                let rest = &first[pos + 3..];
                if let Some(slash) = rest.find('/') {
                    let path = &rest[slash..];
                    out.insert(0, ("custom".into(), format!("{}{}", prefix, path)));
                }
            }
        }
    }
    if !cfg.preferred_source.is_empty() && cfg.preferred_source != "auto" {
        out.sort_by_key(|(sid, _)| if *sid == cfg.preferred_source { 0 } else { 1 });
    }
    out
}

// ============================ 进度与回调 ============================

#[derive(Clone, Debug)]
pub struct Progress {
    pub state: String,       // idle | probing | downloading | extracting | done | failed
    pub source: String,
    pub downloaded: u64,
    pub total: u64,
    pub speed: f64,          // bytes/s
    pub eta: f64,            // 秒
    pub pct: u32,
    pub message: String,
}
impl Progress {
    pub fn idle() -> Self {
        Self { state: "idle".into(), source: String::new(), downloaded: 0, total: 0,
               speed: 0.0, eta: 0.0, pct: 0, message: "就绪".into() }
    }
}

pub trait Reporter: Send + Sync {
    fn progress(&self, p: &Progress);
    fn log(&self, line: &str);
}

pub struct NullReporter;
impl Reporter for NullReporter {
    fn progress(&self, _p: &Progress) {}
    // 必须彻底静默：MCP stdio 与 CLI 都把 stdout 当作协议通道，
    // 这里一旦 println! 就会在 JSON-RPC / JSON envelope 前插入非协议文本（缺陷 #27）。
    fn log(&self, _line: &str) {}
}

// ============================ 选盘安装 ============================

pub fn model_root_for(letter: char) -> String {
    format!("{}:\\{}模型", letter.to_ascii_uppercase(), APP_DIR)
}

pub fn ensure_model_root(root: &str) -> Result<(), String> {
    if root.trim().is_empty() { return Err("尚未选择模型安装位置".into()); }
    win32::ensure_dir(root).map_err(|e| format!("创建模型目录失败：{}", e))?;
    let test = Path::new(root).join(".write_test");
    fs::write(&test, b"ok").map_err(|e| format!("该位置不可写：{}", e))?;
    let _ = fs::remove_file(test);
    Ok(())
}

pub fn installed_manifest_path(root: &str) -> PathBuf {
    Path::new(root).join(".installed.json")
}

pub fn read_installed(root: &str) -> Vec<J> {
    let p = installed_manifest_path(root);
    match fs::read_to_string(p) {
        Ok(t) => match util::parse(&t) {
            Ok(J::Arr(a)) => a,
            _ => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

pub fn write_installed(root: &str, entry: &ModelEntry, source: &str, bytes: u64) -> Result<(), String> {
    let mut list = read_installed(root);
    list.retain(|x| x.str("id") != entry.id);
    list.push(util::obj(vec![
        ("id", util::s(entry.id)),
        ("name", util::s(entry.name)),
        ("engine", util::s(entry.engine)),
        ("target", util::s(entry.target)),
        ("source", util::s(source)),
        ("bytes", util::i(bytes as i64)),
        ("installed_at", util::s(&util::stamp())),
    ]));
    let p = installed_manifest_path(root);
    let mut f = fs::File::create(p).map_err(|e| e.to_string())?;
    f.write_all(util::pretty(&J::Arr(list)).as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

// ============================ 下载管理器 ============================

fn curl_cmd(cfg: &Config) -> String {
    if !cfg.curl_path.is_empty() && win32::exists(&cfg.curl_path) { return cfg.curl_path.clone(); }
    let sys = "C:\\Windows\\System32\\curl.exe";
    if win32::exists(sys) { return sys.into(); }
    "curl.exe".into()
}

fn run_quiet(program: &str, args: &[&str], _timeout: Option<Duration>) -> (i32, String, String) {
    let mut c = Command::new(program);
    c.args(args).stdout(Stdio::piped()).stderr(Stdio::piped())
        .stdin(Stdio::null());
    match c.output() {
        Ok(o) => (o.status.code().unwrap_or(-1),
                  String::from_utf8_lossy(&o.stdout).into_owned(),
                  String::from_utf8_lossy(&o.stderr).into_owned()),
        Err(e) => (-99, String::new(), e.to_string()),
    }
}

/// 对候选源做轻量 HEAD 探测，返回 (source_id, url, 延迟ms)，按延迟升序。
pub fn probe_sources(entry: &ModelEntry, cfg: &Config, rp: &dyn Reporter) -> Vec<(String, String, f64)> {
    let urls = effective_urls(entry, cfg);
    if urls.is_empty() {
        rp.log("该模型为内置引擎，无需下载。");
        return Vec::new();
    }
    let mut out = Vec::new();
    rp.progress(&Progress { state: "probing".into(), source: "自动测速".into(),
        message: format!("正在探测 {} 条下载线路…", urls.len()), ..Progress::idle() });
    for (sid, url) in &urls {
        let args = [
            "-s", "-I", "-L", "--max-redirs", "3",
            "--connect-timeout", "3", "--max-time", "5",
            "-o", "NUL", "-w", "%{http_code}|%{time_total}", url.as_str(),
        ];
        let (code, so, se) = run_quiet(&curl_cmd(cfg), &args, None);
        let parts: Vec<&str> = so.trim().split('|').collect();
        let http = parts.first().and_then(|x| x.parse::<u32>().ok()).unwrap_or(0);
        let t = parts.get(1).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);
        let ms = t * 1000.0;
        let ok = code == 0 && http > 0 && http < 400;
        if ok {
            rp.log(&format!("  ✓ {}：{:.0} ms（HTTP {}）", source_name(sid), ms, http));
            out.push((sid.clone(), url.clone(), ms));
        } else {
            let why = if code != 0 { se.trim().lines().last().unwrap_or("连接失败").to_string() }
                      else { format!("HTTP {}", http) };
            rp.log(&format!("  ✗ {}：不可达（{}），跳过", source_name(sid), why));
        }
        rp.progress(&Progress { state: "probing".into(), source: source_name(sid),
            message: format!("已探测 {}/{}", out.len() + 1, urls.len()), ..Progress::idle() });
    }
    out.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// 下载单个源：返回临时归档路径。进度通过轮询文件大小实时上报。
fn download_one(url: &str, sid: &str, entry: &ModelEntry,
                cfg: &Config, rp: &dyn Reporter, part: &Path) -> Result<u64, String> {
    let curl = curl_cmd(cfg);
    let part_s = part.to_string_lossy().into_owned();
    let args: Vec<String> = vec![
        "-L".into(), "--fail".into(), "--retry".into(), "1".into(),
        "--connect-timeout".into(), "10".into(), "--max-time".into(), "7200".into(),
        "-C".into(), "-".into(), "-o".into(), part_s.clone(), url.into(),
    ];
    let mut child = Command::new(&curl)
        .args(&args).stdout(Stdio::null()).stderr(Stdio::null())
        .stdin(Stdio::null()).spawn()
        .map_err(|e| format!("无法启动 curl.exe：{}", e))?;

    let total = entry.size_mb.saturating_mul(1048576);
    let start = Instant::now();
    let mut last = 0u64;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => return Err(format!("等待下载进程失败：{}", e)),
        }
        let sz = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
        let elapsed = start.elapsed().as_secs_f64().max(0.1);
        let speed = if sz > last { (sz - last) as f64 / 0.2 } else { sz as f64 / elapsed };
        let pct = if total > 0 { (sz * 100 / total).min(100) as u32 } else { 0 };
        rp.progress(&Progress {
            state: "downloading".into(), source: source_name(sid),
            downloaded: sz, total, speed,
            eta: if speed > 0.0 && total > sz { (total - sz) as f64 / speed } else { 0.0 },
            pct, message: format!("{} · {} / {} · {:.1} MB/s",
                source_name(sid), util::human_bytes(sz), util::human_bytes(total),
                speed / 1048576.0),
        });
        last = sz;
        thread::sleep(Duration::from_millis(200));
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let sz = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if !status.success() || sz == 0 {
        return Err(format!("curl 退出码 {:?}，已下载 {}（网络中断 / 源不可用 / 404）",
            status.code(), util::human_bytes(sz)));
    }
    Ok(sz)
}

fn extract_archive(part: &Path, entry: &ModelEntry, root: &str, rp: &dyn Reporter) -> Result<(), String> {
    rp.progress(&Progress { state: "extracting".into(), source: "本地解包".into(),
        message: "正在解包并整理模型文件…".into(), ..Progress::idle() });
    let target = Path::new(root).join(entry.target);
    let tmp = Path::new(root).join(format!("_extract_{}", entry.id));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    let tar = "C:\\Windows\\System32\\tar.exe";
    let tar = if win32::exists(tar) { tar.to_string() } else { "tar.exe".into() };
    let (code, _so, _se) = run_quiet(&tar, &[
        "-xf", &part.to_string_lossy(), "-C", &tmp.to_string_lossy()
    ], None);
    if code != 0 {
        // 单文件模型（piper onnx）不走归档，直接放进 target 目录
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        let name = part.file_name().unwrap_or_default().to_string_lossy().into_owned();
        fs::copy(part, target.join(&name)).map_err(|e| e.to_string())?;
        rp.log(&format!("  单文件模型已放置：{}", name));
        return Ok(());
    }
    // flatten：解包可能多套一层目录（已有项目踩过的坑）
    let mut src = tmp.clone();
    for _ in 0..3 {
        let mut entries: Vec<PathBuf> = fs::read_dir(&src)
            .map(|it| it.filter_map(|e| e.ok()).map(|e| e.path()).collect())
            .unwrap_or_default();
        if entries.len() == 1 && entries[0].is_dir() { src = entries.remove(0); }
        else { break; }
    }
    let _ = fs::remove_dir_all(&target);
    fs::create_dir_all(target.parent().unwrap_or(Path::new(root))).map_err(|e| e.to_string())?;
    if fs::rename(&src, &target).is_err() { copy_dir(&src, &target)?; }
    let _ = fs::remove_dir_all(&tmp);
    let _ = fs::remove_file(part);
    rp.log(&format!("  模型已安装到：{}", target.display()));
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for e in fs::read_dir(src).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path(); let d = dst.join(e.file_name());
        if p.is_dir() { copy_dir(&p, &d)?; }
        else { fs::copy(&p, &d).map_err(|e| e.to_string())?; }
    }
    Ok(())
}

/// 一键安装模型：自动选源 → 逐源尝试（失败即提示并切换）→ 解包 → 登记。
pub fn install_model(entry_id: &str, cfg: &Config, rp: &dyn Reporter) -> Result<String, String> {
    let entry = find_model(entry_id).ok_or_else(|| format!("未知模型：{}", entry_id))?;
    if entry.urls.is_empty() {
        rp.log("内置校样引擎，无需下载模型。");
        return Ok("(内置引擎)".into());
    }
    let root = &cfg.model_root;
    ensure_model_root(root)?;

    let urls = effective_urls(&entry, cfg);
    // 优先用已测速/用户指定的源；否则按内置顺序逐个试
    let mut ordered = urls.clone();
    if cfg.preferred_source == "auto" {
        let probed = probe_sources(&entry, cfg, rp);
        if !probed.is_empty() {
            ordered = probed.into_iter().map(|(s, u, _)| (s, u)).collect();
        }
        rp.log(&format!("自动适配完成：优先使用「{}」", ordered[0].0));
    }

    let part = Path::new(root).join(format!("_download_{}.part", entry.id));
    let mut failures: Vec<String> = Vec::new();
    for (idx, (sid, url)) in ordered.iter().enumerate() {
        rp.log(&format!("[{}/{}] 尝试下载源：{}", idx + 1, ordered.len(), source_name(sid)));
        rp.progress(&Progress { state: "downloading".into(), source: source_name(sid),
            message: format!("连接 {}…", source_name(sid)), ..Progress::idle() });
        match download_one(url, sid, &entry, cfg, rp, &part) {
            Ok(bytes) => {
                rp.log(&format!("  ✓ 下载完成：{}", util::human_bytes(bytes)));
                let _ = fs::remove_file(&part.with_extension("bad"));
                extract_archive(&part, &entry, root, rp)?;
                write_installed(root, &entry, sid, bytes)?;
                rp.progress(&Progress { state: "done".into(), source: source_name(sid),
                    pct: 100, downloaded: bytes, total: bytes,
                    message: format!("模型已安装：{}", entry.name), ..Progress::idle() });
                rp.log(&format!("✓ 安装完成：{}（源：{}）", entry.name, source_name(sid)));
                return Ok(sid.clone());
            }
            Err(e) => {
                let msg = format!("源 {} 失败：{}", source_name(sid), e);
                rp.log(&format!("  ✗ {} → 自动切换下一个源", msg));
                failures.push(msg.clone());
                let bad = part.with_extension("bad");
                let _ = fs::remove_file(&bad);
                let _ = fs::rename(&part, &bad);  // 断点文件留着，下次可续
                rp.progress(&Progress { state: "failed".into(), source: source_name(sid),
                    message: msg.clone(), ..Progress::idle() });
            }
        }
    }
    Err(format!("全部 {} 个下载源均失败：\n  {}", ordered.len(), failures.join("\n  ")))
}

