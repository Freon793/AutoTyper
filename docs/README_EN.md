<div align="center">

# AutoTyper

**Smart character input assistant — bypass clipboard restrictions and simulate real keyboard input in any text box**

[![Rust 1.74+](https://img.shields.io/badge/rust-1.74+-dea584.svg)](https://www.rust-lang.org/)
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

Since v2.0 the project is fully rewritten in **Rust**: zero runtime dependencies, a single-file EXE, and millisecond startup. The original Python implementation is archived under [`legacy-python/`](../legacy-python/).

## Quick Start

### Option 1: Download the EXE (recommended)

Download the latest `AutoTyper-vX.Y.Z-win64.exe` from [GitHub Releases](https://github.com/Freon793/AutoTyper/releases) and double-click to run.

Release binaries are built automatically by GitHub Actions in a clean environment, strictly from this repository's source.

### Option 2: Build from Source

Requirements:

- Windows 10 / 11
- [Rust](https://rustup.rs/) 1.74 or later (stable-msvc toolchain)

```bash
git clone https://github.com/Freon793/AutoTyper.git
cd AutoTyper
cargo build --release
# artifact: target\release\AutoTyper.exe
```

### Usage

GUI mode (recommended):

```bash
AutoTyper.exe
```

CLI mode:

```bash
# Type text directly
AutoTyper.exe --cli --text "Hello, World!"

# Read from a file
AutoTyper.exe --cli --file answer.py

# Custom parameters
AutoTyper.exe --cli --file answer.py --interval 0.02 --countdown 3

# Use a saved snippet
AutoTyper.exe --cli --snippet "my-snippet"
AutoTyper.exe --cli --list-snippets
```

During development, use `cargo run` (GUI) or `cargo run -- --cli --text "..."` (CLI).

See the [User Guide](USAGE.md) for the full parameter reference.

## Features

| Feature | Description |
|------|------|
| Character-level injection | `PostMessageW(WM_CHAR)`, character by character, full Unicode support |
| Adjustable speed | 5–100ms per character, independent line delay |
| Countdown start | Configurable countdown before typing begins |
| Emergency stop | Move the mouse to any screen corner, or press `Ctrl+C`, to abort instantly |
| Dual mode | Graphical interface (egui) and command line |
| Snippet management | Save, load, import and export reusable text snippets |
| Persistent config | User preferences stored in `config.json` |
| Native Win32 FFI | Hand-written FFI declarations, no `windows` crate, small binary |

## Project Structure

```
AutoTyper/
├── .github/
│   ├── ISSUE_TEMPLATE/        # Issue templates (bug report / feature request)
│   └── workflows/
│       ├── ci.yml             # Continuous integration: cargo fmt --check + cargo test on push / PR
│       └── release.yml        # Release: build EXE on version tag and publish to GitHub Releases
├── src/
│   ├── main.rs                # Unified entry point (--cli switch, windows subsystem in release)
│   ├── lib.rs                 # Library root (platform gate, version)
│   ├── win32.rs               # Win32 FFI declarations (PostMessageW / console / failsafe probe)
│   ├── engine.rs              # Input engine (WM_CHAR / VK_RETURN / VK_TAB injection loop)
│   ├── cli.rs                 # CLI argument parsing and run flow
│   ├── gui.rs                 # GUI (egui/eframe)
│   └── config.rs              # Config file management (serde, order-preserving)
├── tests/
│   └── window_injection.rs    # End-to-end test: real hidden probe window verifies the message sequence
├── legacy-python/             # Archived v1.x Python implementation (tkinter + pytest)
├── docs/
│   ├── README_EN.md           # English README
│   ├── USAGE.md               # User guide
│   └── SPEC.md                # Requirements spec
├── Cargo.toml                 # Rust manifest (single source of the version number)
├── config.example.json        # Config template (config.json is generated at runtime)
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

The config format is fully compatible with v1.x — an existing `config.json` carries over as-is.

## How It Works

```
User starts → target window focused → countdown → engine.rs
                                                    │
                                                    ▼
                                      user32::PostMessageW(
                                          hwnd,    // target window handle
                                          WM_CHAR, // 0x0102
                                          char,    // Unicode code point
                                          0        // lParam
                                      )
```

Unlike `SendInput` / `keybd_event` style global keyboard simulation, `PostMessageW` writes straight to the target window's message queue and is immune to keyboard-event interception.

Return and tab are sent as `WM_KEYDOWN` / `WM_KEYUP` pairs (`VK_RETURN` / `VK_TAB`), preserving the same message sequence as real keystrokes.

## Release Builds

The repository contains no binary artifacts. EXEs are built and published automatically by GitHub Actions:

1. Make sure the version number is consistent (`version` in `Cargo.toml`)
2. Push a version tag:

   ```bash
   git tag v2.0.0
   git push origin v2.0.0
   ```

3. The `release.yml` workflow runs `cargo test` on windows-latest, then `cargo build --release`, and publishes `AutoTyper-v2.0.0-win64.exe` to GitHub Releases

To build manually on your own machine:

```bash
cargo build --release
```

The artifact is written to `target\release\AutoTyper.exe` (`target/` is excluded by `.gitignore`). Release builds enable LTO and symbol stripping, and use the windows subsystem (no console window in GUI mode; CLI mode re-attaches to the parent terminal for output).

## Running Tests

```bash
cargo test
```

Tests come in three layers:

- Unit tests for the engine and config (inline in `src/`, pure logic, never touching real windows)
- Unit tests for CLI parsing and output (mutual exclusion, exit codes, snippet listing, progress-bar rendering)
- End-to-end injection tests (`tests/window_injection.rs`: a real hidden probe window verifies the full `WM_CHAR` / `WM_KEYDOWN` / `WM_KEYUP` sequence and Unicode code points)

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

Please make sure `cargo fmt --check` and `cargo test` pass before submitting.

## License

Released under the [MIT](../LICENSE) license.

---

<div align="center">

If this project helps you, please consider giving it a Star.

</div>
