use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config;
use crate::library;
use crate::state::{AppState, LogLine, RunningServer, SharedStdin};

/// Default JVM heap (MB) for managed starts when no per-tag value is set.
pub const DEFAULT_RAM_MB: u64 = 6144;

/// Aikar's G1GC flags used by the managed start script.
const JVM_FLAGS: &str = "--add-modules=jdk.incubator.vector -XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:+AlwaysPreTouch -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1 -Dusing.aikars.flags=https://mcflags.emc.gs -Daikars.new.flags=true -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20";

/// Name of the start script generated and re-written by the manager.
const MANAGED_SCRIPT_BASE: &str = "onepixel-start";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusPayload {
    tag: String,
    running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
}

/// Locate `server.properties` inside an extracted server pack (root or one level deep).
pub fn find_properties(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("server.properties");
    if direct.is_file() {
        return Some(direct);
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let sub = entry.path();
            if sub.is_dir() {
                let candidate = sub.join("server.properties");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Write a line to the server process's stdin. Returns false when the
/// process is gone or stdin was closed.
fn write_stdin_line(stdin: &SharedStdin, text: &str) -> bool {
    use std::io::Write;
    let mut guard = stdin.lock().unwrap();
    match guard.as_mut() {
        Some(stream) => {
            let ok = stream.write_all(text.as_bytes()).is_ok()
                && stream.write_all(b"\r\n").is_ok()
                && stream.flush().is_ok();
            if !ok {
                *guard = None;
            }
            ok
        }
        None => false,
    }
}

const AGREEMENT_PROMPT: &str = "type 'i agree'";

fn spawn_reader(
    app: AppHandle,
    tag: String,
    stream: impl std::io::Read + Send + 'static,
    auto_agree: Option<SharedStdin>,
) {
    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    // ServerPackCreator scripts ask for consent before
                    // installing Java via Jabba — accept automatically.
                    if auto_agree.is_some() && l.to_lowercase().contains(AGREEMENT_PROMPT) {
                        if let Some(handle) = &auto_agree {
                            if write_stdin_line(handle, "I agree") {
                                let _ = app.emit(
                                    "server-log",
                                    LogLine {
                                        tag: tag.clone(),
                                        line: "[manager] accepted Java installation prompt"
                                            .into(),
                                    },
                                );
                            }
                        }
                    }
                    let _ = app.emit("server-log", LogLine { tag: tag.clone(), line: l });
                }
                Err(_) => break,
            }
        }
    });
}

/// Locate a start script inside an extracted server pack.
pub fn find_script(dir: &Path) -> Option<PathBuf> {
    const PRIORITY: &[&str] = &[
        "run.bat",
        "start.bat",
        "launchserver.bat",
        "run.sh",
        "start.sh",
        "launchserver.sh",
    ];

    let matches_name = |p: &Path| -> bool {
        p.is_file()
            && p.extension()
                .map(|e| {
                    let e = e.to_string_lossy().to_lowercase();
                    e == "bat" || e == "sh" || e == "cmd"
                })
                .unwrap_or(false)
            && PRIORITY
                .iter()
                .any(|n| p.file_name().map(|f| f.eq_ignore_ascii_case(n)).unwrap_or(false))
    };

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if matches_name(&p) {
                return Some(p);
            }
        }
    }
    // One level deep (some packs nest the actual server folder).
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let sub = entry.path();
            if sub.is_dir() {
                if let Ok(inner) = std::fs::read_dir(&sub) {
                    for e2 in inner.flatten() {
                        let p = e2.path();
                        if matches_name(&p) {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    None
}

fn spawn_watcher(
    app: AppHandle,
    tag: String,
    child: Arc<Mutex<Option<Child>>>,
    stdin_handle: Option<SharedStdin>,
) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(400));
        let mut guard = child.lock().unwrap();
        if guard.is_none() {
            break;
        }
        let code = match guard.as_mut().unwrap().try_wait() {
            Ok(Some(status)) => status.code(),
            Ok(None) => continue,
            Err(_) => None,
        };
        *guard = None;
        drop(guard);
        // Close our side of stdin so pending writers fail fast.
        if let Some(handle) = &stdin_handle {
            handle.lock().unwrap().take();
        }
        if let Some(state) = app.try_state::<AppState>() {
            state.servers.lock().unwrap().remove(&tag);
        }
        if code == Some(9009) {
            // Windows: "command not found" — almost always a missing Java.
            let _ = app.emit(
                "server-log",
                LogLine {
                    tag: tag.clone(),
                    line: "[manager] exited with code 9009 (command not found): Java is missing from PATH. Install Java 17+ or set its path in Settings.".into(),
                },
            );
        }
        let _ = app.emit(
            "server-status",
            StatusPayload {
                tag,
                running: false,
                exit_code: code,
            },
        );
        break;
    });
}

/// Locate a standalone `server.jar` inside an extracted server pack
/// (root or one level deep). Returns its containing directory.
pub fn find_server_jar(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("server.jar");
    if direct.is_file() {
        return Some(direct.parent().unwrap_or(dir).to_path_buf());
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let sub = entry.path();
            if sub.is_dir() {
                let candidate = sub.join("server.jar");
                if candidate.is_file() {
                    return candidate.parent().map(<Path as ToOwned>::to_owned);
                }
            }
        }
    }
    None
}

/// Write the manager-generated launch script that runs `server.jar` with
/// Aikar's flags and the configured heap size. Returns the script path.
fn write_managed_script(jar_dir: &Path, ram_mb: u64, java_exe: &str) -> Result<PathBuf, String> {
    #[cfg(windows)]
    let script = jar_dir.join(format!("{MANAGED_SCRIPT_BASE}.bat"));
    #[cfg(not(windows))]
    let script = jar_dir.join(format!("{MANAGED_SCRIPT_BASE}.sh"));

    #[cfg(windows)]
    let content = format!(
        "@echo off\r\nrem Generated by ONEPIXEL Manager\r\ncd /d \"%~dp0\"\r\n\"{java_exe}\" -Xms{ram_mb}M -Xmx{ram_mb}M {JVM_FLAGS} -jar server.jar --nogui\r\nexit /b %errorlevel%\r\n"
    );
    #[cfg(not(windows))]
    let content = format!(
        "#!/bin/sh\n# Generated by ONEPIXEL Manager\ncd \"$(dirname \"$0\")\"\nexec \"{java_exe}\" -Xms{ram_mb}M -Xmx{ram_mb}M {JVM_FLAGS} -jar server.jar --nogui\n"
    );

    std::fs::write(&script, content).map_err(|e| format!("Failed to write start script: {e}"))?;

    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
    }
    Ok(script)
}

/// Set the JVM heap size (MB) used by the managed start script. Applies on
/// the next server start.
#[tauri::command]
pub async fn set_server_ram(tag: String, mb: u64) -> Result<u64, String> {
    if !(512..=65536).contains(&mb) {
        return Err("RAM must be between 512 and 65536 MB".into());
    }
    let mut cfg = config::load_config();
    cfg.server_ram.insert(tag, mb);
    config::save_config(&cfg).map_err(|e| format!("Failed to save config: {e}"))?;
    Ok(mb)
}

/// Persist an explicit Java executable for managed starts (None clears it).
#[tauri::command]
pub async fn set_java_path(path: Option<String>) -> Result<(), String> {
    if let Some(p) = path.as_deref().filter(|s| !s.is_empty()) {
        let pb = std::path::PathBuf::from(p);
        if !pb.is_file() {
            return Err("Executable not found at the given path".into());
        }
        cfg_set_java(Some(p.to_string()))?;
        return Ok(());
    }
    cfg_set_java(None)
}

fn cfg_set_java(value: Option<String>) -> Result<(), String> {
    let mut cfg = config::load_config();
    cfg.java_path = value;
    config::save_config(&cfg).map_err(|e| format!("Failed to save config: {e}"))
}

/// Start the server pack for `tag`. Returns the process id.
#[tauri::command]
pub async fn start_server(
    app: AppHandle,
    state: State<'_, AppState>,
    tag: String,
) -> Result<u32, String> {
    {
        let servers = state.servers.lock().unwrap();
        if servers.contains_key(&tag) {
            return Err("Server is already running".into());
        }
    }

    let dir = library::server_dir(&tag);
    if !dir.is_dir() {
        return Err(format!("Server pack {tag} is not installed"));
    }

    // Prefer the manager-generated script around server.jar (Fabric etc.);
    // fall back to whatever start script the pack shipped.
    let (script, workdir) = match find_server_jar(&dir) {
        Some(jar_dir) => {
            // Resolve (or auto-provision) Java before generating the script.
            let java = crate::java::ensure_java(&app, &state, &tag).await?;
            let ram = config::load_config()
                .server_ram
                .get(&tag)
                .copied()
                .unwrap_or(DEFAULT_RAM_MB);
            (
                write_managed_script(&jar_dir, ram, &java.path)?,
                jar_dir,
            )
        }
        None => {
            let script = find_script(&dir).ok_or_else(|| {
                "No server.jar and no start script (run.bat / start.bat) found in this pack"
                    .to_string()
            })?;
            let workdir = script
                .parent()
                .map(<Path as ToOwned>::to_owned)
                .unwrap_or_else(|| dir.clone());
            (script, workdir)
        }
    };

    let mut command = build_command(&script, &workdir);
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped());

    let mut child = command.spawn().map_err(|e| format!("Failed to start server: {e}"))?;
    let pid = child.id();

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdin: SharedStdin = Arc::new(Mutex::new(child.stdin.take()));
    let child_arc = Arc::new(Mutex::new(Some(child)));

    if let Some(out) = stdout {
        spawn_reader(app.clone(), tag.clone(), out, Some(stdin.clone()));
    }
    if let Some(err) = stderr {
        spawn_reader(app.clone(), tag.clone(), err, None);
    }

    state.servers.lock().unwrap().insert(
        tag.clone(),
        RunningServer {
            pid,
            stdin: stdin.clone(),
        },
    );

    spawn_watcher(app.clone(), tag.clone(), child_arc, Some(stdin));

    let _ = app.emit(
        "server-status",
        StatusPayload {
            tag,
            running: true,
            exit_code: None,
        },
    );
    Ok(pid)
}

#[cfg(windows)]
fn build_command(script: &Path, workdir: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let name = script.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").arg(name).current_dir(workdir);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
fn build_command(script: &Path, workdir: &Path) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg(script).current_dir(workdir);
    cmd
}

/// Stop the running server for `tag` by killing its whole process tree.
#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>, tag: String) -> Result<(), String> {
    let pid = {
        let servers = state.servers.lock().unwrap();
        servers.get(&tag).map(|s| s.pid)
    };
    let Some(pid) = pid else {
        return Err(format!("Server {tag} is not running"));
    };

    kill_tree(pid)?;

    // Give the watcher a moment to reap the process; also cancel any pending flag.
    if let Some(flag) = state.cancels.lock().unwrap().get(&format!("server:{tag}")) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Send a console command to the running server's stdin
/// (e.g. `stop`, `say hello`, `/whitelist add …`).
#[tauri::command]
pub async fn send_server_command(
    state: State<'_, AppState>,
    tag: String,
    command: String,
) -> Result<(), String> {
    let stdin = {
        let servers = state.servers.lock().unwrap();
        servers.get(&tag).map(|s| s.stdin.clone())
    };
    let Some(stdin) = stdin else {
        return Err(format!("Server {tag} is not running"));
    };
    if write_stdin_line(&stdin, command.trim()) {
        Ok(())
    } else {
        Err("Server is shutting down — stdin is closed".into())
    }
}

fn kill_tree(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to stop server: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Command::new("kill")
            .arg("-9")
            .arg(pid.to_string())
            .spawn()
            .map_err(|e| format!("Failed to stop server: {e}"))?;
        Ok(())
    }
}
