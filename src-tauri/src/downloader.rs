use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::github::{self, AssetKind};
use crate::library;
use crate::state::AppState;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    id: String,
    received: u64,
    total: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DonePayload {
    id: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExtractPayload {
    id: String,
    current: u64,
    total: u64,
}

fn download_id(kind: AssetKind, tag: &str) -> String {
    format!("{}:{tag}", kind.as_str())
}

async fn download_file(
    app: &AppHandle,
    client: &reqwest::Client,
    id: &str,
    url: &str,
    dest: PathBuf,
    expected_sha256: Option<String>,
    cancel: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    if cancel.load(Ordering::Relaxed) {
        anyhow::bail!("cancelled");
    }

    let response = client.get(url).send().await?.error_for_status()?;
    let total = response.content_length().unwrap_or(0);

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp).await?;

    let mut stream = response.bytes_stream();
    let mut hasher = Sha256::new();
    let mut received: u64 = 0;
    let mut last_emit = Instant::now() - Duration::from_secs(1);

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = tokio::fs::remove_file(&tmp).await;
            anyhow::bail!("cancelled");
        }
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        if last_emit.elapsed() >= Duration::from_millis(90) || received == total {
            last_emit = Instant::now();
            let _ = app.emit(
                "download-progress",
                ProgressPayload {
                    id: id.to_owned(),
                    received,
                    total,
                },
            );
        }
    }
    file.flush().await?;
    drop(file);

    if let Some(expected) = expected_sha256 {
        let actual = hex::encode(hasher.finalize());
        if !actual.eq_ignore_ascii_case(&expected) {
            let _ = tokio::fs::remove_file(&tmp).await;
            anyhow::bail!("checksum mismatch: downloaded file is corrupted");
        }
    }

    if dest.exists() {
        let _ = tokio::fs::remove_file(&dest).await;
    }
    tokio::fs::rename(&tmp, &dest).await?;

    let _ = app.emit(
        "download-progress",
        ProgressPayload {
            id: id.to_owned(),
            received,
            total: received,
        },
    );
    Ok(())
}

fn extract_zip_blocking(
    app: AppHandle,
    id: String,
    archive_path: PathBuf,
    target_dir: PathBuf,
    cancel: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(&target_dir)?;
    let file = std::fs::File::open(&archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let total = archive.len() as u64;
    let mut last_emit = Instant::now() - Duration::from_secs(1);

    for i in 0..archive.len() {
        if cancel.load(Ordering::Relaxed) {
            anyhow::bail!("cancelled");
        }
        let mut entry = archive.by_index(i)?;
        if let Some(rel) = entry.enclosed_name() {
            let out_path = target_dir.join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out_path)?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&out_path)?;
                std::io::copy(&mut entry, &mut out)?;
            }
        }
        let current = (i + 1) as u64;
        if last_emit.elapsed() >= Duration::from_millis(120) || current == total {
            last_emit = Instant::now();
            let _ = app.emit(
                "extract-progress",
                ExtractPayload {
                    id: id.clone(),
                    current,
                    total,
                },
            );
        }
    }
    Ok(())
}

async fn finish_server_pack(
    app: AppHandle,
    id: String,
    tag: String,
    zip_path: PathBuf,
    archive_name: String,
    archive_size: u64,
    cancel: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let target = library::server_dir(&tag);
    let zip_in_task = zip_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        extract_zip_blocking(app, id, zip_in_task, target, cancel)
    })
    .await
    .map_err(|e| anyhow::anyhow!(e.to_string()))??;

    let _ = std::fs::remove_file(&zip_path);
    library::write_meta(
        &library::server_dir(&tag),
        &tag,
        "server",
        &archive_name,
        archive_size,
    );
    Ok(())
}

/// Start downloading an asset of `kind` ("client" | "server" | "zip") for `tag`.
/// Returns the download id used in progress events.
#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    tag: String,
) -> Result<String, String> {
    let asset_kind =
        AssetKind::parse(&kind).ok_or_else(|| format!("unknown asset kind: {kind}"))?;
    let release = github::get_cached_or_fetch(&state, &state.releases, &tag)
        .await
        .map_err(|e| e.to_string())?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.kind == asset_kind)
        .ok_or_else(|| format!("Release {tag} has no {kind} asset"))?
        .clone();

    let id = download_id(asset_kind, &tag);
    {
        let cancels = state.cancels.lock().unwrap();
        if let Some(flag) = cancels.get(&id) {
            if !flag.load(Ordering::Relaxed) {
                return Err("This file is already downloading".into());
            }
        }
    }
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .cancels
        .lock()
        .unwrap()
        .insert(id.clone(), cancel.clone());

    let task_app = app.clone();
    let task_id = id.clone();
    let cleanup_id = id.clone();
    let task_tag = tag.clone();    let task_cancel = cancel.clone();
    let task_url = asset.download_url.clone();
    let task_sha = asset.sha256.clone();
    let task_name = asset.name.clone();
    let task_size = asset.size;

    tauri::async_runtime::spawn(async move {
        let state = task_app.state::<AppState>();
        let result = download_file(
            &task_app,
            &state.client,
            &task_id,
            &task_url,
            match asset_kind {
                AssetKind::ClientPack | AssetKind::FullZip => {
                    library::version_dir(&task_tag).join(&task_name)
                }
                AssetKind::ServerPack => {
                    crate::config::servers_dir().join(format!("{task_tag}.zip"))
                }
            },
            task_sha,
            task_cancel.clone(),
        )
        .await;

        let outcome: anyhow::Result<()> = match result {
            Ok(()) => {
                if asset_kind == AssetKind::ServerPack {
                    finish_server_pack(
                        task_app.clone(),
                        task_id.clone(),
                        task_tag.clone(),
                        crate::config::servers_dir().join(format!("{task_tag}.zip")),
                        task_name.clone(),
                        task_size,
                        task_cancel,
                    )
                    .await
                } else {
                    library::write_meta(
                        &library::version_dir(&task_tag),
                        &task_tag,
                        asset_kind.as_str(),
                        &task_name,
                        task_size,
                    );
                    Ok(())
                }
            }
            Err(e) => Err(e),
        };

        match outcome {
            Ok(()) => {
                let _ = task_app.emit(
                    "download-done",
                    DonePayload {
                        id: task_id,
                        ok: true,
                        message: None,
                    },
                );
            }
            Err(e) => {
                let cancelled = e.to_string() == "cancelled";
                let _ = task_app.emit(
                    "download-done",
                    DonePayload {
                        id: task_id,
                        ok: false,
                        message: Some(if cancelled {
                            "cancelled".into()
                        } else {
                            e.to_string()
                        }),
                    },
                );
            }
        }

        task_app
            .state::<AppState>()
            .cancels
            .lock()
            .unwrap()
            .remove(&cleanup_id);
    });

    Ok(id)
}

/// Cancel an active download by id.
#[tauri::command]
pub async fn cancel_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let cancels = state.cancels.lock().unwrap();
    match cancels.get(&id) {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            Ok(())
        }
        None => Err("No active download with this id".into()),
    }
}
