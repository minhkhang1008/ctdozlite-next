//! Colour emoji rasterisation from the operating system's emoji font.
//!
//! egui draws text through a single-channel (coverage-only) glyph atlas, so it can never show
//! colour glyphs: every emoji comes out as a tinted monochrome outline. This module extracts the
//! embedded colour bitmap (`sbix` on macOS, `CBDT` on Linux) for an emoji cluster so the GUI can
//! draw it as a texture instead.
//!
//! Must only run off the UI thread (it touches a ~180 MB memory-mapped font file and decodes
//! PNG); the GUI reaches it through `WorkerCmd::RasterizeEmoji`.

use std::sync::OnceLock;

use memmap2::Mmap;
use rustybuzz::ttf_parser::{GlyphId, RasterImageFormat};

/// Candidate colour emoji fonts, in priority order. Missing files are skipped.
const EMOJI_FONT_PATHS: [&str; 4] = [
    "/System/Library/Fonts/Apple Color Emoji.ttc",
    "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/google-noto-emoji/NotoColorEmoji.ttf",
];

/// Largest bitmap edge we keep; bigger strikes are downscaled to bound texture memory.
const MAX_EDGE: u32 = 160;

/// A decoded, square, straight-alpha RGBA bitmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiBitmap {
    pub size: u32,
    pub rgba: Vec<u8>,
}

fn emoji_font() -> Option<&'static Mmap> {
    static FONT: OnceLock<Option<Mmap>> = OnceLock::new();
    FONT.get_or_init(|| {
        EMOJI_FONT_PATHS.iter().find_map(|path| {
            let file = std::fs::File::open(path).ok()?;
            // SAFETY: the file is a read-only system font that is not modified while the app
            // runs; the map is never written and lives for the whole process ('static OnceLock).
            unsafe { Mmap::map(&file) }.ok()
        })
    })
    .as_ref()
}

/// Rasterise one emoji cluster (a single emoji, ZWJ sequence, flag or skin-tone variant).
///
/// `pixels_per_em` picks the closest embedded strike at or above that size. Returns `None` when
/// no colour emoji font is installed or the font has no bitmap for this cluster.
pub fn rasterize(cluster: &str, pixels_per_em: u16) -> Option<EmojiBitmap> {
    let data = emoji_font()?;
    rasterize_from_font(data, cluster, pixels_per_em)
}

fn rasterize_from_font(font: &[u8], cluster: &str, pixels_per_em: u16) -> Option<EmojiBitmap> {
    if cluster.is_empty() {
        return None;
    }
    let face = rustybuzz::Face::from_slice(font, 0)?;

    // Shaping applies the font's ligature rules (morx/GSUB) so multi-codepoint sequences such
    // as 👨‍👩‍👧, 🇻🇳 or 👍🏽 collapse into their single combined glyph.
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(cluster);
    let shaped = rustybuzz::shape(&face, &[], buffer);
    let gid = shaped
        .glyph_infos()
        .iter()
        .map(|g| g.glyph_id)
        .find(|&id| id != 0)?;
    let gid = GlyphId(u16::try_from(gid).ok()?);

    let raster = face.glyph_raster_image(gid, pixels_per_em)?;
    if raster.format != RasterImageFormat::PNG {
        return None;
    }
    let img = image::load_from_memory(raster.data).ok()?.into_rgba8();
    Some(square_and_bound(img))
}

/// Centre the bitmap on a transparent square canvas and cap its edge at `MAX_EDGE`.
fn square_and_bound(img: image::RgbaImage) -> EmojiBitmap {
    let (w, h) = img.dimensions();
    let edge = w.max(h).max(1);
    let mut canvas = image::RgbaImage::new(edge, edge);
    image::imageops::overlay(
        &mut canvas,
        &img,
        i64::from((edge - w) / 2),
        i64::from((edge - h) / 2),
    );
    let canvas = if edge > MAX_EDGE {
        image::imageops::resize(
            &canvas,
            MAX_EDGE,
            MAX_EDGE,
            image::imageops::FilterType::Triangle,
        )
    } else {
        canvas
    };
    EmojiBitmap {
        size: canvas.width(),
        rgba: canvas.into_raw(),
    }
}

/// Stable texture-cache key for an emoji cluster (codepoints in hex).
pub fn texture_key(cluster: &str) -> String {
    let hex: Vec<String> = cluster.chars().map(|c| format!("{:x}", c as u32)).collect();
    format!("emoji_{}", hex.join("-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_key_is_codepoint_based() {
        assert_eq!(texture_key("🐧"), "emoji_1f427");
        assert_eq!(texture_key("❤️"), "emoji_2764-fe0f");
    }

    #[test]
    fn square_and_bound_pads_and_caps() {
        let bmp = square_and_bound(image::RgbaImage::new(20, 10));
        assert_eq!(bmp.size, 20);
        assert_eq!(bmp.rgba.len(), 20 * 20 * 4);
        let big = square_and_bound(image::RgbaImage::new(400, 400));
        assert_eq!(big.size, MAX_EDGE);
    }

    #[test]
    fn garbage_font_data_yields_none() {
        assert!(rasterize_from_font(b"not a font", "🐧", 96).is_none());
        assert!(rasterize_from_font(b"", "", 96).is_none());
    }

    /// Exercises the real system font; skipped where it is not installed.
    #[test]
    fn rasterizes_single_and_sequence_emoji_when_font_present() {
        if emoji_font().is_none() {
            return;
        }
        for cluster in ["🐧", "💌", "❤️", "👍🏽", "🇻🇳", "👨‍👩‍👧"] {
            let bmp = rasterize(cluster, 96).unwrap_or_else(|| panic!("no bitmap for {cluster}"));
            assert!(bmp.size >= 32, "{cluster} too small: {}", bmp.size);
            assert!(bmp.rgba.iter().skip(3).step_by(4).any(|&a| a > 0));
        }
        // Shaping must pick the combined glyph, not the first component.
        assert_ne!(rasterize("👨‍👩‍👧", 96), rasterize("👨", 96));
        assert_ne!(rasterize("👍🏽", 96), rasterize("👍", 96));
        assert_ne!(rasterize("🇻🇳", 96), rasterize("🇻", 96));
    }
}
