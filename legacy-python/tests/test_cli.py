# -*- coding: utf-8 -*-
"""
AutoTyper - 命令行模式测试

覆盖：CLI 必须把紧急停止回调真正接到输入引擎上
（此前 CLI 只设置了 pyautogui.FAILSAFE，却从不调用 pyautogui 的检查函数，
「鼠标移至屏幕四角停止」实际从未生效）。
"""

import sys
import unittest
from unittest import mock

import src.cli.main as cli
import src.cli as cli_pkg
import main as entry


class _StubEngine:
    """替身引擎：记录调用参数，绝不向真实桌面注入按键"""

    last = {}

    def __init__(self, **kwargs):
        _StubEngine.last = {"kwargs": kwargs}

    def get_foreground_window(self) -> int:
        return 0

    def type_text(self, hwnd, text, progress_callback=None, cancel_check=None) -> int:
        _StubEngine.last.update(
            {"hwnd": hwnd, "text": text, "cancel_check": cancel_check,
             "progress_callback": progress_callback}
        )
        return len(text)


class TestCliCancelCheck(unittest.TestCase):

    def _run(self, argv):
        _StubEngine.last = {}  # 每个用例独立，避免读到上一个用例的记录
        with mock.patch.object(cli, "InputEngine", _StubEngine):
            return cli.run_cli(argv)

    def test_engine_receives_cancel_check(self):
        """CLI 必须传入 cancel_check，否则鼠标紧急停止不会生效"""
        code = self._run(["--text", "hello", "--countdown", "0"])

        self.assertEqual(code, 0)
        self.assertEqual(_StubEngine.last["text"], "hello")
        self.assertTrue(callable(_StubEngine.last["cancel_check"]))

    def test_failsafe_check_is_invoked(self):
        """启用 failsafe 时，取消回调会去查询鼠标位置（角落实时抛异常）"""
        self._run(["--text", "hello", "--countdown", "0"])

        with mock.patch.object(cli.pyautogui, "failSafeCheck") as check:
            cancel = _StubEngine.last["cancel_check"]
            self.assertFalse(cancel())
            self.assertTrue(check.called, "未调用 pyautogui.failSafeCheck")

    def test_no_failsafe_skips_check(self):
        """--no-failsafe 时不再查询鼠标位置"""
        self._run(["--text", "hello", "--countdown", "0", "--no-failsafe"])

        with mock.patch.object(cli.pyautogui, "failSafeCheck") as check:
            cancel = _StubEngine.last["cancel_check"]
            self.assertFalse(cancel())
            self.assertFalse(check.called)

    def test_list_snippets_does_not_type(self):
        """--list-snippets 只列片段，不进入输入流程"""
        code = self._run(["--list-snippets"])
        self.assertEqual(code, 0)
        self.assertNotIn("text", _StubEngine.last)


class TestMainEntry(unittest.TestCase):
    """main.py 的 --cli 入口开关"""

    def test_cli_flag_is_stripped_before_parsing(self):
        """README 里写的 `python main.py --cli --text ...` 必须能跑通：
        入口开关 --cli 不能漏给 argparse，否则会报 unrecognized arguments"""
        captured = {}

        def fake_run_cli(args=None):
            captured["args"] = args
            return 0

        with mock.patch.object(cli_pkg, "run_cli", fake_run_cli), \
                mock.patch.object(sys, "argv", ["main.py", "--cli", "--text", "hi"]):
            with self.assertRaises(SystemExit) as ctx:
                entry.main()

        self.assertEqual(ctx.exception.code, 0)
        self.assertEqual(captured["args"], ["--text", "hi"])

    def test_cli_flag_only(self):
        """只给 --cli 时参数列表为空（交由 CLI 自己提示用法）"""
        captured = {}

        def fake_run_cli(args=None):
            captured["args"] = args
            return 1

        with mock.patch.object(cli_pkg, "run_cli", fake_run_cli), \
                mock.patch.object(sys, "argv", ["main.py", "--cli"]):
            with self.assertRaises(SystemExit) as ctx:
                entry.main()

        self.assertEqual(ctx.exception.code, 1)
        self.assertEqual(captured["args"], [])


if __name__ == "__main__":
    unittest.main()
