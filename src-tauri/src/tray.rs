use tauri::{
    menu::{Menu, MenuId, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

pub const SHOW_ID: &str = "show";
pub const REFRESH_ID: &str = "refresh";
pub const QUIT_ID: &str = "quit";

#[cfg(test)]
pub fn tray_menu_ids() -> [&'static str; 3] {
    [SHOW_ID, REFRESH_ID, QUIT_ID]
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, SHOW_ID, "Show", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, REFRESH_ID, "Refresh", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &refresh, &quit])?;

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| handle_menu_event(app, event.id()))
        .build(app)?;

    Ok(())
}

fn handle_menu_event(app: &AppHandle, id: &MenuId) {
    match id.as_ref() {
        SHOW_ID => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        REFRESH_ID => {
            let _ = app.emit("refresh-requested", ());
        }
        QUIT_ID => app.exit(0),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_menu_contains_v0_actions() {
        assert_eq!(tray_menu_ids(), [SHOW_ID, REFRESH_ID, QUIT_ID]);
    }
}
