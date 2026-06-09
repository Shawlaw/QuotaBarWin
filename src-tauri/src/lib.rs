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

use tauri::{Emitter, Manager};

pub use app_info::get_app_version;
pub use config::{
    get_config, get_config_storage_info, migrate_config_file, open_config_folder, reset_config,
    save_config, set_portable_mode, AppConfig,
};
pub use diagnostics::export_diagnostics;
pub use presets::{get_provider_presets, test_provider};
pub use quota::{
    get_cached_snapshot, refresh_provider, refresh_snapshot, AppSnapshot, ProviderSnapshot,
    QuotaWindow,
};

fn window_title(version: &str) -> String {
    format!("QuotaBarWin V{version}")
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
                let _ = window.emit("single-instance", "QuotaBarWin 已在运行，无需重新启动。");
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .setup(|app| {
            let title = window_title(&app.package_info().version.to_string());
            if let Some(window) = app.get_webview_window("main") {
                window.set_title(&title)?;
            }
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_config_storage_info,
            get_provider_presets,
            get_app_version,
            export_diagnostics,
            open_config_folder,
            reset_config,
            save_config,
            set_portable_mode,
            refresh_snapshot,
            refresh_provider,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_title_contains_version() {
        assert_eq!(window_title("1.2.3"), "QuotaBarWin V1.2.3");
    }
}
