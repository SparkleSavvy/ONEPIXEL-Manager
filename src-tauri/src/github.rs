use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

pub const REPO_OWNER: &str = "SparkleSavvy";
pub const REPO_NAME: &str = "ONEPIXEL";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// Modrinth modpack (.mrpack) for launchers.
    ClientPack,
    /// Ready-to-run dedicated server archive (*server_pack*.zip).
    ServerPack,
    /// Full client archive (.zip) for old launchers.
    FullZip,
}

impl AssetKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "client" => Some(AssetKind::ClientPack),
            "server" => Some(AssetKind::ServerPack),
            "zip" => Some(AssetKind::FullZip),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AssetKind::ClientPack => "client",
            AssetKind::ServerPack => "server",
            AssetKind::FullZip => "zip",
        }
    }

    /// Classify a plain file name (used when scanning installed files).
    pub fn from_name(name: &str) -> Option<Self> {
        classify_asset(name)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetInfo {
    pub name: String,
    pub size: u64,
    pub download_url: String,
    pub kind: AssetKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub tag: String,
    pub name: String,
    pub body: Option<String>,
    pub published_at: Option<String>,
    pub prerelease: bool,
    pub assets: Vec<AssetInfo>,
}

// Raw GitHub API payloads -------------------------------------------------

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    published_at: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

fn classify_asset(name: &str) -> Option<AssetKind> {
    let lower = name.to_lowercase();
    if lower.ends_with(".mrpack") {
        Some(AssetKind::ClientPack)
    } else if lower.contains("server_pack") && lower.ends_with(".zip") {
        Some(AssetKind::ServerPack)
    } else if lower.ends_with(".zip") {
        Some(AssetKind::FullZip)
    } else {
        None
    }
}

fn convert_release(raw: GhRelease) -> ReleaseInfo {
    let assets = raw
        .assets
        .into_iter()
        .filter_map(|a| {
            let kind = classify_asset(&a.name)?;
            let sha256 = a
                .digest
                .as_deref()
                .and_then(|d| d.strip_prefix("sha256:"))
                .map(str::to_owned);
            Some(AssetInfo {
                name: a.name,
                size: a.size,
                download_url: a.browser_download_url,
                kind,
                sha256,
            })
        })
        .collect();
    let tag = raw.tag_name;
    let name = raw.name.unwrap_or_else(|| tag.clone());
    ReleaseInfo {
        tag,
        name,
        body: raw.body,
        published_at: raw.published_at,
        prerelease: raw.prerelease,
        assets,
    }
}

async fn fetch_releases_raw(client: &reqwest::Client) -> anyhow::Result<Vec<GhRelease>> {
    let url = format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases?per_page=100");
    let releases = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<GhRelease>>()
        .await?;
    Ok(releases)
}

/// List modpack releases (tool-only releases like the downloader are skipped).
#[tauri::command]
pub async fn fetch_releases(
    state: State<'_, AppState>,
) -> Result<Vec<ReleaseInfo>, String> {
    let raw = fetch_releases_raw(&state.client)
        .await
        .map_err(|e| format!("Failed to fetch releases: {e}"))?;

    let mut releases: Vec<ReleaseInfo> = raw
        .into_iter()
        .filter(|r| !r.draft)
        .map(convert_release)
        .filter(|r| !r.assets.is_empty())
        .collect();

    // Newest first by publish date; GitHub usually returns this order already.
    releases.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    *state.releases.lock().unwrap() = releases
        .iter()
        .cloned()
        .map(|r| (r.tag.clone(), r))
        .collect();

    Ok(releases)
}

/// Look up one release, refreshing the cache if the tag is unknown.
pub async fn get_cached_or_fetch(
    state: &AppState,
    releases_lock: &Mutex<HashMap<String, ReleaseInfo>>,
    tag: &str,
) -> anyhow::Result<ReleaseInfo> {
    if let Some(r) = releases_lock.lock().unwrap().get(tag) {
        return Ok(r.clone());
    }
    let raw = fetch_releases_raw(&state.client).await?;
    let mut map = releases_lock.lock().unwrap();
    for r in raw.into_iter().filter(|r| !r.draft).map(convert_release) {
        map.insert(r.tag.clone(), r);
    }
    map.get(tag)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("release {tag} not found"))
}
