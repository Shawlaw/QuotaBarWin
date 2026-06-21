use tauri::{
    menu::{Menu, MenuId, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    utils::config::Color,
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder, Window,
};
use tauri_runtime::ResizeDirection;

use crate::{
    app_info,
    config::{self, AppLanguage, TrayPopupPosition, TrayPopupSize},
};

use std::{
    env,
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetUserDefaultLocaleName(locale_name: *mut u16, locale_name_length: i32) -> i32;
    fn GetUserDefaultUILanguage() -> u16;
}

pub const TRAY_ID: &str = "main";
pub const SHOW_ID: &str = "show";
pub const VERSION_ID: &str = "version";
pub const OPEN_APP_FOLDER_ID: &str = "open-app-folder";
pub const QUIT_ID: &str = "quit";
pub const TRAY_POPUP_LABEL: &str = "tray-popup";
pub const TRAY_POPUP_VIEW: &str = "index.html?view=tray";
const GITHUB_URL: &str = "https://github.com/Shawlaw/QuotaBarWin";
const TRAY_POPUP_WIDTH: f64 = 380.0;
const TRAY_POPUP_HEIGHT: f64 = 520.0;
const TRAY_POPUP_MIN_WIDTH: f64 = 320.0;
const TRAY_POPUP_MIN_HEIGHT: f64 = 360.0;
const TRAY_POPUP_MAX_RESTORED_WIDTH: f64 = 2000.0;
const TRAY_POPUP_MAX_RESTORED_HEIGHT: f64 = 2000.0;
const TRAY_POPUP_OFFSET: f64 = 12.0;
const TRAY_POPUP_DRAG_FOCUS_GRACE: Duration = Duration::from_secs(2);
const TRAY_POPUP_FOCUS_LOST_HIDE_DELAY: Duration = Duration::from_millis(180);
const TRAY_POPUP_POSITION_SAVE_GRACE: Duration = Duration::from_secs(30);
static TRAY_POPUP_FOCUS_HIDE_SUPPRESSED_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);
static TRAY_POPUP_POSITION_SAVE_ALLOWED_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);
static TRAY_POPUP_PRESENTATION_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TrayPopupShownPayload {
    presentation_id: u64,
}

#[cfg(test)]
pub fn tray_menu_ids() -> [&'static str; 4] {
    [SHOW_ID, VERSION_ID, OPEN_APP_FOLDER_ID, QUIT_ID]
}

#[cfg(test)]
pub fn tray_popup_view() -> &'static str {
    TRAY_POPUP_VIEW
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_tray_menu(app)?;
    let icon = tray_icon_image(app)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(icon)
        .tooltip("QuotaBarWin")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu_event(app, event.id()))
        .on_tray_icon_event(|tray, event| handle_tray_event(tray.app_handle(), event))
        .build(app)?;

    Ok(())
}

pub fn refresh_tray_menu(app: &AppHandle) -> Result<(), String> {
    let menu = build_tray_menu(app).map_err(|error| error.to_string())?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu))
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let labels = tray_menu_labels_for_app(app);
    let show = MenuItem::with_id(app, SHOW_ID, labels.show_main_window, true, None::<&str>)?;
    let version = MenuItem::with_id(
        app,
        VERSION_ID,
        format!("{} {}", labels.version, app_info::app_display_version()),
        true,
        None::<&str>,
    )?;
    let open_app_folder = MenuItem::with_id(
        app,
        OPEN_APP_FOLDER_ID,
        labels.open_app_folder,
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, QUIT_ID, labels.quit, true, None::<&str>)?;

    Menu::with_items(app, &[&show, &version, &open_app_folder, &quit])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedTrayLanguage {
    En,
    ZhCn,
}

struct TrayMenuLabels {
    show_main_window: &'static str,
    version: &'static str,
    open_app_folder: &'static str,
    quit: &'static str,
}

fn tray_menu_labels_for_app(app: &AppHandle) -> TrayMenuLabels {
    let language = config::config_path_for_app(app)
        .and_then(|path| config::load_or_create_config(&path).map(|loaded| loaded.config.language))
        .unwrap_or(AppLanguage::System);

    tray_menu_labels(&language)
}

fn tray_menu_labels(language: &AppLanguage) -> TrayMenuLabels {
    match resolve_tray_language(language) {
        ResolvedTrayLanguage::En => TrayMenuLabels {
            show_main_window: "Show Main Window",
            version: "Version",
            open_app_folder: "Open App Folder",
            quit: "Quit",
        },
        ResolvedTrayLanguage::ZhCn => TrayMenuLabels {
            show_main_window: "显示主窗口",
            version: "版本",
            open_app_folder: "打开程序所在目录",
            quit: "退出",
        },
    }
}

fn resolve_tray_language(language: &AppLanguage) -> ResolvedTrayLanguage {
    match language {
        AppLanguage::En => ResolvedTrayLanguage::En,
        AppLanguage::ZhCn => ResolvedTrayLanguage::ZhCn,
        AppLanguage::System => system_tray_language(),
    }
}

fn system_tray_language() -> ResolvedTrayLanguage {
    const LANGUAGE_ENV_VARS: [&str; 4] = ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"];

    if LANGUAGE_ENV_VARS
        .iter()
        .filter_map(|name| env::var(name).ok())
        .any(|language| is_chinese_language_tag(&language))
    {
        return ResolvedTrayLanguage::ZhCn;
    }

    #[cfg(windows)]
    if windows_system_language_prefers_zh() {
        return ResolvedTrayLanguage::ZhCn;
    }

    ResolvedTrayLanguage::En
}

fn is_chinese_language_tag(language: &str) -> bool {
    language
        .split([':', ';', ',', '.'])
        .any(|part| part.trim().to_ascii_lowercase().starts_with("zh"))
}

#[cfg(windows)]
fn windows_system_language_prefers_zh() -> bool {
    windows_user_default_locale_name()
        .as_deref()
        .is_some_and(is_chinese_language_tag)
        || windows_user_default_ui_language_primary() == Some(WINDOWS_LANG_CHINESE)
}

#[cfg(windows)]
fn windows_user_default_locale_name() -> Option<String> {
    const LOCALE_NAME_MAX_LENGTH: usize = 85;
    let mut buffer = [0_u16; LOCALE_NAME_MAX_LENGTH];
    let length =
        unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), LOCALE_NAME_MAX_LENGTH as i32) };
    if length <= 1 {
        return None;
    }

    Some(String::from_utf16_lossy(&buffer[..length as usize - 1]))
}

#[cfg(windows)]
fn windows_user_default_ui_language_primary() -> Option<u16> {
    let language_id = unsafe { GetUserDefaultUILanguage() };
    if language_id == 0 {
        return None;
    }

    Some(language_id & WINDOWS_PRIMARY_LANGUAGE_MASK)
}

#[cfg(windows)]
const WINDOWS_PRIMARY_LANGUAGE_MASK: u16 = 0x03ff;
#[cfg(windows)]
const WINDOWS_LANG_CHINESE: u16 = 0x04;

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

    let size = tray_popup_size_from_saved(config::load_tray_popup_size_for_app(app));

    WebviewWindowBuilder::new(
        app,
        TRAY_POPUP_LABEL,
        WebviewUrl::App(TRAY_POPUP_VIEW.into()),
    )
    .title("QuotaBarWin")
    .inner_size(size.width, size.height)
    .min_inner_size(TRAY_POPUP_MIN_WIDTH, TRAY_POPUP_MIN_HEIGHT)
    .position(0.0, 0.0)
    .resizable(true)
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
pub fn reset_tray_popup_size(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(TRAY_POPUP_LABEL) {
        let size = default_tray_popup_size();
        window
            .set_size(LogicalSize::new(size.width, size.height))
            .map_err(|error| error.to_string())?;
        config::save_tray_popup_size_for_app(&app, size)?;
    }

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

#[tauri::command]
pub fn start_tray_popup_resizing(app: AppHandle) -> Result<(), String> {
    if let Some(webview_window) = app.get_webview_window(TRAY_POPUP_LABEL) {
        let window = webview_window.as_ref().window();
        suppress_focus_hide_for_drag();
        window
            .start_resize_dragging(ResizeDirection::SouthEast)
            .map_err(|error| error.to_string())?;
        suppress_focus_hide_for_drag();
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

pub fn hide_tray_popup_after_focus_lost(window: Window) {
    std::thread::spawn(move || {
        std::thread::sleep(TRAY_POPUP_FOCUS_LOST_HIDE_DELAY);
        if should_hide_tray_popup_after_focus_lost_delay(&window) {
            let _ = window.hide();
        }
    });
}

fn should_hide_tray_popup_after_focus_lost_delay(window: &Window) -> bool {
    should_hide_tray_popup_on_focus_lost()
        && window.is_visible().unwrap_or(false)
        && !window.is_focused().unwrap_or(false)
        && !is_cursor_inside_tray_popup(window)
}

fn is_cursor_inside_tray_popup(window: &Window) -> bool {
    let Ok(cursor) = window.cursor_position() else {
        return false;
    };
    let Ok(position) = window.outer_position() else {
        return false;
    };
    let Ok(size) = window.outer_size() else {
        return false;
    };

    let left = f64::from(position.x);
    let top = f64::from(position.y);
    let right = left + f64::from(size.width);
    let bottom = top + f64::from(size.height);

    cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= bottom
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

pub fn save_tray_popup_size_after_resize(
    app: &AppHandle,
    size: PhysicalSize<u32>,
    scale_factor: f64,
) {
    let logical_size = tray_popup_logical_size_from_physical(size, scale_factor);
    let _ = config::save_tray_popup_size_for_app(app, logical_size);
}

#[tauri::command]
pub fn get_tray_popup_presentation_id() -> u64 {
    TRAY_POPUP_PRESENTATION_ID.load(Ordering::SeqCst)
}

fn handle_menu_event(app: &AppHandle, id: &MenuId) {
    match id.as_ref() {
        SHOW_ID => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
                let _ = window.emit("refresh-requested", ());
            }
        }
        OPEN_APP_FOLDER_ID => {
            if let Err(error) = config::open_app_folder_for_app(app) {
                eprintln!("Failed to open app folder from tray menu: {error}");
            }
        }
        VERSION_ID => {
            if let Err(error) = open_url_external(GITHUB_URL) {
                eprintln!("Failed to open GitHub from tray menu: {error}");
            }
        }
        QUIT_ID => app.exit(0),
        _ => {}
    }
}

fn open_url_external(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("rundll32")
            .arg("url.dll,FileProtocolHandler")
            .arg(url)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
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
        let presentation_id = TRAY_POPUP_PRESENTATION_ID.fetch_add(1, Ordering::SeqCst) + 1;
        let position = tray_popup_position_from_saved_or_anchor(
            config::load_tray_popup_position_for_app(app),
            anchor,
        );
        suppress_focus_hide_for_drag();
        let _ = window.set_position(PhysicalPosition::new(position.0, position.1));
        let _ = window.set_always_on_top(true);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit_to(
            TRAY_POPUP_LABEL,
            "tray-popup-shown",
            TrayPopupShownPayload { presentation_id },
        );
    }
}

fn tray_popup_position(anchor: PhysicalPosition<f64>) -> (i32, i32) {
    let x = (anchor.x - TRAY_POPUP_WIDTH + TRAY_POPUP_OFFSET).max(0.0);
    let y = (anchor.y - TRAY_POPUP_HEIGHT - TRAY_POPUP_OFFSET).max(0.0);
    (x.round() as i32, y.round() as i32)
}

fn default_tray_popup_size() -> TrayPopupSize {
    TrayPopupSize {
        width: TRAY_POPUP_WIDTH,
        height: TRAY_POPUP_HEIGHT,
    }
}

fn tray_popup_size_from_saved(saved: Option<TrayPopupSize>) -> TrayPopupSize {
    saved
        .filter(|size| size.width.is_finite() && size.height.is_finite())
        .map(|size| TrayPopupSize {
            width: size
                .width
                .clamp(TRAY_POPUP_MIN_WIDTH, TRAY_POPUP_MAX_RESTORED_WIDTH),
            height: size
                .height
                .clamp(TRAY_POPUP_MIN_HEIGHT, TRAY_POPUP_MAX_RESTORED_HEIGHT),
        })
        .unwrap_or_else(default_tray_popup_size)
}

fn tray_popup_logical_size_from_physical(
    size: PhysicalSize<u32>,
    scale_factor: f64,
) -> TrayPopupSize {
    let safe_scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    tray_popup_size_from_saved(Some(TrayPopupSize {
        width: f64::from(size.width) / safe_scale_factor,
        height: f64::from(size.height) / safe_scale_factor,
    }))
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
    fn tray_menu_contains_current_actions() {
        assert_eq!(
            tray_menu_ids(),
            [SHOW_ID, VERSION_ID, OPEN_APP_FOLDER_ID, QUIT_ID]
        );
    }

    #[test]
    fn tray_menu_labels_are_localized() {
        let en = tray_menu_labels(&AppLanguage::En);
        assert_eq!(en.show_main_window, "Show Main Window");
        assert_eq!(en.version, "Version");
        assert_eq!(en.open_app_folder, "Open App Folder");
        assert_eq!(en.quit, "Quit");

        let zh_cn = tray_menu_labels(&AppLanguage::ZhCn);
        assert_eq!(zh_cn.show_main_window, "显示主窗口");
        assert_eq!(zh_cn.version, "版本");
        assert_eq!(zh_cn.open_app_folder, "打开程序所在目录");
        assert_eq!(zh_cn.quit, "退出");
    }

    #[test]
    fn chinese_language_tags_are_detected() {
        assert!(is_chinese_language_tag("zh-CN"));
        assert!(is_chinese_language_tag("zh_Hans_CN.UTF-8"));
        assert!(is_chinese_language_tag("en-US:zh-CN"));
        assert!(!is_chinese_language_tag("en-US"));
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

    #[test]
    fn tray_popup_size_defaults_and_clamps_saved_size() {
        assert_eq!(tray_popup_size_from_saved(None), default_tray_popup_size());
        assert_eq!(
            tray_popup_size_from_saved(Some(TrayPopupSize {
                width: 100.0,
                height: 120.0
            })),
            TrayPopupSize {
                width: TRAY_POPUP_MIN_WIDTH,
                height: TRAY_POPUP_MIN_HEIGHT
            }
        );
        assert_eq!(
            tray_popup_size_from_saved(Some(TrayPopupSize {
                width: f64::NAN,
                height: 520.0
            })),
            default_tray_popup_size()
        );
    }

    #[test]
    fn tray_popup_resize_event_saves_logical_size() {
        assert_eq!(
            tray_popup_logical_size_from_physical(PhysicalSize::new(760, 1040), 2.0),
            TrayPopupSize {
                width: 380.0,
                height: 520.0
            }
        );
    }
}
