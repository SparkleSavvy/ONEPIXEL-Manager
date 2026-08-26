use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::config::{self, AppConfig, LauncherConfig};
use crate::library;
use crate::state::AppState;

pub const LAUNCHER_KINDS: &[&str] = &["elyprism", "prism", "xmcl", "custom"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedLauncher {
    pub kind: String,
    pub exe_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub message: String,
}

fn kind_for_exe(file_name: &str) -> Option<&'static str> {
    let lower = file_name.to_lowercase();
    if lower.contains("prismlauncher") {
        Some("prism")
    } else if lower.contains("ely") && lower.contains("prism") || lower.contains("elyprism") {
        Some("elyprism")
    } else if lower.contains("xmcl") {
        Some("xmcl")
    } else {
        None
    }
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

fn push_candidate(out: &mut Vec<DetectedLauncher>, path: PathBuf) {
    if !path.is_file() {
        return;
    }
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
        return;
    };
    let Some(kind) = kind_for_exe(&name) else { return };
    let entry = DetectedLauncher {
        kind: kind.to_string(),
        exe_path: path.to_string_lossy().to_string(),
    };
    if !out.iter().any(|d| d.exe_path == entry.exe_path) {
        out.push(entry);
    }
}

/// Scan common install locations for known launchers.
#[tauri::command]
pub fn detect_launchers() -> Vec<DetectedLauncher> {
    let mut found: Vec<DetectedLauncher> = Vec::new();

    let local = env_path("LOCALAPPDATA");
    let progfiles = env_path("ProgramFiles");
    let progfiles86 = env_path("ProgramFiles(x86)");

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(l) = &local {
        roots.push(l.join("Programs"));
        roots.push(l.clone());
    }
    for pf in [&progfiles, &progfiles86].into_iter().flatten() {
        roots.push(pf.clone());
    }

    // Direct candidates first.
    let mut direct: Vec<PathBuf> = Vec::new();
    for root in &roots {
        direct.push(root.join("PrismLauncher/prismlauncher.exe"));
        direct.push(root.join("PrismLauncher/PrismLauncher.exe"));
        direct.push(root.join("ElyPrism/elyprism.exe"));
        direct.push(root.join("ElyPrism/ElyPrism.exe"));
        direct.push(root.join("ely-prism/elyprism.exe"));
        direct.push(root.join("xmcl/XMCL.exe"));
        direct.push(root.join("XMCL/XMCL.exe"));
    }
    for p in direct {
        push_candidate(&mut found, p);
    }

    // Wildcard scan of install dirs' top-level executables.
    for root in &roots {
        let Ok(entries) = std::fs::read_dir(root) else { continue };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let Ok(exes) = std::fs::read_dir(&dir) else { continue };
            for exe in exes.flatten() {
                let p = exe.path();
                if p.extension().map(|e| e.eq_ignore_ascii_case("exe")).unwrap_or(false) {
                    push_candidate(&mut found, p);
                }
            }
        }
    }

    found
}

/// Current app configuration (launcher choice + self-update repo).
#[tauri::command]
pub fn get_config() -> AppConfig {
    config::load_config()
}

/// Persist the selected launcher.
#[tauri::command]
pub fn set_launcher(kind: String, exe_path: Option<String>) -> Result<(), String> {
    if !LAUNCHER_KINDS.contains(&kind.as_str()) {
        return Err(format!("Unknown launcher kind: {kind}"));
    }
    if let Some(p) = &exe_path {
        if !p.is_empty() && !PathBuf::from(p).is_file() {
            return Err("Executable not found at the given path".into());
        }
    }
    let mut cfg = config::load_config();
    cfg.launcher = Some(LauncherConfig {
        kind,
        exe_path: exe_path.filter(|p| !p.is_empty()),
    });
    config::save_config(&cfg).map_err(|e| format!("Failed to save config: {e}"))
}

fn find_mrpack(tag: &str) -> anyhow::Result<PathBuf> {
    let dir = library::version_dir(tag);
    let mut best: Option<PathBuf> = None;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().map(|x| x.eq_ignore_ascii_case("mrpack")).unwrap_or(false)
                && p.is_file()
            {
                best = Some(p);
                break;
            }
        }
    }
    best.ok_or_else(|| anyhow::anyhow!(
        "Download the modpack version {tag} first"
    ))
}

#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn spawn_detached(exe: &str, args: &[String]) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(exe)
            .args(args)
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map_err(|e| format!("Failed to launch {exe}: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new(exe)
            .args(args)
            .spawn()
            .map_err(|e| format!("Failed to launch {exe}: {e}"))?;
        Ok(())
    }
}

/// Open the mrpack with its default associated application.
fn open_with_association(path: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to open file: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {e}"))?;
        Ok(())
    }
}

/// Install a downloaded modpack into the configured launcher.
///
/// Prism-compatible launchers are invoked with `-I <mrpack>` which opens their
/// native import dialog; XMCL and custom launchers receive the file path as an
/// argument, falling back to the system file association when no exe is set.
#[tauri::command]
pub async fn install_to_launcher(
    state: State<'_, AppState>,
    tag: String,
) -> Result<InstallResult, String> {
    let _ = state; // reserved for future download guards

    let cfg = config::load_config();
    let launcher = cfg
        .launcher
        .ok_or_else(|| "No launcher configured — pick one in Settings".to_string())?;

    let mrpack = find_mrpack(&tag).map_err(|e| e.to_string())?;
    let mrpack_str = mrpack.to_string_lossy().to_string();

    let exe = launcher
        .exe_path
        .filter(|p| PathBuf::from(p).is_file());

    match launcher.kind.as_str() {
        "prism" | "elyprism" => match &exe {
            Some(exe) => {
                spawn_detached(exe, &["-I".into(), mrpack_str.clone()])?;
                Ok(InstallResult {
                    message: format!("Sent to {} for import", launcher.kind),
                })
            }
            None => {
                open_with_association(&mrpack)?;
                Ok(InstallResult {
                    message: format!(
                        "Opened {mrpack_str} via system association"
                    ),
                })
            }
        },
        _ => match &exe {
            Some(exe) => {
                spawn_detached(exe, std::slice::from_ref(&mrpack_str))?;
                Ok(InstallResult {
                    message: "Opened in the selected launcher".into(),
                })
            }
            None => {
                open_with_association(&mrpack)?;
                Ok(InstallResult {
                    message: format!("Opened {mrpack_str} via system association"),
                })
            }
        },
    }
}
