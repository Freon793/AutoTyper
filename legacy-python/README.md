# legacy-python — v1.x Python 实现归档

本目录是 AutoTyper v1.0.0 的完整 Python 实现（tkinter GUI + argparse CLI + pytest），
自 v2.0.0 起项目已全面重写为 Rust（见仓库根目录 `src/`），本目录仅作历史归档，
不再维护、不参与 CI。

## 目录内容

- `main.py` — 原统一入口（`--cli` 开关）
- `src/` — 原实现（core/engine.py、cli/main.py、gui/app.py、config/manager.py）
- `tests/` — 原 pytest 测试（30 个用例）
- `requirements.txt` — 原 Python 依赖（pyautogui）

## 如需运行旧版

```bash
cd legacy-python
pip install -r requirements.txt pytest
python main.py          # GUI 模式
python main.py --cli --text "..."   # CLI 模式
python -m pytest tests -v           # 测试
```

## 归档原因

Rust 重写消除了 Python 运行时与 PyInstaller 打包链：发行 EXE 体积更小、
启动更快、无杀软误报的 bootloader 结构，配置格式（`config.json`）与 CLI
参数、退出码约定均与 v1.x 完全兼容。重写动机与决策详见
`../docs/SPEC.md` 第 4.3 节及 `../CHANGELOG.md`。
