use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LauncherConfig {
    /// One of: elyprism | prism | xmcl | custom
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exe_path: Option<String>,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            kind: "prism".into(),
            exe_path: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub launcher: Option<LauncherConfig>,
    /// owner/name of the manager's own GitHub repository. Reserved for the
    /// future self-update feature; self-update stays inert until it is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manager_repo: Option<String>,
    /// Per-tag JVM heap size in MB for managed server starts.
    pub server_ram: HashMap<String, u64>,
    /// Explicit path to a java executable used by managed server starts.
    /// When empty, the manager auto-detects or auto-downloads one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_path: Option<String>,
}

pub fn app_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("onepixel-manager")
}

pub fn versions_dir() -> PathBuf {
    app_dir().join("versions")
}

pub fn servers_dir() -> PathBuf {
    app_dir().join("servers")
}

fn config_path() -> PathBuf {
    app_dir().join("config.json")
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(config: &AppConfig) -> std::io::Result<()> {
    let dir = app_dir();
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(config)?;
    fs::write(config_path(), json)
}
