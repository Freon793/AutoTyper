# -*- coding: utf-8 -*-
"""
AutoTyper - 输入引擎测试
"""

import unittest

from src.core import InputEngine


class TestInputEngine(unittest.TestCase):
    """InputEngine 单元测试"""

    def test_default_values(self):
        """测试默认参数"""
        engine = InputEngine()
        self.assertEqual(engine.interval, 0.01)
        self.assertEqual(engine.line_delay, 0.05)

    def test_custom_values(self):
        """测试自定义参数"""
        engine = InputEngine(interval=0.02, line_delay=0.1)
        self.assertEqual(engine.interval, 0.02)
        self.assertEqual(engine.line_delay, 0.1)

    def test_get_foreground_window_returns_int(self):
        """测试获取前台窗口句柄返回整数"""
        hwnd = InputEngine.get_foreground_window()
        self.assertIsInstance(hwnd, int)


class TestCancelCheck(unittest.TestCase):
    """type_text 的中止回调（停止按钮 / 紧急停止）"""

    def _engine(self):
        """构造一个不真正调用 Windows API 的引擎"""
        engine = InputEngine(interval=0.0, line_delay=0.0)
        engine.sent = []
        engine.send_char = lambda hwnd, char: engine.sent.append(char) or True
        engine.send_key = lambda hwnd, vk: True
        return engine

    def test_types_all_chars_without_cancel_check(self):
        """未传入 cancel_check 时输入完整文本"""
        engine = self._engine()
        sent = engine.type_text(0, "abc")
        self.assertEqual(sent, 3)
        self.assertEqual("".join(engine.sent), "abc")

    def test_cancel_check_aborts_midway(self):
        """cancel_check 返回 True 后立即停止发送"""
        engine = self._engine()
        calls = {"n": 0}

        def cancel() -> bool:
            calls["n"] += 1
            return calls["n"] > 4

        sent = engine.type_text(0, "abcdefghij", cancel_check=cancel)

        self.assertLess(sent, 10)
        self.assertEqual(sent, len(engine.sent))
        self.assertEqual("".join(engine.sent), "abcd"[:sent])

    def test_cancel_before_first_char(self):
        """一开始就请求取消时不发送任何字符"""
        engine = self._engine()
        sent = engine.type_text(0, "abcdef", cancel_check=lambda: True)
        self.assertEqual(sent, 0)
        self.assertEqual(engine.sent, [])

    def test_cancel_exception_propagates(self):
        """cancel_check 抛出的异常向上传播（GUI 依赖它捕获紧急停止）"""
        engine = self._engine()

        class Boom(Exception):
            pass

        def cancel() -> bool:
            raise Boom("stop")

        with self.assertRaises(Boom):
            engine.type_text(0, "abcdef", cancel_check=cancel)

    def test_multi_line_cancel(self):
        """多行文本在行边界也会检查取消"""
        engine = self._engine()
        lines_seen = {"n": 0}

        def cancel() -> bool:
            lines_seen["n"] += 1
            return False

        sent = engine.type_text(0, "ab\ncd", cancel_check=cancel)
        self.assertEqual(sent, 4)
        # 每个字符 + 每行起始都会检查
        self.assertGreaterEqual(lines_seen["n"], 4)


if __name__ == "__main__":
    unittest.main()
