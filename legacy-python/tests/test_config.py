# -*- coding: utf-8 -*-
"""
AutoTyper - 配置管理测试
"""

import json
import os
import tempfile
import unittest

from src.config import ConfigManager


class TestConfigManager(unittest.TestCase):
    """ConfigManager 单元测试"""

    def setUp(self):
        """每个测试前创建临时配置文件"""
        self.temp_dir = tempfile.mkdtemp()
        self.config_path = os.path.join(self.temp_dir, "test_config.json")
        self.config = ConfigManager(self.config_path)

    def tearDown(self):
        """每个测试后清理临时文件"""
        if os.path.exists(self.config_path):
            os.remove(self.config_path)
        os.rmdir(self.temp_dir)

    def test_default_config_created(self):
        """测试默认配置被正确创建"""
        self.assertEqual(self.config.get("interval"), 0.01)
        self.assertEqual(self.config.get("line_delay"), 0.05)
        self.assertEqual(self.config.get("countdown"), 5)
        self.assertTrue(self.config.get("failsafe"))

    def test_config_file_created(self):
        """测试配置文件被创建"""
        self.assertTrue(os.path.exists(self.config_path))

    def test_set_and_get(self):
        """测试设置和获取配置项"""
        self.config.set("interval", 0.02)
        self.assertEqual(self.config.get("interval"), 0.02)

    def test_save_and_load(self):
        """测试保存和加载配置"""
        self.config.set("interval", 0.03)
        self.config.save()

        # 重新加载
        new_config = ConfigManager(self.config_path)
        self.assertEqual(new_config.get("interval"), 0.03)

    def test_add_snippet(self):
        """测试添加文本片段"""
        result = self.config.add_snippet("test_snippet", "Hello, World!")
        self.assertTrue(result)
        snippets = self.config.get_snippets()
        self.assertIn("test_snippet", snippets)
        self.assertEqual(snippets["test_snippet"], "Hello, World!")

    def test_remove_snippet(self):
        """测试删除文本片段"""
        self.config.add_snippet("to_remove", "content")
        result = self.config.remove_snippet("to_remove")
        self.assertTrue(result)
        self.assertNotIn("to_remove", self.config.get_snippets())

    def test_remove_nonexistent_snippet(self):
        """测试删除不存在的文本片段"""
        result = self.config.remove_snippet("nonexistent")
        self.assertFalse(result)

    def test_export_import_snippets(self):
        """测试导出和导入文本片段"""
        self.config.add_snippet("snippet1", "Content 1")
        self.config.add_snippet("snippet2", "Content 2")

        export_path = os.path.join(self.temp_dir, "export.json")
        self.config.export_snippets(export_path)

        # 创建新配置并导入
        new_config = ConfigManager(os.path.join(self.temp_dir, "new_config.json"))
        new_config.import_snippets(export_path)

        snippets = new_config.get_snippets()
        self.assertEqual(snippets["snippet1"], "Content 1")
        self.assertEqual(snippets["snippet2"], "Content 2")

        # 清理
        os.remove(export_path)
        os.remove(os.path.join(self.temp_dir, "new_config.json"))

    def test_corrupted_file_fallback(self):
        """测试配置文件损坏时使用默认配置"""
        with open(self.config_path, "w") as f:
            f.write("invalid json{{{")

        config = ConfigManager(self.config_path)
        self.assertEqual(config.get("interval"), 0.01)


if __name__ == "__main__":
    unittest.main()
