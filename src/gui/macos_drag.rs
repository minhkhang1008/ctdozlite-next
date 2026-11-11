//! Cursor position while an OS file drag hovers the window (macOS).
//!
//! During an AppKit dragging session the window gets `draggingUpdated:` callbacks instead of
//! mouse-move events, and winit 0.30 only forwards `HoveredFile` without a position. egui's
//! pointer therefore stays frozen at wherever it was before the drag. `NSEvent.mouseLocation`
//! is updated throughout the drag, so we read it directly and map it into egui points.

use eframe::egui;
use objc2_app_kit::{NSEvent, NSScreen};
use objc2_foundation::MainThreadMarker;

/// Current mouse position in egui points relative to the window's content area, or `None`
/// when not on the main thread or the window geometry is unknown.
pub(super) fn cursor_in_window(ctx: &egui::Context) -> Option<egui::Pos2> {
    let mtm = MainThreadMarker::new()?;
    let (inner, native_ppp) = ctx.input(|i| {
        (
            i.viewport().inner_rect,
            i.viewport().native_pixels_per_point,
        )
    });
    let inner = inner?;
    let ppp = ctx.pixels_per_point();
    // egui zoom: egui points = native (AppKit) points / zoom.
    let zoom = native_ppp
        .map(|n| ppp / n)
        .filter(|z| *z > 0.0)
        .unwrap_or(1.0);
    // AppKit global coordinates have a bottom-left origin on the primary screen; winit (and so
    // egui's `inner_rect`) uses a top-left origin on that same screen.
    // SAFETY: `screens` / `mouseLocation` are plain getters, called on the main thread.
    let primary_h = unsafe { NSScreen::screens(mtm).firstObject() }?
        .frame()
        .size
        .height;
    let mouse = unsafe { NSEvent::mouseLocation() };
    let global = egui::Pos2::new(mouse.x as f32, (primary_h - mouse.y) as f32);
    let origin = inner.min.to_vec2() * zoom;
    Some(((global - origin).to_vec2() / zoom).to_pos2())
}
