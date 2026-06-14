use tauri::{
    menu::{Menu, MenuId, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    utils::config::Color,
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder,
};

use crate::config::{self, TrayPopupPosition};

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

pub const SHOW_ID: &str = "show";
pub const REFRESH_ID: &str = "refresh";
pub const QUIT_ID: &str = "quit";
pub const TRAY_POPUP_LABEL: &str = "tray-popup";
pub const TRAY_POPUP_VIEW: &str = "index.html?view=tray";
const TRAY_POPUP_WIDTH: f64 = 380.0;
const TRAY_POPUP_HEIGHT: f64 = 520.0;
const TRAY_POPUP_OFFSET: f64 = 12.0;
const TRAY_POPUP_DRAG_FOCUS_GRACE: Duration = Duration::from_secs(2);
const TRAY_POPUP_POSITION_SAVE_GRACE: Duration = Duration::from_secs(30);
static TRAY_POPUP_FOCUS_HIDE_SUPPRESSED_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);
static TRAY_POPUP_POSITION_SAVE_ALLOWED_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

#[cfg(test)]
pub fn tray_menu_ids() -> [&'static str; 3] {
    [SHOW_ID, REFRESH_ID, QUIT_ID]
}

#[cfg(test)]
pub fn tray_popup_view() -> &'static str {
    TRAY_POPUP_VIEW
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, SHOW_ID, "Show", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, REFRESH_ID, "Refresh", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &refresh, &quit])?;
    let icon = tray_icon_image(app)?;

    TrayIconBuilder::new()
        .menu(&menu)
        .icon(icon)
        .tooltip("QuotaBarWin")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu_event(app, event.id()))
        .on_tray_icon_event(|tray, event| handle_tray_event(tray.app_handle(), event))
        .build(app)?;

    Ok(())
}

fn tray_icon_image(app: &AppHandle) -> tauri::Result<tauri::image::Image<'static>> {
    if let Some(icon) = app.default_window_icon() {
        return Ok(icon.clone().to_owned());
    }

    Ok(fallback_tray_icon())
}

fn fallback_tray_icon() -> tauri::image::Image<'static> {
    const SIZE: u32 = 16;
    const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];
    const ACCENT: [u8; 4] = [55, 125, 255, 255];
    const HIGHLIGHT: [u8; 4] = [255, 255, 255, 255];

    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as i32 - 7;
            let dy = y as i32 - 7;
            let pixel = if dx * dx + dy * dy <= 49 {
                if (4..=10).contains(&x) && (4..=6).contains(&y)
                    || (7..=9).contains(&x) && (4..=11).contains(&y)
                    || (6..=11).contains(&x) && (9..=11).contains(&y)
                {
                    HIGHLIGHT
                } else {
                    ACCENT
                }
            } else {
                TRANSPARENT
            };
            rgba.extend_from_slice(&pixel);
        }
    }

    tauri::image::Image::new_owned(rgba, SIZE, SIZE)
}

pub fn create_tray_popup_window(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window(TRAY_POPUP_LABEL).is_some() {
        return Ok(());
    }

    WebviewWindowBuilder::new(
        app,
        TRAY_POPUP_LABEL,
        WebviewUrl::App(TRAY_POPUP_VIEW.into()),
    )
    .title("QuotaBarWin")
    .inner_size(TRAY_POPUP_WIDTH, TRAY_POPUP_HEIGHT)
    .min_inner_size(TRAY_POPUP_WIDTH, TRAY_POPUP_HEIGHT)
    .max_inner_size(TRAY_POPUP_WIDTH, TRAY_POPUP_HEIGHT)
    .position(0.0, 0.0)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .shadow(true)
    .background_color(Color(255, 255, 255, 255))
    .build()?;

    Ok(())
}

#[tauri::command]
pub fn hide_tray_popup(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(TRAY_POPUP_LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn start_tray_popup_dragging(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(TRAY_POPUP_LABEL) {
        suppress_focus_hide_for_drag();
        allow_position_save_for_drag();
        window.start_dragging().map_err(|error| error.to_string())?;
        suppress_focus_hide_for_drag();
        allow_position_save_for_drag();
        if let Ok(position) = window.outer_position() {
            save_tray_popup_position_after_user_move(&app, position);
        }
    }

    Ok(())
}

pub fn should_hide_tray_popup_on_focus_lost() -> bool {
    match TRAY_POPUP_FOCUS_HIDE_SUPPRESSED_UNTIL.lock() {
        Ok(mut suppressed_until) => match *suppressed_until {
            Some(until) if Instant::now() < until => false,
            Some(_) => {
                *suppressed_until = None;
                true
            }
            None => true,
        },
        Err(_) => true,
    }
}

fn suppress_focus_hide_for_drag() {
    if let Ok(mut suppressed_until) = TRAY_POPUP_FOCUS_HIDE_SUPPRESSED_UNTIL.lock() {
        *suppressed_until = Some(Instant::now() + TRAY_POPUP_DRAG_FOCUS_GRACE);
    }
}

fn allow_position_save_for_drag() {
    if let Ok(mut allowed_until) = TRAY_POPUP_POSITION_SAVE_ALLOWED_UNTIL.lock() {
        *allowed_until = Some(Instant::now() + TRAY_POPUP_POSITION_SAVE_GRACE);
    }
}

fn should_save_tray_popup_position_on_move() -> bool {
    match TRAY_POPUP_POSITION_SAVE_ALLOWED_UNTIL.lock() {
        Ok(mut allowed_until) => match *allowed_until {
            Some(until) if Instant::now() < until => true,
            Some(_) => {
                *allowed_until = None;
                false
            }
            None => false,
        },
        Err(_) => false,
    }
}

pub fn save_tray_popup_position_after_user_move(app: &AppHandle, position: PhysicalPosition<i32>) {
    if should_save_tray_popup_position_on_move() {
        let _ = config::save_tray_popup_position_for_app(
            app,
            TrayPopupPosition {
                x: position.x,
                y: position.y,
            },
        );
    }
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

fn handle_tray_event(app: &AppHandle, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        position,
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        show_tray_popup(app, position);
    }
}

fn show_tray_popup(app: &AppHandle, anchor: PhysicalPosition<f64>) {
    if let Some(window) = app.get_webview_window(TRAY_POPUP_LABEL) {
        let position = tray_popup_position_from_saved_or_anchor(
            config::load_tray_popup_position_for_app(app),
            anchor,
        );
        let _ = window.set_position(PhysicalPosition::new(position.0, position.1));
        let _ = window.set_always_on_top(true);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit_to(TRAY_POPUP_LABEL, "tray-popup-shown", ());
    }
}

fn tray_popup_position(anchor: PhysicalPosition<f64>) -> (i32, i32) {
    let x = (anchor.x - TRAY_POPUP_WIDTH + TRAY_POPUP_OFFSET).max(0.0);
    let y = (anchor.y - TRAY_POPUP_HEIGHT - TRAY_POPUP_OFFSET).max(0.0);
    (x.round() as i32, y.round() as i32)
}

fn tray_popup_position_from_saved_or_anchor(
    saved: Option<TrayPopupPosition>,
    anchor: PhysicalPosition<f64>,
) -> (i32, i32) {
    saved
        .map(|position| (position.x, position.y))
        .unwrap_or_else(|| tray_popup_position(anchor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_menu_contains_v0_actions() {
        assert_eq!(tray_menu_ids(), [SHOW_ID, REFRESH_ID, QUIT_ID]);
    }

    #[test]
    fn tray_popup_uses_dedicated_view_route() {
        assert_eq!(tray_popup_view(), "index.html?view=tray");
    }

    #[test]
    fn tray_popup_position_anchors_above_click() {
        assert_eq!(
            tray_popup_position(PhysicalPosition::new(800.0, 900.0)),
            (432, 368)
        );
    }

    #[test]
    fn tray_popup_position_clamps_to_screen_origin() {
        assert_eq!(
            tray_popup_position(PhysicalPosition::new(120.0, 80.0)),
            (0, 0)
        );
    }

    #[test]
    fn tray_popup_position_uses_saved_position_when_present() {
        assert_eq!(
            tray_popup_position_from_saved_or_anchor(
                Some(TrayPopupPosition { x: -120, y: 240 }),
                PhysicalPosition::new(800.0, 900.0),
            ),
            (-120, 240)
        );
    }

    #[test]
    fn focus_loss_hide_is_suppressed_after_drag_starts() {
        suppress_focus_hide_for_drag();

        assert!(!should_hide_tray_popup_on_focus_lost());
    }

    #[test]
    fn position_save_requires_drag_window() {
        assert!(!should_save_tray_popup_position_on_move());
        allow_position_save_for_drag();
        assert!(should_save_tray_popup_position_on_move());
    }
}
