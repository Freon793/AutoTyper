//! AutoTyper - 命令行模式
//!
//! 通过命令行参数控制文本输入。
//! 退出码约定（与 Python 版一致）：
//! - 0：成功（含 --help、--list-snippets）
//! - 1：输入错误（文本为空 / 文件不可读 / 片段不存在 / 未指定输入源）
//! - 2：用法解析错误（argparse 约定）或紧急停止（鼠标移至屏幕四角）
//! - 3：Ctrl+C 用户中断

use crate::config::{ConfigManager, Snippets};
use crate::engine::InputEngine;
use crate::win32::{install_ctrl_handler, mouse_at_screen_corner, CTRL_C_PRESSED};
use std::cell::Cell;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

/// 解析后的命令行选项
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CliOptions {
    pub text: Option<String>,
    pub file: Option<String>,
    pub snippet: Option<String>,
    pub interval: Option<f64>,
    pub line_delay: Option<f64>,
    pub countdown: Option<i64>,
    pub no_failsafe: bool,
    pub list_snippets: bool,
    pub help: bool,
}

/// 入口开关处理：`--cli` 是模式开关，必须从参数中剔除后再解析
/// （与 Python main.py 行为一致）。返回 None 表示未请求 CLI 模式。
pub fn split_entry_flag(args: &[String]) -> Option<Vec<String>> {
    if args.iter().any(|a| a == "--cli") {
        Some(args.iter().filter(|a| *a != "--cli").cloned().collect())
    } else {
        None
    }
}

pub fn help_text() -> String {
    format!(
        "\
AutoTyper - 智能字符输入助手

用法: AutoTyper --cli [选项]

输入源（三者互斥，必选其一）:
  -t, --text <文本>        直接指定要输入的文本
  -f, --file <路径>        从文件读取要输入的文本
  -s, --snippet <名称>     使用已保存的文本片段名称

选项:
  --interval <秒>          字符间隔，默认 0.01
  --line-delay <秒>        行间延迟，默认 0.05
  --countdown <秒>         启动前倒计时，默认 5
  --no-failsafe            禁用鼠标紧急停止
  --list-snippets          列出所有已保存的文本片段
  -h, --help               显示此帮助信息

使用示例:
  AutoTyper --cli --text \"Hello, World!\"
  AutoTyper --cli --file answer.py
  AutoTyper --cli --file answer.py --interval 0.02 --countdown 3
  AutoTyper --cli --snippet \"常用代码段\"
"
    )
}

fn take_value(args: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    // 支持 `--flag=value` 形式
    if let Some(eq) = args[*i].find('=') {
        return Ok(args[*i][eq + 1..].to_string());
    }
    *i += 1;
    args.get(*i)
        .cloned()
        .ok_or_else(|| format!("参数 {flag} 需要一个值"))
}

/// 解析命令行参数。Err 中为可直接打印的错误描述。
pub fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    let mut opts = CliOptions::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "-h" | "--help" => opts.help = true,
            "--list-snippets" => opts.list_snippets = true,
            "--no-failsafe" => opts.no_failsafe = true,
            "-t" | "--text" => {
                opts.text = Some(take_value(args, &mut i, arg)?);
            }
            "-f" | "--file" => {
                opts.file = Some(take_value(args, &mut i, arg)?);
            }
            "-s" | "--snippet" => {
                opts.snippet = Some(take_value(args, &mut i, arg)?);
            }
            "--interval" => {
                let v = take_value(args, &mut i, arg)?;
                opts.interval = Some(
                    v.parse::<f64>()
                        .map_err(|_| format!("参数 --interval 无效: {v}（应为秒数）"))?,
                );
            }
            "--line-delay" => {
                let v = take_value(args, &mut i, arg)?;
                opts.line_delay = Some(
                    v.parse::<f64>()
                        .map_err(|_| format!("参数 --line-delay 无效: {v}（应为秒数）"))?,
                );
            }
            "--countdown" => {
                let v = take_value(args, &mut i, arg)?;
                opts.countdown = Some(
                    v.parse::<i64>()
                        .map_err(|_| format!("参数 --countdown 无效: {v}（应为整数秒）"))?,
                );
            }
            other => {
                if let Some(long) = other.strip_prefix("--") {
                    if long.contains('=') {
                        // 形如 --text=xxx：按等号前缀重新匹配
                        let name = &long[..long.find('=').unwrap()];
                        let value = long[long.find('=').unwrap() + 1..].to_string();
                        match name {
                            "text" => opts.text = Some(value),
                            "file" => opts.file = Some(value),
                            "snippet" => opts.snippet = Some(value),
                            "interval" => {
                                opts.interval = Some(value.parse::<f64>().map_err(|_| {
                                    format!("参数 --interval 无效: {value}（应为秒数）")
                                })?)
                            }
                            "line-delay" => {
                                opts.line_delay = Some(value.parse::<f64>().map_err(|_| {
                                    format!("参数 --line-delay 无效: {value}（应为秒数）")
                                })?)
                            }
                            "countdown" => {
                                opts.countdown = Some(value.parse::<i64>().map_err(|_| {
                                    format!("参数 --countdown 无效: {value}（应为整数秒）")
                                })?)
                            }
                            _ => return Err(format!("无法识别的参数: {other}")),
                        }
                    } else {
                        return Err(format!("无法识别的参数: {other}"));
                    }
                } else {
                    return Err(format!("无法识别的参数: {other}"));
                }
            }
        }
        i += 1;
    }

    // 互斥组：--text / --file / --snippet
    let sources = [&opts.text, &opts.file, &opts.snippet]
        .iter()
        .filter(|o| o.is_some())
        .count();
    if sources > 1 {
        return Err("参数 --text、--file、--snippet 互斥，只能指定其一".to_string());
    }

    Ok(opts)
}

/// 渲染片段列表（与 Python 版输出格式一致）
pub fn format_snippet_list(snippets: &Snippets) -> String {
    if snippets.is_empty() {
        return "暂无保存的文本片段。".to_string();
    }
    let mut out = String::new();
    out.push_str(&format!("已保存的文本片段（共 {} 个）：\n", snippets.len()));
    out.push_str(&"-".repeat(40));
    out.push('\n');
    for (name, content) in snippets {
        let flat: String = content
            .chars()
            .map(|c| if c == '\n' { ' ' } else { c })
            .collect();
        let preview: String = flat.chars().take(50).collect();
        let suffix = if flat.chars().count() > 50 { "..." } else { "" };
        out.push_str(&format!("  • {name}: {preview}{suffix}\n"));
    }
    out.push_str(&"-".repeat(40));
    out
}

/// 渲染进度条行（30 格 █/░，与 Python 版一致）
pub fn progress_line(current: usize, total: usize) -> String {
    let bar_len = 30usize;
    let percent = if total > 0 {
        current as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    let filled = if total > 0 {
        bar_len * current / total
    } else {
        0
    };
    let filled = filled.min(bar_len);
    let bar: String = "█"
        .repeat(filled)
        .chars()
        .chain("░".repeat(bar_len - filled).chars())
        .collect();
    format!("[{bar}] {percent:.1}%")
}

/// 取消原因
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    /// 鼠标移至屏幕四角（紧急停止）
    FailSafe,
    /// Ctrl+C
    CtrlC,
}

/// 运行 CLI 模式，返回退出码。
pub fn run_cli(args: &[String]) -> i32 {
    install_ctrl_handler();

    let opts = match parse_args(args) {
        Ok(opts) => opts,
        Err(msg) => {
            println!("[错误] {msg}");
            print!("{}", help_text());
            return 2;
        }
    };

    if opts.help {
        print!("{}", help_text());
        return 0;
    }

    let config = ConfigManager::open_default();

    // 列出文本片段
    if opts.list_snippets {
        println!("{}", format_snippet_list(config.get_snippets()));
        return 0;
    }

    // 确定输入文本
    let text = if let Some(text) = &opts.text {
        text.clone()
    } else if let Some(path) = &opts.file {
        match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                println!("[错误] 无法读取文件: {e}");
                return 1;
            }
        }
    } else if let Some(name) = &opts.snippet {
        match config.get_snippets().get(name.as_str()) {
            Some(content) => content.clone(),
            None => {
                println!("[错误] 未找到文本片段: {name}");
                return 1;
            }
        }
    } else {
        println!("[错误] 请指定 --text、--file 或 --snippet");
        print!("{}", help_text());
        return 1;
    };

    if text.trim().is_empty() {
        println!("[错误] 输入文本为空");
        return 1;
    }

    // 确定参数（命令行优先，其次配置文件）
    let interval = opts.interval.unwrap_or(config.config.interval).max(0.0);
    let line_delay = opts.line_delay.unwrap_or(config.config.line_delay).max(0.0);
    let countdown = opts.countdown.unwrap_or(config.config.countdown).max(0);
    let failsafe = !opts.no_failsafe;

    let engine = InputEngine::new(interval, line_delay);

    // 倒计时
    println!("\n{countdown} 秒后开始输入...");
    println!("请将光标放到目标输入框中");
    println!("紧急停止：鼠标移至屏幕四角 或 Ctrl+C\n");

    for i in (1..=countdown).rev() {
        if CTRL_C_PRESSED.load(std::sync::atomic::Ordering::SeqCst) {
            println!("\n\n用户中断（Ctrl+C）");
            println!("已发送 0 个字符");
            return 3;
        }
        println!("  {i}...");
        // 分片睡眠以便及时响应 Ctrl+C
        for _ in 0..10 {
            sleep(Duration::from_millis(100));
            if CTRL_C_PRESSED.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
        }
    }

    if CTRL_C_PRESSED.load(std::sync::atomic::Ordering::SeqCst) {
        println!("\n\n用户中断（Ctrl+C）");
        println!("已发送 0 个字符");
        return 3;
    }

    println!("\n开始输入");

    let hwnd = InputEngine::get_foreground_window();
    println!("目标窗口句柄: {hwnd}");

    let reason: Cell<Option<CancelReason>> = Cell::new(None);

    let chars_sent = engine.type_text(
        hwnd,
        &text,
        Some(|current: usize, total: usize| {
            print!("\r  {}", progress_line(current, total));
            let _ = std::io::stdout().flush();
        }),
        Some(|| -> bool {
            if CTRL_C_PRESSED.load(std::sync::atomic::Ordering::SeqCst) {
                reason.set(Some(CancelReason::CtrlC));
                return true;
            }
            if failsafe && mouse_at_screen_corner(2) {
                reason.set(Some(CancelReason::FailSafe));
                return true;
            }
            false
        }),
    );

    match reason.get() {
        Some(CancelReason::FailSafe) => {
            println!("\n\n紧急停止（鼠标移至屏幕角落）");
            println!("已发送 {chars_sent} 个字符");
            2
        }
        Some(CancelReason::CtrlC) => {
            println!("\n\n用户中断（Ctrl+C）");
            println!("已发送 {chars_sent} 个字符");
            3
        }
        None => {
            println!("\n\n输入完成，共发送 {chars_sent} 个字符");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn split_entry_flag_strips_cli() {
        let rest = split_entry_flag(&argv(&["--cli", "--text", "hi"]));
        assert_eq!(rest, Some(argv(&["--text", "hi"])));
    }

    #[test]
    fn split_entry_flag_only_cli() {
        assert_eq!(split_entry_flag(&argv(&["--cli"])), Some(vec![]));
    }

    #[test]
    fn split_entry_flag_absent() {
        assert_eq!(split_entry_flag(&argv(&["--text", "hi"])), None);
    }

    #[test]
    fn parse_text_and_short_flag() {
        let opts = parse_args(&argv(&["--text", "hello"])).unwrap();
        assert_eq!(opts.text.as_deref(), Some("hello"));
        let opts = parse_args(&argv(&["-t", "hello"])).unwrap();
        assert_eq!(opts.text.as_deref(), Some("hello"));
        let opts = parse_args(&argv(&["--text=hello"])).unwrap();
        assert_eq!(opts.text.as_deref(), Some("hello"));
    }

    #[test]
    fn parse_numeric_options() {
        let opts = parse_args(&argv(&[
            "-t",
            "x",
            "--interval",
            "0.02",
            "--line-delay",
            "0.1",
            "--countdown",
            "3",
        ]))
        .unwrap();
        assert_eq!(opts.interval, Some(0.02));
        assert_eq!(opts.line_delay, Some(0.1));
        assert_eq!(opts.countdown, Some(3));
    }

    #[test]
    fn parse_boolean_flags() {
        let opts = parse_args(&argv(&["--list-snippets", "--no-failsafe", "-h"])).unwrap();
        assert!(opts.list_snippets);
        assert!(opts.no_failsafe);
        assert!(opts.help);
    }

    #[test]
    fn parse_mutually_exclusive_sources() {
        let err = parse_args(&argv(&["--text", "a", "--file", "b"])).unwrap_err();
        assert!(err.contains("互斥"));
    }

    #[test]
    fn parse_unknown_argument() {
        let err = parse_args(&argv(&["--bogus"])).unwrap_err();
        assert!(err.contains("无法识别的参数"));
    }

    #[test]
    fn parse_missing_value() {
        let err = parse_args(&argv(&["--text"])).unwrap_err();
        assert!(err.contains("需要一个值"));
    }

    #[test]
    fn parse_invalid_number() {
        let err = parse_args(&argv(&["-t", "x", "--interval", "abc"])).unwrap_err();
        assert!(err.contains("--interval 无效"));
        let err = parse_args(&argv(&["-t", "x", "--countdown", "1.5"])).unwrap_err();
        assert!(err.contains("--countdown 无效"));
    }

    #[test]
    fn snippet_list_empty() {
        let snippets = Snippets::new();
        assert_eq!(format_snippet_list(&snippets), "暂无保存的文本片段。");
    }

    #[test]
    fn snippet_list_format() {
        let mut snippets = Snippets::new();
        snippets.insert("片段A".to_string(), "line1\nline2".to_string());
        let out = format_snippet_list(&snippets);
        assert!(out.contains("已保存的文本片段（共 1 个）："));
        assert!(out.contains("  • 片段A: line1 line2"));
        assert!(out.contains(&"-".repeat(40)));
    }

    #[test]
    fn snippet_preview_truncated_at_50_chars() {
        let mut snippets = Snippets::new();
        let long: String = "字".repeat(60);
        snippets.insert("长片段".to_string(), long);
        let out = format_snippet_list(&snippets);
        let line = out.lines().find(|l| l.contains("长片段")).unwrap();
        // 50 个字符预览 + 省略号
        assert!(line.contains(&"字".repeat(50)));
        assert!(line.ends_with("..."));
        assert!(!line.contains(&"字".repeat(51)));
    }

    #[test]
    fn progress_bar_rendering() {
        assert_eq!(progress_line(0, 100), format!("[{}] 0.0%", "░".repeat(30)));
        let half = progress_line(50, 100);
        assert!(half.starts_with(&format!("[{}", "█".repeat(15))));
        assert!(half.ends_with("50.0%"));
        assert_eq!(
            progress_line(100, 100),
            format!("[{}] 100.0%", "█".repeat(30))
        );
        // total=0 不 panic
        assert_eq!(progress_line(0, 0), format!("[{}] 0.0%", "░".repeat(30)));
    }

    #[test]
    fn list_snippets_exits_zero_without_typing() {
        // --list-snippets 只列片段，不进入输入流程（不依赖桌面前台窗口）
        let code = run_cli(&argv(&["--list-snippets"]));
        assert_eq!(code, 0);
    }

    #[test]
    fn empty_text_returns_one() {
        let code = run_cli(&argv(&["--text", "   ", "--countdown", "0"]));
        assert_eq!(code, 1);
    }

    #[test]
    fn missing_snippet_returns_one() {
        let code = run_cli(&argv(&[
            "--snippet",
            "肯定不存在的片段名",
            "--countdown",
            "0",
        ]));
        assert_eq!(code, 1);
    }

    #[test]
    fn no_source_returns_one() {
        let code = run_cli(&argv(&["--countdown", "0"]));
        assert_eq!(code, 1);
    }

    #[test]
    fn unreadable_file_returns_one() {
        let code = run_cli(&argv(&[
            "--file",
            "Z:/不存在的路径/xx.txt",
            "--countdown",
            "0",
        ]));
        assert_eq!(code, 1);
    }

    #[test]
    fn usage_error_returns_two() {
        let code = run_cli(&argv(&["--text", "a", "--file", "b"]));
        assert_eq!(code, 2);
    }
}
