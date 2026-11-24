//! 本地朗读工厂 · Mini（Rust / Win32 原生 / 零外部依赖）
//!
//! 单 exe 三种入口：
//!   * 无参数 / .epub 路径 / --detailed / --auto-start  → GUI 轻量入口
//!   * capabilities / device / source / config / run / doctor  → CLI（JSON envelope）
//!   * mcp  → MCP stdio JSON-RPC（tools/list、tools/call）
//!
//! 契约与 .NET 主线一致：{"ok":bool,"api_version":"1.0","data"|"error":…}

mod audio;
mod model;
mod ui;
mod util;
mod win32;

use model::{Config, NullReporter, Progress, Reporter};
use std::io::{self, BufRead, Write};
use util::J;

const VERSION: &str = "0.5.0";

struct CliReporter { quiet: bool }
impl Reporter for CliReporter {
    fn progress(&self, p: &Progress) {
        if self.quiet { return; }
        // 下载按字节 + 速度显示；合成按"段"显示（message 里已有 n/m）
        if p.state == "downloading" && p.total > 102400 {
            eprintln!("[{}] {} {}%  {} / {} · {:.1} MB/s", p.state, p.source, p.pct,
                util::human_bytes(p.downloaded), util::human_bytes(p.total),
                p.speed / 1048576.0);
        } else {
            eprintln!("[{}] {} {}%  {}", p.state, p.source, p.pct, p.message);
        }
    }
    fn log(&self, line: &str) {
        if !self.quiet { eprintln!("{}", line); }
    }
}

fn out(j: &J, pretty: bool) {
    if pretty { println!("{}", util::pretty(j)); }
    else { println!("{}", util::stringify(j)); }
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key)
        .and_then(|i| args.get(i + 1)).cloned()
}
fn has(args: &[String], key: &str) -> bool { args.iter().any(|a| a == key) }

fn capabilities_data() -> J {
    let srcs: Vec<J> = model::sources().iter().map(|s| util::obj(vec![
        ("id", util::s(s.id)), ("name", util::s(s.name)), ("note", util::s(s.note)),
    ])).collect();
    let models: Vec<J> = model::models().iter().map(|m| util::obj(vec![
        ("id", util::s(m.id)), ("name", util::s(m.name)),
        ("engine", util::s(m.engine)), ("device", util::s(m.device)),
        ("size_mb", util::i(m.size_mb as i64)),
        ("lines", util::i(m.urls.len() as i64)),
    ])).collect();
    util::obj(vec![
        ("product", util::s("本地朗读工厂 Mini")),
        ("edition", util::s("Mini")),
        ("version", util::s(VERSION)),
        ("runtime", util::s("rust-win32-native")),
        ("dependencies", util::arr(vec![util::s("Windows 10/11 系统组件（curl.exe / tar.exe / PowerShell System.Speech）")])),
        ("commands", util::arr(vec![
            util::s("capabilities"), util::s("device list"), util::s("source list"),
            util::s("source probe <id>"), util::s("source pull <id> --root <path>"),
            util::s("source remove <id>"), util::s("config show"),
            util::s("config import <file>"), util::s("config set <k> <v>"),
            util::s("run --input <epub>"), util::s("doctor"), util::s("mcp"), util::s("gui"),
        ])),
        ("devices", util::arr(vec![util::s("auto"), util::s("cpu"), util::s("gpu"), util::s("hybrid")])),
        ("sources", util::arr(srcs)),
        ("models", util::arr(models)),
        ("mcp", util::obj(vec![
            ("protocol_version", util::s("2025-06-18")),
            ("tools", util::arr(vec![util::s("source_list"), util::s("source_pull"),
                util::s("config_show"), util::s("job_start"), util::s("book_inspect")])),
        ])),
        ("envelope", util::s("{\"ok\":bool,\"api_version\":\"1.0\",\"data\":...|\"error\":...}")),
    ])
}

fn device_data(cfg: &Config) -> J {
    let drives: Vec<J> = win32::list_drives().iter().map(|d| util::obj(vec![
        ("letter", util::s(&d.letter.to_string())),
        ("root", util::s(&d.root)),
        ("label", util::s(&win32::drive_label(d))),
        ("free_bytes", util::i(d.free as i64)),
        ("total_bytes", util::i(d.total as i64)),
        ("kind", util::i(d.kind as i64)),
    ])).collect();
    util::obj(vec![
        ("device", util::s(&cfg.device)),
        ("gpu_index", util::i(cfg.gpu_index as i64)),
        ("cpu_threads", util::i(cfg.cpu_threads as i64)),
        ("display", util::s(&cfg.device_display())),
        ("drives", util::arr(drives)),
        ("full_cli", util::s(&model::detect_full_cli(cfg).unwrap_or_default())),
    ])
}

fn source_data(cfg: &Config) -> J {
    let installed = model::read_installed(&cfg.model_root);
    let entries: Vec<J> = model::models().iter().map(|m| util::obj(vec![
        ("id", util::s(m.id)), ("name", util::s(m.name)),
        ("engine", util::s(m.engine)), ("size_mb", util::i(m.size_mb as i64)),
        ("lines", util::i(model::effective_urls(m, cfg).len() as i64)),
        ("installed", util::b(installed.iter().any(|x| x.str("id") == m.id))),
    ])).collect();
    util::obj(vec![
        ("model_root", util::s(&cfg.model_root)),
        ("sources", util::arr(model::sources().iter().map(|s| util::obj(vec![
            ("id", util::s(s.id)), ("name", util::s(s.name)),
        ])).collect())),
        ("models", util::arr(entries)),
        ("installed", util::arr(installed)),
    ])
}

fn doctor_data(cfg: &Config) -> J {
    let checks = vec![
        ("curl.exe", win32::exists("C:\\Windows\\System32\\curl.exe")),
        ("tar.exe", win32::exists("C:\\Windows\\System32\\tar.exe")),
        ("powershell.exe", win32::exists("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe")),
        ("config_dir", win32::exists(&Config::dir())),
        ("model_root", !cfg.model_root.is_empty() && win32::exists(&cfg.model_root)),
        ("full_cli", model::detect_full_cli(cfg).is_some()),
    ];
    util::obj(vec![
        ("ok", util::b(checks.iter().all(|(_, v)| *v))),
        ("checks", util::arr(checks.into_iter().map(|(k, v)| util::obj(vec![
            ("name", util::s(k)), ("ok", util::b(v)),
        ])).collect())),
        ("note", util::s("full_cli=false 属正常：Mini 会用 Windows 自带语音兜底；装了 Full 则委托真 TTS 模型。")),
    ])
}

fn config_set(cfg: &mut Config, k: &str, v: &str) -> Result<(), String> {
    match k {
        "device" | "device.mode" => cfg.device = v.into(),
        "gpu_index" | "gpu" => cfg.gpu_index = v.parse().unwrap_or(-1),
        "cpu_threads" | "threads" => cfg.cpu_threads = v.parse().unwrap_or(4),
        "voice" => cfg.voice = v.into(),
        "rate" => cfg.rate = v.parse().unwrap_or(1.0),
        "engine" => cfg.engine = v.into(),
        "model_root" | "root" => cfg.model_root = v.into(),
        "preferred_source" | "source" => cfg.preferred_source = v.into(),
        "custom_source" => cfg.custom_source = v.into(),
        "curl_path" => cfg.curl_path = v.into(),
        "full_cli" => cfg.full_cli = v.into(),
        _ => return Err(format!("未知配置项：{}", k)),
    }
    cfg.save()
}

fn run_command(args: &[String], cfg: &Config, pretty: bool, quiet: bool) -> i32 {
    let input = arg_value(args, "--input").or_else(|| arg_value(args, "-i"));
    let input = match input {
        Some(p) => p,
        None => { out(&util::err("ARGS_INVALID", "缺少 --input <epub>", "bookctl-mini run --input 书.epub"), pretty); return 2; }
    };
    if !win32::exists(&input) {
        out(&util::err("NOT_FOUND", &format!("找不到文件：{}", input), "检查路径"), pretty);
        return 3;
    }
    let rp = CliReporter { quiet };
    if has(args, "--dry-run") {
        match audio::parse_epub(&input, &rp) {
            Ok(b) => out(&util::ok(util::obj(vec![
                ("status", util::s("planned")), ("title", util::s(&b.title)),
                ("chapters", util::i(b.chapters.len() as i64)),
                ("chars", util::i(b.chars as i64)),
                ("dry_run", util::b(true)),
            ])), pretty),
            Err(e) => { out(&util::err("INGEST_FAILED", &e, "检查 EPUB 完整性"), pretty); return 5; }
        }
        return 0;
    }
    let mut c = cfg.clone();
    if let Some(d) = arg_value(args, "--device") { c.device = d; }
    if let Some(g) = arg_value(args, "--gpu") { c.gpu_index = g.parse().unwrap_or(-1); }
    if let Some(t) = arg_value(args, "--cpu-threads") { c.cpu_threads = t.parse().unwrap_or(c.cpu_threads); }
    if let Some(e) = arg_value(args, "--engine") { c.engine = e; }
    let outdir = arg_value(args, "--output").or_else(|| arg_value(args, "-o"));
    match audio::generate_book(&input, &c, &rp, outdir.as_deref()) {
        Ok(s) => {
            out(&util::ok(util::obj(vec![
                ("status", util::s("completed")), ("chapters", util::i(s.chapters as i64)),
                ("paragraphs", util::i(s.paragraphs as i64)),
                ("seconds", util::n((s.seconds * 100.0).round() / 100.0)),
                ("engine", util::s(&s.engine)), ("out_dir", util::s(&s.out_dir)),
                ("wav_bytes", util::i(s.wav_bytes as i64)),
            ])), pretty);
            0
        }
        Err(e) => { out(&util::err("SYNTH_FAILED", &e, "检查模型/系统语音，或先安装 Full 版"), pretty); 5 }
    }
}

fn cli(args: &[String]) -> i32 {
    win32::init_console_utf8();
    let pretty = has(args, "--pretty");
    let quiet = has(args, "--quiet");
    let cmd = args.first().map(|x| x.as_str()).unwrap_or("gui");
    let cmd = if cmd == "--capabilities" { "capabilities" }
              else if cmd == "--version" || cmd == "-V" { "version" }
              else if cmd == "--help" || cmd == "-h" { "help" }
              else if cmd == "--mcp" { "mcp" }
              else { cmd };
    let mut cfg = Config::load();

    match cmd {
        "capabilities" => { out(&util::ok(capabilities_data(), ), pretty); 0 }
        "version" => { out(&util::ok(util::s(VERSION)), pretty); 0 }
        "help" | "--help" => {
            println!("本地朗读工厂 Mini {} · 用法：", VERSION);
            println!("  BookCtl.Mini.exe                     打开图形界面（可直接拖入 .epub）");
            println!("  BookCtl.Mini.exe <book.epub>         打开界面并导入该书");
            println!("  BookCtl.Mini.exe capabilities       能力清单（JSON envelope）");
            println!("  BookCtl.Mini.exe device list        运行环境 / 盘符一览");
            println!("  BookCtl.Mini.exe source list        内置下载源与模型清单");
            println!("  BookCtl.Mini.exe source pull <id> --root <path>   下载模型到指定盘");
            println!("  BookCtl.Mini.exe config import <f>  导入运行配置");
            println!("  BookCtl.Mini.exe run --input <epub> [--device cpu|gpu|auto] [--dry-run]");
            println!("  BookCtl.Mini.exe mcp                MCP stdio 服务");
            println!("  BookCtl.Mini.exe doctor             环境自检");
            println!("");
            println!("提示：PowerShell 5.1 会把子进程 stderr 按 GBK 解码，中文日志显示为乱码。");
            println!("      这是宿主编解码问题，不是程序输出错误；先执行下面任一行即可正常显示：");
            println!("        chcp 65001");
            println!("        [Console]::OutputEncoding = [Text.Encoding]::UTF8");
            println!("      GUI 界面与 stdout 的 JSON 信封不受影响（始终是干净 UTF-8）。");
            0
        }
        "device" => {
            if args.get(1).map(|x| x.as_str()) == Some("list") {
                out(&util::ok(device_data(&cfg), ), pretty); 0
            } else { out(&util::err("ARGS_INVALID", "用法：device list", ""), pretty); 2 }
        }
        "source" => {
            match args.get(1).map(|x| x.as_str()).unwrap_or("list") {
                "list" => { out(&util::ok(source_data(&cfg), ), pretty); 0 }
                "probe" => {
                    let id = args.get(2).cloned().unwrap_or_default();
                    let rp = CliReporter { quiet };
                    match model::find_model(&id) {
                        Some(m) => {
                            let r = model::probe_sources(&m, &cfg, &rp);
                            out(&util::ok(util::arr(r.into_iter().map(|(s, u, ms)| util::obj(vec![
                                ("source", util::s(&s)), ("url", util::s(&u)),
                                ("latency_ms", util::n((ms * 10.0).round() / 10.0)),
                            ])).collect())), pretty); 0
                        }
                        None => { out(&util::err("NOT_FOUND", &format!("未知模型：{}", id), "source list"), pretty); 3 }
                    }
                }
                "pull" => {
                    let id = args.get(2).cloned().unwrap_or_default();
                    if let Some(r) = arg_value(args, "--root") { cfg.model_root = r; }
                    if cfg.model_root.is_empty() {
                        out(&util::err("ARGS_INVALID", "请先用 --root <盘符路径> 指定模型安装位置", "例如 --root D:\\本地朗读工厂模型"), pretty);
                        return 2;
                    }
                    let _ = cfg.save();
                    let rp = NullReporter;
                    match model::install_model(&id, &cfg, &rp) {
                        Ok(sid) => { out(&util::ok(util::obj(vec![
                            ("status", util::s("installed")), ("id", util::s(&id)),
                            ("source", util::s(&sid)), ("root", util::s(&cfg.model_root)),
                        ])), pretty); 0 }
                        Err(e) => { out(&util::err("DOWNLOAD_FAILED", &e, "换个源或检查网络"), pretty); 69 }
                    }
                }
                "remove" => {
                    let id = args.get(2).cloned().unwrap_or_default();
                    let root = &cfg.model_root;
                    let mut list = model::read_installed(root);
                    list.retain(|x| x.str("id") != id);
                    let p = model::installed_manifest_path(root);
                    let _ = std::fs::write(&p, util::pretty(&J::Arr(list)));
                    let _ = std::fs::remove_dir_all(
                        std::path::Path::new(root).join(model::find_model(&id)
                            .map(|m| m.target).unwrap_or(&id)));
                    out(&util::ok(util::obj(vec![("removed", util::s(&id))])), pretty); 0
                }
                x => { out(&util::err("ARGS_INVALID", &format!("未知子命令：{}", x), "source list"), pretty); 2 }
            }
        }
        "config" => {
            match args.get(1).map(|x| x.as_str()).unwrap_or("show") {
                "show" => { out(&util::ok(cfg.to_j()), pretty); 0 }
                "path" => { out(&util::ok(util::s(&Config::path())), pretty); 0 }
                "import" => {
                    let f = args.get(2).cloned().unwrap_or_default();
                    match Config::import(&f) {
                        Ok(c) => { out(&util::ok(c.to_j()), pretty); 0 }
                        Err(e) => { out(&util::err("CONFIG_INVALID", &e, "检查 JSON"), pretty); 2 }
                    }
                }
                "set" => {
                    let k = args.get(2).cloned().unwrap_or_default();
                    let v = args.get(3).cloned().unwrap_or_default();
                    match config_set(&mut cfg, &k, &v) {
                        Ok(()) => { out(&util::ok(cfg.to_j()), pretty); 0 }
                        Err(e) => { out(&util::err("CONFIG_INVALID", &e, "config show 看字段"), pretty); 2 }
                    }
                }
                x => { out(&util::err("ARGS_INVALID", &format!("未知子命令：{}", x), "config show"), pretty); 2 }
            }
        }
        "run" => run_command(args, &cfg, pretty, quiet),
        "doctor" => { out(&util::ok(doctor_data(&cfg), ), pretty); 0 }
        "mcp" => { mcp_loop(&cfg); 0 }
        "gui" | "--gui" => { ui::run_gui(None, false, false); 0 }
        // 兼容 .NET 主线的命令行参数：拖入 .epub / --detailed / --auto-start
        x if x.ends_with(".epub") => {
            let auto = has(args, "--auto-start");
            ui::run_gui(Some(x), auto, has(args, "--detailed")); 0
        }
        "--detailed" | "--auto-start" | "--on-top" => {
            let epub = args.iter().find(|a| a.ends_with(".epub")).cloned();
            ui::run_gui(epub.as_deref(), has(args, "--auto-start"), has(args, "--detailed")); 0
        }
        x => { out(&util::err("ARGS_INVALID", &format!("未知命令：{}", x), "运行 BookCtl.Mini.exe help"), pretty); 2 }
    }
}

// ---------------- MCP stdio ----------------
fn mcp_tool_defs() -> Vec<J> {
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
        def("source_list", "列出内置下载源、模型与已安装模型", vec![]),
        def("source_probe", "自动测速所有下载源", vec![("id", "string", "模型 id")]),
        def("source_pull", "下载模型到指定盘（失败自动切换源）", vec![
            ("id", "string", "模型 id"), ("root", "string", "模型根目录，如 D:\\本地朗读工厂模型")]),
        def("config_show", "查看当前运行配置", vec![]),
        def("book_inspect", "只读解析 EPUB，返回章节与字数", vec![("input", "string", "EPUB 路径")]),
        def("job_start", "生成整本有声书", vec![
            ("input", "string", "EPUB 路径"), ("device", "string", "auto|cpu|gpu|hybrid")]),
    ]
}

fn mcp_call(cfg: &Config, name: &str, a: &J) -> J {
    let rp = NullReporter;
    match name {
        "source_list" => source_data(cfg),
        "config_show" => cfg.to_j(),
        "source_probe" => {
            let id = a.str("id");
            match model::find_model(&id) {
                Some(m) => util::obj(vec![("sources", util::arr(
                    model::probe_sources(&m, cfg, &rp).into_iter().map(|(s, u, ms)| util::obj(vec![
                        ("source", util::s(&s)), ("url", util::s(&u)),
                        ("latency_ms", util::n(ms)),
                    ])).collect()))]),
                None => util::obj(vec![("error", util::s("未知模型"))]),
            }
        }
        "source_pull" => {
            let mut c = cfg.clone();
            let root = a.str("root"); if !root.is_empty() { c.model_root = root; }
            let id = a.str("id");
            match model::install_model(&id, &c, &rp) {
                Ok(s) => util::obj(vec![("status", util::s("installed")), ("source", util::s(&s))]),
                Err(e) => util::obj(vec![("status", util::s("failed")), ("error", util::s(&e))]),
            }
        }
        "book_inspect" => match audio::parse_epub(&a.str("input"), &rp) {
            Ok(b) => util::obj(vec![
                ("title", util::s(&b.title)), ("chapters", util::i(b.chapters.len() as i64)),
                ("chars", util::i(b.chars as i64)),
                ("chapter_titles", util::arr(b.chapters.iter().map(|c| util::s(&c.title)).collect())),
            ]),
            Err(e) => util::obj(vec![("error", util::s(&e))]),
        },
        "job_start" => {
            let mut c = cfg.clone();
            let d = a.str("device"); if !d.is_empty() { c.device = d; }
            match audio::generate_book(&a.str("input"), &c, &rp, None) {
                Ok(s) => util::obj(vec![
                    ("status", util::s("completed")), ("chapters", util::i(s.chapters as i64)),
                    ("seconds", util::n(s.seconds)), ("engine", util::s(&s.engine)),
                    ("out_dir", util::s(&s.out_dir)),
                ]),
                Err(e) => util::obj(vec![("status", util::s("failed")), ("error", util::s(&e))]),
            }
        }
        _ => util::obj(vec![("error", util::s(&format!("未知工具：{}", name)))]),
    }
}

fn mcp_loop(cfg: &Config) {
    win32::init_console_utf8();
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line { Ok(x) => x, Err(_) => break };
        if line.trim().is_empty() { continue; }
        let req = match util::parse(&line) { Ok(j) => j, Err(_) => continue };
        let id = req.get("id").cloned().unwrap_or(util::null());
        let method = req.str("method");
        // 通知类消息（无 id）不回复
        if req.get("id").is_none() { continue; }
        let result = match method.as_str() {
            "initialize" => util::obj(vec![
                ("protocolVersion", util::s("2025-06-18")),
                ("capabilities", util::obj(vec![("tools", util::obj(vec![("listChanged", util::b(false))]))])),
                ("serverInfo", util::obj(vec![
                    ("name", util::s("bookctl-mini")), ("version", util::s(VERSION)),
                ])),
            ]),
            "tools/list" => util::obj(vec![("tools", util::arr(mcp_tool_defs()))]),
            "tools/call" => {
                let p = req.get("params").cloned().unwrap_or(util::obj(vec![]));
                let name = p.str("name");
                let args = p.get("arguments").cloned().unwrap_or(util::obj(vec![]));
                let data = mcp_call(cfg, &name, &args);
                util::obj(vec![
                    ("content", util::arr(vec![util::obj(vec![
                        ("type", util::s("text")), ("text", util::s(&util::stringify(&data))),
                    ])])),
                    ("isError", util::b(false)),
                ])
            }
            "ping" => util::obj(vec![]),
            _ => util::obj(vec![]),
        };
        let resp = util::obj(vec![
            ("jsonrpc", util::s("2.0")), ("id", id), ("result", result),
        ]);
        let _ = writeln!(stdout, "{}", util::stringify(&resp));
        let _ = stdout.flush();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = cli(&args);
    std::process::exit(code);
}
