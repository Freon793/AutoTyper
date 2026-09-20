# 更新日志

本文件记录 AutoTyper 各版本的显著变更。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [2.0.0] - 2026-09-20

项目由 Python 全面重写为 **Rust**（功能与 v1.0.0 逐项对齐，`config.json` 格式完全兼容）。

### Changed

- 实现语言：Python → Rust（stable-msvc，rust-version 1.74），版本号唯一来源改为 `Cargo.toml`
- GUI 框架：tkinter → egui/eframe（即时模式渲染，高 DPI 原生支持，中文字体自动回退微软雅黑）
- 对话框：tkinter 原生对话框 → rfd（文件打开/保存、消息提示）
- Win32 调用：ctypes/pyautogui → 手写 FFI 声明（`PostMessageW` / `GetForegroundWindow` / `GetCursorPos` / `GetSystemMetrics` / 控制台接回等），不依赖 `windows` crate
- 紧急停止：pyautogui failsafe → 等价的四角检测（`GetCursorPos` + 屏幕尺寸，2px 容差）
- 配置序列化：手写 JSON → serde + indexmap（保持片段插入顺序），损坏配置回退默认值且不覆写文件的行为与 v1 一致
- CLI：argparse → 手写解析器，参数名、互斥规则、输出文案与退出码（0/1/2/3）与 v1 一致；release 构建为 windows 子系统，CLI 模式自动 `AttachConsole` 接回父终端
- 单文件 EXE 体积与启动速度显著优于 PyInstaller onefile 产物

### Added

- 端到端注入测试 `tests/window_injection.rs`：创建真实隐藏窗口作为探针，验证 `WM_CHAR` / `WM_KEYDOWN` / `WM_KEYUP` 完整消息序列与 Unicode 码位
- `legacy-python/` 目录：v1.x Python 实现完整归档（含原测试），附归档说明

### Removed

- Python 运行时依赖（`requirements.txt`、pyautogui、PyInstaller 打包链）

## [1.0.0] - 2026-09-20

首个公开版本。

### Added

- 核心输入引擎：`PostMessageW(WM_CHAR)` 逐字符注入 Unicode 文本，支持回车（`VK_RETURN`）与制表符（`VK_TAB`），不经过剪贴板、不触发键盘 Hook
- GUI 模式（tkinter）：文本编辑区、参数调节（字符间隔 / 行间延迟 / 倒计时 / 紧急停止开关）、文本片段管理（保存 / 导入 / 导出）、实时进度条与状态栏
- CLI 模式：`--text` / `--file` / `--snippet` 三种输入源，`--interval` / `--line-delay` / `--countdown` / `--no-failsafe` / `--list-snippets` 参数，明确的退出码约定
- 紧急停止：鼠标移至屏幕四角（pyautogui failsafe）、GUI 停止按钮、`Ctrl+C`，检测粒度为单字符
- 配置持久化：`config.json` 首次运行自动生成（模板 `config.example.json`），保存用户偏好与文本片段
- 高 DPI 与系统缩放自适应：进程级 DPI 感知 + 按控件请求尺寸自适应窗口，保证关键按钮在任何缩放比例下可见可点
- 测试：30 个 pytest 用例，覆盖引擎、配置、CLI 与 GUI 布局回归
- GitHub Actions：`ci.yml`（push / PR 时在 windows-latest × Python 3.9 / 3.11 / 3.12 上运行测试）与 `release.yml`（推送 `v*` 标签时构建 EXE 并自动发布到 GitHub Releases）
- 文档：中英双语 README、使用手册（`docs/USAGE.md`）、需求规格（`docs/SPEC.md`）、Issue 模板

[Unreleased]: https://github.com/Freon793/AutoTyper/compare/v1.0.0...HEAD
[2.0.0]: https://github.com/Freon793/AutoTyper/compare/v1.0.0...v2.0.0
[1.0.0]: https://github.com/Freon793/AutoTyper/releases/tag/v1.0.0
