mod config;
mod downloader;
mod github;
mod launcher;
mod library;
mod server;
mod state;
mod updater;

use state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            github::fetch_releases,
            downloader::start_download,
            downloader::cancel_download,
            library::list_library,
            library::delete_version,
            library::delete_server,
            library::reveal_path,
            library::set_online_mode,
            launcher::get_config,
            launcher::set_launcher,
            launcher::detect_launchers,
            launcher::install_to_launcher,
            server::start_server,
            server::stop_server,
            server::send_server_command,
            updater::check_updates,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ONEPIXEL Manager");
}
