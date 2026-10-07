#![allow(dependency_on_unit_never_type_fallback)]

mod app_identity;
mod app_info;
mod app_update;
mod builtin_js;
mod cli;
mod config;
mod diagnostics;
mod external_links;
mod local_api;
mod local_api_token;
pub mod logger;
mod managed_secret_commands;
mod managed_secret_store;
mod notifications;
#[cfg(test)]
mod productization;
mod provider_error;
mod provider_setup;
mod proxy;
mod quota;
mod quota_events;
mod redact;
mod refresh_scheduler;
mod remote_provider;
mod remote_provider_commands;
mod remote_provider_runner;
mod secret_encryption;
mod tray;

use tauri::{Emitter, Manager};

const HIDDEN_STARTUP_ARG: &str = "--hidden";

pub use app_info::get_app_version;
pub use app_update::{
    apply_app_update, check_app_update, dismiss_app_update_notice, download_app_update,
    get_app_update_navigation_request, get_app_update_status, AppUpdateNavigationState,
    AppUpdateState,
};
pub use cli::run_cli;
pub use config::{
    dev_clone_release_config, dev_preview_clone_source, dev_restart_app, get_config,
    get_config_storage_info, migrate_config_file, open_config_folder,
    open_remote_provider_guide, open_webhook_template_guide, reset_config, save_config,
    set_portable_mode, AppConfig,
};
pub use diagnostics::export_diagnostics;
pub use local_api::{
    get_local_api_access_token, get_local_api_status, list_local_api_network_interfaces,
    set_local_api_access_token,
};
pub use provider_setup::{get_provider_setup, save_provider_setup, test_provider_setup};
pub use proxy::{test_network_proxy, ProxyConfig, ProxyKind};
pub use quota::{
    get_cached_snapshot, refresh_provider, refresh_snapshot, AppSnapshot, ProviderSnapshot,
    QuotaWindow,
};
pub use remote_provider_commands::{
    apply_remote_update, check_remote_updates, get_installed_remote_provider_manifest,
    get_network_proxy, install_remote_provider_manifest, install_remote_provider_registry,
    migrate_remote_providers_to_registry, preview_remote_provider_registry,
    refresh_remote_provider, remove_remote_provider, set_network_proxy, RegistryInstallFailure,
    RegistryInstallResult, RegistryMigrationResult, RemoteProviderCatalogEntry,
};
pub use tray::{
    e2e_focus_main_window, e2e_is_tray_popup_focused, e2e_is_tray_popup_visible,
    e2e_set_tray_popup_size, e2e_show_tray_popup, e2e_simulate_tray_popup_focus_lost,
    get_tray_popup_presentation_id, hide_tray_popup, reset_tray_popup_size,
    set_tray_popup_auto_height, show_application_update, show_main_window,
    start_tray_popup_dragging, start_tray_popup_resizing,
};

fn window_title(version: &str) -> String {
    format!("QuotaBarWin V{version}")
}

// Dev builds keep the DEV marker even after the runtime title overwrite so
// a dev window is always distinguishable from an installed release.
fn dev_window_title(version: &str) -> String {
    format!("QuotaBarWin DEV V{version}")
}

fn startup_log_message(version: &str, hidden: bool) -> String {
    format!("app started version={version} hidden={hidden}")
}

/// Records an application-level event (startup, applied update) and dispatches
/// it through the enabled notification channels.
fn record_app_event(app: &tauri::AppHandle, pending: quota_events::PendingQuotaEvent) {
    let Ok(path) = config::config_path_for_app(app) else {
        return;
    };
    let Ok(loaded) = config::load_or_create_config(&path) else {
        return;
    };
    let log = logger::LogSink::from_config_path(&path, &loaded.config);
    match quota_events::append_events(&path, vec![pending]) {
        Ok(recorded) => {
            for event in &recorded {
                let _ = log.write(
                    logger::LogLevel::Info,
                    "quota_events",
                    &format!(
                        "quota event recorded id={} type={} providerId={} windowId={}",
                        event.id,
                        event.event_type,
                        event.provider_id.as_deref().unwrap_or("-"),
                        event.window_id.as_deref().unwrap_or("-")
                    ),
                );
            }
            notifications::dispatch_events(
                &path,
                loaded.config.notifications.clone(),
                loaded.config.network_proxy.clone(),
                loaded.config.language.clone(),
                recorded,
            );
        }
        Err(error) => {
            let _ = log.write(
                logger::LogLevel::Warn,
                "quota_events",
                &format!("failed to persist quota event history: {error}"),
            );
        }
    }
}

fn should_start_hidden() -> bool {
    std::env::args().any(|arg| matches!(arg.as_str(), HIDDEN_STARTUP_ARG | "--start-hidden"))
}

pub fn run() {
    if let Err(error) = app_identity::configure_process_identity() {
        eprintln!("{error}");
    }

    tauri::Builder::default()
        .manage(AppUpdateState::default())
        .manage(AppUpdateNavigationState::default())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
                let _ = window.emit("single-instance", "QuotaBarWin 已在运行，无需重新启动。");
            }
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("QuotaBarWin")
                .args([HIDDEN_STARTUP_ARG])
                .build(),
        )
        .setup(|app| {
            let app_version = app_info::app_display_version();
            let start_hidden = should_start_hidden();
            let title = if app_identity::is_dev_build(app.handle()) {
                dev_window_title(&app_version)
            } else {
                window_title(&app_version)
            };
            if let Some(window) = app.get_webview_window("main") {
                window.set_title(&title)?;
                if start_hidden {
                    let _ = window.hide();
                }
            }
            match app_update::acknowledge_applied_update(app.handle()) {
                Ok(true) => record_app_event(
                    app.handle(),
                    quota_events::pending_app_update_applied(&app_info::app_display_version()),
                ),
                Ok(false) => {}
                Err(error) => {
                    eprintln!("Failed to acknowledge applied application update: {error}")
                }
            }
            let app_handle = app.handle().clone();
            match config::config_path_for_app(&app_handle).and_then(|path| {
                let mut loaded = config::load_or_create_config(&path)?;
                let log = logger::LogSink::from_config_path(&path, &loaded.config);
                if let Err(error) =
                    config::repair_remote_provider_cache_paths(&path, &mut loaded.config)
                {
                    let _ = log.write_unfiltered(
                        logger::LogLevel::Warn,
                        "app",
                        &format!("provider cache path migration failed: {error}"),
                    );
                }
                let _ = log.write_unfiltered(
                    logger::LogLevel::Info,
                    "app",
                    &startup_log_message(&app_version, start_hidden),
                );
                if let Ok(recorded) = quota_events::append_events(
                    &path,
                    vec![quota_events::pending_app_started(start_hidden)],
                ) {
                    if !recorded.is_empty() {
                        notifications::dispatch_events(
                            &path,
                            loaded.config.notifications.clone(),
                            loaded.config.network_proxy.clone(),
                            loaded.config.language.clone(),
                            recorded,
                        );
                    }
                }
                Ok((loaded.config.launch_at_startup, loaded.config.theme))
            }) {
                Ok((enabled, theme)) => {
                    config::apply_app_theme(&app_handle, &theme);
                    if let Err(error) = config::sync_launch_at_startup_for_app(&app_handle, enabled)
                    {
                        eprintln!("Failed to sync launch-at-startup setting: {error}");
                    }
                }
                Err(error) => eprintln!("Failed to load launch-at-startup setting: {error}"),
            }
            tray::create_tray_popup_window(app.handle())?;
            tray::create_tray(app.handle())?;
            local_api::start(app.handle().clone());
            refresh_scheduler::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_config_storage_info,
            dev_clone_release_config,
            dev_preview_clone_source,
            dev_restart_app,
            get_app_version,
            app_info::get_is_dev_build,
            get_app_update_navigation_request,
            get_app_update_status,
            check_app_update,
            download_app_update,
            apply_app_update,
            dismiss_app_update_notice,
            external_links::open_project_github,
            external_links::open_app_update_notes,
            external_links::open_external_link,
            export_diagnostics,
            open_config_folder,
            open_remote_provider_guide,
            open_webhook_template_guide,
            reset_config,
            save_config,
            set_portable_mode,
            refresh_snapshot,
            refresh_provider,
            get_cached_snapshot,
            quota_events::get_quota_event_history,
            quota_events::clear_quota_event_history,
            notifications::send_test_notification,
            get_local_api_status,
            list_local_api_network_interfaces,
            get_local_api_access_token,
            set_local_api_access_token,
            managed_secret_commands::get_managed_secrets_encryption_status,
            managed_secret_commands::enable_managed_secrets_encryption,
            managed_secret_commands::disable_managed_secrets_encryption,
            managed_secret_commands::dismiss_managed_secrets_encryption_prompt,
            get_network_proxy,
            set_network_proxy,
            test_network_proxy,
            get_provider_setup,
            save_provider_setup,
            test_provider_setup,
            get_installed_remote_provider_manifest,
            preview_remote_provider_registry,
            install_remote_provider_manifest,
            install_remote_provider_registry,
            migrate_remote_providers_to_registry,
            remove_remote_provider,
            refresh_remote_provider,
            check_remote_updates,
            apply_remote_update,
            get_tray_popup_presentation_id,
            hide_tray_popup,
            tray::hide_main_window,
            show_main_window,
            show_application_update,
            reset_tray_popup_size,
            set_tray_popup_auto_height,
            start_tray_popup_dragging,
            start_tray_popup_resizing,
            e2e_show_tray_popup,
            e2e_set_tray_popup_size,
            e2e_is_tray_popup_visible,
            e2e_is_tray_popup_focused,
            e2e_simulate_tray_popup_focus_lost,
            e2e_focus_main_window
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Focused(true)
                if window.label() == "main" || window.label() == tray::TRAY_POPUP_LABEL =>
            {
                app_update::request_automatic_update_check(window.app_handle().clone());
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if window.label() == "main" {
                    // The renderer owns the main-window close flow so it can
                    // offer to resolve unsaved settings changes first; it
                    // hides the window itself via the window API.
                    let app = window.app_handle();
                    let emit_result = app.emit("main-window-close-requested", ());
                    if let Ok(path) = config::config_path_for_app(app) {
                        if let Ok(loaded) = config::load_or_create_config(&path) {
                            let log = logger::LogSink::from_config_path(&path, &loaded.config);
                            let _ = log.write(
                                logger::LogLevel::Info,
                                "app",
                                &format!(
                                    "main window close requested, forwarded to renderer emitOk={}",
                                    emit_result.is_ok()
                                ),
                            );
                        }
                    }
                } else {
                    let _ = window.hide();
                }
            }
            tauri::WindowEvent::Focused(false) if window.label() == tray::TRAY_POPUP_LABEL => {
                tray::handle_tray_popup_focus_lost(window.clone());
            }
            tauri::WindowEvent::ThemeChanged(_) if window.label() == "main" => {
                tray::sync_tray_popup_background(window.app_handle());
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
    fn dev_window_title_keeps_the_dev_marker() {
        assert_eq!(dev_window_title("1.2.3"), "QuotaBarWin DEV V1.2.3");
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
