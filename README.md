<div align="center">

# AutoTyper

**智能字符输入助手 — 绕过剪贴板限制，在任何输入框中模拟真实键盘输入**

[![Rust 1.74+](https://img.shields.io/badge/rust-1.74+-dea584.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Platform: Windows](https://img.shields.io/badge/platform-Windows-lightgrey.svg)](https://www.microsoft.com/windows)
[![CI](https://github.com/Freon793/AutoTyper/actions/workflows/ci.yml/badge.svg)](https://github.com/Freon793/AutoTyper/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Freon793/AutoTyper)](https://github.com/Freon793/AutoTyper/releases/latest)

[English](docs/README_EN.md) · [中文](README.md) · [使用手册](docs/USAGE.md) · [需求规格](docs/SPEC.md) · [更新日志](CHANGELOG.md)

</div>

---

## 项目简介

部分在线平台的输入框对文本输入做了限制——仅允许粘贴、拦截键盘事件，导致无法正常输入代码或文本。

AutoTyper 通过 Windows 消息队列直接注入字符（`PostMessageW` → `WM_CHAR`），不经过剪贴板、不触发键盘 Hook，在目标窗口中模拟真实的逐字符输入。

v2.0 起项目由 Python 全面重写为 **Rust**：零运行时依赖、单文件 EXE、毫秒级启动。原 Python 实现归档于 [`legacy-python/`](legacy-python/)。

## 快速开始

### 方式一：下载 EXE（推荐）

从 [GitHub Releases](https://github.com/Freon793/AutoTyper/releases) 下载最新版 `AutoTyper-vX.Y.Z-win64.exe`，双击即可运行。

发行版由 GitHub Actions 在干净环境中自动构建，与本仓库源码严格对应。

### 方式二：源码构建

环境要求：

- Windows 10 / 11
- [Rust](https://rustup.rs/) 1.74 或更高版本（stable-msvc 工具链）

```bash
git clone https://github.com/Freon793/AutoTyper.git
cd AutoTyper
cargo build --release
# 产物：target\release\AutoTyper.exe
```

### 使用方式

GUI 模式（推荐）：

```bash
AutoTyper.exe
```

CLI 模式：

```bash
# 直接输入文本
AutoTyper.exe --cli --text "Hello, World!"

# 从文件读取
AutoTyper.exe --cli --file answer.py

# 自定义参数
AutoTyper.exe --cli --file answer.py --interval 0.02 --countdown 3

# 使用已保存的文本片段
AutoTyper.exe --cli --snippet "常用代码段"
AutoTyper.exe --cli --list-snippets
```

开发调试可直接用 `cargo run`（GUI）或 `cargo run -- --cli --text "..."`（CLI）。

完整参数说明见[使用手册](docs/USAGE.md)。

## 功能特性

| 功能 | 说明 |
|------|------|
| 字符级注入 | `PostMessageW(WM_CHAR)` 逐字符输入，完整支持 Unicode |
| 速度可调 | 字符间隔 5–100ms，行间延迟独立配置 |
| 倒计时启动 | 可配置倒计时，预留切换目标窗口的时间 |
| 紧急停止 | 鼠标移至屏幕四角或 `Ctrl+C` 立即中断 |
| 双模式 | 图形界面（egui）与命令行 |
| 文本片段管理 | 保存、加载、导入导出多段常用文本 |
| 配置持久化 | 用户偏好保存在 `config.json` |
| 原生 Win32 FFI | 手写 FFI 声明，不依赖 windows crate，二进制体积小 |

## 项目结构

```
AutoTyper/
├── .github/
│   ├── ISSUE_TEMPLATE/        # Issue 模板（Bug 报告 / 功能建议）
│   └── workflows/
│       ├── ci.yml             # 持续集成：push / PR 时运行 cargo fmt --check 与 cargo test
│       └── release.yml        # 发行：推送版本标签时构建 EXE 并发布到 GitHub Releases
├── src/
│   ├── main.rs                # 统一入口（--cli 开关，release 为 windows 子系统）
│   ├── lib.rs                 # 库根（Windows 平台门禁、版本号）
│   ├── win32.rs               # Win32 FFI 声明（PostMessageW / 控制台 / 紧急停止检测）
│   ├── engine.rs              # 输入引擎（WM_CHAR / VK_RETURN / VK_TAB 注入循环）
│   ├── cli.rs                 # CLI 参数解析与运行流程
│   ├── gui.rs                 # GUI 界面（egui/eframe）
│   └── config.rs              # 配置文件管理（serde + 顺序保持）
├── tests/
│   └── window_injection.rs    # 端到端测试：真实隐藏窗口探针验证消息序列
├── legacy-python/             # v1.x Python 实现归档（tkinter + pytest）
├── docs/
│   ├── README_EN.md           # 英文 README
│   ├── USAGE.md               # 使用手册
│   └── SPEC.md                # 需求规格
├── Cargo.toml                 # Rust 项目清单（版本号的唯一来源）
├── config.example.json        # 配置模板（运行时自动生成 config.json）
├── CHANGELOG.md               # 版本更新日志
└── LICENSE                    # MIT 许可证
```

## 配置说明

首次运行时会在程序目录自动生成 `config.json`（默认值同 `config.example.json`）。该文件保存用户本地数据，不纳入版本控制。

```json
{
  "interval": 0.01,
  "line_delay": 0.05,
  "countdown": 5,
  "failsafe": true,
  "snippets": {}
}
```

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `interval` | float | `0.01` | 字符间隔（秒） |
| `line_delay` | float | `0.05` | 行间延迟（秒） |
| `countdown` | int | `5` | 启动前倒计时（秒） |
| `failsafe` | bool | `true` | 启用鼠标紧急停止 |
| `snippets` | object | `{}` | 已保存的文本片段，格式 `{名称: 内容}` |

配置格式与 v1.x 完全兼容，升级后可直接沿用原有 `config.json`。

## 工作原理

```
用户启动 → 选择目标窗口 → 倒计时 → engine.rs
                                           │
                                           ▼
                               user32::PostMessageW(
                                   hwnd,    // 目标窗口句柄
                                   WM_CHAR, // 0x0102
                                   char,    // Unicode 码位
                                   0        // lParam
                               )
```

与 `SendInput` / `keybd_event` 等全局键盘模拟方式不同，`PostMessageW` 直接向目标窗口的消息队列投递消息，不受键盘事件拦截影响。

回车与制表符以 `WM_KEYDOWN` / `WM_KEYUP`（`VK_RETURN` / `VK_TAB`）成对发送，保持与真实按键一致的消息序列。

## 发行版构建

仓库中不包含任何二进制产物。EXE 由 GitHub Actions 自动构建并发布：

1. 确认版本号一致（`Cargo.toml` 中的 `version`）
2. 推送版本标签：

   ```bash
   git tag v2.0.0
   git push origin v2.0.0
   ```

3. `release.yml` 工作流在 windows-latest 上先运行 `cargo test`，再执行 `cargo build --release`，产物 `AutoTyper-v2.0.0-win64.exe` 自动发布到 GitHub Releases

本地手动构建：

```bash
cargo build --release
```

产物位于 `target\release\AutoTyper.exe`（`target/` 已被 `.gitignore` 排除）。release 构建启用 LTO 与符号剥离，且为 windows 子系统（GUI 模式无控制台窗口；CLI 模式自动接回父终端输出）。

## 运行测试

```bash
cargo test
```

测试分三层：

- 引擎与配置的单元测试（`src/` 内嵌，纯逻辑，不触达真实窗口）
- CLI 解析与输出的单元测试（互斥组、退出码、片段列表、进度条渲染）
- 端到端注入测试（`tests/window_injection.rs`：创建真实隐藏窗口作为探针，验证 `WM_CHAR` / `WM_KEYDOWN` / `WM_KEYUP` 的完整消息序列与 Unicode 码位）

CI 状态由 GitHub Actions 在 Windows 环境自动验证（见 `.github/workflows/ci.yml`）。

## 注意事项

- 仅支持 Windows 系统（依赖 `user32.dll`）
- 输入前请确保目标窗口已获取焦点
- 建议字符间隔不低于 10ms，避免丢字
- 本工具仅供学习和合法用途

## 贡献指南

欢迎提交 Issue 和 Pull Request：

1. Fork 本仓库
2. 创建特性分支（`git checkout -b feature/amazing-feature`）
3. 提交更改（`git commit -m 'feat: add amazing feature'`）
4. 推送分支（`git push origin feature/amazing-feature`）
5. 提交 Pull Request

提交前请确保 `cargo fmt --check` 与 `cargo test` 通过。

## 许可证

本项目基于 [MIT](LICENSE) 许可证开源。

---

<div align="center">

如果这个项目对你有帮助，欢迎点一个 Star 支持。

</div>
