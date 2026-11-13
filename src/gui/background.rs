//! Keep the app alive in the background when the window is closed.
//!
//! Closing the window only hides it; the worker (WebSocket, notifications, calls) keeps running.
//! With a tray icon (Windows/macOS) the window is hidden and the tray menu restores it; on macOS
//! the app is also hidden like `Cmd+H`. Without a tray (Linux) the window is minimised.
//! Real quitting: tray "Thoát", `Ctrl+Q`, auto-update.

/// Hide the app without terminating it.
pub fn hide_app(ctx: &eframe::egui::Context) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSApplication;
        use objc2_foundation::MainThreadMarker;
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).hide(None);
            return;
        }
    }
    if super::tray::is_active() {
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(false));
        return;
    }
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Minimized(true));
}

/// Bring the app back to the foreground (e.g. incoming call while hidden).
pub fn show_app(ctx: &eframe::egui::Context) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSApplication;
        use objc2_foundation::MainThreadMarker;
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            // SAFETY: nil sender is valid for `unhide:`.
            unsafe { app.unhide(None) };
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
        }
    }
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Focus);
}
