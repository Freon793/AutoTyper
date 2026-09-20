//! AutoTyper - 统一入口
//!
//! 默认启动 GUI 模式，加 --cli 参数切换为命令行模式。
//! release 构建为 windows 子系统（双击运行无控制台窗口），
//! CLI 模式启动时通过 AttachConsole 接回父终端输出。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if let Some(rest) = autotyper::cli::split_entry_flag(&args) {
        // CLI 模式：--cli 是入口开关，已从参数中剔除
        autotyper::win32::attach_parent_console();
        std::process::exit(autotyper::cli::run_cli(&rest));
    } else {
        // GUI 模式
        autotyper::gui::run_gui();
    }
}
