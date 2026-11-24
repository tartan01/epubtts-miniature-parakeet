//! 手写 Win32 FFI 与轻量包装函数。
//! 只链接 Windows 系统自带的导入库（rust-lld self-contained 目录里都有），
//! 不依赖 windows/windows-sys crate、不依赖 MSVC / SDK / VC++ Redistributable。

use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr;

pub type HWND = isize;
pub type HANDLE = isize;
pub type HINSTANCE = isize;
pub type HBRUSH = isize;
pub type HFONT = isize;
pub type HDC = isize;
pub type HMENU = isize;
pub type HKEY = isize;
pub type LPARAM = isize;
pub type WPARAM = usize;
pub type LRESULT = isize;
pub type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

// ---------------- 常用常量（仅 mini 用到的） ----------------
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_SETFONT: u32 = 0x0030;
pub const WM_DROPFILES: u32 = 0x0233;
pub const WM_CTLCOLORSTATIC: u32 = 0x0138;
pub const WM_CTLCOLORBTN: u32 = 0x0135;
pub const WM_CTLCOLORDLG: u32 = 0x0136;
pub const WM_CTLCOLOREDIT: u32 = 0x0133;

pub const CB_ADDSTRING: u32 = 0x0143;
pub const CB_RESETCONTENT: u32 = 0x014B;
pub const CB_GETCURSEL: u32 = 0x0147;
pub const CB_SETCURSEL: u32 = 0x014E;
pub const CB_GETCOUNT: u32 = 0x0146;
pub const LB_ADDSTRING: u32 = 0x0180;
pub const LB_RESETCONTENT: u32 = 0x0184;
pub const EM_SETSEL: u32 = 0x00B1;
pub const EM_REPLACESEL: u32 = 0x00C2;
pub const EM_SETREADONLY: u32 = 0x00CF;
pub const PBM_SETPOS: u32 = 0x0402;
pub const PBM_SETMARQUEE: u32 = 0x040A;
pub const STM_SETICON: u32 = 0x0170;

pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const WS_TABSTOP: u32 = 0x0001_0000;
pub const WS_BORDER: u32 = 0x0080_0000;
pub const WS_VSCROLL: u32 = 0x0020_0000;
pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
pub const WS_CLIPCHILDREN: u32 = 0x0200_0000;
pub const ES_MULTILINE: u32 = 0x0004;
pub const ES_AUTOHSCROLL: u32 = 0x0080;
pub const ES_AUTOVSCROLL: u32 = 0x0040;
pub const ES_READONLY: u32 = 0x0800;
pub const CBS_DROPDOWNLIST: u32 = 0x0003;
pub const CBS_HASSTRINGS: u32 = 0x0200;
pub const SS_LEFT: u32 = 0x0000;
pub const BS_PUSHBUTTON: u32 = 0x0000;
pub const BS_DEFPUSHBUTTON: u32 = 0x0001;
pub const PBS_SMOOTH: u32 = 0x0001;
pub const ICC_PROGRESS_CLASS: u32 = 0x0000_0020;

pub const SW_SHOW: i32 = 5;
pub const SW_SHOWNORMAL: i32 = 1;
pub const IDC_ARROW: usize = 32512;
pub const TRANSPARENT: i32 = 1;
pub const MB_OK: u32 = 0;
pub const MB_ICONERROR: u32 = 0x10;
pub const MB_ICONINFORMATION: u32 = 0x40;
pub const MB_YESNO: u32 = 0x4;
pub const IDYES: i32 = 6;

pub const HKEY_CURRENT_USER: HKEY = 0x8000_0001isize as HKEY;
pub const KEY_WRITE: u32 = 0x0002_0006;
pub const KEY_READ: u32 = 0x0002_0019;
pub const REG_SZ: u32 = 1;
pub const REG_DWORD: u32 = 4;
pub const DRIVE_UNKNOWN: u32 = 0;
pub const DRIVE_REMOVABLE: u32 = 2;
pub const DRIVE_FIXED: u32 = 3;
pub const DRIVE_REMOTE: u32 = 4;
pub const DRIVE_CDROM: u32 = 5;
pub const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFF_FFFF;

pub const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
pub const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
pub const OFN_EXPLORER: u32 = 0x0008_0000;
pub const OFN_HIDEREADONLY: u32 = 0x0000_0004;

// 苹果风格浅色配色（COLORREF = 0x00BBGGRR）
pub const CLR_BG: u32 = 0x00EF_EDED;      // #EDEDEF（比 #F5F5F7 略深，让白卡有对比）
pub const CLR_WHITE: u32 = 0x00FF_FFFF;
pub const CLR_TEXT: u32 = 0x001D_1D1F;    // #1D1D1F
pub const CLR_MUTED: u32 = 0x008B_8686;   // #86868B (BGR)
pub const CLR_BLUE: u32 = 0x00E3_7100;    // #0071E3 (BGR)

// ---------------- 结构体 ----------------
#[repr(C)]
pub struct WndClassEx {
    pub cb_size: u32,
    pub style: u32,
    pub lpfn_wnd_proc: WndProc,
    pub cb_cls_extra: i32,
    pub cb_wnd_extra: i32,
    pub h_instance: HINSTANCE,
    pub h_icon: HANDLE,
    pub h_cursor: HANDLE,
    pub hbr_background: HBRUSH,
    pub lpsz_menu_name: *const u16,
    pub lpsz_class_name: *const u16,
    pub h_icon_sm: HANDLE,
}

#[repr(C)]
pub struct Msg {
    pub hwnd: HWND,
    pub message: u32,
    pub wparam: WPARAM,
    pub lparam: LPARAM,
    pub time: u32,
    pub pt_x: i32,
    pub pt_y: i32,
    pub l_private: u32,
}

#[repr(C)]
pub struct Rect { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }

#[repr(C)]
pub struct InitCommonControlsEx { pub dw_size: u32, pub dw_icc: u32 }

#[repr(C)]
pub struct OpenFileNameW {
    pub l_struct_size: u32,
    pub hwnd_owner: HWND,
    pub h_instance: HINSTANCE,
    pub lpstr_filter: *const u16,
    pub lpstr_custom_filter: *mut u16,
    pub n_max_cust_filter: u32,
    pub n_filter_index: u32,
    pub lpstr_file: *mut u16,
    pub n_max_file: u32,
    pub lpstr_file_title: *mut u16,
    pub n_max_file_title: u32,
    pub lpstr_initial_dir: *const u16,
    pub lpstr_title: *const u16,
    pub flags: u32,
    pub n_file_offset: u16,
    pub n_file_extension: u16,
    pub lpstr_def_ext: *const u16,
    pub l_cust_data: LPARAM,
    pub lpfn_hook: *mut c_void,
    pub lp_template_name: *const u16,
    pub pv_reserved: *mut c_void,
    pub dw_reserved: u32,
    pub flags_ex: u32,
}

// ---------------- FFI ----------------
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
    fn GetLogicalDriveStringsW(n: u32, buf: *mut u16) -> u32;
    fn GetDriveTypeW(root: *const u16) -> u32;
    fn GetDiskFreeSpaceExW(dir: *const u16, free_avail: *mut u64, total: *mut u64, total_free: *mut u64) -> i32;
    fn CreateDirectoryW(path: *const u16, sa: *mut c_void) -> i32;
    fn GetFileAttributesW(path: *const u16) -> u32;
    fn DragQueryFileW(hdrop: isize, idx: u32, buf: *mut u16, cch: u32) -> u32;
    fn DragFinish(hdrop: isize);
    fn DeleteFileW(path: *const u16) -> i32;
    fn RemoveDirectoryW(path: *const u16) -> i32;
    fn Sleep(ms: u32);
    fn GetTickCount64() -> u64;
    fn GetTempPathW(n: u32, buf: *mut u16) -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    fn SetConsoleOutputCP(cp: u32) -> i32;
    fn SetConsoleCP(cp: u32) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(hwnd: HWND, text: *const u16, cap: *const u16, ty: u32) -> i32;
    fn RegisterClassExW(wc: *const WndClassEx) -> u16;
    fn CreateWindowExW(ex: u32, cls: *const u16, name: *const u16, style: u32,
                       x: i32, y: i32, w: i32, h: i32, parent: HWND, menu: HMENU,
                       inst: HINSTANCE, param: *mut c_void) -> HWND;
    fn ShowWindow(hwnd: HWND, cmd: i32) -> i32;
    fn UpdateWindow(hwnd: HWND) -> i32;
    fn GetMessageW(msg: *mut Msg, hwnd: HWND, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> LRESULT;
    fn PostQuitMessage(code: i32);
    fn DefWindowProcW(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT;
    fn SendMessageW(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT;
    fn GetDlgItem(hwnd: HWND, id: i32) -> HWND;
    fn SetWindowTextW(hwnd: HWND, text: *const u16) -> i32;
    fn GetWindowTextW(hwnd: HWND, buf: *mut u16, max: i32) -> i32;
    fn GetWindowTextLengthW(hwnd: HWND) -> i32;
    fn EnableWindow(hwnd: HWND, enable: i32) -> i32;
    fn DestroyWindow(hwnd: HWND) -> i32;
    fn SetTimer(hwnd: HWND, id: usize, elapse: u32, proc: *mut c_void) -> usize;
    fn KillTimer(hwnd: HWND, id: usize) -> i32;
    fn GetClientRect(hwnd: HWND, r: *mut Rect) -> i32;
    fn MoveWindow(hwnd: HWND, x: i32, y: i32, w: i32, h: i32, repaint: i32) -> i32;
    fn LoadCursorW(inst: HINSTANCE, name: *const u16) -> HANDLE;
    fn SetForegroundWindow(hwnd: HWND) -> i32;
    fn DragAcceptFiles(hwnd: HWND, accept: i32);
    fn SetWindowLongPtrW(hwnd: HWND, idx: i32, val: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: HWND, idx: i32) -> isize;
    fn InvalidateRect(hwnd: HWND, r: *const Rect, erase: i32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(c: u32) -> HBRUSH;
    fn CreateFontW(h: i32, w: i32, esc: i32, ori: i32, weight: i32,
                   italic: u32, underline: u32, strike: u32, charset: u32,
                   out_prec: u32, clip_prec: u32, quality: u32, pitch: u32,
                   face: *const u16) -> HFONT;
    fn DeleteObject(h: isize) -> i32;
    fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    fn SetTextColor(hdc: HDC, c: u32) -> u32;
    fn SetBkColor(hdc: HDC, c: u32) -> u32;
}

#[link(name = "comctl32")]
extern "system" { fn InitCommonControlsEx(p: *const InitCommonControlsEx) -> i32; }

#[link(name = "comdlg32")]
extern "system" { fn GetOpenFileNameW(p: *mut OpenFileNameW) -> i32; }

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(hwnd: HWND, op: *const u16, file: *const u16,
                     params: *const u16, dir: *const u16, show: i32) -> isize;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegCreateKeyExW(hkey: HKEY, sub: *const u16, reserved: u32, class: *const u16,
                       options: u32, sam: u32, sa: *mut c_void,
                       out: *mut HKEY, disp: *mut u32) -> i32;
    fn RegSetValueExW(hkey: HKEY, name: *const u16, reserved: u32, ty: u32,
                      data: *const u8, len: u32) -> i32;
    fn RegCloseKey(hkey: HKEY) -> i32;
    fn RegOpenKeyExW(hkey: HKEY, sub: *const u16, opt: u32, sam: u32, out: *mut HKEY) -> i32;
    fn RegQueryValueExW(hkey: HKEY, name: *const u16, reserved: *mut u32, ty: *mut u32,
                        data: *mut u8, len: *mut u32) -> i32;
    fn RegDeleteTreeW(hkey: HKEY, sub: *const u16) -> i32;
    fn RegDeleteValueW(hkey: HKEY, name: *const u16) -> i32;
}

// ---------------- 基础工具 ----------------
/// CLI 输出中文不乱码：把控制台代码页切到 UTF-8（65001）。
pub fn init_console_utf8() {
    unsafe { SetConsoleOutputCP(65001); SetConsoleCP(65001); }
}

pub fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }

pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

pub fn instance() -> HINSTANCE { unsafe { GetModuleHandleW(ptr::null()) } }

pub fn msg_box(title: &str, text: &str, flags: u32) -> i32 {
    let t = wide(title); let x = wide(text);
    unsafe { MessageBoxW(0, x.as_ptr(), t.as_ptr(), flags) }
}

pub fn send(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe { SendMessageW(hwnd, msg, wp, lp) }
}

pub fn set_text(hwnd: HWND, s: &str) {
    let w = wide(s);
    unsafe { SetWindowTextW(hwnd, w.as_ptr()); }
}

pub fn get_text(hwnd: HWND) -> String {
    unsafe {
        let n = GetWindowTextLengthW(hwnd);
        if n <= 0 { return String::new(); }
        let mut buf = vec![0u16; n as usize + 2];
        GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        from_wide(&buf)
    }
}

pub fn add_combo(hwnd: HWND, s: &str) -> i32 {
    let w = wide(s);
    send(hwnd, CB_ADDSTRING, 0, w.as_ptr() as LPARAM) as i32
}

pub fn clear_combo(hwnd: HWND) { send(hwnd, CB_RESETCONTENT, 0, 0); }
pub fn combo_sel(hwnd: HWND) -> i32 { send(hwnd, CB_GETCURSEL, 0, 0) as i32 }
pub fn set_combo_sel(hwnd: HWND, i: i32) { send(hwnd, CB_SETCURSEL, i as WPARAM, 0); }
pub fn combo_count(hwnd: HWND) -> i32 { send(hwnd, CB_GETCOUNT, 0, 0) as i32 }

pub fn set_font(hwnd: HWND, font: HFONT) { send(hwnd, WM_SETFONT, font as WPARAM, 1); }

pub fn set_progress(hwnd: HWND, pct: u32) { send(hwnd, PBM_SETPOS, pct.min(100) as WPARAM, 0); }

pub fn append_line(hwnd: HWND, line: &str) {
    // Windows Edit 控件换行必须用 \r\n；漏掉会把所有日志挤成一段。
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    send(hwnd, EM_SETSEL, len as WPARAM, len as LPARAM);
    let w = wide(&format!("{}\r\n", line));
    send(hwnd, EM_REPLACESEL, 0, w.as_ptr() as LPARAM);
}

pub fn make_font(size: i32, bold: bool, face: &str) -> HFONT {
    let f = wide(face);
    unsafe {
        CreateFontW(-size, 0, 0, 0, if bold { 700 } else { 400 },
                    0, 0, 0, 1 /*DEFAULT_CHARSET*/, 0, 0, 0, 0, f.as_ptr())
    }
}

pub fn make_brush(color: u32) -> HBRUSH { unsafe { CreateSolidBrush(color) } }

pub fn init_common_controls() {
    let icc = InitCommonControlsEx {
        dw_size: std::mem::size_of::<InitCommonControlsEx>() as u32,
        dw_icc: ICC_PROGRESS_CLASS,
    };
    unsafe { InitCommonControlsEx(&icc); }
}

pub fn register_class(name: &str, proc: WndProc, bg: HBRUSH) -> u16 {
    let cls = wide(name);
    let wc = WndClassEx {
        cb_size: std::mem::size_of::<WndClassEx>() as u32,
        style: 0,
        lpfn_wnd_proc: proc,
        cb_cls_extra: 0,
        cb_wnd_extra: 0,
        h_instance: instance(),
        h_icon: 0,
        h_cursor: unsafe { LoadCursorW(0, IDC_ARROW as *const u16) },
        hbr_background: bg,
        lpsz_menu_name: ptr::null(),
        lpsz_class_name: cls.as_ptr(),
        h_icon_sm: 0,
    };
    unsafe { RegisterClassExW(&wc) }
}

pub fn create_window(class: &str, title: &str, style: u32,
                     x: i32, y: i32, w: i32, h: i32) -> HWND {
    let c = wide(class); let t = wide(title);
    unsafe {
        CreateWindowExW(0, c.as_ptr(), t.as_ptr(), style, x, y, w, h,
                        0, 0, instance(), ptr::null_mut())
    }
}

pub fn create_ctrl(class: &str, text: &str, style: u32,
                   x: i32, y: i32, w: i32, h: i32, parent: HWND, id: i32) -> HWND {
    let c = wide(class); let t = wide(text);
    unsafe {
        CreateWindowExW(0, c.as_ptr(), t.as_ptr(), style | WS_CHILD | WS_VISIBLE,
                        x, y, w, h, parent, id as HMENU, instance(), ptr::null_mut())
    }
}

pub fn message_loop() {
    unsafe {
        let mut msg: Msg = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, 0, 0, 0);
            if r == 0 || r == -1 { break; }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn def_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(h, m, w, l) }
}
pub fn post_quit() { unsafe { PostQuitMessage(0); } }
pub fn show(h: HWND, cmd: i32) { unsafe { ShowWindow(h, cmd); UpdateWindow(h); SetForegroundWindow(h); } }
pub fn destroy(h: HWND) { unsafe { DestroyWindow(h); } }
pub fn enable(h: HWND, on: bool) { unsafe { EnableWindow(h, on as i32); } }
pub fn set_timer(h: HWND, id: usize, ms: u32) { unsafe { SetTimer(h, id, ms, ptr::null_mut()); } }
pub fn kill_timer(h: HWND, id: usize) { unsafe { KillTimer(h, id); } }
pub fn accept_files(h: HWND, on: bool) { unsafe { DragAcceptFiles(h, on as i32); } }
pub fn client_rect(h: HWND) -> (i32, i32) {
    let mut r: Rect = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(h, &mut r); }
    (r.right - r.left, r.bottom - r.top)
}
pub fn move_window(h: HWND, x: i32, y: i32, w: i32, hh: i32) {
    unsafe { MoveWindow(h, x, y, w, hh, 1); }
}
pub fn set_user_data(h: HWND, v: isize) { unsafe { SetWindowLongPtrW(h, -21, v); } }
pub fn get_user_data(h: HWND) -> isize { unsafe { GetWindowLongPtrW(h, -21) } }
pub fn invalidate(h: HWND) { unsafe { InvalidateRect(h, ptr::null(), 1); } }
pub fn sleep_ms(ms: u32) { unsafe { Sleep(ms); } }
pub fn tick_ms() -> u64 { unsafe { GetTickCount64() } }
pub fn delete_object(h: isize) { unsafe { DeleteObject(h); } }
pub fn color_ctl(hdc: HDC, bg: u32, fg: u32, brush: HBRUSH) -> HBRUSH {
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, fg);
        SetBkColor(hdc, bg);
    }
    brush
}

pub fn temp_dir() -> PathBuf {
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetTempPathW(buf.len() as u32, buf.as_mut_ptr()) };
    if n == 0 { return std::env::temp_dir(); }
    PathBuf::from(from_wide(&buf))
}

// ---------------- 拖放 ----------------
pub fn drag_query(hdrop: isize) -> Vec<String> {
    unsafe {
        let n = DragQueryFileW(hdrop, 0xFFFF_FFFF, ptr::null_mut(), 0);
        let mut out = Vec::new();
        for i in 0..n {
            let len = DragQueryFileW(hdrop, i, ptr::null_mut(), 0);
            let mut buf = vec![0u16; len as usize + 2];
            DragQueryFileW(hdrop, i, buf.as_mut_ptr(), buf.len() as u32);
            out.push(from_wide(&buf));
        }
        DragFinish(hdrop);
        out
    }
}

// ---------------- 文件对话框 ----------------
pub fn open_file_dialog(owner: HWND, title: &str, filter: &str, def_ext: &str) -> Option<String> {
    // filter 形如 "EPUB 电子书\0*.epub\0所有文件\0*.*\0\0"（用 | 分隔书写，这里转换）
    // 过滤器需要以双 NUL 结尾：调用方用 | 分隔并在末尾留一个 |
    let f = wide(&format!("{}\0", filter.replace('|', "\0")));
    let t = wide(title);
    let ext = wide(def_ext);
    let mut file = vec![0u16; 2048];
    let mut ofn: OpenFileNameW = unsafe { std::mem::zeroed() };
    ofn.l_struct_size = std::mem::size_of::<OpenFileNameW>() as u32;
    ofn.hwnd_owner = owner;
    ofn.lpstr_filter = f.as_ptr();
    ofn.lpstr_file = file.as_mut_ptr();
    ofn.n_max_file = file.len() as u32;
    ofn.lpstr_title = t.as_ptr();
    ofn.lpstr_def_ext = ext.as_ptr();
    ofn.flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_EXPLORER | OFN_HIDEREADONLY;
    let r = unsafe { GetOpenFileNameW(&mut ofn) };
    if r != 0 { Some(from_wide(&file)) } else { None }
}

// ---------------- Shell ----------------
pub fn shell_open(path: &str) -> bool {
    let op = wide("open"); let p = wide(path);
    let r = unsafe {
        ShellExecuteW(0, op.as_ptr(), p.as_ptr(), ptr::null(),
                      ptr::null(), SW_SHOWNORMAL)
    };
    r as i64 > 32
}

// ---------------- 文件系统 / 盘符 ----------------
pub struct Drive {
    pub letter: char,
    pub root: String,
    pub free: u64,
    pub total: u64,
    pub kind: u32,
}

pub fn list_drives() -> Vec<Drive> {
    unsafe {
        let mut buf = vec![0u16; 1024];
        let n = GetLogicalDriveStringsW(buf.len() as u32, buf.as_mut_ptr());
        if n == 0 || n >= buf.len() as u32 { return Vec::new(); }
        let raw = from_wide(&buf);
        let mut out = Vec::new();
        for part in raw.split('\0').filter(|x| !x.is_empty()) {
            let root = part.to_string();
            let letter = root.chars().next().unwrap_or('C');
            let w = wide(&root);
            let kind = GetDriveTypeW(w.as_ptr());
            let mut free = 0u64; let mut total = 0u64; let mut tf = 0u64;
            let ok = GetDiskFreeSpaceExW(w.as_ptr(), &mut free, &mut total, &mut tf);
            if ok == 0 { continue; }
            if kind == DRIVE_CDROM || kind == DRIVE_UNKNOWN || total == 0 { continue; }
            out.push(Drive { letter, root, free, total, kind });
        }
        out
    }
}

pub fn drive_label(d: &Drive) -> String {
    let kind = match d.kind {
        DRIVE_REMOVABLE => "可移动",
        DRIVE_REMOTE => "网络",
        DRIVE_FIXED => "本地磁盘",
        _ => "磁盘",
    };
    format!("{}: {} · 可用 {:.1} GB / 共 {:.1} GB",
            d.letter, kind,
            d.free as f64 / 1073741824.0,
            d.total as f64 / 1073741824.0)
}

pub fn ensure_dir(path: &str) -> std::io::Result<()> {
    let p = std::path::Path::new(path);
    if p.is_dir() { return Ok(()); }
    let w = wide(path);
    unsafe { CreateDirectoryW(w.as_ptr(), ptr::null_mut()); }
    // CreateDirectoryW 只建最后一级；对多层路径用 std::fs 更稳。
    std::fs::create_dir_all(p)
}

pub fn exists(path: &str) -> bool {
    let w = wide(path);
    unsafe { GetFileAttributesW(w.as_ptr()) != INVALID_FILE_ATTRIBUTES }
}

pub fn delete_file(path: &str) {
    let w = wide(path);
    unsafe { DeleteFileW(w.as_ptr()); }
}
pub fn delete_dir(path: &str) {
    let w = wide(path);
    unsafe { RemoveDirectoryW(w.as_ptr()); }
}

pub fn local_appdata() -> String {
    std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| format!("{}\\AppData\\Local",
            std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into())))
}

pub fn exe_path() -> PathBuf { std::env::current_exe().unwrap_or_default() }
pub fn exe_dir() -> PathBuf {
    exe_path().parent().map(|p| p.to_path_buf()).unwrap_or_default()
}

// ---------------- 注册表（HKCU，无需管理员） ----------------
pub fn reg_open_or_create(root: HKEY, sub: &str) -> Result<HKEY, String> {
    let w = wide(sub);
    let mut h: HKEY = 0; let mut disp = 0u32;
    let r = unsafe {
        RegCreateKeyExW(root, w.as_ptr(), 0, ptr::null(), 0, KEY_WRITE,
                        ptr::null_mut(), &mut h, &mut disp)
    };
    if r != 0 { Err(format!("RegCreateKeyExW 失败，代码 {}", r)) } else { Ok(h) }
}

pub fn reg_set_str(h: HKEY, name: &str, value: &str) -> Result<(), String> {
    let n = wide(name); let v = wide(value);
    let bytes = v.len() * 2;
    let r = unsafe {
        RegSetValueExW(h, n.as_ptr(), 0, REG_SZ, v.as_ptr() as *const u8, bytes as u32)
    };
    if r != 0 { Err(format!("RegSetValueExW({}) 失败，代码 {}", name, r)) } else { Ok(()) }
}

pub fn reg_set_dword(h: HKEY, name: &str, value: u32) -> Result<(), String> {
    let n = wide(name);
    let r = unsafe {
        RegSetValueExW(h, n.as_ptr(), 0, REG_DWORD,
                       &value as *const u32 as *const u8, 4)
    };
    if r != 0 { Err(format!("RegSetValueExW({}) 失败，代码 {}", name, r)) } else { Ok(()) }
}

pub fn reg_get_str(root: HKEY, sub: &str, name: &str) -> Option<String> {
    let s = wide(sub); let n = wide(name);
    let mut h: HKEY = 0;
    let r = unsafe { RegOpenKeyExW(root, s.as_ptr(), 0, KEY_READ, &mut h) };
    if r != 0 { return None; }
    let mut ty = 0u32; let mut len = 0u32;
    let r2 = unsafe {
        RegQueryValueExW(h, n.as_ptr(), ptr::null_mut(), &mut ty, ptr::null_mut(), &mut len)
    };
    let mut out = None;
    if r2 == 0 && len > 1 {
        let mut buf = vec![0u8; len as usize + 2];
        let r3 = unsafe {
            RegQueryValueExW(h, n.as_ptr(), ptr::null_mut(), &mut ty,
                             buf.as_mut_ptr(), &mut len)
        };
        if r3 == 0 {
            let u16s: Vec<u16> = buf[..len as usize]
                .chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            out = Some(from_wide(&u16s));
        }
    }
    unsafe { RegCloseKey(h); }
    out
}

pub fn reg_delete_tree(root: HKEY, sub: &str) {
    let w = wide(sub);
    unsafe { RegDeleteTreeW(root, w.as_ptr()); }
}
pub fn reg_delete_value(h: HKEY, name: &str) {
    let n = wide(name);
    unsafe { RegDeleteValueW(h, n.as_ptr()); }
}
pub fn reg_close(h: HKEY) { unsafe { RegCloseKey(h); } }
