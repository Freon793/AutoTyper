//! AutoTyper - 智能字符输入助手
//!
//! 通过 PostMessageW 向目标窗口消息队列投递 WM_CHAR 消息，
//! 实现绕过剪贴板限制的字符级输入。
//!
//! 模块划分：
//! - [`win32`]：最小 Win32 API 封装（手写 FFI，不依赖 windows crate）
//! - [`engine`]：字符输入引擎
//! - [`config`]：配置文件与文本片段管理
//! - [`cli`]：命令行模式
//! - [`gui`]：图形界面模式（egui/eframe）

#[cfg(not(windows))]
compile_error!("AutoTyper 仅支持 Windows（依赖 user32.dll）");

pub mod cli;
pub mod config;
pub mod engine;
pub mod gui;
pub mod win32;

/// 版本号（与 Cargo.toml 保持一致）
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
