use std::collections::HashMap;
use std::process::ChildStdin;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::github::ReleaseInfo;

pub type SharedStdin = Arc<Mutex<Option<ChildStdin>>>;

pub struct RunningServer {
    pub pid: u32,
    /// Piped stdin of the server process tree; used for commands and prompts.
    pub stdin: SharedStdin,
}

pub struct AppState {
    pub client: reqwest::Client,
    /// Cached release catalog, keyed by tag.
    pub releases: Mutex<HashMap<String, ReleaseInfo>>,
    /// Active download cancel flags, keyed by download id ("client:tag" | "server:tag" | "zip:tag").
    pub cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Running server processes, keyed by version tag.
    pub servers: Mutex<HashMap<String, RunningServer>>,
}

impl AppState {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!("ONEPIXEL-Manager/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("failed to build http client");
        Self {
            client,
            releases: Mutex::new(HashMap::new()),
            cancels: Mutex::new(HashMap::new()),
            servers: Mutex::new(HashMap::new()),
        }
    }
}
