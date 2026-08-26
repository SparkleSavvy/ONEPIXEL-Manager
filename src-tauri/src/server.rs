use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::library;
use crate::state::{AppState, RunningServer, SharedStdin};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogPayload {
    tag: String,
    line: String,
}

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
                                    LogPayload {
                                        tag: tag.clone(),
                                        line: "[manager] accepted Java installation prompt"
                                            .into(),
                                    },
                                );
                            }
                        }
                    }
                    let _ = app.emit("server-log", LogPayload { tag: tag.clone(), line: l });
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
    let script = find_script(&dir).ok_or_else(|| {
        "No start script (run.bat / start.bat) found in this server pack".to_string()
    })?;
    let workdir = script
        .parent()
        .map(<Path as ToOwned>::to_owned)
        .unwrap_or_else(|| dir.clone());

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
