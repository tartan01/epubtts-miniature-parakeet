//! 本地朗读工厂 · 自解压安装器（Mini / Full 共用同一份 stub）
//!
//! 构建时把便携包 zip 追加到 exe 尾部：
//!   <PE 文件体> <payload zip> <len:u64 LE> <magic "BCTLPKG1">
//! 运行时读自身尾部 → 解压到用户目录 → 写 HKCU 注册表 → 建快捷方式 → 关联 .epub。
//! 不依赖 Inno Setup / 管理员权限 / 任何第三方运行时。

use std::ffi::c_void;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

type HWND = isize;
type HKEY = isize;
const HKEY_CURRENT_USER: HKEY = 0x8000_0001isize as HKEY;
const KEY_WRITE: u32 = 0x0002_0006;
const REG_SZ: u32 = 1;
const REG_DWORD: u32 = 4;
const MB_OK: u32 = 0;
const MB_ICONERROR: u32 = 0x10;
const MB_ICONINFORMATION: u32 = 0x40;
const MAGIC: &[u8; 8] = b"BCTLPKG1";

#[link(name = "user32")]
extern "system" { fn MessageBoxW(h: HWND, text: *const u16, cap: *const u16, ty: u32) -> i32; }
#[link(name = "advapi32")]
extern "system" {
    fn RegCreateKeyExW(hkey: HKEY, sub: *const u16, reserved: u32, class: *const u16,
                       options: u32, sam: u32, sa: *mut c_void,
                       out: *mut HKEY, disp: *mut u32) -> i32;
    fn RegSetValueExW(hkey: HKEY, name: *const u16, reserved: u32, ty: u32,
                      data: *const u8, len: u32) -> i32;
    fn RegCloseKey(hkey: HKEY) -> i32;
    fn RegDeleteTreeW(hkey: HKEY, sub: *const u16) -> i32;
}

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
fn msgbox(title: &str, text: &str, flags: u32) {
    let t = wide(title); let x = wide(text);
    unsafe { MessageBoxW(0, x.as_ptr(), t.as_ptr(), flags); }
}

struct Reg(HKEY);
impl Reg {
    fn create(sub: &str) -> Result<Self, String> {
        let w = wide(sub);
        let mut h: HKEY = 0; let mut d = 0u32;
        let r = unsafe {
            RegCreateKeyExW(HKEY_CURRENT_USER, w.as_ptr(), 0, std::ptr::null(), 0,
                            KEY_WRITE, std::ptr::null_mut(), &mut h, &mut d)
        };
        if r != 0 { Err(format!("创建注册表项失败 {}（代码 {}）", sub, r)) } else { Ok(Self(h)) }
    }
    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        let n = wide(name); let v = wide(value);
        let r = unsafe { RegSetValueExW(self.0, n.as_ptr(), 0, REG_SZ,
                                        v.as_ptr() as *const u8, (v.len() * 2) as u32) };
        if r != 0 { Err(format!("写 {} 失败（代码 {}）", name, r)) } else { Ok(()) }
    }
    fn set_dword(&self, name: &str, value: u32) -> Result<(), String> {
        let n = wide(name);
        let r = unsafe { RegSetValueExW(self.0, n.as_ptr(), 0, REG_DWORD,
                                        &value as *const u32 as *const u8, 4) };
        if r != 0 { Err(format!("写 {} 失败（代码 {}）", name, r)) } else { Ok(()) }
    }
}
impl Drop for Reg {
    fn drop(&mut self) { unsafe { RegCloseKey(self.0); } }
}
fn reg_del_tree(sub: &str) {
    let w = wide(sub);
    unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, w.as_ptr()); }
}

#[derive(Clone)]
struct Meta {
    edition: String,
    app_name: String,
    exe_name: &'static str,
    version: &'static str,
    prog_id: &'static str,
}

fn meta_from_self() -> Meta {
    let name = std::env::current_exe().ok()
        .and_then(|p| p.file_name().map(|x| x.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    // 顺序有意义：阅读器安装包文件名含「阅读器」/Reader，必须最先判定。
    if name.contains("阅读器") || lower.contains("reader") {
        Meta { edition: "Reader".into(), app_name: "本地朗读工厂 阅读器".into(),
               exe_name: "BookCtl.Reader.exe", version: "0.6.0",
               prog_id: "LocalReadingFactory.Reader" }
    } else if name.contains("Mini") {
        Meta { edition: "Mini".into(), app_name: "本地朗读工厂 Mini".into(),
               exe_name: "BookCtl.Mini.exe", version: "0.6.0",
               prog_id: "LocalReadingFactory.Mini" }
    } else {
        Meta { edition: "Full".into(), app_name: "本地朗读工厂".into(),
               exe_name: "BookCtl.App.exe", version: "0.3.0",
               prog_id: "LocalReadingFactory.Full" }
    }
}

fn local_appdata() -> String {
    std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        format!("{}\\AppData\\Local",
            std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into()))
    })
}
fn app_data() -> String {
    std::env::var("APPDATA").unwrap_or_else(|_| {
        format!("{}\\AppData\\Roaming",
            std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into()))
    })
}
fn default_dir(m: &Meta) -> String {
    format!("{}\\本地朗读工厂\\{}", local_appdata(), m.edition)
}

/// 返回 (payload zip, stub 长度)。stub 长度用于生成不含 payload 的卸载程序。
fn read_overlay() -> Result<(Vec<u8>, u64), String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut f = File::open(&me).map_err(|e| format!("无法读取安装器自身：{}", e))?;
    let total = f.metadata().map_err(|e| e.to_string())?.len();
    if total < 32 { return Err("安装包不完整（文件过短）".into()); }
    f.seek(SeekFrom::End(-16)).map_err(|e| e.to_string())?;
    let mut tail = [0u8; 16];
    f.read_exact(&mut tail).map_err(|e| e.to_string())?;
    if &tail[8..16] != MAGIC {
        return Err("这不是完整的安装包（尾部缺少 payload 标记）。\n请确认下载的是 setup.exe 而不是单独的程序文件。".into());
    }
    let len = u64::from_le_bytes(tail[0..8].try_into().unwrap()) as usize;
    if len == 0 || len as u64 + 16 > total {
        return Err("payload 长度不合法，安装包可能已损坏".into());
    }
    let start = total - 16 - len as u64;
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf).map_err(|e| format!("读取 payload 失败：{}", e))?;
    if &buf[0..2] != b"PK" { return Err("payload 不是 zip 文件".into()); }
    Ok((buf, start))
}

fn extract(zip: &[u8], dir: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("创建安装目录失败：{}", e))?;
    let zp = PathBuf::from(std::env::temp_dir()).join(format!("bctl_setup_{}.zip", std::process::id()));
    fs::write(&zp, zip).map_err(|e| e.to_string())?;
    let tar = {
        let p = "C:\\Windows\\System32\\tar.exe";
        if Path::new(p).is_file() { p.to_string() } else { "tar.exe".into() }
    };
    let out = Command::new(&tar)
        .args(["-xf", &zp.to_string_lossy(), "-C", dir])
        .output().map_err(|e| format!("无法启动 tar.exe：{}", e))?;
    let _ = fs::remove_file(&zp);
    if !out.status.success() {
        return Err(format!("解压失败（tar 退出码 {:?}）：{}",
            out.status.code(), String::from_utf8_lossy(&out.stderr)));
    }
    Ok(())
}

fn create_shortcut(lnk: &str, target: &str, work: &str) -> Result<(), String> {
    // 用系统自带的 PowerShell + WScript.Shell COM 生成标准 .lnk
    let ps1 = std::env::temp_dir().join(format!("bctl_lnk_{}.ps1", std::process::id()));
    let script = "param($Lnk,$Target,$Work)\n\
$ErrorActionPreference='Stop'\n\
$s=New-Object -ComObject WScript.Shell\n\
$l=$s.CreateShortcut($Lnk)\n\
$l.TargetPath=$Target\n\
$l.WorkingDirectory=$Work\n\
$l.IconLocation=\"$Target,0\"\n\
$l.Save()\n";
    fs::write(&ps1, script.as_bytes()).map_err(|e| e.to_string())?;
    let pwsh = {
        let p = "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
        if Path::new(p).is_file() { p.to_string() } else { "powershell.exe".into() }
    };
    let out = Command::new(&pwsh).args([
        "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
        &ps1.to_string_lossy(), "-Lnk", lnk, "-Target", target, "-Work", work,
    ]).output().map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&ps1);
    if !out.status.success() {
        return Err(format!("创建快捷方式失败：{}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(())
}

fn write_registry(m: &Meta, dir: &str, exe: &str, model_root: &str, cli: &str,
                  uninstaller: &str) -> Result<(), String> {
    let base = format!("Software\\本地朗读工厂\\{}", m.edition);
    {
        let r = Reg::create(&base)?;
        r.set("InstallPath", dir)?;
        r.set("Version", m.version)?;
        r.set("Edition", &m.edition)?;
        r.set("ExePath", exe)?;
        r.set("ModelRoot", model_root)?;
        if !cli.is_empty() { r.set("CliPath", cli)?; }
    }
    // 标准"程序和功能"卸载项
    let un = format!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\本地朗读工厂 {}", m.edition);
    {
        let r = Reg::create(&un)?;
        r.set("DisplayName", &m.app_name)?;
        r.set("DisplayVersion", m.version)?;
        r.set("Publisher", "本地朗读工厂")?;
        r.set("InstallLocation", dir)?;
        r.set("DisplayIcon", exe)?;
        r.set("UninstallString", &format!("\"{}\" /UNINSTALL /S", uninstaller))?;
        r.set_dword("NoModify", 1)?;
        r.set_dword("NoRepair", 1)?;
        let mut size = 0u64;
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                if let Ok(md) = e.metadata() { if md.is_file() { size += md.len(); } }
            }
        }
        r.set_dword("EstimatedSize", (size / 1024).min(u32::MAX as u64) as u32)?;
    }
    // .epub 关联（OpenWithProgids，不抢占已有默认程序）
    {
        let r = Reg::create(&format!("Software\\Classes\\.epub\\OpenWithProgids"))?;
        r.set(m.prog_id, "")?;
    }
    {
        let r = Reg::create(&format!("Software\\Classes\\{}\\shell\\open\\command", m.prog_id))?;
        r.set("", &format!("\"{}\" \"%1\"", exe))?;
    }
    {
        let r = Reg::create(&format!("Software\\Classes\\{}", m.prog_id))?;
        r.set("", &m.app_name)?;
    }
    Ok(())
}

fn uninstall(m: &Meta, silent: bool, args: &[String]) {
    let from_tmp = args.iter().any(|a| a.eq_ignore_ascii_case("/FROM_TMP"));
    let dir = default_dir(m);

    // 卸载程序就待在自己要删的目录里，Windows 不允许删除运行中的 exe。
    // 两阶段：先把自己复制到 %TEMP%，由副本等待原进程退出后再删目录。
    if !from_tmp {
        let me = std::env::current_exe().ok();
        if let Some(me) = me {
            let tmp = std::env::temp_dir().join(format!("Uninstall-{}.exe", m.edition));
            if fs::copy(&me, &tmp).is_ok() {
                let mut ca: Vec<&str> = vec!["/UNINSTALL", "/FROM_TMP"];
                if silent { ca.push("/S"); }
                let spawned = Command::new(&tmp).args(&ca).spawn();
                if spawned.is_ok() { return; }
            }
        }
    } else {
        std::thread::sleep(Duration::from_millis(600));
    }

    // 保留用户数据：只删程序目录，模型根 / 输出目录不动
    if Path::new(&dir).exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    reg_del_tree(&format!("Software\\本地朗读工厂\\{}", m.edition));
    reg_del_tree(&format!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\本地朗读工厂 {}", m.edition));
    reg_del_tree(&format!("Software\\Classes\\{}", m.prog_id));
    // 开始菜单 + 桌面快捷方式（安装时两处都建了，卸载要两处都清）
    let start_dir = format!("{}\\Microsoft\\Windows\\Start Menu\\Programs\\本地朗读工厂", app_data());
    let _ = fs::remove_file(format!("{}\\{}.lnk", start_dir, m.app_name));
    let _ = fs::remove_dir(&start_dir);
    if let Ok(home) = std::env::var("USERPROFILE") {
        let _ = fs::remove_file(format!("{}\\Desktop\\{}.lnk", home, m.app_name));
    }
    if !silent {
        msgbox(&m.app_name, "卸载完成。\n\n已保留你的模型文件与已生成的有声书。", MB_ICONINFORMATION | MB_OK);
    }
}

fn main() {
    let m = meta_from_self();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let silent = args.iter().any(|a| a.eq_ignore_ascii_case("/S"));
    // 安装器 stub 头部被复制成 Uninstall-<Edition>.exe 时不带 payload，
    // 双击它（无参数）也必须能卸载，而不是掉进安装路径报「不是完整的安装包」。
    // 「程序和功能」走注册表里的 UninstallString（显式带 /UNINSTALL），两条路都覆盖。
    let me_name = std::env::current_exe().ok()
        .and_then(|p| p.file_name().map(|x| x.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let is_uninstaller_exe = me_name.to_ascii_uppercase().starts_with("UNINSTALL-");
    if args.iter().any(|a| a.eq_ignore_ascii_case("/UNINSTALL")) || is_uninstaller_exe {
        uninstall(&m, silent, &args);
        return;
    }
    let dir = args.iter().find_map(|a| a.strip_prefix("/D=").map(|x| x.to_string()))
        .unwrap_or_else(|| default_dir(&m));
    let model_root = args.iter().find_map(|a| a.strip_prefix("/MODELROOT=").map(|x| x.to_string()))
        .unwrap_or_default();

    let result: Result<String, String> = (|| {
        let (payload, stub_len) = read_overlay()?;
        extract(&payload, &dir)?;
        // Full 便携包自带 portable.marker；装成安装版要去掉，让数据走 %LOCALAPPDATA%
        let _ = fs::remove_file(Path::new(&dir).join("portable.marker"));
        let exe = Path::new(&dir).join(m.exe_name);
        if !exe.is_file() {
            return Err(format!("解压后找不到 {}，安装包内容可能不匹配", m.exe_name));
        }
        // 只把安装器 stub 头部（不含 payload）复制进安装目录作为卸载程序，
        // 否则 Full 版会再占一份 198 MB。
        let uninstaller = Path::new(&dir).join(format!("Uninstall-{}.exe", m.edition));
        {
            let me = std::env::current_exe().map_err(|e| e.to_string())?;
            let mut src = File::open(&me).map_err(|e| e.to_string())?;
            let mut dst = File::create(&uninstaller).map_err(|e| e.to_string())?;
            let mut limited = (&mut src).take(stub_len);
            std::io::copy(&mut limited, &mut dst).map_err(|e| e.to_string())?;
        }
        let exe_s = exe.to_string_lossy().into_owned();
        let uninstaller_s = uninstaller.to_string_lossy().into_owned();
        let cli = if m.edition == "Full" {
            let p = Path::new(&dir).join("tools\\BookCtl.Cli\\bookctl.exe");
            if p.is_file() { p.to_string_lossy().into_owned() } else { String::new() }
        } else { String::new() };
        write_registry(&m, &dir, &exe_s, &model_root, &cli, &uninstaller_s)?;

        // 开始菜单 + 桌面快捷方式
        let start_dir = format!("{}\\Microsoft\\Windows\\Start Menu\\Programs\\本地朗读工厂", app_data());
        let _ = fs::create_dir_all(&start_dir);
        let lnk = format!("{}\\{}.lnk", start_dir, m.app_name);
        let _ = create_shortcut(&lnk, &exe_s, &dir);
        if let Ok(desktop) = std::env::var("USERPROFILE") {
            let dlnk = format!("{}\\Desktop\\{}.lnk", desktop, m.app_name);
            let _ = create_shortcut(&dlnk, &exe_s, &dir);
        }
        Ok(exe_s)
    })();

    match result {
        Ok(exe) => {
            if !silent {
                msgbox(&m.app_name, &format!(
                    "安装完成。\n\n主程序：{}\n\n已写入用户目录与注册表，\n并创建了开始菜单 / 桌面快捷方式与 .epub 关联。\n\n模型文件默认存到：{}",
                    exe, if model_root.is_empty() { "（首次运行时在界面里选择任意盘）" } else { &model_root }),
                    MB_ICONINFORMATION | MB_OK);
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            if !silent { msgbox(&m.app_name, &format!("安装失败：\n\n{}", e), MB_ICONERROR | MB_OK); }
            std::process::exit(1);
        }
    }
}
