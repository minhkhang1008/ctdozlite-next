//! macOS clipboard image paste: `Cmd+V` detection and in-process `NSPasteboard` reading.
//!
//! egui-winit 0.29 swallows the `Cmd+V` key event and only emits `Event::Paste` when the
//! clipboard holds non-empty text (`egui-winit/src/lib.rs`, `is_paste_command`). With an
//! image-only clipboard (screenshots, "Copy Image" in a browser, Preview) the app therefore saw
//! no input at all. A local `NSEvent` monitor sees the key before winit does; it needs no
//! Accessibility / Input Monitoring permission because it only observes our own app's events.

use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Once, OnceLock};

use block2::RcBlock;
use objc2::rc::{autoreleasepool, Retained};
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSEvent, NSEventMask, NSEventModifierFlags,
    NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSData, NSDictionary, NSString, NSURL};

/// Virtual key code of the ANSI `V` key (`kVK_ANSI_V`), used only for non-Latin layouts.
const KVK_ANSI_V: u16 = 0x09;
/// Largest copied image file we load from disk (Finder "Copy" of a file).
const MAX_IMAGE_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// A `Cmd+V` older than this is stale and must not trigger a later paste.
const PASTE_WINDOW_MS: u64 = 1_500;
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "heic", "heif", "tif", "tiff", "bmp", "webp",
];

static INSTALL: Once = Once::new();
static REPAINT_CTX: OnceLock<eframe::egui::Context> = OnceLock::new();
static LAST_CMD_V_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Install the `Cmd+V` key monitor once. Must be called on the main (UI) thread.
pub(super) fn install_paste_monitor(ctx: &eframe::egui::Context) {
    let _ = REPAINT_CTX.set(ctx.clone());
    INSTALL.call_once(|| {
        let block = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit passes a valid NSEvent for the duration of the handler call.
            let ev = unsafe { event.as_ref() };
            if is_cmd_v(ev) {
                LAST_CMD_V_MS.store(now_ms(), Ordering::SeqCst);
                if let Some(ctx) = REPAINT_CTX.get() {
                    ctx.request_repaint();
                }
            }
            // Returning the event unchanged lets winit/egui handle it as before.
            event.as_ptr()
        });
        // SAFETY: called on the main thread; AppKit copies the block. The monitor lives for
        // the whole process, so the returned token is intentionally leaked.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
        };
        std::mem::forget(monitor);
        std::mem::forget(block);
    });
}

fn is_cmd_v(ev: &NSEvent) -> bool {
    // SAFETY: plain property getters on a valid key-down NSEvent.
    let (flags, repeat, key_code, chars) = unsafe {
        (
            ev.modifierFlags(),
            ev.isARepeat(),
            ev.keyCode(),
            ev.charactersIgnoringModifiers().map(|s| s.to_string()),
        )
    };
    if repeat
        || !flags.contains(NSEventModifierFlags::NSEventModifierFlagCommand)
        || flags.contains(NSEventModifierFlags::NSEventModifierFlagControl)
        || flags.contains(NSEventModifierFlags::NSEventModifierFlagOption)
    {
        return false;
    }
    match chars.as_deref() {
        Some(c) if c.is_ascii() => c.eq_ignore_ascii_case("v"),
        // Non-Latin layouts: fall back to the physical key, as egui-winit does.
        _ => key_code == KVK_ANSI_V,
    }
}

/// Returns `true` once per `Cmd+V` press seen by the monitor.
pub(super) fn take_cmd_v() -> bool {
    let pressed_at = LAST_CMD_V_MS.swap(0, Ordering::SeqCst);
    pressed_at != 0 && now_ms().saturating_sub(pressed_at) <= PASTE_WINDOW_MS
}

fn nsdata_to_vec(data: &NSData) -> Vec<u8> {
    data.bytes().to_vec()
}

/// Re-encode any format AppKit understands (TIFF, HEIC, BMP, WebP, ...) as PNG bytes.
fn appkit_to_png(data: &NSData) -> Option<Vec<u8>> {
    // SAFETY: NSBitmapImageRep only reads `data`; the empty dictionary means default options.
    unsafe {
        let rep: Retained<NSBitmapImageRep> = NSBitmapImageRep::imageRepWithData(data)?;
        let props = NSDictionary::<NSString, objc2::runtime::AnyObject>::new();
        let png = rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &props)?;
        Some(nsdata_to_vec(&png))
    }
}

fn decode(data: &NSData) -> Option<image::RgbaImage> {
    let bytes = nsdata_to_vec(data);
    if let Ok(img) = image::load_from_memory(&bytes) {
        return Some(img.to_rgba8());
    }
    let png = appkit_to_png(data)?;
    image::load_from_memory(&png).ok().map(|img| img.to_rgba8())
}

fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// File paths of items copied in Finder (`public.file-url`).
fn copied_file_paths(pb: &NSPasteboard) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // SAFETY: read-only pasteboard queries on the main thread.
    unsafe {
        let Some(items) = pb.pasteboardItems() else {
            return out;
        };
        for item in items.iter() {
            let Some(url_str) = item.stringForType(NSPasteboardTypeFileURL) else {
                continue;
            };
            let Some(url) = NSURL::URLWithString(&url_str) else {
                continue;
            };
            if let Some(path) = url.path() {
                out.push(PathBuf::from(path.to_string()));
            }
        }
    }
    out
}

fn load_image_file(path: &Path) -> Option<image::RgbaImage> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_IMAGE_FILE_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let data = NSData::with_bytes(&bytes);
    decode(&data)
}

/// Read an image from the general pasteboard, in priority order:
/// 1. Image files copied in Finder (the TIFF Finder also puts there is only the file icon)
/// 2. `public.png` (screenshots, browsers, most apps)
/// 3. `public.tiff` (Preview, Photos, older apps), re-encoded through AppKit
pub(super) fn read_image() -> Option<image::RgbaImage> {
    autoreleasepool(|_| {
        // SAFETY: read-only pasteboard queries on the main thread.
        let pb = unsafe { NSPasteboard::generalPasteboard() };
        let files = copied_file_paths(&pb);
        if !files.is_empty() {
            return files
                .iter()
                .filter(|p| is_image_path(p))
                .find_map(|p| load_image_file(p));
        }
        // SAFETY: as above.
        unsafe {
            if let Some(data) = pb.dataForType(NSPasteboardTypePNG) {
                if let Some(img) = decode(&data) {
                    return Some(img);
                }
            }
            pb.dataForType(NSPasteboardTypeTIFF)
                .and_then(|data| decode(&data))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_extension_filter() {
        assert!(is_image_path(Path::new("/tmp/a.PNG")));
        assert!(is_image_path(Path::new("/tmp/shot.heic")));
        assert!(!is_image_path(Path::new("/tmp/report.pdf")));
        assert!(!is_image_path(Path::new("/tmp/noext")));
    }

    #[test]
    fn stale_or_missing_cmd_v_does_not_trigger() {
        LAST_CMD_V_MS.store(0, Ordering::SeqCst);
        assert!(!take_cmd_v());
        LAST_CMD_V_MS.store(now_ms() - PASTE_WINDOW_MS - 10, Ordering::SeqCst);
        assert!(!take_cmd_v());
        LAST_CMD_V_MS.store(now_ms(), Ordering::SeqCst);
        assert!(take_cmd_v());
        assert!(!take_cmd_v(), "a press is consumed once");
    }
}
