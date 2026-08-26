//! Java runtime discovery and (on Windows) unattended installation.
//!
//! Resolution order for managed server starts:
//! 1. explicit `javaPath` from config.json
//! 2. existing installations (JAVA_HOME, .jdks, .jabba, Program Files, PATH)
//! 3. automatic download of a Temurin JDK 17 into the app data directory

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::config;
use crate::state::{AppState, LogLine};

pub const MIN_JAVA_MAJOR: u32 = 17;

#[cfg(windows)]
const JAVA_EXE_NAME: &str = "java.exe";
#[cfg(not(windows))]
const JAVA_EXE_NAME: &str = "java";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedJava {
    pub path: String,
    pub major: u32,
}

fn parse_major_from_version_output(text: &str) -> Option<u32> {
    // e.g. openjdk version "17.0.10" / version "1.8.0_392"
    let start = text.find('"')? + 1;
    let end = text[start..].find('"')? + start;
    let mut parts = text[start..end].split('.');
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

fn probe_java(exe: &Path) -> Option<u32> {
    let out = std::process::Command::new(exe).arg("-version").output().ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    parse_major_from_version_output(&text)
}

fn collect_candidate_javas(root: &Path, depth_left: usize, out: &mut Vec<PathBuf>) {
    if depth_left == 0 {
        return;
    }
    let candidate = root.join("bin").join(JAVA_EXE_NAME);
    if candidate.is_file() {
        out.push(candidate);
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_candidate_javas(&p, depth_left - 1, out);
        }
    }
}

#[cfg(windows)]
fn scan_roots(out: &mut Vec<PathBuf>) {
    let env_dir = |var: &str| -> Option<PathBuf> { std::env::var_os(var).map(PathBuf::from) };

    // JAVA_HOME points at the JDK home itself.
    if let Some(home) = env_dir("JAVA_HOME") {
        let exe = home.join("bin").join(JAVA_EXE_NAME);
        if exe.is_file() {
            out.push(exe);
        }
    }

    let user_profile = env_dir("USERPROFILE").unwrap_or_default();
    let local_appdata = env_dir("LOCALAPPDATA").unwrap_or_default();
    let program_files = env_dir("ProgramFiles").unwrap_or_default();

    let mut roots: Vec<(PathBuf, usize)> = vec![
        (user_profile.join(".jdks"), 2),
        (user_profile.join(".jabba").join("jdk"), 4),
        (program_files.join("Java"), 1),
        (program_files.join("Eclipse Adoptium"), 1),
        (program_files.join("Amazon Corretto"), 1),
        (program_files.join("Microsoft"), 1),
        (program_files.join("Zulu"), 2),
        (local_appdata.join("Programs").join("Eclipse Adoptium"), 1),
    ];
    roots.retain(|(p, _)| p.is_dir());
    for (root, depth) in roots {
        collect_candidate_javas(&root, depth, out);
    }
}

#[cfg(not(windows))]
fn scan_roots(out: &mut Vec<PathBuf>) {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        for (root, depth) in [
            (home.join(".jdks"), 2usize),
            (home.join(".sdkman/candidates/java"), 2),
        ] {
            if root.is_dir() {
                collect_candidate_javas(&root, depth, out);
            }
        }
    }
    let exe = PathBuf::from("/usr/bin").join(JAVA_EXE_NAME);
    if exe.is_file() {
        out.push(exe);
    }
}

/// Best-effort absolute path of `java` currently on PATH (`where`/`which`).
fn path_lookup() -> Option<PathBuf> {
    #[cfg(windows)]
    let tool = "where";
    #[cfg(not(windows))]
    let tool = "which";
    let out = std::process::Command::new(tool).arg(JAVA_EXE_NAME).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn probe_all(candidates: Vec<PathBuf>) -> Option<DetectedJava> {
    let mut seen = std::collections::HashSet::new();
    let mut best: Option<DetectedJava> = None;

    for candidate in candidates {
        let key = candidate.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        let Some(major) = probe_java(&candidate) else {
            continue;
        };
        let detected = DetectedJava {
            path: candidate.to_string_lossy().to_string(),
            major,
        };
        best = match best {
            Some(current) if current.major >= major => Some(current),
            _ => Some(detected),
        };
        // Prefer any qualifying runtime; remember the newest otherwise.
        if best.as_ref().is_some_and(|j| j.major >= MIN_JAVA_MAJOR) {
            break;
        }
    }
    best
}

/// Detect an existing Java >= MIN_JAVA_MAJOR without installing anything.
pub fn detect_only() -> Option<DetectedJava> {
    let cfg = config::load_config();
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(p) = cfg.java_path.filter(|s| !s.is_empty()) {
        candidates.push(PathBuf::from(&p));
    }
    let mut scanned = Vec::new();
    scan_roots(&mut scanned);
    candidates.extend(scanned);
    if let Some(p) = path_lookup() {
        candidates.push(p);
    }

    // Configured path first — respect the explicit user choice.
    if let Some(first) = candidates.first().cloned() {
        if let Some(major) = probe_java(&first) {
            return Some(DetectedJava {
                path: first.to_string_lossy().to_string(),
                major,
            });
        }
    }

    probe_all(candidates.into_iter().skip(1).collect())
}

#[cfg(windows)]
async fn install_temurin(
    app: &AppHandle,
    client: &reqwest::Client,
    tag: &str,
) -> Result<DetectedJava, String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let emit_line = |line: String| {
        let _ = app.emit(
            "server-log",
            LogLine {
                tag: tag.to_string(),
                line,
            },
        );
    };

    let url = "https://api.adoptium.net/v3/binary/latest/17/ga/windows/x64/jdk/hotspot/normal/eclipse";
    emit_line("[manager] no suitable Java found — downloading Temurin JDK 17…".into());

    let response = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Failed to download Temurin JDK: {e}"))?;
    let total = response.content_length().unwrap_or(0);

    let java_root = config::app_dir().join("java");
    tokio::fs::create_dir_all(&java_root)
        .await
        .map_err(|e| format!("Failed to create java directory: {e}"))?;
    let zip_path = java_root.parent().unwrap().join("temurin17.zip.part");

    let mut file = tokio::fs::File::create(&zip_path)
        .await
        .map_err(|e| format!("Failed to write archive: {e}"))?;
    let mut stream = response.bytes_stream();
    let mut received: u64 = 0;
    let mut last_report = Instant::now() - Duration::from_secs(1);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download failed: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Download failed: {e}"))?;
        received += chunk.len() as u64;
        if last_report.elapsed() >= Duration::from_millis(700) {
            last_report = Instant::now();
            if let Some(pct) = (received * 100).checked_div(total) {
                emit_line(format!("[manager] downloading Java… {pct}%"));
            }
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);

    emit_line("[manager] extracting Java…".into());
    let zip_final = zip_path.with_extension("zip");
    tokio::fs::rename(&zip_path, &zip_final)
        .await
        .map_err(|e| format!("Failed to finalize archive: {e}"))?;

    let extract_app = app.clone();
    let extract_zip = zip_final.clone();
    let extract_target = java_root.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::downloader::extract_zip_blocking(
            extract_app,
            "java-extract".into(),
            extract_zip,
            extract_target,
            Arc::new(AtomicBool::new(false)),
        )
    })
    .await
    .map_err(|e| anyhow::anyhow!(e.to_string()))
    .and_then(|r| r)
    .map_err(|e| format!("Failed to extract JDK: {e}"))?;
    let _ = tokio::fs::remove_file(&zip_final).await;

    // Temurin zips contain a single top-level jdk-17.x.y+z folder.
    let entries = std::fs::read_dir(&java_root).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let exe = entry.path().join("bin").join(JAVA_EXE_NAME);
        if exe.is_file() {
            if let Some(major) = probe_java(&exe) {
                let path = exe.to_string_lossy().to_string();
                let mut cfg = config::load_config();
                cfg.java_path = Some(path.clone());
                let _ = config::save_config(&cfg);
                emit_line(format!("[manager] Java {major} installed to {path}"));
                return Ok(DetectedJava { path, major });
            }
        }
    }
    Err("Temurin JDK was extracted but no java.exe was found inside".into())
}

/// Find or provision a usable Java runtime for a managed server start.
pub async fn ensure_java(app: &AppHandle, state: &AppState, tag: &str) -> Result<DetectedJava, String> {
    let emit_line = |line: String| {
        let _ = app.emit(
            "server-log",
            LogLine {
                tag: tag.to_string(),
                line,
            },
        );
    };

    if let Some(java) = detect_only() {
        emit_line(format!(
            "[manager] using Java {} at {}",
            java.major, java.path
        ));
        return Ok(java);
    }

    #[cfg(windows)]
    {
        install_temurin(app, &state.client, tag).await
    }
    #[cfg(not(windows))]
    {
        let _ = state;
        Err(format!(
            "No Java {}+ found. Install a JDK and set its path in Settings.",
            MIN_JAVA_MAJOR
        ))
    }
}

/// Current effective Java detection for the Settings page.
#[tauri::command]
pub async fn detect_java() -> Result<Option<DetectedJava>, String> {
    Ok(detect_only())
}
