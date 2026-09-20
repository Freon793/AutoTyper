# -*- coding: utf-8 -*-
"""
AutoTyper - 配置管理

管理用户偏好设置和文本片段的持久化存储。
"""

import json
import os
from typing import Any, Dict, List, Optional

DEFAULT_CONFIG: Dict[str, Any] = {
    "interval": 0.01,
    "line_delay": 0.05,
    "countdown": 5,
    "failsafe": True,
    "snippets": {},
}


class ConfigManager:
    """
    配置文件管理器

    负责加载、保存用户配置和文本片段。
    配置文件使用 JSON 格式存储在用户工作目录。

    Attributes:
        config_path: 配置文件路径
        config: 当前配置字典
    """

    def __init__(self, config_path: str = "config.json"):
        self.config_path = config_path
        self.config: Dict[str, Any] = {}
        self.load()

    def load(self) -> Dict[str, Any]:
        """
        从文件加载配置，若文件不存在则使用默认配置。

        Returns:
            当前配置字典
        """
        if os.path.exists(self.config_path):
            try:
                with open(self.config_path, "r", encoding="utf-8") as f:
                    self.config = json.load(f)
                # 确保默认键存在
                for key, value in DEFAULT_CONFIG.items():
                    if key not in self.config:
                        self.config[key] = value
            except (json.JSONDecodeError, IOError):
                self.config = DEFAULT_CONFIG.copy()
        else:
            self.config = DEFAULT_CONFIG.copy()
            self.save()
        return self.config

    def save(self) -> bool:
        """
        将当前配置保存到文件。

        Returns:
            是否保存成功
        """
        try:
            with open(self.config_path, "w", encoding="utf-8") as f:
                json.dump(self.config, f, ensure_ascii=False, indent=2)
            return True
        except IOError:
            return False

    def get(self, key: str, default: Any = None) -> Any:
        """
        获取配置项。

        Args:
            key: 配置键名
            default: 默认值

        Returns:
            配置值
        """
        return self.config.get(key, default)

    def set(self, key: str, value: Any) -> None:
        """
        设置配置项（不自动保存）。

        Args:
            key: 配置键名
            value: 配置值
        """
        self.config[key] = value

    def get_snippets(self) -> Dict[str, str]:
        """
        获取所有文本片段。

        Returns:
            文本片段字典 {名称: 内容}
        """
        return self.config.get("snippets", {})

    def add_snippet(self, name: str, content: str) -> bool:
        """
        添加或更新文本片段。

        Args:
            name: 片段名称
            content: 片段内容

        Returns:
            是否保存成功
        """
        if "snippets" not in self.config:
            self.config["snippets"] = {}
        self.config["snippets"][name] = content
        return self.save()

    def remove_snippet(self, name: str) -> bool:
        """
        删除文本片段。

        Args:
            name: 片段名称

        Returns:
            是否删除成功
        """
        snippets = self.config.get("snippets", {})
        if name in snippets:
            del snippets[name]
            return self.save()
        return False

    def export_snippets(self, export_path: str) -> bool:
        """
        导出文本片段到文件。

        Args:
            export_path: 导出文件路径

        Returns:
            是否导出成功
        """
        try:
            with open(export_path, "w", encoding="utf-8") as f:
                json.dump(self.get_snippets(), f, ensure_ascii=False, indent=2)
            return True
        except IOError:
            return False

    def import_snippets(self, import_path: str) -> bool:
        """
        从文件导入文本片段。

        Args:
            import_path: 导入文件路径

        Returns:
            是否导入成功
        """
        try:
            with open(import_path, "r", encoding="utf-8") as f:
                snippets = json.load(f)
            if "snippets" not in self.config:
                self.config["snippets"] = {}
            self.config["snippets"].update(snippets)
            return self.save()
        except (json.JSONDecodeError, IOError):
            return False
