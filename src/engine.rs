//! AutoTyper - 输入引擎
//!
//! 通过 PostMessageW 向目标窗口消息队列投递 WM_CHAR 消息，
//! 实现绕过剪贴板限制的字符级输入。

use crate::win32::{
    GetForegroundWindow, PostMessageW, SetForegroundWindow, HWND, VK_RETURN, VK_TAB, WM_CHAR,
    WM_KEYDOWN, WM_KEYUP, WPARAM,
};
use std::thread::sleep;
use std::time::Duration;

/// 字符输入引擎
///
/// 通过 PostMessageW 向目标窗口逐字符发送 WM_CHAR 消息，
/// 绕过剪贴板和键盘事件拦截。
#[derive(Debug, Clone)]
pub struct InputEngine {
    /// 字符间隔（秒），默认 0.01
    pub interval: f64,
    /// 行间延迟（秒），默认 0.05
    pub line_delay: f64,
}

impl Default for InputEngine {
    fn default() -> Self {
        Self {
            interval: 0.01,
            line_delay: 0.05,
        }
    }
}

impl InputEngine {
    pub fn new(interval: f64, line_delay: f64) -> Self {
        Self {
            interval,
            line_delay,
        }
    }

    /// 向目标窗口发送单个 Unicode 字符，返回是否发送成功。
    pub fn send_char(&self, hwnd: HWND, ch: char) -> bool {
        unsafe { PostMessageW(hwnd, WM_CHAR, ch as u32 as WPARAM, 0) != 0 }
    }

    /// 向目标窗口发送按键事件（KeyDown + KeyUp）。
    pub fn send_key(&self, hwnd: HWND, vk_code: u32) -> bool {
        unsafe {
            PostMessageW(hwnd, WM_KEYDOWN, vk_code as WPARAM, 0);
        }
        sleep(Duration::from_secs_f64(0.005));
        unsafe {
            PostMessageW(hwnd, WM_KEYUP, vk_code as WPARAM, 0);
        }
        true
    }

    /// 向目标窗口逐字符输入文本，返回已发送的字符总数。
    ///
    /// - `progress`：进度回调，接收 `(已发送字符数, 总字符数)`
    /// - `cancel`：取消回调，返回 `true` 时立即中止并返回已发送字符数
    ///   （供「停止」按钮与鼠标紧急停止使用）
    pub fn type_text<F, G>(
        &self,
        hwnd: HWND,
        text: &str,
        mut progress: Option<G>,
        mut cancel: Option<F>,
    ) -> usize
    where
        F: FnMut() -> bool,
        G: FnMut(usize, usize),
    {
        let lines: Vec<&str> = text.split('\n').collect();
        let total_lines = lines.len();
        let total_chars = text.chars().count();
        let mut chars_sent: usize = 0;

        let cancelled = |cancel: &mut Option<F>| -> bool { cancel.as_mut().map_or(false, |f| f()) };

        for (line_idx, line) in lines.iter().enumerate() {
            if cancelled(&mut cancel) {
                break;
            }

            if line.is_empty() {
                self.send_key(hwnd, VK_RETURN);
                sleep(Duration::from_secs_f64(self.line_delay));
                continue;
            }

            for ch in line.chars() {
                if cancelled(&mut cancel) {
                    return chars_sent;
                }

                if ch == '\t' {
                    self.send_key(hwnd, VK_TAB);
                } else {
                    self.send_char(hwnd, ch);
                }
                sleep(Duration::from_secs_f64(self.interval));
                chars_sent += 1;

                if let Some(cb) = progress.as_mut() {
                    cb(chars_sent, total_chars);
                }
            }

            // 换行（最后一行除外）
            if line_idx < total_lines - 1 {
                self.send_key(hwnd, VK_RETURN);
                sleep(Duration::from_secs_f64(self.line_delay));
            }
        }

        chars_sent
    }

    /// 获取当前前台窗口句柄
    pub fn get_foreground_window() -> HWND {
        unsafe { GetForegroundWindow() }
    }

    /// 将指定窗口设为前台
    pub fn set_foreground_window(hwnd: HWND) -> bool {
        unsafe { SetForegroundWindow(hwnd) != 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// 向 hwnd=0 投递是线程消息，不触达任何真实窗口，可安全用于逻辑测试
    const NULL_HWND: HWND = 0;

    #[test]
    fn default_values() {
        let engine = InputEngine::default();
        assert_eq!(engine.interval, 0.01);
        assert_eq!(engine.line_delay, 0.05);
    }

    #[test]
    fn custom_values() {
        let engine = InputEngine::new(0.02, 0.1);
        assert_eq!(engine.interval, 0.02);
        assert_eq!(engine.line_delay, 0.1);
    }

    #[test]
    fn types_all_chars_without_cancel() {
        let engine = InputEngine::new(0.0, 0.0);
        let sent = engine.type_text(
            NULL_HWND,
            "abc",
            None::<fn(usize, usize)>,
            None::<fn() -> bool>,
        );
        assert_eq!(sent, 3);
    }

    #[test]
    fn progress_callback_counts_all_chars() {
        let engine = InputEngine::new(0.0, 0.0);
        let seen = Cell::new((0usize, 0usize));
        let sent = engine.type_text(
            NULL_HWND,
            "abcd",
            Some(|cur: usize, total: usize| seen.set((cur, total))),
            None::<fn() -> bool>,
        );
        assert_eq!(sent, 4);
        assert_eq!(seen.get(), (4, 4));
    }

    #[test]
    fn cancel_aborts_midway() {
        let engine = InputEngine::new(0.0, 0.0);
        let calls = Cell::new(0u32);
        let sent = engine.type_text(
            NULL_HWND,
            "abcdefghij",
            None::<fn(usize, usize)>,
            Some(|| {
                calls.set(calls.get() + 1);
                calls.get() > 4
            }),
        );
        // 回调顺序：行起始检查 1 次，随后每字符「先检查后发送」——
        // 第 2/3/4 次回调放行 a/b/c，第 5 次回调（>4）触发取消，故发送 3 个字符
        assert_eq!(sent, 3);
        assert!(sent < 10);
    }

    #[test]
    fn cancel_before_first_char() {
        let engine = InputEngine::new(0.0, 0.0);
        let sent = engine.type_text(NULL_HWND, "abcdef", None::<fn(usize, usize)>, Some(|| true));
        assert_eq!(sent, 0);
    }

    #[test]
    fn multi_line_cancel_checked_per_char_and_line() {
        let engine = InputEngine::new(0.0, 0.0);
        let checks = Cell::new(0u32);
        let sent = engine.type_text(
            NULL_HWND,
            "ab\ncd",
            None::<fn(usize, usize)>,
            Some(|| {
                checks.set(checks.get() + 1);
                false
            }),
        );
        assert_eq!(sent, 4);
        // 每个字符 + 每行起始都会检查
        assert!(checks.get() >= 4);
    }

    #[test]
    fn total_chars_includes_newlines() {
        // 与 Python len(text) 语义对齐：总数按全部字符（含 \n）计
        let engine = InputEngine::new(0.0, 0.0);
        let last_total = Cell::new(0usize);
        engine.type_text(
            NULL_HWND,
            "ab\ncd",
            Some(|_cur: usize, total: usize| last_total.set(total)),
            None::<fn() -> bool>,
        );
        assert_eq!(last_total.get(), 5);
    }

    #[test]
    fn unicode_chars_supported() {
        let engine = InputEngine::new(0.0, 0.0);
        let sent = engine.type_text(
            NULL_HWND,
            "你好，世界！∈∀",
            None::<fn(usize, usize)>,
            None::<fn() -> bool>,
        );
        // 8 个 Unicode 字符（含全角标点与数学符号）
        assert_eq!(sent, 8);
    }
}
