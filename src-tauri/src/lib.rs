mod app_info;
mod command_provider;
mod config;
mod diagnostics;
pub mod logger;
mod parser;
mod presets;
#[cfg(test)]
mod productization;
mod providers;
mod quota;
mod redact;
mod tray;

pub use config::{get_config, migrate_config_file, save_config, AppConfig};
pub use diagnostics::export_diagnostics;
pub use app_info::get_app_version;
pub use presets::{get_provider_presets, test_provider};
pub use quota::{
    get_cached_snapshot, refresh_snapshot, AppSnapshot, ProviderSnapshot, QuotaWindow,
};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .setup(|app| {
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_provider_presets,
            get_app_version,
            export_diagnostics,
            save_config,
            refresh_snapshot,
            get_cached_snapshot,
            test_provider
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running QuotaBarWin");
}
