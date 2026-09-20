//! AutoTyper - 图形界面模式
//!
//! 使用 egui/eframe 构建的桌面 GUI 应用（对应 Python 版的 tkinter 界面）。
//! 布局自上而下：菜单栏、文本编辑区、参数设置、按钮行、状态栏。
//! 输入工作在后台线程执行，通过共享状态与界面通信；
//! 「停止」按钮与鼠标四角紧急停止通过取消回调协作生效。

use crate::config::ConfigManager;
use crate::engine::InputEngine;
use crate::win32::mouse_at_screen_corner;
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

/// 工作线程与界面之间的共享状态
#[derive(Default)]
struct WorkerShared {
    /// 状态栏文本
    status: String,
    /// 进度百分比 0..=100
    progress: f32,
    /// 工作线程是否仍在运行
    running: bool,
}

pub struct AutoTyperApp {
    config: ConfigManager,
    text: String,

    interval_ms: i32,
    line_delay_ms: i32,
    countdown_s: i32,
    failsafe: bool,

    running: bool,
    stop_flag: Arc<AtomicBool>,
    shared: Arc<Mutex<WorkerShared>>,

    /// 片段命名模态框（None = 关闭）
    snippet_dialog: Option<String>,
}

impl AutoTyperApp {
    fn new() -> Self {
        let config = ConfigManager::open_default();
        Self {
            interval_ms: (config.config.interval * 1000.0).round() as i32,
            line_delay_ms: (config.config.line_delay * 1000.0).round() as i32,
            countdown_s: config.config.countdown.clamp(1, 30) as i32,
            failsafe: config.config.failsafe,
            config,
            text: String::new(),
            running: false,
            stop_flag: Arc::new(AtomicBool::new(false)),
            shared: Arc::new(Mutex::new(WorkerShared {
                status: "就绪".to_string(),
                progress: 0.0,
                running: false,
            })),
            snippet_dialog: None,
        }
    }

    // ---- 菜单动作 ----

    fn load_file(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("选择文本文件")
            .add_filter("所有文件", &["*"])
            .add_filter("文本文件", &["txt"])
            .add_filter("Python", &["py"])
            .add_filter("Java", &["java"])
            .add_filter("C/C++", &["c", "cpp", "h"])
            .pick_file();
        if let Some(path) = picked {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    self.text = content;
                    self.set_status(&format!("已加载: {}", path.display()));
                }
                Err(e) => {
                    error_dialog(&format!("无法读取文件:\n{e}"));
                }
            }
        }
    }

    fn save_text(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("保存文本")
            .add_filter("文本文件", &["txt"])
            .add_filter("所有文件", &["*"])
            .save_file();
        if let Some(path) = picked {
            match std::fs::write(&path, &self.text) {
                Ok(()) => self.set_status(&format!("已保存: {}", path.display())),
                Err(e) => error_dialog(&format!("无法保存文件:\n{e}")),
            }
        }
    }

    fn confirm_save_snippet(&mut self, name: String) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let content = self.text.trim().to_string();
        if content.is_empty() {
            warning_dialog("文本为空，无法保存");
            return;
        }
        self.config.add_snippet(&name, &content);
        self.set_status(&format!("已保存片段: {name}"));
    }

    fn import_snippets(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("导入文本片段")
            .add_filter("JSON", &["json"])
            .add_filter("所有文件", &["*"])
            .pick_file();
        if let Some(path) = picked {
            if self.config.import_snippets(&path) {
                self.set_status(&format!("已导入片段: {}", path.display()));
            } else {
                error_dialog("导入失败");
            }
        }
    }

    fn export_snippets(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("导出文本片段")
            .add_filter("JSON", &["json"])
            .set_file_name("snippets.json")
            .save_file();
        if let Some(path) = picked {
            if self.config.export_snippets(&path) {
                self.set_status(&format!("已导出片段: {}", path.display()));
            } else {
                error_dialog("导出失败");
            }
        }
    }

    fn show_help(&self) {
        let help_text = "\
使用说明：

1. 在上方文本框中输入或粘贴要输入的文本
2. 调节字符间隔、行间延迟和倒计时参数
3. 点击「开始输入」，迅速将光标放到目标输入框
4. 等待倒计时结束，自动开始输入
5. 输入过程中可点击「停止」立即中止；紧急情况可将鼠标移至屏幕四角

快捷键：
  Ctrl+O  打开文件
  Ctrl+S  保存文本
";
        rfd::MessageDialog::new()
            .set_title("使用说明")
            .set_description(help_text)
            .set_level(rfd::MessageLevel::Info)
            .show();
    }

    fn show_about(&self) {
        rfd::MessageDialog::new()
            .set_title("关于 AutoTyper")
            .set_description(&format!(
                "AutoTyper v{}\n\n智能字符输入助手\n通过 Windows 消息队列直接注入字符\n\n基于 MIT 许可证开源",
                crate::VERSION
            ))
            .set_level(rfd::MessageLevel::Info)
            .show();
    }

    // ---- 输入控制 ----

    fn start_typing(&mut self, _ctx: &egui::Context) {
        let text = self.text.trim().to_string();
        if text.is_empty() {
            warning_dialog("请输入要输入的文本");
            return;
        }

        self.running = true;
        self.stop_flag.store(false, Ordering::SeqCst);
        {
            let mut shared = self.shared.lock().unwrap();
            shared.progress = 0.0;
            shared.status = "准备中...".to_string();
            shared.running = true;
        }

        let interval = self.interval_ms.max(0) as f64 / 1000.0;
        let line_delay = self.line_delay_ms.max(0) as f64 / 1000.0;
        let countdown = self.countdown_s.max(0);
        let failsafe = self.failsafe;

        let stop_flag = Arc::clone(&self.stop_flag);
        let shared = Arc::clone(&self.shared);

        std::thread::spawn(move || {
            typing_worker(
                text,
                countdown,
                failsafe,
                interval,
                line_delay,
                stop_flag,
                shared,
                InputEngine::get_foreground_window,
            );
        });
    }

    fn stop_typing(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        self.set_status("正在停止...");
    }

    fn set_status(&mut self, text: &str) {
        self.shared.lock().unwrap().status = text.to_string();
    }

    // ---- 界面绘制 ----

    fn draw_menu(&mut self, ctx: &egui::Context) {
        // 快捷键（consume 避免文本编辑区重复响应）
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::O)) {
            self.load_file();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S)) {
            self.save_text();
        }

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("文件", |ui| {
                    if ui.button("打开文件...   Ctrl+O").clicked() {
                        ui.close_menu();
                        self.load_file();
                    }
                    if ui.button("保存文本...   Ctrl+S").clicked() {
                        ui.close_menu();
                        self.save_text();
                    }
                    ui.separator();
                    if ui.button("退出").clicked() {
                        ui.close_menu();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("文本片段", |ui| {
                    if ui.button("保存当前文本为片段...").clicked() {
                        ui.close_menu();
                        self.snippet_dialog = Some(String::new());
                    }
                    if ui.button("导入片段...").clicked() {
                        ui.close_menu();
                        self.import_snippets();
                    }
                    if ui.button("导出片段...").clicked() {
                        ui.close_menu();
                        self.export_snippets();
                    }
                });
                ui.menu_button("帮助", |ui| {
                    if ui.button("使用说明").clicked() {
                        ui.close_menu();
                        self.show_help();
                    }
                    if ui.button("关于").clicked() {
                        ui.close_menu();
                        self.show_about();
                    }
                });
            });
        });
    }

    fn draw_status_bar(&self, ctx: &egui::Context) {
        let status = self.shared.lock().unwrap().status.clone();
        egui::TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .min_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(status);
                });
            });
    }

    fn draw_button_row(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("button_row")
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.running, egui::Button::new("开始输入"))
                        .clicked()
                    {
                        self.start_typing(ctx);
                    }
                    if ui
                        .add_enabled(self.running, egui::Button::new("停止"))
                        .clicked()
                    {
                        self.stop_typing();
                    }

                    let progress = self.shared.lock().unwrap().progress;
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(6.0);
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label("进度:");
                            ui.add(
                                egui::ProgressBar::new(progress / 100.0)
                                    .desired_width(200.0)
                                    .show_percentage(),
                            );
                        });
                    });
                });
            });
    }

    fn draw_control_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("control_panel")
            .resizable(false)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("参数设置").strong());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("字符间隔(ms):");
                        ui.add(egui::Slider::new(&mut self.interval_ms, 5..=100));
                        ui.label(format!("{}ms", self.interval_ms));

                        ui.add_space(16.0);
                        ui.label("行间延迟(ms):");
                        ui.add(egui::Slider::new(&mut self.line_delay_ms, 10..=200));
                        ui.label(format!("{}ms", self.line_delay_ms));
                    });
                    ui.horizontal(|ui| {
                        ui.label("倒计时(s):");
                        ui.add(egui::DragValue::new(&mut self.countdown_s).range(1..=30));

                        ui.add_space(16.0);
                        ui.checkbox(&mut self.failsafe, "启用紧急停止（鼠标移至屏幕四角）");
                    });
                });
            });
    }

    fn draw_text_area(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new("输入文本").strong());
                ui.add_space(4.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.text)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(12),
                    );
                });
            });
        });
    }

    fn draw_snippet_dialog(&mut self, ctx: &egui::Context) {
        let Some(buffer) = self.snippet_dialog.as_mut() else {
            return;
        };
        let mut close = false;
        let mut confirmed: Option<String> = None;

        egui::Window::new("保存文本片段")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("请输入片段名称:");
                let response = ui.add(egui::TextEdit::singleline(buffer).desired_width(240.0));
                response.request_focus();
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    confirmed = Some(buffer.clone());
                }
                ui.horizontal(|ui| {
                    if ui.button("确定").clicked() {
                        confirmed = Some(buffer.clone());
                    }
                    if ui.button("取消").clicked() {
                        close = true;
                    }
                });
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });

        if let Some(name) = confirmed {
            self.snippet_dialog = None;
            self.confirm_save_snippet(name);
        } else if close {
            self.snippet_dialog = None;
        }
    }
}

impl eframe::App for AutoTyperApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 工作线程运行期间保持界面刷新（约 20fps，对应 Python 版 after(50) 轮询）
        if self.running {
            let still_running = self.shared.lock().unwrap().running;
            if still_running {
                ctx.request_repaint_after(Duration::from_millis(50));
            } else {
                self.running = false;
            }
        }

        self.draw_menu(ctx);
        self.draw_status_bar(ctx);
        self.draw_button_row(ctx);
        self.draw_control_panel(ctx);
        self.draw_text_area(ctx);
        self.draw_snippet_dialog(ctx);
    }
}

/// 输入工作线程主体（对应 Python 版 _typing_worker）。
///
/// `get_hwnd` 在倒计时结束后调用，取得当时前台窗口句柄——
/// 用户正是在倒计时期间切换到目标窗口的。测试可注入返回 0 的
/// 提供者（线程消息队列），避免向真实桌面注入按键。
fn typing_worker<F>(
    text: String,
    countdown: i32,
    failsafe: bool,
    interval: f64,
    line_delay: f64,
    stop_flag: Arc<AtomicBool>,
    shared: Arc<Mutex<WorkerShared>>,
    get_hwnd: F,
) where
    F: FnOnce() -> crate::win32::HWND,
{
    let set_status = |s: String| {
        shared.lock().unwrap().status = s;
    };
    let stopped = || stop_flag.load(Ordering::SeqCst);
    let finish = || {
        shared.lock().unwrap().running = false;
    };

    // 倒计时
    for i in (1..=countdown).rev() {
        if stopped() {
            set_status("已取消输入".to_string());
            finish();
            return;
        }
        set_status(format!("{i} 秒后开始输入..."));
        for _ in 0..10 {
            sleep(Duration::from_millis(100));
            if stopped() {
                break;
            }
        }
    }
    if stopped() {
        set_status("已取消输入".to_string());
        finish();
        return;
    }

    set_status("正在输入...".to_string());
    let hwnd = get_hwnd();
    let engine = InputEngine::new(interval, line_delay);

    let failsafe_hit = Arc::new(AtomicBool::new(false));
    let failsafe_hit_cb = Arc::clone(&failsafe_hit);
    let shared_progress = Arc::clone(&shared);
    let stop_cb = Arc::clone(&stop_flag);

    let chars_sent = engine.type_text(
        hwnd,
        &text,
        Some(move |current: usize, total: usize| {
            let percent = if total > 0 {
                current as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            let mut s = shared_progress.lock().unwrap();
            s.progress = percent as f32;
            s.status = format!("正在输入... {percent:.1}%");
        }),
        Some(move || -> bool {
            if stop_cb.load(Ordering::SeqCst) {
                return true;
            }
            if failsafe && mouse_at_screen_corner(2) {
                failsafe_hit_cb.store(true, Ordering::SeqCst);
                return true;
            }
            false
        }),
    );

    if stop_flag.load(Ordering::SeqCst) {
        set_status(format!("已停止，已发送 {chars_sent} 个字符"));
    } else if failsafe_hit.load(Ordering::SeqCst) {
        set_status(format!("紧急停止，已发送 {chars_sent} 个字符"));
    } else {
        let mut s = shared.lock().unwrap();
        s.progress = 100.0;
        s.status = format!("输入完成，共发送 {chars_sent} 个字符");
    }
    finish();
}

/// 加载中文字体回退（微软雅黑），保证中文文本与界面完整渲染
fn install_cjk_fonts(ctx: &egui::Context) {
    const CANDIDATES: [&str; 3] = [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\msyh.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
    ];
    let mut fonts = egui::FontDefinitions::default();
    for path in CANDIDATES {
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert("cjk".to_owned(), egui::FontData::from_owned(data).into());
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                family.push("cjk".to_owned());
            }
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                family.push("cjk".to_owned());
            }
            break;
        }
    }
    ctx.set_fonts(fonts);
}

fn error_dialog(msg: &str) {
    rfd::MessageDialog::new()
        .set_title("错误")
        .set_description(msg)
        .set_level(rfd::MessageLevel::Error)
        .show();
}

fn warning_dialog(msg: &str) {
    rfd::MessageDialog::new()
        .set_title("警告")
        .set_description(msg)
        .set_level(rfd::MessageLevel::Warning)
        .show();
}

/// 启动 GUI 模式的入口函数
pub fn run_gui() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([780.0, 620.0])
            .with_min_inner_size([480.0, 360.0]),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "AutoTyper — 智能字符输入助手",
        options,
        Box::new(|cc| {
            install_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(AutoTyperApp::new()))
        }),
    ) {
        eprintln!("GUI 启动失败: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_cancel_during_countdown() {
        // 倒计时期间按下停止：工作线程应立即结束并给出「已取消输入」
        let stop = Arc::new(AtomicBool::new(true));
        let shared = Arc::new(Mutex::new(WorkerShared {
            status: String::new(),
            progress: 0.0,
            running: true,
        }));
        let ctx = egui::Context::default();
        let _ = ctx;
        typing_worker(
            "hello".to_string(),
            3,
            false,
            0.0,
            0.0,
            Arc::clone(&stop),
            Arc::clone(&shared),
            || 0isize,
        );
        let s = shared.lock().unwrap();
        assert_eq!(s.status, "已取消输入");
        assert!(!s.running);
        assert_eq!(s.progress, 0.0);
    }

    #[test]
    fn worker_stop_midway_reports_stopped() {
        // countdown=0、目标为线程消息队列（hwnd=0，不触达真实窗口）；
        // 发送 2 个字符后置停止位，应报告「已停止」
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Mutex::new(WorkerShared {
            status: String::new(),
            progress: 0.0,
            running: true,
        }));
        let stop_cb = Arc::clone(&stop);
        let shared_cb = Arc::clone(&shared);
        std::thread::spawn(move || {
            typing_worker(
                "abcdefgh".to_string(),
                0,
                false,
                0.01,
                0.0,
                stop_cb,
                shared_cb,
                || 0isize,
            );
        });
        // 等输入开始后按下停止
        for _ in 0..200 {
            sleep(Duration::from_millis(5));
            if shared.lock().unwrap().status.starts_with("正在输入") {
                break;
            }
        }
        stop.store(true, Ordering::SeqCst);
        for _ in 0..200 {
            sleep(Duration::from_millis(5));
            if !shared.lock().unwrap().running {
                break;
            }
        }
        let s = shared.lock().unwrap();
        assert!(!s.running);
        assert!(
            s.status.starts_with("已停止，已发送 "),
            "status={}",
            s.status
        );
    }

    #[test]
    fn worker_completes_with_full_progress() {
        // 正常完成：进度置 100，状态为「输入完成」
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Mutex::new(WorkerShared {
            status: String::new(),
            progress: 0.0,
            running: true,
        }));
        typing_worker(
            "abc".to_string(),
            0,
            false,
            0.0,
            0.0,
            stop,
            Arc::clone(&shared),
            || 0isize,
        );
        let s = shared.lock().unwrap();
        assert_eq!(s.status, "输入完成，共发送 3 个字符");
        assert_eq!(s.progress, 100.0);
        assert!(!s.running);
    }
}
