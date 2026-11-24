//! Mini 的 Win32 原生 GUI：轻量入口。
//!
//! 一个窗口承载全部交互：拖入 EPUB → 选运行环境 → 选源/选盘装模型 → 看进度
//! → 打开模型目录 → 一键生成朗读。所有后台任务在 worker 线程跑，
//! 通过全局 Shared 状态 + WM_TIMER 把进度/日志刷回控件（不阻塞界面）。

use crate::audio;
use crate::model::{self, Config, Progress, Reporter};
use crate::win32 as w;
use std::sync::{Mutex, OnceLock};
use std::thread;

// ---------------- 控件 ID ----------------
const CARD1: i32 = 900; const CARD2: i32 = 901; const CARD3: i32 = 902;
const EDIT_INPUT: i32 = 1001;
const BTN_BROWSE: i32 = 1002;
const CMB_DEVICE: i32 = 1003;
const CMB_GPU: i32 = 1004;
const BTN_IMPORT_CFG: i32 = 1005;
const BTN_SAVE_CFG: i32 = 1006;
const CMB_MODEL: i32 = 1007;
const BTN_PROBE: i32 = 1008;
const CMB_DRIVE: i32 = 1009;
const BTN_INSTALL: i32 = 1010;
const BTN_OPEN_DIR: i32 = 1011;
const PROGRESS: i32 = 1012;
const LBL_STATUS: i32 = 1013;
const EDIT_LOG: i32 = 1014;
const BTN_GENERATE: i32 = 1015;
const BTN_OPEN_OUT: i32 = 1016;
const CMB_SOURCE: i32 = 1017;
const LBL_HINT: i32 = 1018;
const TIMER_ID: usize = 1;

#[derive(Clone, Copy, Default)]
struct Controls {
    card1: w::HWND, card2: w::HWND, card3: w::HWND,
    edit_input: w::HWND, btn_browse: w::HWND,
    cmb_device: w::HWND, cmb_gpu: w::HWND,
    btn_import_cfg: w::HWND, btn_save_cfg: w::HWND,
    cmb_model: w::HWND, cmb_source: w::HWND, btn_probe: w::HWND,
    cmb_drive: w::HWND, btn_install: w::HWND, btn_open_dir: w::HWND,
    progress: w::HWND, lbl_status: w::HWND, lbl_hint: w::HWND,
    edit_log: w::HWND, btn_generate: w::HWND, btn_open_out: w::HWND,
}

struct Shared {
    cfg: Config,
    ctrl: Controls,
    drives: Vec<w::Drive>,
    progress: Progress,
    logs: Vec<String>,
    log_idx: usize,
    busy: bool,
    input: String,
    selected_model: String,
    last_out_dir: String,
    last_error: String,
    popup_yesno: Option<String>,
    source_latency: Vec<(String, f64)>,
    refresh_sources: bool,
    bg_brush: w::HBRUSH,
    white_brush: w::HBRUSH,
    font: w::HFONT,
    font_bold: w::HFONT,
    detailed: bool,
}

impl Shared {
    fn new() -> Self {
        let cfg = Config::load();
        Self {
            cfg, ctrl: Controls::default(), drives: Vec::new(),
            progress: Progress::idle(), logs: Vec::new(), log_idx: 0,
            busy: false, input: String::new(),
            selected_model: "sherpa-vits-zh-fanchen".into(),
            last_out_dir: String::new(), last_error: String::new(),
            popup_yesno: None, source_latency: Vec::new(), refresh_sources: false,
            bg_brush: 0, white_brush: 0, font: 0, font_bold: 0, detailed: false,
        }
    }
    fn log(&mut self, line: &str) {
        self.logs.push(format!("· {}", line));
        if self.logs.len() > 400 { self.logs.remove(0); self.log_idx = self.log_idx.saturating_sub(1); }
    }
}

static SHARED: OnceLock<Mutex<Shared>> = OnceLock::new();
fn shared() -> &'static Mutex<Shared> {
    SHARED.get_or_init(|| Mutex::new(Shared::new()))
}

pub struct GuiReporter;
impl Reporter for GuiReporter {
    fn progress(&self, p: &Progress) {
        if let Ok(mut s) = shared().lock() { s.progress = p.clone(); }
    }
    fn log(&self, line: &str) {
        if let Ok(mut s) = shared().lock() { s.log(line); }
    }
}

fn slog(line: &str) { GuiReporter.log(line); }
fn serror(msg: String) {
    if let Ok(mut s) = shared().lock() { s.last_error = msg; s.log("✗ 操作未成功，详见上方提示"); }
}
fn spopup_yesno(msg: String) {
    if let Ok(mut s) = shared().lock() { s.popup_yesno = Some(msg); }
}

fn spawn_task<F: FnOnce() + Send + 'static>(label: &str, f: F) {
    {
        let mut s = match shared().lock() { Ok(x) => x, Err(_) => return };
        if s.busy {
            drop(s);
            w::msg_box("本地朗读工厂 · Mini", "已有任务在进行中，请等它完成。", w::MB_ICONINFORMATION);
            return;
        }
        s.busy = true;
        s.log(&format!("▶ {}", label));
        s.progress = Progress {
            state: "downloading".into(), source: label.into(),
            message: format!("{}…", label), ..Progress::idle()
        };
    }
    thread::spawn(move || {
        f();
        if let Ok(mut s) = shared().lock() { s.busy = false; }
    });
}

fn read_cfg_from_ui() {
    let mut s = match shared().lock() { Ok(x) => x, Err(_) => return };
    let c = s.ctrl;
    if c.cmb_device != 0 {
        let i = w::combo_sel(c.cmb_device);
        s.cfg.device = match i { 1 => "cpu", 2 => "gpu", 3 => "hybrid", _ => "auto" }.into();
    }
    if c.cmb_gpu != 0 {
        let i = w::combo_sel(c.cmb_gpu);
        s.cfg.gpu_index = if i <= 0 { -1 } else { i - 1 };
    }
    if c.cmb_source != 0 {
        let i = w::combo_sel(c.cmb_source);
        let ids = ["auto", "github", "gh-proxy", "ghfast", "ghproxy-net", "gitmirror",
                   "hf-mirror", "huggingface", "custom"];
        s.cfg.preferred_source = ids.get(i.max(0) as usize).unwrap_or(&"auto").to_string();
    }
    if c.cmb_drive != 0 {
        let i = w::combo_sel(c.cmb_drive);
        if let Some(d) = s.drives.get(i.max(0) as usize) {
            s.cfg.model_root = model::model_root_for(d.letter);
        }
    }
    if c.cmb_model != 0 {
        let i = w::combo_sel(c.cmb_model);
        if let Some(m) = model::models().get(i.max(0) as usize) {
            s.selected_model = m.id.into();
        }
    }
    if c.edit_input != 0 { s.input = w::get_text(c.edit_input); }
}

fn apply_cfg_to_ui() {
    let mut s = match shared().lock() { Ok(x) => x, Err(_) => return };
    let c = s.ctrl;
    if c.cmb_device == 0 { return; }
    let idx = match s.cfg.device.as_str() { "cpu" => 1, "gpu" => 2, "hybrid" => 3, _ => 0 };
    w::set_combo_sel(c.cmb_device, idx);
    w::set_combo_sel(c.cmb_gpu, if s.cfg.gpu_index < 0 { 0 } else { (s.cfg.gpu_index + 1).min(3) });
    let ids = ["auto", "github", "gh-proxy", "ghfast", "ghproxy-net", "gitmirror",
               "hf-mirror", "huggingface", "custom"];
    let si = ids.iter().position(|x| *x == s.cfg.preferred_source).unwrap_or(0) as i32;
    w::set_combo_sel(c.cmb_source, si);
    w::set_text(c.edit_input, &s.input);
}

fn rebuild_source_combo() {
    let mut s = match shared().lock() { Ok(x) => x, Err(_) => return };
    let c = s.ctrl;
    if c.cmb_source == 0 { return; }
    w::clear_combo(c.cmb_source);
    w::add_combo(c.cmb_source, "自动选最快");
    for src in model::sources() {
        let lat = s.source_latency.iter().find(|(id, _)| *id == src.id).map(|(_, v)| *v);
        let label = match lat {
            Some(v) if v >= 0.0 => format!("{} · {:.0} ms ✓", src.name, v),
            Some(_) => format!("{} · 不可达", src.name),
            None => src.name.to_string(),
        };
        w::add_combo(c.cmb_source, &label);
    }
    let ids = ["auto", "github", "gh-proxy", "ghfast", "ghproxy-net", "gitmirror",
               "hf-mirror", "huggingface", "custom"];
    let si = ids.iter().position(|x| *x == s.cfg.preferred_source).unwrap_or(0) as i32;
    w::set_combo_sel(c.cmb_source, si);
    s.refresh_sources = false;
}

// ---------------- 窗口过程 ----------------
unsafe extern "system" fn wnd_proc(h: w::HWND, m: u32, wp: w::WPARAM, lp: w::LPARAM) -> w::LRESULT {
    match m {
        w::WM_DESTROY => { w::kill_timer(h, TIMER_ID); w::post_quit(); return 0; }

        w::WM_CTLCOLORSTATIC => {
            let ctrl = lp as w::HWND;
            let s = shared().lock().unwrap();
            let is_card = ctrl == s.ctrl.card1 || ctrl == s.ctrl.card2 || ctrl == s.ctrl.card3;
            let brush = if is_card { s.white_brush } else { s.bg_brush };
            let fg = if ctrl == s.ctrl.lbl_hint { w::CLR_MUTED } else { w::CLR_TEXT };
            return w::color_ctl(wp as w::HDC, w::CLR_BG, fg, brush) as w::LRESULT;
        }
        w::WM_CTLCOLORDLG => {
            let s = shared().lock().unwrap();
            return s.bg_brush as w::LRESULT;
        }
        w::WM_CTLCOLOREDIT | w::WM_CTLCOLORBTN => {
            let s = shared().lock().unwrap();
            return w::color_ctl(wp as w::HDC, w::CLR_WHITE, w::CLR_TEXT, s.white_brush) as w::LRESULT;
        }

        w::WM_DROPFILES => {
            let files = w::drag_query(wp as isize);
            let mut epub: Option<String> = None;
            let mut json: Option<String> = None;
            for f in files {
                if f.to_ascii_lowercase().ends_with(".epub") && epub.is_none() { epub = Some(f); }
                else if f.to_ascii_lowercase().ends_with(".json") && json.is_none() { json = Some(f); }
            }
            if let Some(ref ep) = epub {
                let p = ep.clone();
                let s = shared().lock().unwrap();
                w::set_text(s.ctrl.edit_input, &p);
                drop(s);
                if let Ok(mut s) = shared().lock() { s.input = p.clone(); s.log(&format!("已拖入电子书：{}", p)); }
            }
            if let Some(ref jp) = json {
                let p = jp.clone();
                match Config::import(&p) {
                    Ok(c) => {
                        if let Ok(mut s) = shared().lock() { s.cfg = c; }
                        apply_cfg_to_ui();
                        slog(&format!("已导入运行配置：{}", p));
                    }
                    Err(e) => slog(&format!("配置导入失败：{}", e)),
                }
            }
            if epub.is_none() && json.is_none() {
                slog("拖入的内容里没有 .epub 或 .json，请拖入电子书或配置文件");
            }
            return 0;
        }

        w::WM_TIMER => {
            // 一次加锁取出快照后立刻释放；绝不能在持锁时再次 lock（会死锁）
            // 或在持锁时弹 MessageBox（worker 会被卡住）。
            let (c, pct, msg, new_logs, busy, refresh, error, yesno) = {
                let mut s = shared().lock().unwrap();
                let c = s.ctrl;
                let p = s.progress.clone();
                let new_logs: Vec<String> = s.logs[s.log_idx.min(s.logs.len())..].to_vec();
                s.log_idx = s.logs.len();
                let refresh = std::mem::replace(&mut s.refresh_sources, false);
                let error = std::mem::take(&mut s.last_error);
                let yesno = s.popup_yesno.take();
                (c, p.pct, p.message, new_logs, s.busy, refresh, error, yesno)
            };
            if c.progress != 0 {
                w::set_progress(c.progress, pct);
                w::set_text(c.lbl_status, &msg);
                for line in new_logs { w::append_line(c.edit_log, &line); }
                for b in [c.btn_browse, c.btn_probe, c.btn_install, c.btn_generate,
                          c.btn_import_cfg, c.btn_save_cfg, c.btn_open_dir, c.btn_open_out] {
                    w::enable(b, !busy);
                }
            }
            if refresh { rebuild_source_combo(); }
            if !error.is_empty() {
                w::msg_box("本地朗读工厂 · Mini · 提示", &error, w::MB_ICONERROR | w::MB_OK);
            }
            if let Some(q) = yesno {
                let r = w::msg_box("生成完成", &q, w::MB_ICONINFORMATION | w::MB_YESNO);
                if r == w::IDYES {
                    let p = shared().lock().unwrap().last_out_dir.clone();
                    w::shell_open(&p);
                }
            }
            return 0;
        }

        w::WM_COMMAND => {
            let id = (wp & 0xFFFF) as i32;
            let code = ((wp >> 16) & 0xFFFF) as u32;
            if code == 1 { // CBN_SELCHANGE：即时落配置
                read_cfg_from_ui();
                if let Ok(mut s) = shared().lock() {
                    let _ = s.cfg.save();
                    w::set_text(s.ctrl.lbl_hint, &format!("当前：{} · 模型目录 {}",
                        s.cfg.device_display(), s.cfg.model_root));
                }
                return 0;
            }
            if code != 0 { return 0; }
            match id {
                BTN_BROWSE => {
                    if let Some(p) = w::open_file_dialog(h, "选择 EPUB 电子书",
                        "EPUB 电子书|*.epub|所有文件|*.*|", "epub") {
                        let s = shared().lock().unwrap();
                        w::set_text(s.ctrl.edit_input, &p);
                        drop(s);
                        if let Ok(mut s) = shared().lock() { s.input = p.clone(); s.log(&format!("已选择电子书：{}", p)); }
                    }
                }
                BTN_IMPORT_CFG => {
                    if let Some(p) = w::open_file_dialog(h, "导入运行配置 JSON",
                        "配置文件|*.json|所有文件|*.*|", "json") {
                        match Config::import(&p) {
                            Ok(c) => {
                                if let Ok(mut s) = shared().lock() { s.cfg = c; }
                                apply_cfg_to_ui();
                                slog(&format!("✓ 已导入配置：{}", p));
                            }
                            Err(e) => serror(format!("导入配置失败：{}", e)),
                        }
                    }
                }
                BTN_SAVE_CFG => {
                    read_cfg_from_ui();
                    let r = { shared().lock().unwrap().cfg.save() };
                    match r {
                        Ok(()) => slog(&format!("✓ 运行配置已保存到 {}", Config::path())),
                        Err(e) => serror(format!("保存配置失败：{}", e)),
                    }
                }
                BTN_PROBE => {
                    read_cfg_from_ui();
                    let (cfg, mid) = {
                        let s = shared().lock().unwrap();
                        (s.cfg.clone(), s.selected_model.clone())
                    };
                    spawn_task("自动测速选源", move || {
                        let rp = GuiReporter;
                        let entry = match model::find_model(&mid) {
                            Some(m) => m,
                            None => { serror(format!("未知模型：{}", mid)); return; }
                        };
                        let r = model::probe_sources(&entry, &cfg, &rp);
                        if let Ok(mut s) = shared().lock() {
                            s.source_latency = model::sources().iter().map(|src| {
                                let v = r.iter().find(|(id, _, _)| *id == src.id)
                                    .map(|(_, _, ms)| *ms).unwrap_or(-2.0);
                                (src.id.to_string(), v)
                            }).collect();
                            s.refresh_sources = true;
                        }
                        if r.is_empty() {
                            serror("所有下载源都不可达：请检查网络，或在配置里填 custom_source 自建镜像。".into());
                        } else {
                            let best = &r[0];
                            slog(&format!("✓ 自动适配完成：优先「{}」({:.0} ms)，共 {} 条可用线路",
                                model::source_name(&best.0), best.2, r.len()));
                            if let Ok(mut s) = shared().lock() { s.cfg.preferred_source = best.0.clone(); }
                        }
                        rp.progress(&Progress { state: "done".into(), pct: 100,
                            message: "测速完成，已自动选最快线路".into(), ..Progress::idle() });
                    });
                }
                BTN_INSTALL => {
                    read_cfg_from_ui();
                    let (cfg, mid) = {
                        let s = shared().lock().unwrap();
                        (s.cfg.clone(), s.selected_model.clone())
                    };
                    if cfg.model_root.trim().is_empty() {
                        serror("请先选择模型安装盘（C/D/E…）".into()); return 0;
                    }
                    spawn_task("下载并安装模型", move || {
                        let rp = GuiReporter;
                        match model::install_model(&mid, &cfg, &rp) {
                            Ok(sid) => {
                                slog(&format!("✓ 模型已就绪（来源：{}），目录：{}",
                                    model::source_name(&sid), cfg.model_root));
                                rp.progress(&Progress { state: "done".into(), pct: 100,
                                    message: "模型安装完成，可以生成朗读了".into(), ..Progress::idle() });
                            }
                            Err(e) => serror(format!("模型下载失败（已自动尝试所有内置源）：\n{}", e)),
                        }
                    });
                }
                BTN_OPEN_DIR => {
                    read_cfg_from_ui();
                    let root = shared().lock().unwrap().cfg.model_root.clone();
                    if root.is_empty() { serror("尚未选择模型安装盘".into()); }
                    else {
                        match model::ensure_model_root(&root) {
                            Ok(()) => { if !w::shell_open(&root) { serror(format!("无法打开目录：{}", root)); } }
                            Err(e) => serror(e),
                        }
                    }
                }
                BTN_GENERATE => {
                    read_cfg_from_ui();
                    let (cfg, input) = {
                        let s = shared().lock().unwrap();
                        (s.cfg.clone(), s.input.clone())
                    };
                    if input.trim().is_empty() {
                        serror("请先拖入 .epub 电子书，或点「浏览…」选择。".into()); return 0;
                    }
                    if !w::exists(&input) { serror(format!("找不到文件：{}", input)); return 0; }
                    spawn_task("生成整本有声书", move || {
                        let rp = GuiReporter;
                        match audio::generate_book(&input, &cfg, &rp, None) {
                            Ok(s) => {
                                if let Ok(mut sh) = shared().lock() { sh.last_out_dir = s.out_dir.clone(); }
                                slog(&format!("✓ 生成完成：{} 章 / {} 段 / {:.1} 秒 / 引擎 {}",
                                    s.chapters, s.paragraphs, s.seconds, s.engine));
                                spopup_yesno(format!(
                                    "有声书已生成：\n{}\n\n{} 章 · {:.1} 秒\n\n现在打开输出文件夹吗？",
                                    s.out_dir, s.chapters, s.seconds));
                            }
                            Err(e) => serror(format!("生成失败：{}", e)),
                        }
                    });
                }
                BTN_OPEN_OUT => {
                    let p = shared().lock().unwrap().last_out_dir.clone();
                    if p.is_empty() { serror("还没有生成过有声书".into()); }
                    else if !w::shell_open(&p) { serror(format!("无法打开目录：{}", p)); }
                }
                _ => {}
            }
            return 0;
        }
        _ => {}
    }
    w::def_proc(h, m, wp, lp)
}

// ---------------- 构建界面 ----------------
fn label(text: &str, x: i32, y: i32, w_: i32, h_: i32, parent: w::HWND,
         id: i32, bold: bool, muted: bool) -> w::HWND {
    let c = w::create_ctrl("STATIC", text, w::SS_LEFT, x, y, w_, h_, parent, id);
    let (font, muted_) = {
        let s = shared().lock().unwrap();
        (if bold { s.font_bold } else { s.font }, muted)
    };
    w::set_font(c, font);
    if muted_ { w::set_text(c, text); }
    c
}
fn button(text: &str, x: i32, y: i32, w_: i32, h_: i32, parent: w::HWND, id: i32) -> w::HWND {
    let c = w::create_ctrl("BUTTON", text, w::BS_PUSHBUTTON | w::WS_TABSTOP,
                           x, y, w_, h_, parent, id);
    w::set_font(c, shared().lock().unwrap().font);
    c
}
fn combo(x: i32, y: i32, w_: i32, h_: i32, parent: w::HWND, id: i32) -> w::HWND {
    let c = w::create_ctrl("COMBOBOX", "", w::CBS_DROPDOWNLIST | w::CBS_HASSTRINGS | w::WS_TABSTOP,
                           x, y, w_, h_, parent, id);
    w::set_font(c, shared().lock().unwrap().font);
    c
}

pub fn run_gui(initial_epub: Option<&str>, auto_start: bool, detailed: bool) {
    w::init_console_utf8();
    w::init_common_controls();

    {
        let mut s = shared().lock().unwrap();
        s.cfg = Config::load();
        s.detailed = detailed;
        s.bg_brush = w::make_brush(w::CLR_BG);
        s.white_brush = w::make_brush(w::CLR_WHITE);
        s.font = w::make_font(15, false, "Microsoft YaHei UI");
        s.font_bold = w::make_font(22, true, "Microsoft YaHei UI");
        s.drives = w::list_drives();
        if let Some(p) = initial_epub { s.input = p.to_string(); }
        // 自动适配：没选过盘就推荐剩余空间最大的盘
        if s.cfg.model_root.is_empty() && !s.drives.is_empty() {
            let (letter, free) = {
                let best = s.drives.iter().max_by_key(|d| d.free).unwrap();
                (best.letter, best.free)
            };
            s.cfg.model_root = model::model_root_for(letter);
            s.log(&format!("自动推荐模型安装盘 {}（可用 {:.1} GB），可在下拉里改到任意盘",
                letter, free as f64 / 1073741824.0));
        }
    }

    let atom = w::register_class("BookCtlMiniWnd", wnd_proc,
                                 shared().lock().unwrap().bg_brush);
    if atom == 0 {
        w::msg_box("本地朗读工厂 · Mini", "窗口类注册失败。", w::MB_ICONERROR);
        return;
    }
    let hwnd = w::create_window("BookCtlMiniWnd", "本地朗读工厂 · Mini",
        w::WS_OVERLAPPEDWINDOW | w::WS_CLIPCHILDREN,
        120, 80, 1000, 730);
    if hwnd == 0 {
        w::msg_box("本地朗读工厂 · Mini", "创建窗口失败。", w::MB_ICONERROR);
        return;
    }

    {
        let mut s = shared().lock().unwrap();
        let mut c = s.ctrl;
        // 卡片背景（先创建，位于底层）
        c.card1 = w::create_ctrl("STATIC", "", w::SS_LEFT, 20, 82, 944, 92, hwnd, CARD1);
        c.card2 = w::create_ctrl("STATIC", "", w::SS_LEFT, 20, 184, 944, 92, hwnd, CARD2);
        c.card3 = w::create_ctrl("STATIC", "", w::SS_LEFT, 20, 286, 944, 320, hwnd, CARD3);
        s.ctrl = c;
    }

    // 标题
    label("本地朗读工厂 · Mini", 24, 14, 700, 36, hwnd, 0, true, false);
    label("轻量入口 · 内置多下载源自动适配，模型可装到 C/D/E 任意盘", 24, 52, 940, 22, hwnd, 0, false, true);

    // 卡片 1：输入
    label("输入 · 支持从桌面直接拖入 .epub", 36, 92, 600, 20, hwnd, 0, false, true);
    let e_input = w::create_ctrl("EDIT", "", w::ES_AUTOHSCROLL | w::WS_BORDER | w::WS_TABSTOP,
        36, 118, 700, 30, hwnd, EDIT_INPUT);
    w::set_font(e_input, shared().lock().unwrap().font);
    // 不写占位文字：get_text 会把占位符误判成真实路径。提示由下方 label 承担。
    let b_browse = button("浏览…", 746, 118, 200, 30, hwnd, BTN_BROWSE);
    label("把 .epub 从桌面或文件夹直接拖进窗口即可，也可以拖入 config.json 导入配置",
          36, 154, 900, 18, hwnd, 0, false, true);

    // 卡片 2：运行配置
    label("运行配置 · 显卡 / CPU 等运行环境，可导入已有配置", 36, 194, 700, 20, hwnd, 0, false, true);
    let cm_device = combo(36, 220, 190, 200, hwnd, CMB_DEVICE);
    let cm_gpu = combo(236, 220, 190, 200, hwnd, CMB_GPU);
    let b_icfg = button("导入配置…", 436, 220, 130, 30, hwnd, BTN_IMPORT_CFG);
    let b_scfg = button("保存配置", 576, 220, 130, 30, hwnd, BTN_SAVE_CFG);
    let l_hint = label("当前：自动", 716, 224, 230, 20, hwnd, LBL_HINT, false, true);

    // 卡片 3：模型下载管理
    label("模型下载管理 · 多源自动适配 / 失败自动切换 / 实时进度", 36, 296, 800, 20, hwnd, 0, false, true);
    let cm_model = combo(36, 324, 300, 220, hwnd, CMB_MODEL);
    let cm_source = combo(346, 324, 260, 220, hwnd, CMB_SOURCE);
    let b_probe = button("自动测速选源", 616, 324, 150, 30, hwnd, BTN_PROBE);
    label("内置 8 条线路", 776, 328, 170, 20, hwnd, 0, false, true);
    let cm_drive = combo(36, 362, 470, 220, hwnd, CMB_DRIVE);
    let b_odir = button("打开模型目录", 516, 362, 170, 30, hwnd, BTN_OPEN_DIR);
    label("资源定位", 696, 366, 120, 20, hwnd, 0, false, true);
    let b_install = button("安装 / 下载模型", 36, 400, 180, 32, hwnd, BTN_INSTALL);
    let pb = w::create_ctrl("msctls_progress32", "", w::PBS_SMOOTH,
                            226, 404, 710, 22, hwnd, PROGRESS);
    let l_status = label("就绪", 36, 434, 908, 20, hwnd, LBL_STATUS, false, true);
    let e_log = w::create_ctrl("EDIT", "",
        w::ES_MULTILINE | w::ES_AUTOVSCROLL | w::ES_READONLY | w::WS_VSCROLL | w::WS_BORDER,
        36, 460, 908, 132, hwnd, EDIT_LOG);
    w::set_font(e_log, shared().lock().unwrap().font);
    w::send(e_log, w::EM_SETREADONLY, 1, 0);

    // 底部
    let b_gen = button("一键生成朗读", 20, 618, 240, 48, hwnd, BTN_GENERATE);
    let b_oout = button("打开输出文件夹", 270, 618, 190, 48, hwnd, BTN_OPEN_OUT);
    label("生成后在这里打开项目文档文件夹（分章节 WAV / manifest.json / chapters.txt）",
          476, 632, 480, 20, hwnd, 0, false, true);

    {
        let mut s = shared().lock().unwrap();
        let c = &mut s.ctrl;
        c.edit_input = e_input; c.btn_browse = b_browse;
        c.cmb_device = cm_device; c.cmb_gpu = cm_gpu;
        c.btn_import_cfg = b_icfg; c.btn_save_cfg = b_scfg;
        c.cmb_model = cm_model; c.cmb_source = cm_source; c.btn_probe = b_probe;
        c.cmb_drive = cm_drive; c.btn_install = b_install; c.btn_open_dir = b_odir;
        c.progress = pb; c.lbl_status = l_status; c.lbl_hint = l_hint;
        c.edit_log = e_log; c.btn_generate = b_gen; c.btn_open_out = b_oout;
    }

    // 填充下拉
    for t in ["自动（推荐）", "纯 CPU", "强制 GPU", "GPU + CPU 混合"] {
        w::add_combo(cm_device, t);
    }
    for t in ["自动选卡", "GPU 0", "GPU 1", "GPU 2"] {
        w::add_combo(cm_gpu, t);
    }
    for m in model::models() {
        w::add_combo(cm_model, &format!("{}  ·  {} MB", m.name, m.size_mb));
    }
    {
        let s = shared().lock().unwrap();
        let mi = model::models().iter().position(|m| m.id == s.selected_model).unwrap_or(1) as i32;
        w::set_combo_sel(cm_model, mi);
        for d in &s.drives { w::add_combo(cm_drive, &w::drive_label(d)); }
        if !s.drives.is_empty() {
            let cur = s.cfg.model_root.chars().next().unwrap_or('C');
            let di = s.drives.iter().position(|d| d.letter == cur).unwrap_or(0) as i32;
            w::set_combo_sel(cm_drive, di);
        }
    }
    rebuild_source_combo();
    apply_cfg_to_ui();
    {
        let s = shared().lock().unwrap();
        w::set_text(s.ctrl.lbl_hint, &format!("当前：{} · 模型目录 {}",
            s.cfg.device_display(), s.cfg.model_root));
    }
    slog("欢迎使用 Mini 轻量版：拖入 .epub → 选运行环境 → 装模型 → 一键生成。");
    slog(&format!("配置位置：{}", Config::path()));
    if model::detect_full_cli(&shared().lock().unwrap().cfg).is_some() {
        slog("已检测到重量级主线（Full），生成时会自动委托真 TTS 模型。");
    } else {
        slog("未检测到重量级主线：将使用 Windows 自带语音（SAPI）生成，装模型+Full 版可获真人级音质。");
    }
    let _ = shared().lock().unwrap().cfg.save();

    w::accept_files(hwnd, true);
    w::set_timer(hwnd, TIMER_ID, 200);
    w::show(hwnd, w::SW_SHOW);

    if auto_start && !shared().lock().unwrap().input.is_empty() {
        // 与 .NET 主线 --auto-start 语义一致：命令行传书后自动开始生成
        let (cfg, input) = {
            let s = shared().lock().unwrap();
            (s.cfg.clone(), s.input.clone())
        };
        spawn_task("命令行自动生成本书", move || {
            let rp = GuiReporter;
            match audio::generate_book(&input, &cfg, &rp, None) {
                Ok(s) => {
                    if let Ok(mut sh) = shared().lock() { sh.last_out_dir = s.out_dir.clone(); }
                    slog(&format!("✓ 自动生成完成：{} 章 / {:.1} 秒", s.chapters, s.seconds));
                }
                Err(e) => serror(format!("生成失败：{}", e)),
            }
        });
    }

    w::message_loop();
    {
        let s = shared().lock().unwrap();
        w::delete_object(s.bg_brush); w::delete_object(s.white_brush);
        w::delete_object(s.font); w::delete_object(s.font_bold);
    }
}
