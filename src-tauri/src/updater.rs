use serde::Serialize;
use tauri::State;

use crate::config;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// Whether the self-update repository is configured yet.
    pub configured: bool,
    pub current_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    pub update_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

fn parse_numeric(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .map(|part| {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse().unwrap_or(0)
        })
        .collect()
}

fn is_newer(candidate: &str, current: &str) -> bool {
    let a = parse_numeric(candidate);
    let b = parse_numeric(current);
    for i in 0..a.len().max(b.len()) {
        let av = a.get(i).copied().unwrap_or(0);
        let bv = b.get(i).copied().unwrap_or(0);
        if av != bv {
            return av > bv;
        }
    }
    false
}

/// Default repository used for self-update checks.
pub const DEFAULT_MANAGER_REPO: &str = "SparkleSavvy/ONEPIXEL-Manager";

/// Check the manager's own GitHub repo for a newer release.
///
/// Set `managerRepo` (`owner/name`) in config.json to override the default
/// repository, or set it to an invalid value to disable checks.
#[tauri::command]
pub async fn check_updates(state: State<'_, AppState>) -> Result<UpdateStatus, String> {
    let cfg = config::load_config();
    let current = env!("CARGO_PKG_VERSION").to_string();

    let Some(repo) = cfg
        .manager_repo
        .filter(|r| r.contains('/'))
        .or_else(|| Some(DEFAULT_MANAGER_REPO.to_string()))
    else {
        return Ok(UpdateStatus {
            configured: false,
            current_version: current,
            latest_version: None,
            update_available: false,
            url: None,
        });
    };

    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    #[derive(serde::Deserialize)]
    struct GhLatest {
        tag_name: String,
        html_url: String,
    }

    let latest: GhLatest = state
        .client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    Ok(UpdateStatus {
        configured: true,
        latest_version: Some(latest.tag_name.clone()),
        update_available: is_newer(&latest.tag_name, &current),
        url: Some(latest.html_url),
        current_version: current,
    })
}
