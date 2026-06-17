#![allow(dependency_on_unit_never_type_fallback)]

mod app_info;
mod config;
mod diagnostics;
pub mod logger;
mod presets;
#[cfg(test)]
mod productization;
mod providers;
mod proxy;
mod quota;
mod redact;
mod refresh_scheduler;
mod remote_provider;
mod remote_provider_commands;
mod remote_provider_runner;
mod tray;

use tauri::{Emitter, Manager};

const HIDDEN_STARTUP_ARG: &str = "--hidden";

pub use app_info::get_app_version;
pub use config::{
    get_config, get_config_storage_info, migrate_config_file, open_config_folder,
    open_remote_provider_guide, reset_config, save_config, set_portable_mode, AppConfig,
};
pub use diagnostics::export_diagnostics;
pub use presets::get_provider_presets;
pub use proxy::{ProxyConfig, ProxyKind};
pub use quota::{
    get_cached_snapshot, refresh_provider, refresh_snapshot, AppSnapshot, ProviderSnapshot,
    QuotaWindow,
};
pub use remote_provider_commands::{
    apply_remote_update, check_remote_updates, get_network_proxy, install_remote_provider_registry,
    refresh_remote_provider, remove_remote_provider, set_network_proxy, RegistryInstallFailure,
    RegistryInstallResult,
};
pub use tray::{hide_tray_popup, start_tray_popup_dragging};

fn window_title(version: &str) -> String {
    format!("QuotaBarWin V{version}")
}

fn should_start_hidden() -> bool {
    std::env::args().any(|arg| matches!(arg.as_str(), HIDDEN_STARTUP_ARG | "--start-hidden"))
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
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("QuotaBarWin")
                .args([HIDDEN_STARTUP_ARG])
                .build(),
        )
        .setup(|app| {
            let title = window_title(&app.package_info().version.to_string());
            if let Some(window) = app.get_webview_window("main") {
                window.set_title(&title)?;
                if should_start_hidden() {
                    let _ = window.hide();
                }
            }
            tray::create_tray_popup_window(app.handle())?;
            tray::create_tray(app.handle())?;
            let app_handle = app.handle().clone();
            match config::config_path_for_app(&app_handle)
                .and_then(|path| config::load_or_create_config(&path))
                .map(|loaded| loaded.config.launch_at_startup)
            {
                Ok(enabled) => {
                    if let Err(error) = config::sync_launch_at_startup_for_app(&app_handle, enabled)
                    {
                        eprintln!("Failed to sync launch-at-startup setting: {error}");
                    }
                }
                Err(error) => eprintln!("Failed to load launch-at-startup setting: {error}"),
            }
            refresh_scheduler::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_config_storage_info,
            get_provider_presets,
            get_app_version,
            export_diagnostics,
            open_config_folder,
            open_remote_provider_guide,
            reset_config,
            save_config,
            set_portable_mode,
            refresh_snapshot,
            refresh_provider,
            get_cached_snapshot,
            get_network_proxy,
            set_network_proxy,
            install_remote_provider_registry,
            remove_remote_provider,
            refresh_remote_provider,
            check_remote_updates,
            apply_remote_update,
            hide_tray_popup,
            start_tray_popup_dragging
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            tauri::WindowEvent::Focused(false)
                if window.label() == tray::TRAY_POPUP_LABEL
                    && tray::should_hide_tray_popup_on_focus_lost() =>
            {
                let _ = window.hide();
            }
            tauri::WindowEvent::Moved(position) if window.label() == tray::TRAY_POPUP_LABEL => {
                tray::save_tray_popup_position_after_user_move(window.app_handle(), *position);
            }
            _ => {}
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

    #[test]
    fn hidden_startup_arg_is_stable() {
        assert_eq!(HIDDEN_STARTUP_ARG, "--hidden");
    }
}
