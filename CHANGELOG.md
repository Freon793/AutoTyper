# 更新日志

本文件记录 AutoTyper 各版本的显著变更。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

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
[1.0.0]: https://github.com/Freon793/AutoTyper/releases/tag/v1.0.0
