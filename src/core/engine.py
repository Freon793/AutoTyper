# -*- coding: utf-8 -*-
"""
AutoTyper - 输入引擎

通过 PostMessageW 向目标窗口消息队列投递 WM_CHAR 消息，
实现绕过剪贴板限制的字符级输入。
"""

import time
import ctypes
from ctypes import wintypes

# Windows API 常量
WM_CHAR = 0x0102
WM_KEYDOWN = 0x0100
WM_KEYUP = 0x0101
VK_RETURN = 0x0D
VK_TAB = 0x09

# 加载 user32.dll
user32 = ctypes.WinDLL("user32", use_last_error=True)


def _setup_api_prototypes():
    """设置 Windows API 函数原型（类型安全）"""
    user32.PostMessageW.argtypes = [
        wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM
    ]
    user32.PostMessageW.restype = wintypes.BOOL

    user32.GetForegroundWindow.argtypes = []
    user32.GetForegroundWindow.restype = wintypes.HWND

    user32.SetForegroundWindow.argtypes = [wintypes.HWND]
    user32.SetForegroundWindow.restype = wintypes.BOOL


_setup_api_prototypes()


class InputEngine:
    """
    字符输入引擎

    通过 PostMessageW 向目标窗口逐字符发送 WM_CHAR 消息，
    绕过剪贴板和键盘事件拦截。

    Attributes:
        interval: 字符间隔（秒），默认 0.01
        line_delay: 行间延迟（秒），默认 0.05
    """

    def __init__(self, interval: float = 0.01, line_delay: float = 0.05):
        self.interval = interval
        self.line_delay = line_delay

    def send_char(self, hwnd: int, char: str) -> bool:
        """
        向目标窗口发送单个 Unicode 字符。

        Args:
            hwnd: 目标窗口句柄
            char: 要发送的字符

        Returns:
            是否发送成功
        """
        code_point = ord(char)
        result = user32.PostMessageW(hwnd, WM_CHAR, code_point, 0)
        return bool(result)

    def send_key(self, hwnd: int, vk_code: int) -> bool:
        """
        向目标窗口发送按键事件（KeyDown + KeyUp）。

        Args:
            hwnd: 目标窗口句柄
            vk_code: 虚拟键码

        Returns:
            是否发送成功
        """
        user32.PostMessageW(hwnd, WM_KEYDOWN, vk_code, 0)
        time.sleep(0.005)
        user32.PostMessageW(hwnd, WM_KEYUP, vk_code, 0)
        return True

    def type_text(self, hwnd: int, text: str, progress_callback=None, cancel_check=None) -> int:
        """
        向目标窗口逐字符输入文本。

        Args:
            hwnd: 目标窗口句柄
            text: 要输入的文本
            progress_callback: 进度回调函数，接收 (current_char, total_chars)
            cancel_check: 可选回调，返回 True 时立即中止输入并返回已发送字符数
                （供「停止」按钮与鼠标紧急停止使用；若回调抛异常也会向上传播）

        Returns:
            已发送的字符总数
        """
        lines = text.split("\n")
        total = len(lines)
        total_chars = len(text)
        chars_sent = 0

        for line_idx, line in enumerate(lines):
            if cancel_check and cancel_check():
                break

            if line == "":
                self.send_key(hwnd, VK_RETURN)
                time.sleep(self.line_delay)
                continue

            for char in line:
                if cancel_check and cancel_check():
                    return chars_sent

                if char == "\t":
                    self.send_key(hwnd, VK_TAB)
                else:
                    self.send_char(hwnd, char)
                time.sleep(self.interval)
                chars_sent += 1

                if progress_callback:
                    progress_callback(chars_sent, total_chars)

            # 换行（最后一行除外）
            if line_idx < total - 1:
                self.send_key(hwnd, VK_RETURN)
                time.sleep(self.line_delay)

        return chars_sent

    @staticmethod
    def get_foreground_window() -> int:
        """获取当前前台窗口句柄"""
        return user32.GetForegroundWindow()

    @staticmethod
    def set_foreground_window(hwnd: int) -> bool:
        """将指定窗口设为前台"""
        return bool(user32.SetForegroundWindow(hwnd))
