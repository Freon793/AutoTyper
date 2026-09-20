//! 最小 Win32 API 封装
//!
//! 手写 FFI 声明（仅覆盖本项目用到的少量函数），不引入 windows crate，
//! 避免庞大的依赖树与跨版本 API 漂移。所有句柄以 isize/usize 表示，
//! 保证跨线程传递安全（HWND 本身是进程内全局有效的句柄值）。

#![allow(non_snake_case)]

use std::sync::atomic::{AtomicBool, Ordering};

/// 窗口句柄
pub type HWND = isize;
pub type WPARAM = usize;
pub type LPARAM = isize;

// ---- 窗口消息常量 ----
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
pub const WM_CHAR: u32 = 0x0102;

// ---- 虚拟键码 ----
pub const VK_TAB: u32 = 0x09;
pub const VK_RETURN: u32 = 0x0D;

// ---- GetSystemMetrics 索引 ----
pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[link(name = "user32")]
extern "system" {
    pub fn PostMessageW(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> i32;
    pub fn GetForegroundWindow() -> HWND;
    pub fn SetForegroundWindow(hwnd: HWND) -> i32;
    pub fn GetCursorPos(point: *mut POINT) -> i32;
    pub fn GetSystemMetrics(index: i32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
    fn AttachConsole(process_id: u32) -> i32;
    fn GetStdHandle(n_std_handle: u32) -> isize;
    fn SetStdHandle(n_std_handle: u32, handle: isize) -> i32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        sa: *const core::ffi::c_void,
        disposition: u32,
        flags: u32,
        template: *mut core::ffi::c_void,
    ) -> isize;
}

const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const OPEN_EXISTING: u32 = 3;
const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5; // (u32)(-11)
const INVALID_HANDLE_VALUE: isize = -1;
const CTRL_C_EVENT: u32 = 0;

/// Ctrl+C 是否已按下（由控制台事件处理器置位）
pub static CTRL_C_PRESSED: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> i32 {
    if ctrl_type == CTRL_C_EVENT {
        CTRL_C_PRESSED.store(true, Ordering::SeqCst);
        1 // 已处理，阻止进程被直接终止
    } else {
        0
    }
}

/// 安装 Ctrl+C 处理器（幂等）。CLI 模式用它实现协作式中断。
pub fn install_ctrl_handler() {
    unsafe {
        SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);
    }
}

/// 将当前进程附加到父进程的控制台，并重新绑定 stdout。
///
/// release 构建为 windows 子系统（无控制台窗口），CLI 模式需要借此
/// 把输出接到调用它的终端上。附加失败（例如双击运行）时静默跳过。
pub fn attach_parent_console() {
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }
        // AttachConsole 之后必须重新打开 CONOUT$ 并覆盖标准输出句柄，
        // 否则 Rust 的 stdout 仍指向进程启动时的空句柄。
        let name: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
        let handle = CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            core::ptr::null(),
            OPEN_EXISTING,
            0,
            core::ptr::null_mut(),
        );
        if handle != INVALID_HANDLE_VALUE {
            SetStdHandle(STD_OUTPUT_HANDLE, handle);
            let _ = GetStdHandle(STD_OUTPUT_HANDLE);
        }
    }
}

/// 鼠标是否位于主屏幕四角（紧急停止判定）。
///
/// 与 pyautogui 的 fail-safe 行为对齐：四个角点各带少量容差。
pub fn mouse_at_screen_corner(tolerance: i32) -> bool {
    unsafe {
        let mut p = POINT::default();
        if GetCursorPos(&mut p) == 0 {
            return false;
        }
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        if w <= 0 || h <= 0 {
            return false;
        }
        [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]
            .iter()
            .any(|(cx, cy)| (p.x - cx).abs() <= tolerance && (p.y - cy).abs() <= tolerance)
    }
}
