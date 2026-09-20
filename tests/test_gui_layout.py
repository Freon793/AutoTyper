# -*- coding: utf-8 -*-
"""
AutoTyper - GUI 布局与交互回归测试

覆盖缺陷：构建出的 EXE 在高 DPI / 窗口尺寸写死的情况下，按钮行被文本区挤出可视区域，
界面上根本看不到「开始输入」按钮，程序无法使用。

注意：整个测试进程只创建一个 Tk 根窗口。反复创建/销毁 Tk 根窗口会触发
"invalid command name tcl_findLibrary" 这类解释器竞态（tkinter 的已知限制），
与产品代码无关，因此这里用 setUpClass 共享窗口。
"""

import time
import unittest
from unittest import mock


class _StubEngine:
    """替身引擎：不接触真实窗口与桌面，只驱动进度与取消回调"""

    def __init__(self, delay: float = 0.0):
        self.delay = delay
        self.last_text = None

    def get_foreground_window(self) -> int:
        return 0

    def type_text(self, hwnd, text, progress_callback=None, cancel_check=None) -> int:
        self.last_text = text
        sent = 0
        for i, _char in enumerate(text, 1):
            if cancel_check and cancel_check():
                return sent
            sent = i
            if progress_callback:
                progress_callback(sent, len(text))
            if self.delay:
                time.sleep(self.delay)
        return sent


class TestGui(unittest.TestCase):
    """界面布局 + 开始/停止流程"""

    app = None

    @classmethod
    def setUpClass(cls):
        try:
            import tkinter  # noqa: F401
        except ImportError:
            raise unittest.SkipTest("当前环境没有 tkinter")

        from src.gui.app import AutoTyperGUI

        try:
            cls.app = AutoTyperGUI()
        except Exception as e:  # TclError：无桌面会话（如无头 CI）
            raise unittest.SkipTest(f"当前环境无法创建窗口: {e}")

        cls.app.root.update_idletasks()
        cls.app.root.update()

    @classmethod
    def tearDownClass(cls):
        if cls.app is not None:
            try:
                cls.app._on_close()   # 正常关闭路径，会取消挂起的定时器
            except Exception:
                pass
            cls.app = None

    def setUp(self):
        """每个用例从干净状态开始"""
        app = self.app
        app.stop_flag = False
        app.is_typing = False
        app.text_area.delete("1.0", "end")
        app.progress_var.set(0)
        app.status_var.set("就绪")
        app._reset_buttons()
        app.root.update_idletasks()
        app.root.update()

    def pump(self, predicate, timeout: float = 5.0) -> bool:
        """等待条件成立，期间保持主线程处理界面事件（含队列刷新）"""
        deadline = time.time() + timeout
        while time.time() < deadline:
            self.app.root.update()
            if predicate():
                return True
            time.sleep(0.02)
        return predicate()

    def start_with(self, text: str, engine) -> None:
        """
        用替身引擎点击「开始输入」。

        _start_typing 会依据界面参数重新构造 InputEngine，因此这里对构造点打桩，
        确保测试绝不向真实桌面注入键盘消息。
        """
        self.app.text_area.delete("1.0", "end")
        self.app.text_area.insert("1.0", text)
        self.app.countdown_var.set(0)     # 跳过倒计时
        self.app.failsafe_var.set(False)  # 不读取真实鼠标位置

        with mock.patch("src.gui.app.InputEngine", return_value=engine):
            self.app.start_btn.invoke()

    # ---------- 布局可见性（本缺陷的回归测试） ----------

    KEY_CONTROLS = ("start_btn", "stop_btn", "progress_bar", "text_area")

    def test_key_controls_are_mapped(self):
        """开始/停止按钮、进度条、文本区都必须被布局显示"""
        for name in self.KEY_CONTROLS:
            widget = getattr(self.app, name)
            self.assertTrue(
                widget.winfo_ismapped(),
                f"{name} 未被显示（不可见也就无法点击）",
            )

    def test_status_bar_is_mapped(self):
        """状态栏不能被主区域挤掉"""
        labels = [w for w in self.app.root.winfo_children() if w.winfo_class() == "TLabel"]
        self.assertTrue(labels, "未找到状态栏控件")
        self.assertTrue(labels[0].winfo_ismapped(), "状态栏不可见")

    def test_key_controls_inside_window(self):
        """控件不能被挤出窗口可视区域"""
        root = self.app.root
        top = root.winfo_rooty()
        bottom = top + root.winfo_height()

        for name in self.KEY_CONTROLS:
            widget = getattr(self.app, name)
            w_top = widget.winfo_rooty()
            w_bottom = w_top + widget.winfo_height()
            self.assertGreaterEqual(w_top, top, f"{name} 超出窗口上边界")
            self.assertLessEqual(w_bottom, bottom, f"{name} 被窗口下边界裁掉")
            self.assertGreater(widget.winfo_height(), 1, f"{name} 高度为 0")

    def test_window_fits_requested_size(self):
        """窗口尺寸不小于控件请求尺寸（写死几何尺寸正是本缺陷的成因）"""
        root = self.app.root
        max_w = int(root.winfo_screenwidth() * 0.95)
        max_h = int(root.winfo_screenheight() * 0.90)

        self.assertGreaterEqual(root.winfo_width(), min(root.winfo_reqwidth(), max_w))
        self.assertGreaterEqual(root.winfo_height(), min(root.winfo_reqheight(), max_h))

    # ---------- 交互流程 ----------

    def test_start_button_runs_to_completion(self):
        """点击开始按钮后输入完成，按钮状态复位、进度到 100"""
        engine = _StubEngine()
        self.start_with("hello world", engine)

        self.assertTrue(
            self.pump(lambda: not self.app.is_typing and not self.app.stop_flag),
            "输入线程未结束",
        )
        self.assertTrue(
            self.pump(lambda: "输入完成" in self.app.status_var.get()),
            f"状态栏未报告完成: {self.app.status_var.get()!r}",
        )
        self.assertEqual(engine.last_text, "hello world")
        self.assertEqual(self.app.progress_var.get(), 100)
        self.assertTrue(self.app.start_btn.instate(["!disabled"]), "开始按钮未复位")
        self.assertTrue(self.app.stop_btn.instate(["disabled"]), "停止按钮未复位")

    def test_stop_button_aborts_typing(self):
        """输入过程中点击停止，输入会立即中止"""
        engine = _StubEngine(delay=0.01)
        self.start_with("x" * 300, engine)

        self.assertTrue(self.pump(lambda: self.app.is_typing), "输入线程未启动")
        self.assertTrue(self.app.stop_btn.instate(["!disabled"]), "停止按钮应可用")

        time.sleep(0.1)
        self.app.stop_btn.invoke()

        self.assertTrue(
            self.pump(lambda: not self.app.is_typing),
            "点击停止后输入线程仍在运行",
        )
        self.assertTrue(
            self.pump(lambda: "已停止" in self.app.status_var.get()),
            f"状态栏未报告停止: {self.app.status_var.get()!r}",
        )
        self.assertTrue(self.app.start_btn.instate(["!disabled"]), "开始按钮未复位")

    def test_empty_text_shows_warning_and_does_not_start(self):
        """文本为空时不应启动输入（弹窗用替身替换，避免阻塞测试）"""
        import src.gui.app as gui_app

        shown = []
        original = gui_app.messagebox.showwarning
        gui_app.messagebox.showwarning = lambda *a, **k: shown.append(a)
        try:
            self.app.text_area.delete("1.0", "end")
            self.app.start_btn.invoke()
        finally:
            gui_app.messagebox.showwarning = original

        self.assertTrue(shown, "未提示文本为空")
        self.assertFalse(self.app.is_typing)
        self.assertTrue(self.app.start_btn.instate(["!disabled"]))


if __name__ == "__main__":
    unittest.main()
