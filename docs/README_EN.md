<div align="center">

# AutoTyper

**Smart character input assistant — bypass clipboard restrictions and simulate real keyboard input in any text box**

[![Python 3.8+](https://img.shields.io/badge/python-3.8+-blue.svg)](https://www.python.org/downloads/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Platform: Windows](https://img.shields.io/badge/platform-Windows-lightgrey.svg)](https://www.microsoft.com/windows)
[![CI](https://github.com/Freon793/AutoTyper/actions/workflows/ci.yml/badge.svg)](https://github.com/Freon793/AutoTyper/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Freon793/AutoTyper)](https://github.com/Freon793/AutoTyper/releases/latest)

[English](README_EN.md) · [中文](../README.md) · [User Guide](USAGE.md) · [Spec](SPEC.md) · [Changelog](../CHANGELOG.md)

</div>

---

## Overview

Some online platforms restrict text input — allowing only clipboard paste or intercepting keyboard events — which makes it impossible to type code or text normally.

AutoTyper injects characters directly through the Windows message queue (`PostMessageW` → `WM_CHAR`). It never touches the clipboard and never triggers keyboard hooks, simulating genuine character-by-character input in the target window.

## Quick Start

### Option 1: Download the EXE (recommended, no Python required)

Download the latest `AutoTyper-vX.Y.Z-win64.exe` from [GitHub Releases](https://github.com/Freon793/AutoTyper/releases) and double-click to run.

Release binaries are built automatically by GitHub Actions in a clean environment, strictly from this repository's source.

### Option 2: Run from Source

Requirements:

- Windows 10 / 11
- Python 3.8 or later

Install dependencies:

```bash
git clone https://github.com/Freon793/AutoTyper.git
cd AutoTyper
pip install -r requirements.txt
```

### Usage

GUI mode (recommended):

```bash
python main.py
```

CLI mode:

```bash
# Type text directly
python main.py --cli --text "Hello, World!"

# Read from a file
python main.py --cli --file answer.py

# Custom parameters
python main.py --cli --file answer.py --interval 0.02 --countdown 3

# Use a saved snippet
python main.py --cli --snippet "my-snippet"
python main.py --cli --list-snippets
```

See the [User Guide](USAGE.md) for the full parameter reference.

## Features

| Feature | Description |
|------|------|
| Character-level injection | `PostMessageW(WM_CHAR)`, character by character, full Unicode support |
| Adjustable speed | 5–100ms per character, independent line delay |
| Countdown start | Configurable countdown before typing begins |
| Emergency stop | Move the mouse to any screen corner, or press `Ctrl+C`, to abort instantly |
| Dual mode | Graphical interface (tkinter) and command line |
| Snippet management | Save, load, import and export reusable text snippets |
| Persistent config | User preferences stored in `config.json` |

## Project Structure

```
AutoTyper/
├── .github/
│   ├── ISSUE_TEMPLATE/        # Issue templates (bug report / feature request)
│   └── workflows/
│       ├── ci.yml             # Continuous integration: unit tests on push / PR
│       └── release.yml        # Release: build EXE on version tag and publish to GitHub Releases
├── src/
│   ├── core/
│   │   └── engine.py          # Input engine (PostMessageW wrapper)
│   ├── cli/
│   │   └── main.py            # CLI entry point
│   ├── gui/
│   │   └── app.py             # GUI (tkinter)
│   └── config/
│       └── manager.py         # Config file management
├── tests/                     # Unit tests (pytest)
├── docs/
│   ├── README_EN.md           # English README
│   ├── USAGE.md               # User guide
│   └── SPEC.md                # Requirements spec
├── main.py                    # Unified entry point
├── config.example.json        # Config template (config.json is generated at runtime)
├── requirements.txt           # Python dependencies
├── CHANGELOG.md               # Version changelog
└── LICENSE                    # MIT License
```

## Configuration

`config.json` is generated automatically in the program directory on first run (defaults match `config.example.json`). It holds user-local data and is not tracked by version control.

```json
{
  "interval": 0.01,
  "line_delay": 0.05,
  "countdown": 5,
  "failsafe": true,
  "snippets": {}
}
```

| Key | Type | Default | Description |
|------|------|--------|------|
| `interval` | float | `0.01` | Delay between characters (seconds) |
| `line_delay` | float | `0.05` | Delay between lines (seconds) |
| `countdown` | int | `5` | Countdown before typing starts (seconds) |
| `failsafe` | bool | `true` | Enable mouse-corner emergency stop |
| `snippets` | object | `{}` | Saved text snippets, `{name: content}` |

## How It Works

```
User starts → target window focused → countdown → engine.py
                                                    │
                                                    ▼
                                      user32.PostMessageW(
                                          hwnd,    // target window handle
                                          WM_CHAR, // 0x0102
                                          char,    // Unicode code point
                                          0        // lParam
                                      )
```

Unlike `SendInput` / `keybd_event` style global keyboard simulation, `PostMessageW` writes straight to the target window's message queue and is immune to keyboard-event interception.

## Release Builds

The repository contains no binary artifacts. EXEs are built and published automatically by GitHub Actions:

1. Make sure the version number is consistent (`__version__` in `src/__init__.py`)
2. Push a version tag:

   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

3. The `release.yml` workflow runs the unit tests on windows-latest, packages the app with PyInstaller, and publishes `AutoTyper-v1.0.0-win64.exe` to GitHub Releases

To build manually on your own machine:

```bash
pip install pyinstaller
python -m PyInstaller --onefile --windowed --name "AutoTyper" ^
    --exclude-module PyQt5 --exclude-module numpy --exclude-module cv2 ^
    --exclude-module PIL --exclude-module pyscreenshot --exclude-module pymsgbox ^
    --exclude-module pytweening --exclude-module pygetwindow --exclude-module pyrect ^
    --exclude-module pyScreeze --exclude-module mouseinfo ^
    main.py
```

The artifact is written to `dist/AutoTyper.exe` (`build/`, `dist/` and `*.spec` are excluded by `.gitignore`).

## Running Tests

```bash
pip install pytest
python -m pytest tests -v
```

CI is verified automatically on Windows via GitHub Actions (`.github/workflows/ci.yml`).

## Notes

- Windows only (depends on `user32.dll`)
- Make sure the target window has focus before typing starts
- Keep the character interval at 10ms or above to avoid dropped characters
- For learning and lawful use only

## Contributing

Issues and pull requests are welcome:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

Released under the [MIT](../LICENSE) license.

---

<div align="center">

If this project helps you, please consider giving it a Star.

</div>
