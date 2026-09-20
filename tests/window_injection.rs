//! 端到端注入测试：创建真实（不可见）窗口作为探针，
//! 用 InputEngine 向其注入文本，在窗口过程中记录收到的消息序列，
//! 验证 WM_CHAR / WM_KEYDOWN / WM_KEYUP 的完整投递顺序与 Unicode 码位。
//!
//! 对应 Python 版发布验证用的 WM_CHAR 探针目标程序。

#![allow(non_snake_case)]

use autotyper::engine::InputEngine;
use autotyper::win32::{GetForegroundWindow, VK_RETURN, VK_TAB, WM_CHAR, WM_KEYDOWN, WM_KEYUP};
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
    fn DestroyWindow(hwnd: HWND) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
}

/// 探针窗口收到的 (消息, wParam) 序列
static RECEIVED: Mutex<Vec<(u32, usize)>> = Mutex::new(Vec::new());

/// 序列化所有探针测试：RECEIVED 为进程级共享，cargo test 并行运行时
/// 各测试的事件会交叉污染，必须整个 run_probe 期间独占
static TEST_LOCK: Mutex<()> = Mutex::new(());

unsafe extern "system" fn probe_wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_CHAR || msg == WM_KEYDOWN || msg == WM_KEYUP {
        RECEIVED.lock().unwrap().push((msg, w));
    }
    DefWindowProcW(hwnd, msg, w, l)
}

const PM_REMOVE: u32 = 1;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 创建探针窗口，注入文本并泵消息，返回探针收到的事件序列
fn run_probe(payload: &str) -> (usize, Vec<(u32, usize)>) {
    // 毒锁恢复：即使某次测试 panic 也继续序列化后续测试
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    RECEIVED.lock().unwrap().clear();

    unsafe {
        let class_name = wide("AutoTyperProbeW");
        let title = wide("AutoTyper Probe");
        let hinst = GetModuleHandleW(std::ptr::null());

        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: probe_wndproc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 0,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        // 同一进程内重复注册会失败，忽略错误继续（类已存在即可用）
        RegisterClassW(&wc);

        // 不带 WS_VISIBLE：窗口真实存在但不可见
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            0,
            0,
            0,
            100,
            100,
            0,
            0,
            hinst,
            std::ptr::null_mut(),
        );
        assert_ne!(hwnd, 0, "CreateWindowExW 失败");

        let engine = InputEngine::new(0.0, 0.0);
        let sent = engine.type_text(
            hwnd,
            payload,
            None::<fn(usize, usize)>,
            None::<fn() -> bool>,
        );

        // 泵消息直到收满预期数量或超时（消息由本线程队列派发）
        let expected = count_expected_events(payload);
        let start = Instant::now();
        while RECEIVED.lock().unwrap().len() < expected && start.elapsed() < Duration::from_secs(5)
        {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) != 0 {
                DispatchMessageW(&msg);
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        let received = RECEIVED.lock().unwrap().clone();
        DestroyWindow(hwnd);
        (sent, received)
    }
}

/// 按 Python 版算法预期 payload 会产生的消息事件数
fn count_expected_events(payload: &str) -> usize {
    let lines: Vec<&str> = payload.split('\n').collect();
    let mut events = 0usize;
    for (idx, line) in lines.iter().enumerate() {
        if line.is_empty() {
            events += 2; // KEYDOWN + KEYUP
            continue;
        }
        for ch in line.chars() {
            events += if ch == '\t' { 2 } else { 1 };
        }
        if idx < lines.len() - 1 {
            events += 2;
        }
    }
    events
}

#[test]
fn injects_ascii_chars_in_order() {
    let (sent, received) = run_probe("abc");
    assert_eq!(sent, 3);
    assert_eq!(
        received,
        vec![
            (WM_CHAR, 'a' as u32 as usize),
            (WM_CHAR, 'b' as u32 as usize),
            (WM_CHAR, 'c' as u32 as usize),
        ]
    );
}

#[test]
fn injects_unicode_code_points() {
    let (sent, received) = run_probe("你好");
    assert_eq!(sent, 2);
    assert_eq!(received, vec![(WM_CHAR, 0x4F60), (WM_CHAR, 0x597D)]);
}

#[test]
fn newline_becomes_return_key_events() {
    let (sent, received) = run_probe("ab\ncd");
    assert_eq!(sent, 4);
    assert_eq!(
        received,
        vec![
            (WM_CHAR, 'a' as u32 as usize),
            (WM_CHAR, 'b' as u32 as usize),
            (WM_KEYDOWN, VK_RETURN as usize),
            (WM_KEYUP, VK_RETURN as usize),
            (WM_CHAR, 'c' as u32 as usize),
            (WM_CHAR, 'd' as u32 as usize),
        ]
    );
}

#[test]
fn tab_becomes_tab_key_events() {
    let (sent, received) = run_probe("a\tb");
    assert_eq!(sent, 3);
    assert_eq!(
        received,
        vec![
            (WM_CHAR, 'a' as u32 as usize),
            (WM_KEYDOWN, VK_TAB as usize),
            (WM_KEYUP, VK_TAB as usize),
            (WM_CHAR, 'b' as u32 as usize),
        ]
    );
}

#[test]
fn empty_line_still_sends_return() {
    // 与 Python 版行为一致：空行也发送 RETURN
    let (sent, received) = run_probe("a\n\nb");
    assert_eq!(sent, 2);
    assert_eq!(
        received,
        vec![
            (WM_CHAR, 'a' as u32 as usize),
            (WM_KEYDOWN, VK_RETURN as usize),
            (WM_KEYUP, VK_RETURN as usize),
            (WM_KEYDOWN, VK_RETURN as usize),
            (WM_KEYUP, VK_RETURN as usize),
            (WM_CHAR, 'b' as u32 as usize),
        ]
    );
}

#[test]
fn mixed_payload_full_sequence() {
    let (sent, received) = run_probe("ab\n你\t");
    assert_eq!(sent, 4);
    assert_eq!(
        received,
        vec![
            (WM_CHAR, 'a' as u32 as usize),
            (WM_CHAR, 'b' as u32 as usize),
            (WM_KEYDOWN, VK_RETURN as usize),
            (WM_KEYUP, VK_RETURN as usize),
            (WM_CHAR, 0x4F60),
            (WM_KEYDOWN, VK_TAB as usize),
            (WM_KEYUP, VK_TAB as usize),
        ]
    );
}

#[test]
fn foreground_window_handle_is_obtainable() {
    // 对应 Python 版 test_get_foreground_window_returns_int
    let hwnd = unsafe { GetForegroundWindow() };
    // 测试环境可能没有前台窗口（句柄为 0 也合法），只验证调用不崩溃
    let _ = hwnd;
}
