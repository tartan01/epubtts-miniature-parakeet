// 工具链自检：验证 windows-gnu + rust-lld 能链接 mini 需要的全部 Win32 导入库。
// 只调用不会阻塞的函数；其余符号用地址引用的方式保留，确保链接器真的加载了对应导入库。
use std::ffi::c_void;

#[link(name = "user32")]
extern "system" {
    fn GetSystemMetrics(index: i32) -> i32;
    fn MessageBoxW(hwnd: *mut c_void, text: *const u16, caption: *const u16, utype: u32) -> i32;
}

#[link(name = "gdi32")]
extern "system" { fn GetDeviceCaps(hdc: isize, index: i32) -> i32; }

#[link(name = "advapi32")]
extern "system" {
    fn RegCloseKey(key: isize) -> i32;
    fn RegOpenKeyExW(key: isize, sub: *const u16, opt: u32, sam: u32, out: *mut isize) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(hwnd: isize, op: *const u16, file: *const u16,
                     params: *const u16, dir: *const u16, show: i32) -> isize;
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
    fn CoCreateInstance(clsid: *const c_void, outer: *mut c_void, ctx: u32,
                        iid: *const c_void, out: *mut *mut c_void) -> i32;
}

#[link(name = "oleaut32")]
extern "system" { fn SysAllocString(s: *const u16) -> *mut u16; }

#[link(name = "comctl32")]
extern "system" { fn InitCommonControlsEx(picce: *mut c_void) -> i32; }

#[link(name = "comdlg32")]
extern "system" { fn GetOpenFileNameW(ofn: *mut c_void) -> i32; }

#[link(name = "winhttp")]
extern "system" { fn WinHttpOpen(agent: *const u16, access: u32, proxy: *const u16,
                                 bypass: *const u16, flags: u32) -> *mut c_void; }

fn main() {
    let w = unsafe { GetSystemMetrics(0) };
    let h = unsafe { GetSystemMetrics(1) };
    let hr = unsafe { CoInitializeEx(std::ptr::null_mut(), 0) };
    println!("win32-link-ok screen={}x{} coinit_hr={}", w, h, hr);
    // 保留符号引用，防止 --gc-sections 把导入库优化掉。
    let _keep = (MessageBoxW as usize) ^ (GetDeviceCaps as usize)
        ^ (RegCloseKey as usize) ^ (RegOpenKeyExW as usize)
        ^ (ShellExecuteW as usize) ^ (CoCreateInstance as usize)
        ^ (SysAllocString as usize) ^ (InitCommonControlsEx as usize)
        ^ (GetOpenFileNameW as usize) ^ (WinHttpOpen as usize);
    println!("symbols_kept={}", _keep != 0);
}
