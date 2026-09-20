# -*- coding: utf-8 -*-
"""
AutoTyper - 智能字符输入助手

统一入口：默认启动 GUI 模式，加 --cli 参数切换为命令行模式。
"""

import sys


def main():
    """主入口函数"""
    if "--cli" in sys.argv:
        # CLI 模式：--cli 是入口开关，必须从参数中剔除，
        # 否则 argparse 会报 "unrecognized arguments: --cli"
        argv = [arg for arg in sys.argv[1:] if arg != "--cli"]
        from src.cli import run_cli
        sys.exit(run_cli(argv))
    else:
        # GUI 模式
        from src.gui import run_gui
        run_gui()


if __name__ == "__main__":
    main()
