//! 本地朗读工厂 · 阅读器（v0.6）
//!
//! 边看 EPUB 正文，边听当前段落朗读；后台只把「当前位置往后 N 段」的语音提前生成好。
//! 人眼读一段要几秒，机器合成一段只要几百毫秒，所以预取窗口永远跑在阅读进度前面，
//! 不需要把整本书一次性合成完。
//!
//! 复用 mini 的 `audio::parse_epub` / `audio::synth_paragraph` / `Config` / JSON envelope，
//! 不新建第三条代码线。三种使用方式：GUI / CLI（含 --dry-run 与 --headless）/ MCP tools。

use crate::audio::{self, Book};
use crate::model::{self, Config, NullReporter};
use crate::util::{self, J};
use crate::win32 as w;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub const VERSION: &str = "0.6.0";
pub const DEFAULT_PREFETCH: usize = 6;
const TIMER_ID: usize = 7;

// ---------------- 控件 ID ----------------
const ID_LB_CH: i32 = 1001;
const ID_EDIT_TEXT: i32 = 1002;
const ID_LB_PARA: i32 = 1003;
const ID_BTN_OPEN: i32 = 1004;
const ID_BTN_PREV: i32 = 1005;
const ID_BTN_PLAY: i32 = 1006;
const ID_BTN_NEXT: i32 = 1007;
const ID_BTN_SLOW: i32 = 1008;
const ID_BTN_FAST: i32 = 1009;
const ID_BTN_REDO: i32 = 1010;
const ID_BTN_FOLLOW: i32 = 1011;
const ID_BTN_EXPORT: i32 = 1012;
const ID_CMB_PREFETCH: i32 = 1013;
const ID_LBL_STATUS: i32 = 1014;
const ID_PROGRESS: i32 = 1015;

// ---------------- 数据结构 ----------------
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SegState { Pending, Generating, Ready, Failed }
impl SegState {
    fn mark(self) -> &'static str {
        match self {
            SegState::Pending => "○", SegState::Generating => "⋯",
            SegState::Ready => "·", SegState::Failed => "!",
        }
    }
    fn name(self) -> &'static str {
        match self {
            SegState::Pending => "pending", SegState::Generating => "generating",
            SegState::Ready => "ready", SegState::Failed => "failed",
        }
    }
}

#[derive(Clone)]
pub struct Seg {
    pub ci: usize,
    pub pi: usize,
    pub text: String,
    pub wav: PathBuf,
    pub state: SegState,
    pub bytes: u64,
}

pub struct ReaderState {
    pub epub: String,
    pub book: Book,
    pub cfg: Config,
    pub cache_dir: PathBuf,
    pub segs: Vec<Seg>,
    pub cursor: usize,
    pub playing: bool,
    pub follow: bool,
    pub prefetch: usize,
    /// 每次跳读 +1，让旧位置上的生成结果不抢占当前进度。
    pub gen: u64,
    pub stop: bool,
    pub chapter: usize,
    pub book_dirty: bool,
    pub ui_dirty: bool,
    pub last_event: String,
    pub engine: String,
    pub full_available: bool,
    pub generated: usize,
    pub cache_hits: usize,
    pub last_error: String,
}

fn fnv1a(s: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x0100_0000_01b3); }
    h
}

/// 缓存 key = 书路径 + 文件大小 + 语速；改语速换目录，旧缓存不混用。
/// 注意：缓存必须放在 `ReaderCache`，**不能**放 `Reader` ——
/// 后者是阅读器版的安装目录，卸载会整目录删除，放里面等于删用户数据。
fn cache_dir_for(epub: &str, rate: f64) -> PathBuf {
    // 必须规范化成绝对路径：同一本书用 `testdata/书.epub`、`testdata\书.epub`、
    // 或从快捷方式传进来的绝对路径，都应该命中同一份缓存，否则会重复合成。
    let canon = fs::canonicalize(epub).ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| epub.to_string());
    let size = fs::metadata(epub).map(|m| m.len()).unwrap_or(0);
    let key = format!("{}|{}|{}", canon, size, (rate * 100.0).round() as i64);
    PathBuf::from(format!("{}\\ReaderCache\\{:016x}", Config::dir(), fnv1a(&key)))
}

/// 记住上次打开的书：双击阅读器直接续上次的进度，不用每次重新拖文件。
fn last_book_path() -> String {
    format!("{}\\ReaderCache\\last.txt", Config::dir())
}
fn recall_last_book() -> Option<String> {
    let s = fs::read_to_string(last_book_path()).ok()?;
    let p = s.trim().to_string();
    if !p.is_empty() && Path::new(&p).is_file() { Some(p) } else { None }
}
fn remember_book(epub: &str) {
    let dir = format!("{}\\ReaderCache", Config::dir());
    let _ = w::ensure_dir(&dir);
    // 必须存绝对路径：从开始菜单/桌面快捷方式启动时工作目录是安装目录，
    // 相对路径会解析失败。canonicalize 在 Windows 带 \\?\ 前缀，去掉后用普通路径。
    let abs = fs::canonicalize(epub).ok().map(|p| {
        let s = p.to_string_lossy().into_owned();
        s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
    }).unwrap_or_else(|| epub.to_string());
    let _ = fs::write(last_book_path(), abs.as_bytes());
}

fn build_segs(book: &Book, dir: &PathBuf) -> Vec<Seg> {
    let mut v = Vec::new();
    for (ci, ch) in book.chapters.iter().enumerate() {
        for (pi, text) in ch.paragraphs.iter().enumerate() {
            let wav = dir.join(format!("c{:03}_p{:04}.wav", ci, pi));
            let (state, bytes) = match fs::metadata(&wav) {
                Ok(m) if m.len() > 100 => (SegState::Ready, m.len()),
                _ => (SegState::Pending, 0),
            };
            v.push(Seg { ci, pi, text: text.clone(), wav, state, bytes });
        }
    }
    v
}

impl ReaderState {
    fn blank(cfg: Config, prefetch: usize) -> Self {
        Self {
            epub: String::new(),
            book: Book { title: "（尚未打开电子书）".into(), chapters: Vec::new(), chars: 0 },
            cfg: cfg.clone(),
            cache_dir: cache_dir_for("", cfg.rate),
            segs: Vec::new(), cursor: 0, playing: false, follow: true,
            prefetch, gen: 0, stop: false, chapter: 0,
            book_dirty: true, ui_dirty: true,
            last_event: "把 .epub 从桌面或文件夹拖进窗口，或点「打开电子书…」".into(),
            engine: "sapi-instant".into(),
            full_available: false,
            generated: 0, cache_hits: 0, last_error: String::new(),
        }
    }

    fn new(epub: &str, cfg: &Config, prefetch: usize) -> Result<Self, String> {
        let mut s = Self::blank(cfg.clone(), prefetch);
        s.reload(epub, cfg)?;
        Ok(s)
    }

    fn reload(&mut self, epub: &str, cfg: &Config) -> Result<(), String> {
        let book = audio::parse_epub(epub, &NullReporter)?;
        let dir = cache_dir_for(epub, cfg.rate);
        w::ensure_dir(&dir.to_string_lossy())
            .map_err(|e| format!("创建朗读缓存目录失败：{}", e))?;
        let segs = build_segs(&book, &dir);
        let hits = segs.iter().filter(|x| x.state == SegState::Ready).count();
        let full = model::detect_full_cli(cfg).is_some();
        self.epub = epub.to_string();
        self.book = book;
        self.cfg = cfg.clone();
        self.cache_dir = dir;
        self.segs = segs;
        self.cursor = 0;
        self.playing = false;
        self.chapter = 0;
        self.gen = self.gen.wrapping_add(1);
        self.book_dirty = true;
        self.ui_dirty = true;
        self.cache_hits = hits;
        self.full_available = full;
        self.engine = "sapi-instant".into();
        self.last_error.clear();
        self.last_event = format!("已打开《{}》· {} 章 / {} 段 · 缓存命中 {} 段",
            self.book.title, self.book.chapters.len(), self.segs.len(), hits);
        remember_book(epub);
        Ok(())
    }

    fn log(&mut self, s: impl Into<String>) {
        self.last_event = s.into();
        self.ui_dirty = true;
    }
    fn total(&self) -> usize { self.segs.len() }
    fn first_of_chapter(&self, ci: usize) -> usize {
        self.segs.iter().position(|x| x.ci == ci).unwrap_or(0)
    }
    fn ready_count(&self) -> usize {
        self.segs.iter().filter(|x| x.state == SegState::Ready).count()
    }
    /// 预取窗口末端：至少 cursor+prefetch，并且**保证伸进下一章前 2 段**。
    fn window_end(&self) -> usize {
        let n = self.total();
        if n == 0 || self.cursor >= n { return n; }
        let mut end = (self.cursor + self.prefetch).min(n);
        let cur_ci = self.segs[self.cursor].ci;
        if let Some(next) = self.segs.iter().position(|x| x.ci > cur_ci) {
            end = end.max((next + 2).min(n));
        }
        end
    }
    fn set_cursor(&mut self, idx: usize, why: &str) {
        if self.total() == 0 { return; }
        let i = idx.min(self.total() - 1);
        self.cursor = i;
        self.chapter = self.segs[i].ci;
        self.gen = self.gen.wrapping_add(1);
        let msg = format!("{} → 第 {} 章 · 第 {} 段（全书 {}/{}）",
            why, self.chapter + 1, self.segs[i].pi + 1, i + 1, self.total());
        self.log(msg);
    }
}

fn new_state(epub: &str, cfg: &Config, prefetch: usize) -> Result<Arc<Mutex<ReaderState>>, String> {
    Ok(Arc::new(Mutex::new(ReaderState::new(epub, cfg, prefetch)?)))
}

// ---------------- 单段生成（GUI 线程 / headless 共用）----------------
/// 返回 (是否命中已有缓存, WAV 字节数, 本次耗时 ms)。
fn ensure_segment(st: &Arc<Mutex<ReaderState>>, idx: usize,
                  force: bool) -> Result<(bool, u64, u64), String> {
    let (text, wav, cfg) = {
        let mut s = st.lock().unwrap();
        if idx >= s.segs.len() { return Err("段落索引越界".into()); }
        if s.segs[idx].state == SegState::Ready && !force {
            return Ok((true, s.segs[idx].bytes, 0));
        }
        if force { let _ = fs::remove_file(&s.segs[idx].wav); }
        s.segs[idx].state = SegState::Generating;
        s.ui_dirty = true;
        (s.segs[idx].text.clone(), s.segs[idx].wav.clone(), s.cfg.clone())
    };
    let t0 = Instant::now();
    let real_voice = audio::synth_paragraph(&text, &wav, &cfg)?;
    let ms = t0.elapsed().as_millis() as u64;
    let bytes = fs::metadata(&wav).map(|m| m.len()).unwrap_or(0);
    {
        let mut s = st.lock().unwrap();
        s.segs[idx].state = if bytes > 100 { SegState::Ready } else { SegState::Failed };
        s.segs[idx].bytes = bytes;
        if !force { s.generated += 1; }
        s.ui_dirty = true;
        if !real_voice {
            s.log("⚠ Windows 系统语音不可用，该段写入占位音（装 Full 版可用模型音质导出整本）");
        }
    }
    Ok((false, bytes, ms))
}

// ---------------- 后台线程 ----------------
fn generator_loop(st: Arc<Mutex<ReaderState>>) {
    loop {
        let task = {
            let mut s = st.lock().unwrap();
            if s.stop { break; }
            let end = s.window_end();
            let mut found = None;
            for i in s.cursor..end {
                if s.segs[i].state == SegState::Pending {
                    s.segs[i].state = SegState::Generating;
                    found = Some((i, s.gen, s.segs[i].text.clone(),
                                  s.segs[i].wav.clone(), s.cfg.clone()));
                    break;
                }
            }
            found
        };
        match task {
            Some((idx, gen, text, wav, cfg)) => {
                let t0 = Instant::now();
                let res = audio::synth_paragraph(&text, &wav, &cfg);
                let ms = t0.elapsed().as_millis() as u64;
                let bytes = fs::metadata(&wav).map(|m| m.len()).unwrap_or(0);
                let mut s = st.lock().unwrap();
                if s.gen != gen {
                    // 用户已经跳到别处：WAV 仍写进缓存（可复用），但不改当前窗口状态。
                    s.segs[idx].state = if bytes > 100 { SegState::Ready } else { SegState::Pending };
                    if bytes > 100 { s.segs[idx].bytes = bytes; }
                } else {
                    match res {
                        Ok(real) => {
                            s.segs[idx].state = if bytes > 100 { SegState::Ready } else { SegState::Failed };
                            s.segs[idx].bytes = bytes;
                            s.generated += 1;
                            let lo = s.cursor.min(s.total().saturating_sub(1));
                            let hi = s.window_end();
                            let ready_win = s.segs[lo..hi].iter()
                                .filter(|x| x.state == SegState::Ready).count();
                            let win_len = hi.saturating_sub(lo);
                            let msg = format!("✓ 后台已备好第 {} 段（{} ms，{}）· 窗口内已备 {}/{}",
                                idx + 1, ms, util::human_bytes(bytes), ready_win, win_len);
                            s.log(msg);
                            if !real {
                                s.log("⚠ 系统语音不可用，后台段落写入占位音；安装 Full 版可用模型音质导出整本");
                            }
                        }
                        Err(e) => {
                            s.segs[idx].state = SegState::Failed;
                            let msg = format!("✗ 第 {} 段生成失败：{}（点「重合成」可重试）", idx + 1, e);
                            s.log(msg);
                        }
                    }
                }
                s.ui_dirty = true;
            }
            None => thread::sleep(Duration::from_millis(90)),
        }
    }
}

fn player_loop(st: Arc<Mutex<ReaderState>>) {
    loop {
        let (stop, playing, cursor, gen, wav, ready, total) = {
            let s = st.lock().unwrap();
            let n = s.total();
            let seg = s.segs.get(s.cursor);
            (s.stop, s.playing, s.cursor, s.gen,
             seg.map(|x| x.wav.clone()).unwrap_or_default(),
             seg.map(|x| x.state == SegState::Ready).unwrap_or(false), n)
        };
        if stop { break; }
        if !playing || total == 0 || cursor >= total {
            thread::sleep(Duration::from_millis(40));
            continue;
        }
        if !ready {
            // 人不阅读时也不会卡死：生成线程会很快把这段补齐。
            thread::sleep(Duration::from_millis(40));
            continue;
        }
        w::play_wav_sync(&wav.to_string_lossy());
        let mut s = st.lock().unwrap();
        if s.gen == gen && s.playing && s.cursor == cursor {
            s.cursor += 1;
            if s.cursor < s.total() { s.chapter = s.segs[s.cursor].ci; }
            s.ui_dirty = true;
            let next_no = s.cursor.min(s.total().saturating_sub(1)) + 1;
            let ready_n = s.ready_count();
            let msg = format!("♪ 朗读完第 {} 段 → 继续第 {} 段（后台已备 {} 段）",
                cursor + 1, next_no, ready_n);
            s.log(msg);
        }
    }
}

// ---------------- CLI / MCP 可复用的 JSON ----------------
fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).cloned()
}
fn arg_usize(args: &[String], key: &str, dflt: usize) -> usize {
    arg_value(args, key).and_then(|x| x.parse::<usize>().ok()).unwrap_or(dflt)
}

fn plan_json(epub: &str, cfg: &Config, prefetch: usize) -> Result<J, String> {
    let st = new_state(epub, cfg, prefetch)?;
    let s = st.lock().unwrap();
    let chapters: Vec<J> = s.book.chapters.iter().enumerate().map(|(ci, c)| util::obj(vec![
        ("index", util::i((ci + 1) as i64)),
        ("title", util::s(&c.title)),
        ("paragraphs", util::i(c.paragraphs.len() as i64)),
        ("first_segment", util::i(s.first_of_chapter(ci) as i64)),
    ])).collect();
    let window: Vec<J> = (s.cursor..s.window_end()).map(|i| util::obj(vec![
        ("segment", util::i((i + 1) as i64)),
        ("chapter", util::i((s.segs[i].ci + 1) as i64)),
        ("state", util::s(s.segs[i].state.name())),
        ("preview", util::s(&s.segs[i].text.chars().take(20).collect::<String>())),
    ])).collect();
    Ok(util::obj(vec![
        ("mode", util::s("reader")), ("version", util::s(VERSION)),
        ("title", util::s(&s.book.title)), ("epub", util::s(epub)),
        ("chapters", util::i(s.book.chapters.len() as i64)),
        ("paragraphs", util::i(s.total() as i64)),
        ("chars", util::i(s.book.chars as i64)),
        ("prefetch", util::i(prefetch as i64)),
        ("window_end", util::i(s.window_end() as i64)),
        ("cache_dir", util::s(&s.cache_dir.to_string_lossy())),
        ("cache_hits", util::i(s.cache_hits as i64)),
        ("ready", util::i(s.ready_count() as i64)),
        ("engine", util::s(&s.engine)),
        ("full_available", util::b(s.full_available)),
        ("chapter_list", util::arr(chapters)),
        ("prefetch_window", util::arr(window)),
    ]))
}

fn status_json(epub: &str, cfg: &Config, prefetch: usize) -> Result<J, String> {
    let st = new_state(epub, cfg, prefetch)?;
    let s = st.lock().unwrap();
    Ok(util::obj(vec![
        ("mode", util::s("reader")), ("status", util::s("ok")),
        ("title", util::s(&s.book.title)), ("epub", util::s(epub)),
        ("total", util::i(s.total() as i64)),
        ("ready", util::i(s.ready_count() as i64)),
        ("cache_hits", util::i(s.cache_hits as i64)),
        ("wav_bytes", util::i(s.segs.iter().map(|x| x.bytes).sum::<u64>() as i64)),
        ("cache_dir", util::s(&s.cache_dir.to_string_lossy())),
        ("rate", util::n(s.cfg.rate)),
        ("engine", util::s(&s.engine)),
        ("full_available", util::b(s.full_available)),
    ]))
}

/// 无窗口验证"播放链路"：合成第 1 段后用 PlaySoundW 同步播到结束。
/// 无声卡 / 无音频会话的沙箱会返回 playback_failed，属正常现象。
fn play_test_json(epub: &str, cfg: &Config, prefetch: usize) -> Result<J, String> {
    let st = new_state(epub, cfg, prefetch)?;
    let (hit, bytes, synth_ms) = ensure_segment(&st, 0, false)?;
    if bytes <= 100 { return Err("第 1 段没有生成有效音频".into()); }
    let wav = { st.lock().unwrap().segs[0].wav.clone() };
    let t0 = Instant::now();
    let played = w::play_wav_sync(&wav.to_string_lossy());
    let play_ms = t0.elapsed().as_millis() as u64;
    Ok(util::obj(vec![
        ("mode", util::s("reader")),
        ("status", util::s(if played { "played" } else { "playback_failed" })),
        ("segment", util::i(1)), ("wav_bytes", util::i(bytes as i64)),
        ("cache_hit", util::b(hit)),
        ("synth_ms", util::i(synth_ms as i64)), ("play_ms", util::i(play_ms as i64)),
        ("playback_available", util::b(w::wav_playback_available())),
        ("note", util::s("PlaySoundW 同步播放到段落结束；沙箱无声卡时 playback_failed 属正常")),
    ]))
}

fn prefetch_json(epub: &str, cfg: &Config, from: usize, count: usize,
                 prefetch: usize) -> Result<J, String> {
    let st = new_state(epub, cfg, prefetch)?;
    let total = st.lock().unwrap().total();
    let end = (from + count).min(total);
    let mut rows: Vec<J> = Vec::new();
    let mut hits = 0usize;
    let mut bytes = 0u64;
    for i in from..end {
        let (hit, b, ms) = ensure_segment(&st, i, false)?;
        if hit { hits += 1; }
        bytes += b;
        let row = {
            let s = st.lock().unwrap();
            let g = &s.segs[i];
            util::obj(vec![
                ("segment", util::i((i + 1) as i64)),
                ("chapter", util::i((g.ci + 1) as i64)),
                ("paragraph", util::i((g.pi + 1) as i64)),
                ("state", util::s(g.state.name())),
                ("wav_bytes", util::i(g.bytes as i64)),
                ("elapsed_ms", util::i(ms as i64)),
                ("cache_hit", util::b(hit)),
                ("text", util::s(&g.text.chars().take(40).collect::<String>())),
            ])
        };
        rows.push(row);
    }
    let (cache_dir, engine) = {
        let s = st.lock().unwrap();
        (s.cache_dir.to_string_lossy().into_owned(), s.engine.clone())
    };
    Ok(util::obj(vec![
        ("mode", util::s("reader")), ("status", util::s("completed")),
        ("engine", util::s(&engine)), ("from", util::i(from as i64)),
        ("count", util::i(rows.len() as i64)), ("total", util::i(total as i64)),
        ("cache_hits", util::i(hits as i64)), ("wav_bytes", util::i(bytes as i64)),
        ("cache_dir", util::s(&cache_dir)),
        ("segments", util::arr(rows)),
    ]))
}

/// 阅读器子命令入口；输出仍是与主线一致的 JSON envelope。
pub fn handle_cli(args: &[String], cfg: &Config, pretty: bool) -> i32 {
    w::init_console_utf8();
    let emit = |j: &J| {
        if pretty { println!("{}", util::pretty(j)); }
        else { println!("{}", util::stringify(j)); }
    };
    let sub = args.get(1).map(|x| x.as_str()).unwrap_or("");
    let dry = args.iter().any(|a| a == "--dry-run");
    let headless = args.iter().any(|a| a == "--headless");
    let play_test = args.iter().any(|a| a == "--play-test");
    let prefetch = arg_usize(args, "--prefetch", DEFAULT_PREFETCH);
    let count = arg_usize(args, "--count", 5);
    let from = arg_usize(args, "--from", 0);
    let input = arg_value(args, "--input").unwrap_or_default();

    let need_input = dry || headless || play_test || sub == "status" || sub == "prefetch";
    if need_input && input.is_empty() {
        emit(&util::err("ARGS_INVALID", "缺少 --input <book.epub>",
            "reader --input 书.epub --dry-run"));
        return 2;
    }

    let result: Result<J, String> = if play_test {
        play_test_json(&input, cfg, prefetch)
    } else if dry {
        plan_json(&input, cfg, prefetch)
    } else if headless {
        prefetch_json(&input, cfg, 0, count, prefetch)
    } else {
        match sub {
            "status" => status_json(&input, cfg, prefetch),
            "prefetch" => prefetch_json(&input, cfg, from, count, prefetch),
            _ => {
                run_reader_gui(if input.is_empty() { None } else { Some(&input) },
                               cfg.clone(), prefetch);
                return 0;
            }
        }
    };
    match result {
        Ok(j) => { emit(&util::ok(j)); 0 }
        Err(e) => { emit(&util::err("READER_FAILED", &e, "确认 .epub 路径可读，或先运行 doctor")); 1 }
    }
}

pub fn mcp_tools() -> Vec<J> {
    let def = |name: &str, desc: &str, props: Vec<(&str, &str, &str)>| -> J {
        util::obj(vec![
            ("name", util::s(name)), ("description", util::s(desc)),
            ("inputSchema", util::obj(vec![
                ("type", util::s("object")),
                ("properties", util::obj(props.into_iter().map(|(k, t, d)| (k,
                    util::obj(vec![("type", util::s(t)), ("description", util::s(d))]))).collect())),
            ])),
        ])
    };
    vec![
        def("reader_open", "阅读器：只读解析 EPUB，返回章节/段落/预取窗口/缓存命中", vec![
            ("input", "string", "EPUB 路径")]),
        def("reader_prefetch", "阅读器：预生成指定区间的段落语音（不弹界面）", vec![
            ("input", "string", "EPUB 路径"), ("from", "number", "起始段索引（0 基）"),
            ("count", "number", "生成段数")]),
        def("reader_status", "阅读器：查看缓存命中与已生成段数", vec![
            ("input", "string", "EPUB 路径")]),
    ]
}

pub fn mcp_call(cfg: &Config, name: &str, a: &J) -> J {
    let input = a.str("input");
    let r = match name {
        "reader_open" => plan_json(&input, cfg, DEFAULT_PREFETCH),
        "reader_prefetch" => prefetch_json(&input, cfg,
            a.int("from").max(0) as usize, a.int("count").max(1) as usize, DEFAULT_PREFETCH),
        "reader_status" => status_json(&input, cfg, DEFAULT_PREFETCH),
        _ => Err(format!("未知工具：{}", name)),
    };
    match r {
        Ok(j) => j,
        Err(e) => util::obj(vec![("status", util::s("failed")), ("error", util::s(&e))]),
    }
}

/// 安装成阅读器版时主 exe 改名为 `BookCtl.Reader.exe`，按文件名自动进入阅读器模式，
/// 快捷方式与 .epub 关联都不需要额外参数。
pub fn is_reader_exe() -> bool {
    std::env::current_exe().ok()
        .and_then(|p| p.file_name().map(|x| x.to_string_lossy().into_owned()))
        .map(|n| n.to_ascii_lowercase().contains("reader"))
        .unwrap_or(false)
}

// ---------------- GUI ----------------
#[derive(Clone, Copy, Default)]
struct Ctrl {
    lb_ch: w::HWND, edit_text: w::HWND, lb_para: w::HWND,
    btn_open: w::HWND, btn_prev: w::HWND, btn_play: w::HWND, btn_next: w::HWND,
    btn_slow: w::HWND, btn_fast: w::HWND, btn_redo: w::HWND,
    btn_follow: w::HWND, btn_export: w::HWND, cmb_prefetch: w::HWND,
    lbl_status: w::HWND, progress: w::HWND, lbl_title: w::HWND,
}
struct Gfx { bg: w::HBRUSH, white: w::HBRUSH, font: w::HFONT, bold: w::HFONT }

static STATE: OnceLock<Arc<Mutex<ReaderState>>> = OnceLock::new();
static CTRL: OnceLock<Mutex<Ctrl>> = OnceLock::new();
static GFX: OnceLock<Gfx> = OnceLock::new();
static SUPPRESS: AtomicBool = AtomicBool::new(false);
/// 上次渲染的（章节号, 书名）。正文只在换书/换章时重设，避免打断用户手动滚动。
static LAST_RENDER: OnceLock<Mutex<(usize, String)>> = OnceLock::new();

fn state() -> &'static Arc<Mutex<ReaderState>> { STATE.get().expect("reader state not initialized") }
fn ctrl() -> Ctrl { *CTRL.get().expect("reader ctrl not initialized").lock().unwrap() }
fn gfx() -> &'static Gfx { GFX.get().expect("reader gfx not initialized") }

fn label(text: &str, x: i32, y: i32, w_: i32, h: i32,
         parent: w::HWND, id: i32, bold: bool, muted: bool) -> w::HWND {
    let h_ = w::create_ctrl("STATIC", text, w::SS_LEFT, x, y, w_, h, parent, id);
    w::set_font(h_, if bold { gfx().bold } else { gfx().font });
    let _ = muted;
    h_
}
fn button(text: &str, x: i32, y: i32, w_: i32, h: i32,
          parent: w::HWND, id: i32) -> w::HWND {
    let b = w::create_ctrl("BUTTON", text,
        w::BS_PUSHBUTTON | w::WS_TABSTOP, x, y, w_, h, parent, id);
    w::set_font(b, gfx().font);
    b
}

/// 渲染当前章节正文，返回 UTF-16 字符偏移范围（EM_SETSEL 用的是 UTF-16 单元）。
fn chapter_text(s: &ReaderState) -> (String, Vec<(usize, usize)>) {
    if s.total() == 0 {
        return ("把 .epub 从桌面或文件夹直接拖进窗口，或点「打开电子书…」。\r\n\r\n\
                 打开后左边选章节、右边点段落，正文在中间显示；\r\n\
                 点「播放」开始边看边听，后台会自动把后面章节的语音提前生成好。".into(),
                Vec::new());
    }
    let ci = s.chapter.min(s.book.chapters.len().saturating_sub(1));
    let head = format!("《{}》\r\n第 {} 章 · {}\r\n\r\n",
        s.book.title, ci + 1,
        s.book.chapters.get(ci).map(|c| c.title.as_str()).unwrap_or(""));
    let mut text = head.clone();
    let mut off = head.encode_utf16().count();
    let mut ranges = Vec::new();
    for seg in s.segs.iter().filter(|x| x.ci == ci) {
        let line = format!("{}\r\n\r\n", seg.text);
        let start = off;
        let sel_len = seg.text.encode_utf16().count();
        ranges.push((start, start + sel_len));
        off += line.encode_utf16().count();
        text.push_str(&line);
    }
    (text, ranges)
}

fn rebuild_lists(c: Ctrl, s: &ReaderState) {
    SUPPRESS.store(true, Ordering::Relaxed);
    if s.book_dirty {
        w::list_clear(c.lb_ch);
        for (i, ch) in s.book.chapters.iter().enumerate() {
            w::list_add(c.lb_ch, &format!("{:02} · {}", i + 1, ch.title));
        }
    }
    w::list_clear(c.lb_para);
    let ci = s.chapter;
    for seg in s.segs.iter().filter(|x| x.ci == ci) {
        let mark = if seg.state == SegState::Ready { "·" }
                   else { seg.state.mark() };
        let preview: String = seg.text.chars().take(16).collect();
        w::list_add(c.lb_para, &format!("{:02} {} {}", seg.pi + 1, mark, preview));
    }
    if !s.book.chapters.is_empty() { w::set_list_sel(c.lb_ch, ci as i32); }
    if s.total() > 0 {
        let pi = s.segs[s.cursor.min(s.total() - 1)].pi;
        w::set_list_sel(c.lb_para, pi as i32);
    }
    SUPPRESS.store(false, Ordering::Relaxed);
}

fn refresh_ui(c: Ctrl, _hwnd: w::HWND) {
    let snap = {
        let mut s = state().lock().unwrap();
        if !s.ui_dirty { return; }
        s.ui_dirty = false;
        let (text, ranges) = chapter_text(&s);
        let engine_note = if s.full_available {
            "检测到 Full（「整本导出」可用模型音质）"
        } else { "系统语音即时朗读" };
        let cur_no = if s.total() == 0 { 0 } else { s.cursor.min(s.total() - 1) + 1 };
        let book_disp = if s.book.title.starts_with('（') { s.book.title.clone() }
                        else { format!("《{}》", s.book.title) };
        let status = format!("{} · 当前第 {}/{} 段 · 全书已备 {} 段 · 预取 {} 段 · {} · {}",
            book_disp, cur_no, s.total(), s.ready_count(),
            s.prefetch, s.engine, engine_note);
        let pct = if s.total() == 0 { 0 }
                  else { (s.cursor.min(s.total()) * 100 / s.total()) as u32 };
        (text, ranges, s.cursor, s.chapter, s.book_dirty, s.follow,
         s.playing, s.last_event.clone(), status, pct, s.total(),
         s.book.title.clone(),
         if s.total() > 0 { s.segs[s.cursor.min(s.total() - 1)].pi } else { 0 })
    };
    let (text, ranges, _cursor, chapter, book_dirty, follow, playing,
         last_event, status, pct, total, title, pi_now) = snap;

    // 正文只在换书 / 换章时重设；段落状态变化只刷右侧导航，不打断用户滚动。
    let need_text = {
        let mut lr = LAST_RENDER.get_or_init(|| Mutex::new((usize::MAX, String::new())))
            .lock().unwrap();
        let n = book_dirty || lr.0 != chapter || lr.1 != title;
        if n { *lr = (chapter, title.clone()); }
        n
    };
    if need_text {
        // 空状态的书名本身已带括号，别再套一层书名号。
        let disp = if title.starts_with('（') { title.clone() } else { format!("《{}》", title) };
        w::set_text(c.lbl_title, &disp);
        w::set_text(c.edit_text, &text);
    }
    if book_dirty { state().lock().unwrap().book_dirty = false; }
    {
        let s = state().lock().unwrap();
        rebuild_lists(c, &s);
    }
    if total > 0 && !ranges.is_empty() && pi_now < ranges.len() {
        let (a, b) = ranges[pi_now];
        w::set_sel(c.edit_text, a as isize, b as isize);
        if follow { w::scroll_caret(c.edit_text); }
    }
    w::set_text(c.btn_play, if playing { "⏸ 暂停朗读" } else { "▶ 开始朗读" });
    w::set_text(c.btn_follow, if follow { "跟随：开" } else { "跟随：关" });
    w::set_text(c.lbl_status, &format!("{}   ｜   {}", status, last_event));
    w::set_progress(c.progress, pct);
}

fn open_book(path: &str, cfg: &Config) {
    let mut s = state().lock().unwrap();
    match s.reload(path, cfg) {
        Ok(()) => { w::stop_wav(); }
        Err(e) => {
            s.last_error = e.clone();
            s.log(format!("✗ 打开失败：{}", e));
        }
    }
}
fn jump(idx: usize, why: &str) {
    let mut s = state().lock().unwrap();
    s.set_cursor(idx, why);
    w::stop_wav();
}
fn toggle_play() {
    let mut s = state().lock().unwrap();
    if s.total() == 0 { s.log("请先打开一本 .epub"); return; }
    s.playing = !s.playing;
    if !s.playing { w::stop_wav(); }
    let msg = if s.playing { "▶ 开始边看边听（后台会自动预生成后面章节）" } else { "⏸ 已暂停" };
    s.log(msg);
}
fn change_rate(delta: f64) {
    let (epub, mut cfg) = {
        let s = state().lock().unwrap();
        (s.epub.clone(), s.cfg.clone())
    };
    if epub.is_empty() { return; }
    cfg.rate = (cfg.rate + delta).clamp(0.5, 2.0);
    let _ = cfg.save();
    open_book(&epub, &cfg);
    let mut s = state().lock().unwrap();
    s.log(format!("语速改为 {:.2}×，已按新语速切换到独立缓存", cfg.rate));
}
fn redo_current() {
    let idx = { state().lock().unwrap().cursor };
    let st = state().clone();
    match ensure_segment(&st, idx, true) {
        Ok((_, bytes, ms)) => {
            let mut s = state().lock().unwrap();
            s.gen = s.gen.wrapping_add(1);
            s.log(format!("✓ 第 {} 段已重新合成（{} ms，{}）", idx + 1, ms,
                util::human_bytes(bytes)));
        }
        Err(e) => state().lock().unwrap().log(format!("✗ 重合成失败：{}", e)),
    }
}
fn export_book() {
    let (epub, cfg) = {
        let s = state().lock().unwrap();
        (s.epub.clone(), s.cfg.clone())
    };
    if epub.is_empty() { return; }
    state().lock().unwrap().log("⏳ 已在后台导出整本有声书（走既有批量流水线）…");
    let st = state().clone();
    thread::spawn(move || {
        match audio::generate_book(&epub, &cfg, &NullReporter, None) {
            Ok(sum) => {
                let mut s = st.lock().unwrap();
                s.log(format!("✓ 整本导出完成：{} 章 / {:.1}s / {} → {}",
                    sum.chapters, sum.seconds, sum.engine, sum.out_dir));
                let _ = w::shell_open(&sum.out_dir);
            }
            Err(e) => st.lock().unwrap().log(format!("✗ 整本导出失败：{}", e)),
        }
    });
}

unsafe extern "system" fn wnd_proc(h: w::HWND, m: u32,
                                   wp: w::WPARAM, lp: w::LPARAM) -> w::LRESULT {
    match m {
        w::WM_DESTROY => {
            w::kill_timer(h, TIMER_ID);
            w::stop_wav();
            let mut s = state().lock().unwrap();
            s.stop = true; s.playing = false;
            w::post_quit();
            return 0;
        }
        w::WM_CTLCOLORSTATIC | w::WM_CTLCOLORDLG => {
            return w::color_ctl(wp as w::HDC, w::CLR_BG, w::CLR_TEXT, gfx().bg) as w::LRESULT;
        }
        w::WM_CTLCOLOREDIT | w::WM_CTLCOLORBTN => {
            return w::color_ctl(wp as w::HDC, w::CLR_WHITE, w::CLR_TEXT, gfx().white) as w::LRESULT;
        }
        w::WM_DROPFILES => {
            let files = w::drag_query(wp as isize);
            if let Some(p) = files.iter().find(|f| f.to_ascii_lowercase().ends_with(".epub")) {
                let cfg = { state().lock().unwrap().cfg.clone() };
                open_book(p, &cfg);
            } else {
                state().lock().unwrap().log("拖入的内容里没有 .epub，请拖入电子书");
            }
            return 0;
        }
        w::WM_TIMER => {
            refresh_ui(ctrl(), h);
            return 0;
        }
        w::WM_COMMAND => {
            if SUPPRESS.load(Ordering::Relaxed) { return 0; }
            let id = (wp & 0xFFFF) as i32;
            let code = ((wp >> 16) & 0xFFFF) as i32;
            let c = ctrl();
            match id {
                ID_LB_CH if code == 1 => {
                    let ci = w::list_sel(c.lb_ch).max(0) as usize;
                    let idx = { state().lock().unwrap().first_of_chapter(ci) };
                    jump(idx, "跳到章节");
                }
                ID_LB_PARA if code == 1 => {
                    let pi = w::list_sel(c.lb_para).max(0) as usize;
                    let idx = {
                        let s = state().lock().unwrap();
                        s.first_of_chapter(s.chapter) + pi
                    };
                    jump(idx, "跳到段落");
                }
                ID_CMB_PREFETCH if code == 1 => {
                    let i = w::combo_sel(c.cmb_prefetch).max(0) as usize;
                    let v = [2usize, 4, 6, 10, 16].get(i).copied().unwrap_or(DEFAULT_PREFETCH);
                    let mut s = state().lock().unwrap();
                    s.prefetch = v;
                    s.log(format!("预取深度改为 {} 段（跨章节）", v));
                }
                ID_BTN_OPEN => {
                    if let Some(p) = w::open_file_dialog(h, "选择 EPUB 电子书",
                        "EPUB 电子书 (*.epub)|*.epub|所有文件 (*.*)|*.*|", "epub") {
                        let cfg = { state().lock().unwrap().cfg.clone() };
                        open_book(&p, &cfg);
                    }
                }
                ID_BTN_PREV => {
                    let i = { state().lock().unwrap().cursor.saturating_sub(1) };
                    jump(i, "上一段");
                }
                ID_BTN_NEXT => {
                    let i = { state().lock().unwrap().cursor + 1 };
                    jump(i, "下一段");
                }
                ID_BTN_PLAY => toggle_play(),
                ID_BTN_SLOW => change_rate(-0.15),
                ID_BTN_FAST => change_rate(0.15),
                ID_BTN_REDO => redo_current(),
                ID_BTN_FOLLOW => {
                    let mut s = state().lock().unwrap();
                    s.follow = !s.follow;
                    let msg = if s.follow { "自动跟随已开：文字随朗读滚动" }
                              else { "自动跟随已关：可自由翻页，朗读位置看右侧导航" };
                    s.log(msg);
                }
                ID_BTN_EXPORT => export_book(),
                _ => {}
            }
            return 0;
        }
        _ => w::def_proc(h, m, wp, lp),
    }
}

pub fn run_reader_gui(initial_epub: Option<&str>, cfg: Config, prefetch: usize) {
    w::init_console_utf8();
    w::init_common_controls();

    let st = match initial_epub {
        Some(p) => match new_state(p, &cfg, prefetch) {
            Ok(s) => s,
            Err(e) => {
                w::msg_box("本地朗读工厂 · 阅读器",
                    &format!("打开电子书失败：\n\n{}", e), w::MB_ICONINFORMATION);
                Arc::new(Mutex::new(ReaderState::blank(cfg.clone(), prefetch)))
            }
        },
        // 双击启动（无参数）时恢复上次打开的书；文件不在了就回到空状态。
        None => match recall_last_book() {
            Some(p) => match new_state(&p, &cfg, prefetch) {
                Ok(s) => s,
                Err(_) => Arc::new(Mutex::new(ReaderState::blank(cfg.clone(), prefetch))),
            },
            None => Arc::new(Mutex::new(ReaderState::blank(cfg.clone(), prefetch))),
        },
    };
    let _ = STATE.set(st.clone());
    let _ = CTRL.set(Mutex::new(Ctrl::default()));
    let _ = GFX.set(Gfx {
        bg: w::make_brush(w::CLR_BG),
        white: w::make_brush(w::CLR_WHITE),
        font: w::make_font(16, false, "Microsoft YaHei UI"),
        bold: w::make_font(22, true, "Microsoft YaHei UI"),
    });

    let atom = w::register_class("BookCtlReaderWnd", wnd_proc, gfx().bg);
    if atom == 0 {
        w::msg_box("本地朗读工厂 · 阅读器", "窗口类注册失败。", w::MB_ICONERROR);
        return;
    }
    let hwnd = w::create_window("BookCtlReaderWnd", "本地朗读工厂 · 阅读器",
        w::WS_OVERLAPPEDWINDOW | w::WS_CLIPCHILDREN, 100, 60, 1200, 820);
    if hwnd == 0 {
        w::msg_box("本地朗读工厂 · 阅读器", "创建窗口失败。", w::MB_ICONERROR);
        return;
    }

    label("本地朗读工厂 · 阅读器", 20, 12, 700, 34, hwnd, 0, true, false);
    label("边看 EPUB 文字，边听当前段落朗读；后台自动把后面章节的语音提前生成好",
          20, 50, 1140, 22, hwnd, 0, false, true);
    let btn_open = button("打开电子书…", 20, 80, 150, 32, hwnd, ID_BTN_OPEN);
    let lbl_title = label("（尚未打开电子书）", 184, 86, 520, 24, hwnd, 0, false, false);
    label("把 .epub 从桌面或文件夹直接拖进窗口即可换书", 720, 86, 440, 22, hwnd, 0, false, true);

    label("章节", 20, 122, 210, 20, hwnd, 0, false, true);
    let lb_ch = w::create_ctrl("LISTBOX", "", w::LBS_NOTIFY | w::WS_VSCROLL | w::WS_BORDER,
        20, 144, 210, 494, hwnd, ID_LB_CH);
    w::set_font(lb_ch, gfx().font);

    label("正文 · 当前朗读段落高亮（跟随可关，关后可自由滚动）", 242, 122, 690, 20, hwnd, 0, false, true);
    let edit_text = w::create_ctrl("EDIT", "",
        w::ES_MULTILINE | w::ES_AUTOVSCROLL | w::ES_READONLY | w::WS_VSCROLL | w::WS_BORDER,
        242, 144, 690, 494, hwnd, ID_EDIT_TEXT);
    w::set_font(edit_text, gfx().font);
    w::send(edit_text, w::EM_SETREADONLY, 1, 0);

    label("段落导航 · 点击直接跳转", 944, 122, 220, 20, hwnd, 0, false, true);
    let lb_para = w::create_ctrl("LISTBOX", "", w::LBS_NOTIFY | w::WS_VSCROLL | w::WS_BORDER,
        944, 144, 220, 494, hwnd, ID_LB_PARA);
    w::set_font(lb_para, gfx().font);

    let btn_prev = button("⏮ 上一段", 20, 650, 110, 36, hwnd, ID_BTN_PREV);
    let btn_play = button("▶ 开始朗读", 138, 650, 140, 36, hwnd, ID_BTN_PLAY);
    let btn_next = button("⏭ 下一段", 286, 650, 110, 36, hwnd, ID_BTN_NEXT);
    let btn_slow = button("语速 −", 404, 650, 88, 36, hwnd, ID_BTN_SLOW);
    let btn_fast = button("语速 +", 500, 650, 88, 36, hwnd, ID_BTN_FAST);
    let btn_redo = button("重合成本段", 596, 650, 116, 36, hwnd, ID_BTN_REDO);
    let btn_follow = button("跟随：开", 720, 650, 110, 36, hwnd, ID_BTN_FOLLOW);
    let btn_export = button("整本导出…", 838, 650, 130, 36, hwnd, ID_BTN_EXPORT);
    label("预取", 978, 658, 50, 20, hwnd, 0, false, true);
    let cmb_prefetch = w::create_ctrl("COMBOBOX", "",
        w::CBS_DROPDOWNLIST | w::CBS_HASSTRINGS | w::WS_TABSTOP,
        1024, 650, 136, 200, hwnd, ID_CMB_PREFETCH);
    w::set_font(cmb_prefetch, gfx().font);
    for t in ["2 段", "4 段", "6 段（推荐）", "10 段", "16 段"] { w::add_combo(cmb_prefetch, t); }
    w::set_combo_sel(cmb_prefetch, 2);

    let lbl_status = label("就绪", 20, 696, 1140, 22, hwnd, ID_LBL_STATUS, false, false);
    let progress = w::create_ctrl("msctls_progress32", "", w::PBS_SMOOTH,
        20, 722, 1140, 16, hwnd, ID_PROGRESS);
    label("提示：人看一段要几秒，机器合成一段只要几百毫秒 —— 所以你翻到哪一章，那一章的语音基本已经备好了。",
          20, 746, 1140, 20, hwnd, 0, false, true);

    {
        let mut c = CTRL.get().unwrap().lock().unwrap();
        *c = Ctrl {
            lb_ch, edit_text, lb_para, btn_open, btn_prev, btn_play, btn_next,
            btn_slow, btn_fast, btn_redo, btn_follow, btn_export, cmb_prefetch,
            lbl_status, progress, lbl_title,
        };
    }
    {
        let s = state().lock().unwrap();
        rebuild_lists(ctrl(), &s);
    }
    w::accept_files(hwnd, true);
    w::set_timer(hwnd, TIMER_ID, 200);
    w::show(hwnd, w::SW_SHOW);
    // 只读 EDIT 失焦时不渲染选中高亮，当前朗读段落会看不见；给它焦点。
    w::focus(edit_text);

    let g = st.clone();
    thread::spawn(move || generator_loop(g));
    let p = st.clone();
    thread::spawn(move || player_loop(p));

    w::message_loop();
    w::stop_wav();
}
