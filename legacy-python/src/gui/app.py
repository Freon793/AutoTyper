# -*- coding: utf-8 -*-
"""
AutoTyper - 图形界面模式

使用 tkinter 构建的桌面 GUI 应用。
"""

import queue
import sys
import threading
import time
import tkinter as tk
from tkinter import filedialog, messagebox, ttk

import pyautogui

from ..config import ConfigManager
from ..core import InputEngine


def _enable_dpi_awareness() -> None:
    """
    在创建窗口之前启用 DPI 感知。

    必须在 tk.Tk() 之前调用：否则 Tk 会按 96 DPI 计算布局、再由 Windows 缩放窗口，
    在缩放比例大于 100% 的屏幕上窗口内容会被裁剪到可视区域之外（按钮看不见也点不到）。
    仅 Windows 有效，其余平台静默跳过。
    """
    if sys.platform != "win32":
        return
    try:
        from ctypes import windll

        try:
            windll.shcore.SetProcessDpiAwareness(1)  # Windows 8.1+：系统级 DPI 感知
        except Exception:
            windll.user32.SetProcessDPIAware()       # Windows 7 回退
    except Exception:
        pass


class AutoTyperGUI:
    """
    AutoTyper 图形界面

    提供文本编辑、参数调节、文本片段管理等功能。
    """

    def __init__(self):
        self.config = ConfigManager()
        self.engine = InputEngine(
            interval=self.config.get("interval", 0.01),
            line_delay=self.config.get("line_delay", 0.05),
        )
        self.is_typing = False
        self.stop_flag = False
        # 工作线程通过该队列向主线程投递界面更新（Tk 控件只能在主线程操作）
        self.ui_queue: "queue.Queue" = queue.Queue()
        self._drain_job = None

        self._build_ui()

    def _build_ui(self) -> None:
        """构建 GUI 界面"""
        _enable_dpi_awareness()

        self.root = tk.Tk()
        self.root.title("AutoTyper — 智能字符输入助手")
        self.root.protocol("WM_DELETE_WINDOW", self._on_close)

        self._create_menu()
        # 创建顺序即 pack 的空间分配优先级，详见 _create_main_layout 的说明
        self._create_status_bar()
        self._create_main_layout()
        self._apply_window_size()
        self._drain_ui_queue()

    def _create_menu(self) -> None:
        """创建菜单栏"""
        menubar = tk.Menu(self.root)

        # 文件菜单
        file_menu = tk.Menu(menubar, tearoff=0)
        file_menu.add_command(label="打开文件...", command=self._load_file, accelerator="Ctrl+O")
        file_menu.add_command(label="保存文本...", command=self._save_text, accelerator="Ctrl+S")
        file_menu.add_separator()
        file_menu.add_command(label="退出", command=self._on_close)
        menubar.add_cascade(label="文件", menu=file_menu)

        # 片段菜单
        snippet_menu = tk.Menu(menubar, tearoff=0)
        snippet_menu.add_command(label="保存当前文本为片段...", command=self._save_snippet)
        snippet_menu.add_command(label="导入片段...", command=self._import_snippets)
        snippet_menu.add_command(label="导出片段...", command=self._export_snippets)
        menubar.add_cascade(label="文本片段", menu=snippet_menu)

        # 帮助菜单
        help_menu = tk.Menu(menubar, tearoff=0)
        help_menu.add_command(label="使用说明", command=self._show_help)
        help_menu.add_command(label="关于", command=self._show_about)
        menubar.add_cascade(label="帮助", menu=help_menu)

        self.root.config(menu=menubar)

        # 绑定快捷键
        self.root.bind("<Control-o>", lambda e: self._load_file())
        self.root.bind("<Control-s>", lambda e: self._save_text())

    def _create_status_bar(self) -> None:
        """
        创建状态栏。

        必须先于主区域 pack：pack 是按调用顺序分配空间的，若主区域先以
        expand=True 占满窗口，后 pack 的状态栏只能分到 0 高度而不可见。
        """
        self.status_var = tk.StringVar(value="就绪")
        status_bar = ttk.Label(
            self.root, textvariable=self.status_var, relief=tk.SUNKEN, anchor=tk.W, padding=(5, 2)
        )
        status_bar.pack(side=tk.BOTTOM, fill=tk.X)

    def _create_main_layout(self) -> None:
        """
        创建主布局。

        顺序很关键：pack 按调用顺序切分剩余空间，先分配的控件拿到自己请求的高度，
        最后才轮到 expand=True 的文本区吃掉剩余空间。因此“按钮行 → 参数行 → 文本区”
        的顺序能保证窗口再小或系统 DPI 再高，按钮也不会被挤出可视区域。
        """
        main_frame = ttk.Frame(self.root, padding=10)
        main_frame.pack(fill=tk.BOTH, expand=True)

        self._create_button_row(main_frame)
        self._create_control_row(main_frame)
        self._create_text_area(main_frame)

    def _create_text_area(self, parent: tk.Widget) -> None:
        """创建文本编辑区（最后 pack，吸收剩余高度）"""
        text_frame = ttk.LabelFrame(parent, text="输入文本", padding=5)
        text_frame.pack(side=tk.TOP, fill=tk.BOTH, expand=True)

        self.text_area = tk.Text(
            text_frame, wrap=tk.WORD, font=("Consolas", 11), height=12, width=60
        )
        self.text_area.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        scrollbar = ttk.Scrollbar(text_frame, orient=tk.VERTICAL, command=self.text_area.yview)
        scrollbar.pack(side=tk.RIGHT, fill=tk.Y)
        self.text_area.config(yscrollcommand=scrollbar.set)

    def _create_control_row(self, parent: tk.Widget) -> None:
        """创建参数设置区"""
        control_frame = ttk.LabelFrame(parent, text="参数设置", padding=8)
        control_frame.pack(side=tk.BOTTOM, fill=tk.X, pady=(0, 8))

        # 字符间隔
        ttk.Label(control_frame, text="字符间隔(ms):").grid(row=0, column=0, sticky=tk.W)
        self.interval_var = tk.IntVar(value=int(self.config.get("interval", 0.01) * 1000))
        interval_scale = ttk.Scale(
            control_frame, from_=5, to=100, variable=self.interval_var,
            orient=tk.HORIZONTAL, length=150
        )
        interval_scale.grid(row=0, column=1, padx=5)
        self.interval_label = ttk.Label(control_frame, text=f"{self.interval_var.get()}ms")
        self.interval_label.grid(row=0, column=2, padx=5)
        self.interval_var.trace_add("write", lambda *a: self.interval_label.config(text=f"{self.interval_var.get()}ms"))

        # 行间延迟
        ttk.Label(control_frame, text="行间延迟(ms):").grid(row=0, column=3, sticky=tk.W, padx=(20, 0))
        self.line_delay_var = tk.IntVar(value=int(self.config.get("line_delay", 0.05) * 1000))
        line_delay_scale = ttk.Scale(
            control_frame, from_=10, to=200, variable=self.line_delay_var,
            orient=tk.HORIZONTAL, length=150
        )
        line_delay_scale.grid(row=0, column=4, padx=5)
        self.line_delay_label = ttk.Label(control_frame, text=f"{self.line_delay_var.get()}ms")
        self.line_delay_label.grid(row=0, column=5, padx=5)
        self.line_delay_var.trace_add("write", lambda *a: self.line_delay_label.config(text=f"{self.line_delay_var.get()}ms"))

        # 倒计时
        ttk.Label(control_frame, text="倒计时(s):").grid(row=1, column=0, sticky=tk.W, pady=(5, 0))
        self.countdown_var = tk.IntVar(value=self.config.get("countdown", 5))
        countdown_spin = ttk.Spinbox(
            control_frame, from_=1, to=30, textvariable=self.countdown_var, width=5
        )
        countdown_spin.grid(row=1, column=1, sticky=tk.W, padx=5, pady=(5, 0))

        # 紧急停止开关
        self.failsafe_var = tk.BooleanVar(value=self.config.get("failsafe", True))
        failsafe_check = ttk.Checkbutton(
            control_frame, text="启用紧急停止（鼠标移至屏幕四角）",
            variable=self.failsafe_var
        )
        failsafe_check.grid(row=1, column=3, columnspan=3, sticky=tk.W, padx=(20, 0), pady=(5, 0))

    def _create_button_row(self, parent: tk.Widget) -> None:
        """创建操作按钮区（最先 pack，优先级最高，任何情况下都可见）"""
        button_frame = ttk.Frame(parent)
        button_frame.pack(side=tk.BOTTOM, fill=tk.X)

        self.start_btn = ttk.Button(
            button_frame, text="开始输入", command=self._start_typing, style="Accent.TButton"
        )
        self.start_btn.pack(side=tk.LEFT, padx=(0, 5))

        self.stop_btn = ttk.Button(
            button_frame, text="停止", command=self._stop_typing, state=tk.DISABLED
        )
        self.stop_btn.pack(side=tk.LEFT, padx=5)

        # 进度条
        self.progress_var = tk.DoubleVar(value=0)
        self.progress_bar = ttk.Progressbar(
            button_frame, variable=self.progress_var, maximum=100, length=200
        )
        self.progress_bar.pack(side=tk.RIGHT, padx=5)

        ttk.Label(button_frame, text="进度:").pack(side=tk.RIGHT)

    def _apply_window_size(self) -> None:
        """
        按控件实际需要的尺寸设置窗口大小。

        字体尺寸随系统 DPI 变化，写死 720x580 在高分屏上装不下全部控件
        （按钮被挤出窗口），这里改为按请求尺寸自适应，并限制在屏幕范围内。
        """
        self.root.update_idletasks()

        req_w = self.root.winfo_reqwidth()
        req_h = self.root.winfo_reqheight()
        screen_w = self.root.winfo_screenwidth()
        screen_h = self.root.winfo_screenheight()

        max_w = max(480, int(screen_w * 0.95))
        max_h = max(360, int(screen_h * 0.90))
        width = min(max(req_w, 720), max_w)
        height = min(max(req_h, 580), max_h)

        x = max(0, (screen_w - width) // 2)
        y = max(0, (screen_h - height) // 3)
        self.root.geometry(f"{width}x{height}+{x}+{y}")
        self.root.minsize(min(req_w, max_w), min(req_h, max_h))

    def _drain_ui_queue(self) -> None:
        """在主线程中消费工作线程投递的界面更新"""
        while True:
            try:
                kind, value = self.ui_queue.get_nowait()
            except queue.Empty:
                break

            if kind == "status":
                self.status_var.set(value)
            elif kind == "progress":
                self.progress_var.set(value)
            elif kind == "reset":
                self._reset_buttons()

        try:
            self._drain_job = self.root.after(50, self._drain_ui_queue)
        except tk.TclError:
            self._drain_job = None

    def _on_close(self) -> None:
        """
        关闭窗口。

        必须先取消挂起的定时器：窗口销毁后定时器仍会触发，届时回调对应的
        Tcl 命令已不存在，会在后台抛出 "invalid command name" 错误。
        """
        if self._drain_job is not None:
            try:
                self.root.after_cancel(self._drain_job)
            except Exception:
                pass
            self._drain_job = None

        self.root.destroy()

    def _set_status(self, text: str) -> None:
        """线程安全地更新状态栏"""
        self.ui_queue.put(("status", text))

    def _set_progress(self, percent: float) -> None:
        """线程安全地更新进度条"""
        self.ui_queue.put(("progress", percent))

    def _load_file(self) -> None:
        """从文件加载文本"""
        filepath = filedialog.askopenfilename(
            title="选择文本文件",
            filetypes=[
                ("所有文件", "*.*"),
                ("文本文件", "*.txt"),
                ("Python", "*.py"),
                ("Java", "*.java"),
                ("C/C++", "*.c *.cpp *.h"),
            ],
        )
        if filepath:
            try:
                with open(filepath, "r", encoding="utf-8") as f:
                    content = f.read()
                self.text_area.delete("1.0", tk.END)
                self.text_area.insert("1.0", content)
                self.status_var.set(f"已加载: {filepath}")
            except IOError as e:
                messagebox.showerror("错误", f"无法读取文件:\n{e}")

    def _save_text(self) -> None:
        """保存文本到文件"""
        filepath = filedialog.asksaveasfilename(
            title="保存文本",
            defaultextension=".txt",
            filetypes=[("文本文件", "*.txt"), ("所有文件", "*.*")],
        )
        if filepath:
            try:
                with open(filepath, "w", encoding="utf-8") as f:
                    f.write(self.text_area.get("1.0", tk.END))
                self.status_var.set(f"已保存: {filepath}")
            except IOError as e:
                messagebox.showerror("错误", f"无法保存文件:\n{e}")

    def _save_snippet(self) -> None:
        """保存当前文本为片段"""
        name = self._ask_snippet_name()
        if name:
            content = self.text_area.get("1.0", tk.END).strip()
            if content:
                self.config.add_snippet(name, content)
                self.status_var.set(f"已保存片段: {name}")
            else:
                messagebox.showwarning("警告", "文本为空，无法保存")

    def _ask_snippet_name(self) -> str:
        """弹出对话框获取片段名称"""
        dialog = tk.Toplevel(self.root)
        dialog.title("保存文本片段")
        dialog.geometry("300x100")
        dialog.transient(self.root)
        dialog.grab_set()

        ttk.Label(dialog, text="请输入片段名称:").pack(pady=(10, 5))
        name_var = tk.StringVar()
        entry = ttk.Entry(dialog, textvariable=name_var, width=30)
        entry.pack(pady=5)
        entry.focus()

        result = {"name": ""}

        def on_ok():
            result["name"] = name_var.get().strip()
            dialog.destroy()

        def on_cancel():
            dialog.destroy()

        btn_frame = ttk.Frame(dialog)
        btn_frame.pack(pady=5)
        ttk.Button(btn_frame, text="确定", command=on_ok).pack(side=tk.LEFT, padx=5)
        ttk.Button(btn_frame, text="取消", command=on_cancel).pack(side=tk.LEFT, padx=5)

        entry.bind("<Return>", lambda e: on_ok())
        entry.bind("<Escape>", lambda e: on_cancel())

        self.root.wait_window(dialog)
        return result["name"]

    def _import_snippets(self) -> None:
        """导入文本片段"""
        filepath = filedialog.askopenfilename(
            title="导入文本片段",
            filetypes=[("JSON", "*.json"), ("所有文件", "*.*")],
        )
        if filepath:
            if self.config.import_snippets(filepath):
                self.status_var.set(f"已导入片段: {filepath}")
            else:
                messagebox.showerror("错误", "导入失败")

    def _export_snippets(self) -> None:
        """导出文本片段"""
        filepath = filedialog.asksaveasfilename(
            title="导出文本片段",
            defaultextension=".json",
            filetypes=[("JSON", "*.json")],
        )
        if filepath:
            if self.config.export_snippets(filepath):
                self.status_var.set(f"已导出片段: {filepath}")
            else:
                messagebox.showerror("错误", "导出失败")

    def _show_help(self) -> None:
        """显示使用说明"""
        help_text = """使用说明：

1. 在上方文本框中输入或粘贴要输入的文本
2. 调节字符间隔、行间延迟和倒计时参数
3. 点击「开始输入」，迅速将光标放到目标输入框
4. 等待倒计时结束，自动开始输入
5. 输入过程中可点击「停止」立即中止；紧急情况可将鼠标移至屏幕四角

快捷键：
  Ctrl+O  打开文件
  Ctrl+S  保存文本
"""
        messagebox.showinfo("使用说明", help_text)

    def _show_about(self) -> None:
        """显示关于信息"""
        messagebox.showinfo(
            "关于 AutoTyper",
            "AutoTyper v1.0.0\n\n"
            "智能字符输入助手\n"
            "通过 Windows 消息队列直接注入字符\n\n"
            "基于 MIT 许可证开源",
        )

    def _start_typing(self) -> None:
        """开始输入"""
        text = self.text_area.get("1.0", tk.END).strip()
        if not text:
            messagebox.showwarning("警告", "请输入要输入的文本")
            return

        self.start_btn.config(state=tk.DISABLED)
        self.stop_btn.config(state=tk.NORMAL)
        self.is_typing = True
        self.stop_flag = False
        self.progress_var.set(0)

        # 更新引擎参数
        interval = self.interval_var.get() / 1000
        line_delay = self.line_delay_var.get() / 1000
        self.engine = InputEngine(interval=interval, line_delay=line_delay)

        # 配置 PyAutoGUI 紧急停止
        failsafe = self.failsafe_var.get()
        pyautogui.FAILSAFE = failsafe

        countdown = self.countdown_var.get()

        # 在后台线程运行
        thread = threading.Thread(
            target=self._typing_worker,
            args=(text, countdown, failsafe),
            daemon=True,
        )
        thread.start()

    def _typing_worker(self, text: str, countdown: int, failsafe: bool) -> None:
        """输入工作线程"""
        chars_sent = 0
        try:
            # 倒计时
            for i in range(countdown, 0, -1):
                if self.stop_flag:
                    self._set_status("已取消输入")
                    return
                self._set_status(f"{i} 秒后开始输入...")
                time.sleep(1)

            if self.stop_flag:
                self._set_status("已取消输入")
                return

            self._set_status("正在输入...")
            hwnd = self.engine.get_foreground_window()

            def on_progress(current: int, total_chars: int) -> None:
                nonlocal chars_sent
                chars_sent = current
                percent = (current / total_chars) * 100 if total_chars > 0 else 0
                self._set_progress(percent)
                self._set_status(f"正在输入... {percent:.1f}%")

            def should_cancel() -> bool:
                """是否中止输入：按下停止按钮，或启用了紧急停止且鼠标位于屏幕四角"""
                if self.stop_flag:
                    return True
                if failsafe:
                    # 鼠标位于屏幕四角时 failSafeCheck 会抛出 FailSafeException
                    pyautogui.failSafeCheck()
                return False

            self.engine.type_text(
                hwnd, text, progress_callback=on_progress, cancel_check=should_cancel
            )

            if self.stop_flag:
                self._set_status(f"已停止，已发送 {chars_sent} 个字符")
            else:
                self._set_progress(100)
                self._set_status(f"输入完成，共发送 {chars_sent} 个字符")

        except pyautogui.FailSafeException:
            self._set_status(f"紧急停止，已发送 {chars_sent} 个字符")
        except Exception as e:
            self._set_status(f"错误: {e}")
        finally:
            self.is_typing = False
            # 交回主线程处理，避免在工作线程里操作 Tk 控件
            self.ui_queue.put(("reset", None))

    def _stop_typing(self) -> None:
        """停止输入"""
        self.stop_flag = True
        self._set_status("正在停止...")

    def _reset_buttons(self) -> None:
        """重置按钮状态"""
        self.start_btn.config(state=tk.NORMAL)
        self.stop_btn.config(state=tk.DISABLED)

    def run(self) -> None:
        """启动 GUI 主循环"""
        self.root.mainloop()


def run_gui() -> None:
    """启动 GUI 模式的入口函数"""
    app = AutoTyperGUI()
    app.run()


if __name__ == "__main__":
    run_gui()
