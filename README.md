<div align="center">

# AutoTyper

**智能字符输入助手 — 绕过剪贴板限制，在任何输入框中模拟真实键盘输入**

[![Python 3.8+](https://img.shields.io/badge/python-3.8+-blue.svg)](https://www.python.org/downloads/)
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

## 快速开始

### 方式一：下载 EXE（推荐，无需 Python 环境）

从 [GitHub Releases](https://github.com/Freon793/AutoTyper/releases) 下载最新版 `AutoTyper-vX.Y.Z-win64.exe`，双击即可运行。

发行版由 GitHub Actions 在干净环境中自动构建，与本仓库源码严格对应。

### 方式二：源码运行

环境要求：

- Windows 10 / 11
- Python 3.8 或更高版本

安装依赖：

```bash
git clone https://github.com/Freon793/AutoTyper.git
cd AutoTyper
pip install -r requirements.txt
```

### 使用方式

GUI 模式（推荐）：

```bash
python main.py
```

CLI 模式：

```bash
# 直接输入文本
python main.py --cli --text "Hello, World!"

# 从文件读取
python main.py --cli --file answer.py

# 自定义参数
python main.py --cli --file answer.py --interval 0.02 --countdown 3

# 使用已保存的文本片段
python main.py --cli --snippet "常用代码段"
python main.py --cli --list-snippets
```

完整参数说明见[使用手册](docs/USAGE.md)。

## 功能特性

| 功能 | 说明 |
|------|------|
| 字符级注入 | `PostMessageW(WM_CHAR)` 逐字符输入，完整支持 Unicode |
| 速度可调 | 字符间隔 5–100ms，行间延迟独立配置 |
| 倒计时启动 | 可配置倒计时，预留切换目标窗口的时间 |
| 紧急停止 | 鼠标移至屏幕四角或 `Ctrl+C` 立即中断 |
| 双模式 | 图形界面（tkinter）与命令行 |
| 文本片段管理 | 保存、加载、导入导出多段常用文本 |
| 配置持久化 | 用户偏好保存在 `config.json` |

## 项目结构

```
AutoTyper/
├── .github/
│   ├── ISSUE_TEMPLATE/        # Issue 模板（Bug 报告 / 功能建议）
│   └── workflows/
│       ├── ci.yml             # 持续集成：push / PR 时运行单元测试
│       └── release.yml        # 发行：推送版本标签时构建 EXE 并发布到 GitHub Releases
├── src/
│   ├── core/
│   │   └── engine.py          # 输入引擎（PostMessageW 封装）
│   ├── cli/
│   │   └── main.py            # CLI 入口
│   ├── gui/
│   │   └── app.py             # GUI 界面（tkinter）
│   └── config/
│       └── manager.py         # 配置文件管理
├── tests/                     # 单元测试（pytest）
├── docs/
│   ├── README_EN.md           # 英文 README
│   ├── USAGE.md               # 使用手册
│   └── SPEC.md                # 需求规格
├── main.py                    # 统一入口
├── config.example.json        # 配置模板（运行时自动生成 config.json）
├── requirements.txt           # Python 依赖
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

## 工作原理

```
用户启动 → 选择目标窗口 → 倒计时 → engine.py
                                           │
                                           ▼
                               user32.PostMessageW(
                                   hwnd,    // 目标窗口句柄
                                   WM_CHAR, // 0x0102
                                   char,    // Unicode 码位
                                   0        // lParam
                               )
```

与 `SendInput` / `keybd_event` 等全局键盘模拟方式不同，`PostMessageW` 直接向目标窗口的消息队列投递消息，不受键盘事件拦截影响。

## 发行版构建

仓库中不包含任何二进制产物。EXE 由 GitHub Actions 自动构建并发布：

1. 确认版本号一致（`src/__init__.py` 中的 `__version__`）
2. 推送版本标签：

   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

3. `release.yml` 工作流在 windows-latest 上先运行单元测试，再用 PyInstaller 打包，产物 `AutoTyper-v1.0.0-win64.exe` 自动发布到 GitHub Releases

如需在本地手动构建：

```bash
pip install pyinstaller
python -m PyInstaller --onefile --windowed --name "AutoTyper" ^
    --exclude-module PyQt5 --exclude-module numpy --exclude-module cv2 ^
    --exclude-module PIL --exclude-module pyscreenshot --exclude-module pymsgbox ^
    --exclude-module pytweening --exclude-module pygetwindow --exclude-module pyrect ^
    --exclude-module pyScreeze --exclude-module mouseinfo ^
    main.py
```

产物位于 `dist/AutoTyper.exe`（`build/`、`dist/`、`*.spec` 均已被 `.gitignore` 排除）。

## 运行测试

```bash
pip install pytest
python -m pytest tests -v
```

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

## 许可证

本项目基于 [MIT](LICENSE) 许可证开源。

---

<div align="center">

如果这个项目对你有帮助，欢迎点一个 Star 支持。

</div>
