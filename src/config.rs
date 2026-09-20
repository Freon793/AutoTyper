//! AutoTyper - 配置管理
//!
//! 管理用户偏好设置和文本片段的持久化存储。
//! 配置文件使用 JSON 格式存储在程序工作目录（config.json）。

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// 文本片段表 {名称: 内容}（保持插入顺序，与 Python dict 语义一致）
pub type Snippets = IndexMap<String, String>;

fn default_interval() -> f64 {
    0.01
}
fn default_line_delay() -> f64 {
    0.05
}
fn default_countdown() -> i64 {
    5
}
fn default_failsafe() -> bool {
    true
}

/// 应用配置（字段顺序即 JSON 键顺序）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_interval")]
    pub interval: f64,
    #[serde(default = "default_line_delay")]
    pub line_delay: f64,
    #[serde(default = "default_countdown")]
    pub countdown: i64,
    #[serde(default = "default_failsafe")]
    pub failsafe: bool,
    #[serde(default)]
    pub snippets: Snippets,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            interval: default_interval(),
            line_delay: default_line_delay(),
            countdown: default_countdown(),
            failsafe: default_failsafe(),
            snippets: Snippets::new(),
        }
    }
}

/// 配置文件管理器
///
/// 负责加载、保存用户配置和文本片段。
pub struct ConfigManager {
    pub config_path: PathBuf,
    pub config: AppConfig,
}

impl ConfigManager {
    /// 打开（必要时创建）默认路径 config.json 的配置管理器
    pub fn open_default() -> Self {
        Self::new(PathBuf::from("config.json"))
    }

    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        let mut manager = Self {
            config_path: config_path.into(),
            config: AppConfig::default(),
        };
        manager.load();
        manager
    }

    /// 从文件加载配置：
    /// - 文件不存在：使用默认配置并写盘
    /// - 文件损坏：使用默认配置（不写盘，与 Python 版行为一致）
    /// - 缺失键：按字段默认值补齐（serde default）
    pub fn load(&mut self) {
        if self.config_path.exists() {
            match fs::read_to_string(&self.config_path)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str::<AppConfig>(&s).map_err(|e| e.to_string()))
            {
                Ok(config) => self.config = config,
                Err(_) => self.config = AppConfig::default(),
            }
        } else {
            self.config = AppConfig::default();
            self.save();
        }
    }

    /// 将当前配置保存到文件（UTF-8、两空格缩进、非 ASCII 原样输出）
    pub fn save(&self) -> bool {
        match serde_json::to_string_pretty(&self.config) {
            Ok(text) => fs::write(&self.config_path, text).is_ok(),
            Err(_) => false,
        }
    }

    /// 获取所有文本片段
    pub fn get_snippets(&self) -> &Snippets {
        &self.config.snippets
    }

    /// 添加或更新文本片段（立即保存）
    pub fn add_snippet(&mut self, name: &str, content: &str) -> bool {
        self.config
            .snippets
            .insert(name.to_string(), content.to_string());
        self.save()
    }

    /// 删除文本片段（立即保存）；不存在时返回 false
    pub fn remove_snippet(&mut self, name: &str) -> bool {
        if self.config.snippets.shift_remove(name).is_some() {
            self.save()
        } else {
            false
        }
    }

    /// 导出文本片段到 JSON 文件
    pub fn export_snippets(&self, export_path: impl AsRef<Path>) -> bool {
        match serde_json::to_string_pretty(self.get_snippets()) {
            Ok(text) => fs::write(export_path, text).is_ok(),
            Err(_) => false,
        }
    }

    /// 从 JSON 文件导入文本片段（合并进现有片段并保存）
    pub fn import_snippets(&mut self, import_path: impl AsRef<Path>) -> bool {
        let parsed = fs::read_to_string(import_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Snippets>(&s).ok());
        match parsed {
            Some(snippets) => {
                for (name, content) in snippets {
                    self.config.snippets.insert(name, content);
                }
                self.save()
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_SEQ: AtomicU32 = AtomicU32::new(0);

    /// 每个测试独立的临时目录（避免并行测试互相干扰）
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let n = TEST_SEQ.fetch_add(1, Ordering::SeqCst);
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!("autotyper_cfg_test_{pid}_{n}"));
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn default_config_created() {
        let dir = TempDir::new();
        let cfg = ConfigManager::new(dir.join("config.json"));
        assert_eq!(cfg.config.interval, 0.01);
        assert_eq!(cfg.config.line_delay, 0.05);
        assert_eq!(cfg.config.countdown, 5);
        assert!(cfg.config.failsafe);
    }

    #[test]
    fn config_file_created() {
        let dir = TempDir::new();
        let path = dir.join("config.json");
        let _cfg = ConfigManager::new(&path);
        assert!(path.exists());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new();
        let path = dir.join("config.json");
        let mut cfg = ConfigManager::new(&path);
        cfg.config.interval = 0.03;
        assert!(cfg.save());

        let reloaded = ConfigManager::new(&path);
        assert_eq!(reloaded.config.interval, 0.03);
    }

    #[test]
    fn add_snippet() {
        let dir = TempDir::new();
        let mut cfg = ConfigManager::new(dir.join("config.json"));
        assert!(cfg.add_snippet("test_snippet", "Hello, World!"));
        assert_eq!(
            cfg.get_snippets().get("test_snippet").map(String::as_str),
            Some("Hello, World!")
        );
        // 已持久化
        let reloaded = ConfigManager::new(cfg.config_path.clone());
        assert!(reloaded.get_snippets().contains_key("test_snippet"));
    }

    #[test]
    fn remove_snippet() {
        let dir = TempDir::new();
        let mut cfg = ConfigManager::new(dir.join("config.json"));
        cfg.add_snippet("to_remove", "content");
        assert!(cfg.remove_snippet("to_remove"));
        assert!(!cfg.get_snippets().contains_key("to_remove"));
    }

    #[test]
    fn remove_nonexistent_snippet() {
        let dir = TempDir::new();
        let mut cfg = ConfigManager::new(dir.join("config.json"));
        assert!(!cfg.remove_snippet("nonexistent"));
    }

    #[test]
    fn export_import_snippets() {
        let dir = TempDir::new();
        let mut cfg = ConfigManager::new(dir.join("config.json"));
        cfg.add_snippet("snippet1", "Content 1");
        cfg.add_snippet("snippet2", "Content 2");

        let export_path = dir.join("export.json");
        assert!(cfg.export_snippets(&export_path));

        let mut new_cfg = ConfigManager::new(dir.join("new_config.json"));
        assert!(new_cfg.import_snippets(&export_path));
        assert_eq!(
            new_cfg.get_snippets().get("snippet1").map(String::as_str),
            Some("Content 1")
        );
        assert_eq!(
            new_cfg.get_snippets().get("snippet2").map(String::as_str),
            Some("Content 2")
        );
    }

    #[test]
    fn corrupted_file_fallback() {
        let dir = TempDir::new();
        let path = dir.join("config.json");
        fs::write(&path, "invalid json{{{").unwrap();

        let cfg = ConfigManager::new(&path);
        assert_eq!(cfg.config.interval, 0.01);
        // 与 Python 版一致：损坏时不覆盖原文件
        assert_eq!(fs::read_to_string(&path).unwrap(), "invalid json{{{");
    }

    #[test]
    fn missing_keys_filled_with_defaults() {
        let dir = TempDir::new();
        let path = dir.join("config.json");
        fs::write(&path, r#"{"interval": 0.02}"#).unwrap();

        let cfg = ConfigManager::new(&path);
        assert_eq!(cfg.config.interval, 0.02);
        assert_eq!(cfg.config.line_delay, 0.05);
        assert_eq!(cfg.config.countdown, 5);
        assert!(cfg.config.failsafe);
    }

    #[test]
    fn json_is_utf8_and_pretty() {
        let dir = TempDir::new();
        let mut cfg = ConfigManager::new(dir.join("config.json"));
        cfg.add_snippet("中文片段", "内容");
        let raw = fs::read_to_string(&cfg.config_path).unwrap();
        // ensure_ascii=False 语义：非 ASCII 原样存储
        assert!(raw.contains("中文片段"));
        assert!(raw.contains('\n'));
    }

    #[test]
    fn snippet_insertion_order_preserved() {
        let dir = TempDir::new();
        let mut cfg = ConfigManager::new(dir.join("config.json"));
        cfg.add_snippet("b", "2");
        cfg.add_snippet("a", "1");
        let names: Vec<&String> = cfg.get_snippets().keys().collect();
        assert_eq!(names, vec!["b", "a"]);
    }
}
