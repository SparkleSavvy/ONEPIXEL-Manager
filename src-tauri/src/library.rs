use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::config;
use crate::github::AssetKind;
use crate::server;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFile {
    pub name: String,
    pub size: u64,
    pub kind: Option<AssetKind>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledVersion {
    pub tag: String,
    pub dir: String,
    pub files: Vec<InstalledFile>,
    pub installed_at: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledServer {
    pub tag: String,
    pub dir: String,
    pub script: Option<String>,
    pub installed_at: u64,
    /// Path to server.properties when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties_path: Option<String>,
    /// Current value of `online-mode` in server.properties.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_mode: Option<bool>,
    /// A standalone server.jar exists — the manager runs it via its own
    /// generated script with Aikar's flags.
    pub has_server_jar: bool,
    /// Configured JVM heap size in MB for managed starts.
    pub ram_mb: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub versions: Vec<InstalledVersion>,
    pub servers: Vec<InstalledServer>,
    /// Tags with a running server process.
    pub running: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MetaFile {
    #[serde(default)]
    installed_at: u64,
}

pub fn version_dir(tag: &str) -> PathBuf {
    config::versions_dir().join(sanitize(tag))
}

pub fn server_dir(tag: &str) -> PathBuf {
    config::servers_dir().join(sanitize(tag))
}

fn sanitize(tag: &str) -> String {
    tag.chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

fn read_installed_at(dir: &Path) -> u64 {
    fs::read_to_string(dir.join("meta.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<MetaFile>(&text).ok())
        .map(|m| m.installed_at)
        .unwrap_or(0)
}

/// Read `online-mode` from a server.properties file.
fn read_online_mode(path: &Path) -> Option<bool> {
    let content = fs::read_to_string(path).ok()?;
    content.lines().find_map(|line| {
        line.trim()
            .strip_prefix("online-mode=")
            .map(|v| v.trim().eq_ignore_ascii_case("true"))
    })
}

/// Flip `online-mode` in server.properties. Takes effect on next server start.
#[tauri::command]
pub async fn set_online_mode(tag: String, enabled: bool) -> Result<bool, String> {
    let path = crate::server::find_properties(&server_dir(&tag))
        .ok_or_else(|| "server.properties not found in this server pack".to_string())?;
    let content = fs::read_to_string(&path).map_err(|e| format!("Cannot read server.properties: {e}"))?;

    let new_value = if enabled { "true" } else { "false" };
    let mut replaced = false;
    let mut out = String::with_capacity(content.len() + 16);
    for line in content.lines() {
        if !replaced && line.trim_start().starts_with("online-mode=") {
            out.push_str(&format!("online-mode={new_value}"));
            replaced = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if !replaced {
        out.push_str(&format!("online-mode={new_value}\n"));
    }

    fs::write(&path, out).map_err(|e| format!("Cannot write server.properties: {e}"))?;
    Ok(enabled)
}

pub fn write_meta(dir: &Path, tag: &str, kind: &str, file_name: &str, size: u64) {
    let meta = serde_json::json!({
        "tag": tag,
        "kind": kind,
        "fileName": file_name,
        "size": size,
        "installedAt": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    let _ = fs::create_dir_all(dir);
    let _ = fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap_or_default());
}

fn is_active_download(state: &AppState, id_prefixes: &[String]) -> bool {
    let cancels = state.cancels.lock().unwrap();
    id_prefixes.iter().any(|id| {
        cancels
            .get(id)
            .map(|f| !f.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
    })
}

/// Everything currently stored on disk plus which servers are running.
#[tauri::command]
pub fn list_library(state: State<'_, AppState>) -> LibrarySnapshot {
    let cfg = crate::config::load_config();
    let mut versions = Vec::new();
    if let Ok(entries) = fs::read_dir(config::versions_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let mut files = Vec::new();
            if let Ok(files_iter) = fs::read_dir(&path) {
                for f in files_iter.flatten() {
                    let fp = f.path();
                    if !fp.is_file() {
                        continue;
                    }
                    let name = f.file_name().to_string_lossy().to_string();
                    if name.ends_with(".part") || name == "meta.json" {
                        continue;
                    }
                    let size = fp.metadata().map(|m| m.len()).unwrap_or(0);
                    let kind = AssetKind::from_name(&name);
                    files.push(InstalledFile {
                        name,
                        size,
                        kind,
                    });
                }
            }
            if files.is_empty() {
                continue;
            }
            let tag = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            versions.push(InstalledVersion {
                tag,
                dir: path.to_string_lossy().to_string(),
                files,
                installed_at: read_installed_at(&path),
            });
        }
    }

    let mut servers = Vec::new();
    if let Ok(entries) = fs::read_dir(config::servers_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let tag = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let has_files = fs::read_dir(&path)
                .map(|mut it| it.next().is_some())
                .unwrap_or(false);
            if !has_files {
                continue;
            }
            let props = crate::server::find_properties(&path);
            let online_mode = props.as_ref().and_then(|p| read_online_mode(p));
            servers.push(InstalledServer {
                has_server_jar: server::find_server_jar(&path).is_some(),
                ram_mb: cfg
                    .server_ram
                    .get(&tag)
                    .copied()
                    .unwrap_or(server::DEFAULT_RAM_MB),
                tag,
                dir: path.to_string_lossy().to_string(),
                script: server::find_script(&path).map(|p| p.to_string_lossy().to_string()),
                installed_at: read_installed_at(&path),
                properties_path: props.map(|p| p.to_string_lossy().to_string()),
                online_mode,
            });
        }
    }

    versions.sort_by(|a, b| b.tag.cmp(&a.tag));
    servers.sort_by(|a, b| b.tag.cmp(&a.tag));

    let running = state
        .servers
        .lock()
        .unwrap()
        .keys()
        .cloned()
        .collect();

    LibrarySnapshot {
        versions,
        servers,
        running,
    }
}

/// Delete an installed modpack version folder.
#[tauri::command]
pub async fn delete_version(state: State<'_, AppState>, tag: String) -> Result<(), String> {
    if is_active_download(
        &state,
        &[
            format!("client:{tag}"),
            format!("zip:{tag}"),
        ],
    ) {
        return Err("Cancel the active download for this version first".into());
    }
    let dir = version_dir(&tag);
    if !dir.is_dir() {
        return Err(format!("Version {tag} is not installed"));
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("Failed to delete: {e}"))
}

/// Delete an extracted server pack.
#[tauri::command]
pub async fn delete_server(state: State<'_, AppState>, tag: String) -> Result<(), String> {
    if state.servers.lock().unwrap().contains_key(&tag) {
        return Err("Stop the server before deleting it".into());
    }
    if is_active_download(&state, &[format!("server:{tag}")]) {
        return Err("Cancel the active download for this server pack first".into());
    }
    let dir = server_dir(&tag);
    if !dir.is_dir() {
        return Err(format!("Server {tag} is not installed"));
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("Failed to delete: {e}"))
}

/// Open a path inside the app data directory in Explorer (reveal file / open dir).
#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    let allowed = config::app_dir();
    let target = PathBuf::from(&path);
    if !(target.starts_with(&allowed) && target.exists()) {
        return Err("Path is outside the manager's data directory".into());
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut cmd = std::process::Command::new("explorer");
        if target.is_dir() {
            cmd.arg(&target);
        } else {
            cmd.arg(format!("/select,{}", target.display()));
        }
        cmd.creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(if target.is_dir() { target.as_os_str() } else { allowed.as_os_str() })
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
