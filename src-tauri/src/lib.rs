mod providers;
mod quota;
mod tray;

pub use quota::{refresh_snapshot, AppSnapshot, ProviderSnapshot, QuotaWindow};

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![refresh_snapshot])
        .run(tauri::generate_context!())
        .expect("error while running QuotaBarWin");
}
