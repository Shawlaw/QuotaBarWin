mod command_provider;
mod config;
mod providers;
mod quota;
mod redact;
mod tray;

pub use config::{get_config, save_config, AppConfig};
pub use quota::{
    get_cached_snapshot, refresh_snapshot, AppSnapshot, ProviderSnapshot, QuotaWindow,
};

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            refresh_snapshot,
            get_cached_snapshot
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
