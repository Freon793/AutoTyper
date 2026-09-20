//! WM_CHAR 探针窗口 — 端到端验证 AutoTyper CLI 注入
//!
//! 创建一个可见窗口并置前台，把收到的每个 WM_CHAR 字符追加写入
//! probe_output.txt（UTF-8），收到 WM_KEYDOWN(VK_RETURN/VK_TAB) 时
//! 写入 [RETURN] / [TAB] 标记，超时或窗口关闭后退出。
//!
//! 用法（两个终端）：
//! ```text
//! cargo run --release --example probe            # 终端 1：启动探针并保持前台
//! target\release\AutoTyper.exe --cli --text "探针OK" --countdown 2   # 终端 2
//! ```
//! 探针退出后检查 probe_output.txt 内容。

#![allow(non_snake_case)]

use std::sync::Mutex;
use std::time::{Duration, Instant};

type HWND = isize;
type WPARAM = usize;
type LPARAM = isize;
type LRESULT = isize;

#[repr(C)]
struct WNDCLASSW {
    style: u32,
    lpfnWndProc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: isize,
    hIcon: isize,
    hCursor: isize,
    hbrBackground: isize,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct MSG {
    hwnd: isize,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    x: i32,
    y: i32,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(cls: *const WNDCLASSW) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: HWND,
        menu: isize,
        instance: isize,
        param: *mut core::ffi::c_void,
    ) -> HWND;
    fn DefWindowProcW(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT;
    fn PeekMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32, remove: u32) -> i32;
    fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    fn PostQuitMessage(code: i32);
    fn DestroyWindow(hwnd: HWND) -> i32;
    fn SetForegroundWindow(hwnd: HWND) -> i32;
    fn GetForegroundWindow() -> HWND;
    fn ShowWindow(hwnd: HWND, cmd: i32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
    fn GetCurrentThreadId() -> u32;
    fn AttachThreadInput(attach: u32, attach_to: u32, attach_flag: i32) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn GetWindowThreadProcessId(hwnd: HWND, process_id: *mut u32) -> u32;
    fn BringWindowToTop(hwnd: HWND) -> i32;
    fn keybd_event(vk: u8, scan: u8, flags: u32, extra: usize);
}

const WM_KEYDOWN: u32 = 0x0100;
const WM_CHAR: u32 = 0x0102;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const VK_RETURN: usize = 0x0D;
const VK_TAB: usize = 0x09;
const PM_REMOVE: u32 = 1;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
const WS_VISIBLE: u32 = 0x10000000;
const SW_SHOW: i32 = 5;

/// 收到的字符缓冲（退出时一次性写入 probe_output.txt）
static BUFFER: Mutex<String> = Mutex::new(String::new());
static CLOSED: Mutex<bool> = Mutex::new(false);

unsafe extern "system" fn probe_wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_CHAR => {
            if let Some(ch) = char::from_u32(w as u32) {
                BUFFER.lock().unwrap().push(ch);
            }
            0
        }
        WM_KEYDOWN => {
            match w {
                VK_RETURN => BUFFER.lock().unwrap().push_str("[RETURN]"),
                VK_TAB => BUFFER.lock().unwrap().push_str("[TAB]"),
                _ => {}
            }
            DefWindowProcW(hwnd, msg, w, l)
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            *CLOSED.lock().unwrap() = true;
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    let timeout = std::env::args()
        .nth(1)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(20);

    unsafe {
        let class_name = wide("AutoTyperProbeWindow");
        let title = wide("AutoTyper Probe — 等待 WM_CHAR 注入");
        let hinst = GetModuleHandleW(std::ptr::null());

        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: probe_wndproc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 6, // COLOR_WINDOW + 1
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        if RegisterClassW(&wc) == 0 {
            eprintln!("RegisterClassW 失败");
            std::process::exit(1);
        }

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            200,
            200,
            480,
            320,
            0,
            0,
            hinst,
            std::ptr::null_mut(),
        );
        if hwnd == 0 {
            eprintln!("CreateWindowExW 失败");
            std::process::exit(1);
        }
        ShowWindow(hwnd, SW_SHOW);
        // 突破前台锁：ALT 键技巧 + AttachThreadInput 到当前前台线程
        const VK_MENU: u8 = 0x12;
        const KEYEVENTF_KEYUP: u32 = 2;
        keybd_event(VK_MENU, 0, 0, 0);
        keybd_event(VK_MENU, 0, KEYEVENTF_KEYUP, 0);

        let fg_hwnd = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg_hwnd, std::ptr::null_mut());
        let cur_thread = GetCurrentThreadId();
        let attached = if fg_thread != 0 && fg_thread != cur_thread {
            AttachThreadInput(cur_thread, fg_thread, 1)
        } else {
            0
        };
        let sfw = SetForegroundWindow(hwnd);
        BringWindowToTop(hwnd);
        if attached != 0 {
            AttachThreadInput(cur_thread, fg_thread, 0);
        }
        let fg_after = GetForegroundWindow();
        // 诊断：探针句柄 / SetForegroundWindow 结果 / 当前前台窗口
        std::fs::write(
            "probe_hwnd.txt",
            format!("hwnd={hwnd}\nsfw={sfw}\nfg_after={fg_after}\n"),
        )
        .ok();

        println!("探针窗口已创建并置前台，{timeout} 秒内等待注入...");

        // 消息循环（带超时）
        let start = Instant::now();
        loop {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) != 0 {
                if *CLOSED.lock().unwrap() {
                    break;
                }
                DispatchMessageW(&msg);
            }
            if *CLOSED.lock().unwrap() || start.elapsed() > Duration::from_secs(timeout) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        DestroyWindow(hwnd);

        let content = BUFFER.lock().unwrap().clone();
        std::fs::write("probe_output.txt", &content).expect("写 probe_output.txt 失败");
        println!("收到 {} 个字符，已写入 probe_output.txt", content.chars().count());
        println!("内容: {content}");
    }
}
