#![allow(dependency_on_unit_never_type_fallback)]

mod app_identity;
mod app_info;
mod config;
mod diagnostics;
pub mod logger;
#[cfg(test)]
mod productization;
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
pub use tray::{
    get_tray_popup_presentation_id, hide_tray_popup, reset_tray_popup_size,
    start_tray_popup_dragging, start_tray_popup_resizing,
};

fn window_title(version: &str) -> String {
    format!("QuotaBarWin V{version}")
}

fn startup_log_message(version: &str, hidden: bool) -> String {
    format!("app started version={version} hidden={hidden}")
}

fn should_start_hidden() -> bool {
    std::env::args().any(|arg| matches!(arg.as_str(), HIDDEN_STARTUP_ARG | "--start-hidden"))
}

pub fn run() {
    if let Err(error) = app_identity::configure_process_identity() {
        eprintln!("{error}");
    }

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
            let app_version = app_info::app_display_version();
            let start_hidden = should_start_hidden();
            let title = window_title(&app_version);
            if let Some(window) = app.get_webview_window("main") {
                window.set_title(&title)?;
                if start_hidden {
                    let _ = window.hide();
                }
            }
            tray::create_tray_popup_window(app.handle())?;
            tray::create_tray(app.handle())?;
            let app_handle = app.handle().clone();
            match config::config_path_for_app(&app_handle).and_then(|path| {
                let loaded = config::load_or_create_config(&path)?;
                let log = logger::LogSink::from_config_path(&path, &loaded.config);
                let _ = log.write(
                    logger::LogLevel::Info,
                    "app",
                    &startup_log_message(&app_version, start_hidden),
                );
                Ok(loaded.config.launch_at_startup)
            }) {
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
            get_tray_popup_presentation_id,
            hide_tray_popup,
            reset_tray_popup_size,
            start_tray_popup_dragging,
            start_tray_popup_resizing
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
                tray::hide_tray_popup_after_focus_lost(window.clone());
            }
            tauri::WindowEvent::Moved(position) if window.label() == tray::TRAY_POPUP_LABEL => {
                tray::save_tray_popup_position_after_user_move(window.app_handle(), *position);
            }
            tauri::WindowEvent::Resized(size) if window.label() == tray::TRAY_POPUP_LABEL => {
                let scale_factor = window.scale_factor().unwrap_or(1.0);
                tray::save_tray_popup_size_after_resize(window.app_handle(), *size, scale_factor);
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
        assert_eq!(
            window_title("1.2.3(abc1234)"),
            "QuotaBarWin V1.2.3(abc1234)"
        );
    }

    #[test]
    fn startup_log_message_includes_display_version_and_hidden_state() {
        assert_eq!(
            startup_log_message("1.2.3(abc1234)", true),
            "app started version=1.2.3(abc1234) hidden=true"
        );
    }

    #[test]
    fn hidden_startup_arg_is_stable() {
        assert_eq!(HIDDEN_STARTUP_ARG, "--hidden");
    }
}
