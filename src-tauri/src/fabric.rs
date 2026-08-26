use std::path::PathBuf;

use tauri::State;

use crate::library;
use crate::state::AppState;

/// Minecraft version the Fabric helper installs by default.
pub const DEFAULT_MC_VERSION: &str = "1.20.1";

async fn latest_stable(
    client: &reqwest::Client,
    url: &str,
    wrap_key: Option<&str>,
) -> anyhow::Result<String> {
    let entries = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let arr = entries
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("unexpected fabric meta response"))?;

    let pick = |v: &serde_json::Value| -> Option<String> {
        let node = match wrap_key {
            Some(k) => v.get(k)?,
            None => v,
        };
        node.get("version")
            .and_then(|x| x.as_str())
            .map(str::to_owned)
    };
    let stable = |v: &serde_json::Value| -> bool {
        let node = match wrap_key {
            Some(k) => v.get(k),
            None => Some(v),
        };
        node.and_then(|n| n.get("stable"))
            .and_then(|s| s.as_bool())
            .unwrap_or(false)
    };

    arr.iter()
        .find(|v| stable(v))
        .or_else(|| arr.first())
        .and_then(pick)
        .ok_or_else(|| anyhow::anyhow!("fabric meta returned no usable versions"))
}

/// Download the standalone Fabric server launcher for `mc_version` into the
/// server pack folder as `server.jar`. A missing eula.txt is created with
/// `eula=true` so the first boot is unattended.
#[tauri::command]
pub async fn install_fabric_server(
    state: State<'_, AppState>,
    tag: String,
    mc_version: Option<String>,
) -> Result<String, String> {
    if state.servers.lock().unwrap().contains_key(&tag) {
        return Err("Stop the server before installing Fabric".into());
    }
    let dir = library::server_dir(&tag);
    if !dir.is_dir() {
        return Err(format!("Server pack {tag} is not installed"));
    }
    let mc = mc_version.unwrap_or_else(|| DEFAULT_MC_VERSION.to_string());

    let loader = latest_stable(
        &state.client,
        &format!("https://meta.fabricmc.net/v2/versions/loader/{mc}"),
        Some("loader"),
    )
    .await
    .map_err(|e| format!("Failed to resolve Fabric loader: {e}"))?;
    let installer = latest_stable(
        &state.client,
        "https://meta.fabricmc.net/v2/versions/installer",
        None,
    )
    .await
    .map_err(|e| format!("Failed to resolve Fabric installer: {e}"))?;

    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{mc}/{loader}/{installer}/server/jar"
    );
    let response = state
        .client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Failed to download Fabric server launcher: {e}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to download Fabric server launcher: {e}"))?;

    let dest: PathBuf = dir.join("server.jar");
    let tmp = dir.join("server.jar.part");
    tokio::fs::write(&tmp, &bytes)
        .await
        .map_err(|e| format!("Failed to write server.jar: {e}"))?;
    if dest.exists() {
        let _ = tokio::fs::remove_file(&dest).await;
    }
    tokio::fs::rename(&tmp, &dest)
        .await
        .map_err(|e| format!("Failed to finalize server.jar: {e}"))?;

    // First-boot convenience: accept EULA when the pack didn't ship a choice.
    if !dir.join("eula.txt").exists() {
        let _ = std::fs::write(dir.join("eula.txt"), "# Accepted via ONEPIXEL Manager\neula=true\n");
    }

    Ok(format!(
        "Fabric {loader} (installer {installer}) for Minecraft {mc} installed as server.jar"
    ))
}
