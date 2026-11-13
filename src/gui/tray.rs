//! System tray / menu-bar icon so the app keeps running after its window is closed.
//!
//! Windows and macOS only (Linux needs GTK + appindicator system libraries; it falls back to
//! minimising). Tray events arrive on OS threads, so they only set atomic flags and request a
//! repaint; the UI loop in `update` consumes the flags.

use std::sync::atomic::{AtomicBool, Ordering};

static TRAY_ACTIVE: AtomicBool = AtomicBool::new(false);
static SHOW_REQUESTED: AtomicBool = AtomicBool::new(false);
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// True when a tray icon exists, so hiding the window is recoverable.
pub fn is_active() -> bool {
    TRAY_ACTIVE.load(Ordering::Relaxed)
}

pub fn take_show_request() -> bool {
    SHOW_REQUESTED.swap(false, Ordering::Relaxed)
}

pub fn take_quit_request() -> bool {
    QUIT_REQUESTED.swap(false, Ordering::Relaxed)
}

/// Owns the tray icon; dropping it removes the icon.
pub struct Tray {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    _icon: tray_icon::TrayIcon,
}

impl Tray {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    pub fn new(ctx: &eframe::egui::Context) -> Option<Self> {
        use tray_icon::menu::{Menu, MenuEvent, MenuItem};
        use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

        let icon_data = super::theme::load_app_icon()?;
        let icon = Icon::from_rgba(icon_data.rgba, icon_data.width, icon_data.height).ok()?;

        let show_item = MenuItem::with_id("show", "Mở Zalo", true, None);
        let quit_item = MenuItem::with_id("quit", "Thoát", true, None);
        let menu = Menu::new();
        menu.append_items(&[&show_item, &quit_item]).ok()?;

        let repaint_ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            match event.id.0.as_str() {
                "show" => SHOW_REQUESTED.store(true, Ordering::Relaxed),
                "quit" => QUIT_REQUESTED.store(true, Ordering::Relaxed),
                _ => {}
            }
            repaint_ctx.request_repaint();
        }));
        let repaint_ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            // Left click / double click restores the window (Windows convention).
            let restore = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );
            if restore {
                SHOW_REQUESTED.store(true, Ordering::Relaxed);
                repaint_ctx.request_repaint();
            }
        }));

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip(crate::APP_DISPLAY_NAME)
            .with_icon(icon)
            .build()
            .ok()?;
        TRAY_ACTIVE.store(true, Ordering::Relaxed);
        Some(Self { _icon: tray })
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    pub fn new(_ctx: &eframe::egui::Context) -> Option<Self> {
        None
    }
}
