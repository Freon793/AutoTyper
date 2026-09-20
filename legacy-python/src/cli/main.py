# -*- coding: utf-8 -*-
"""
AutoTyper - 命令行模式

通过命令行参数控制文本输入。
"""

import argparse
import sys
import time

import pyautogui

from ..config import ConfigManager
from ..core import InputEngine


def create_parser() -> argparse.ArgumentParser:
    """创建命令行参数解析器"""
    parser = argparse.ArgumentParser(
        prog="autotyper",
        description="AutoTyper - 智能字符输入助手",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
使用示例:
  %(prog)s --text "Hello, World!"
  %(prog)s --file answer.py
  %(prog)s --file answer.py --interval 0.02 --countdown 3
  %(prog)s --file answer.py --snippet "常用代码段"
        """,
    )

    input_group = parser.add_mutually_exclusive_group()
    input_group.add_argument(
        "--text", "-t",
        type=str,
        help="直接指定要输入的文本",
    )
    input_group.add_argument(
        "--file", "-f",
        type=str,
        help="从文件读取要输入的文本",
    )
    input_group.add_argument(
        "--snippet", "-s",
        type=str,
        help="使用已保存的文本片段名称",
    )

    parser.add_argument(
        "--interval",
        type=float,
        default=None,
        help="字符间隔（秒），默认 0.01",
    )
    parser.add_argument(
        "--line-delay",
        type=float,
        default=None,
        help="行间延迟（秒），默认 0.05",
    )
    parser.add_argument(
        "--countdown",
        type=int,
        default=None,
        help="启动前倒计时（秒），默认 5",
    )
    parser.add_argument(
        "--no-failsafe",
        action="store_true",
        help="禁用鼠标紧急停止",
    )
    parser.add_argument(
        "--list-snippets",
        action="store_true",
        help="列出所有已保存的文本片段",
    )

    return parser


def list_snippets(config: ConfigManager) -> None:
    """列出所有文本片段"""
    snippets = config.get_snippets()
    if not snippets:
        print("暂无保存的文本片段。")
        return

    print(f"已保存的文本片段（共 {len(snippets)} 个）：")
    print("-" * 40)
    for name, content in snippets.items():
        preview = content[:50].replace("\n", " ")
        if len(content) > 50:
            preview += "..."
        print(f"  • {name}: {preview}")
    print("-" * 40)


def run_cli(args: list = None) -> int:
    """
    运行 CLI 模式。

    Args:
        args: 命令行参数列表

    Returns:
        退出码（0 表示成功）
    """
    config = ConfigManager()
    parser = create_parser()
    parsed = parser.parse_args(args)

    # 列出文本片段
    if parsed.list_snippets:
        list_snippets(config)
        return 0

    # 确定输入文本
    text = ""
    if parsed.text:
        text = parsed.text
    elif parsed.file:
        try:
            with open(parsed.file, "r", encoding="utf-8") as f:
                text = f.read()
        except IOError as e:
            print(f"[错误] 无法读取文件: {e}")
            return 1
    elif parsed.snippet:
        snippets = config.get_snippets()
        if parsed.snippet not in snippets:
            print(f"[错误] 未找到文本片段: {parsed.snippet}")
            return 1
        text = snippets[parsed.snippet]
    else:
        print("[错误] 请指定 --text、--file 或 --snippet")
        parser.print_help()
        return 1

    if not text.strip():
        print("[错误] 输入文本为空")
        return 1

    # 确定参数（命令行优先，其次配置文件）
    interval = parsed.interval if parsed.interval is not None else config.get("interval", 0.01)
    line_delay = parsed.line_delay if parsed.line_delay is not None else config.get("line_delay", 0.05)
    countdown = parsed.countdown if parsed.countdown is not None else config.get("countdown", 5)
    failsafe = not parsed.no_failsafe

    # 配置 PyAutoGUI
    pyautogui.FAILSAFE = failsafe

    # 创建引擎
    engine = InputEngine(interval=interval, line_delay=line_delay)

    # 倒计时
    print(f"\n{countdown} 秒后开始输入...")
    print("请将光标放到目标输入框中")
    print("紧急停止：鼠标移至屏幕四角 或 Ctrl+C\n")

    try:
        for i in range(countdown, 0, -1):
            print(f"  {i}...")
            time.sleep(1)

        print("\n开始输入")

        hwnd = engine.get_foreground_window()
        print(f"目标窗口句柄: {hwnd}")

        total_chars = len(text)
        chars_sent = 0

        def on_progress(current: int, total: int):
            nonlocal chars_sent
            chars_sent = current
            percent = (current / total) * 100 if total > 0 else 0
            bar_len = 30
            filled = int(bar_len * current / total) if total > 0 else 0
            bar = "█" * filled + "░" * (bar_len - filled)
            print(f"\r  [{bar}] {percent:.1f}%", end="", flush=True)

        def should_cancel() -> bool:
            """鼠标移至屏幕四角时中止输入（由 pyautogui 的 fail-safe 抛出异常）"""
            if failsafe:
                pyautogui.failSafeCheck()
            return False

        chars_sent = engine.type_text(
            hwnd, text, progress_callback=on_progress, cancel_check=should_cancel
        )
        print(f"\n\n输入完成，共发送 {chars_sent} 个字符")
        return 0

    except pyautogui.FailSafeException:
        print(f"\n\n紧急停止（鼠标移至屏幕角落）")
        print(f"已发送 {chars_sent} 个字符")
        return 2
    except KeyboardInterrupt:
        print(f"\n\n用户中断（Ctrl+C）")
        print(f"已发送 {chars_sent} 个字符")
        return 3


if __name__ == "__main__":
    sys.exit(run_cli())
