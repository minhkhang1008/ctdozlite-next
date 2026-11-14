//! Local quick-message macros and composer matching rules.

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const IMAGE_DIRECTORY: &str = "quick-message-images";
const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 25_000_000;
/// Most quick messages accepted from settings and exposed in the manager.
pub const MAX_ITEMS: usize = 100;
/// Most Unicode scalar values persisted in one quick-message body.
pub const MAX_CONTENT_CHARS: usize = 5_000;

/// A reusable message inserted into the composer after typing its slash shortcut.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuickMessage {
    /// Shortcut without the leading slash.
    pub shortcut: String,
    /// Text inserted into the composer. It may be empty when an image is attached.
    pub content: String,
    /// App-data-relative path of the optional private image copy.
    pub image_path: Option<String>,
}

/// Validate a shortcut against syntax and case-insensitive uniqueness rules.
///
/// `editing_index` excludes the item being edited from duplicate detection.
///
/// # Errors
/// Returns a Vietnamese validation message when the shortcut is invalid or duplicated.
pub fn validate_shortcut(
    shortcut: &str,
    items: &[QuickMessage],
    editing_index: Option<usize>,
) -> Result<(), String> {
    let count = shortcut.chars().count();
    if count == 0 {
        return Err("Phím tắt không được để trống".into());
    }
    if count > 20 {
        return Err("Phím tắt chỉ được dài tối đa 20 ký tự".into());
    }
    if shortcut
        .chars()
        .any(|ch| ch == '/' || ch.is_whitespace() || ch.is_control())
    {
        return Err("Phím tắt không được chứa '/', khoảng trắng hoặc ký tự điều khiển".into());
    }
    let folded = shortcut.to_lowercase();
    if items
        .iter()
        .enumerate()
        .any(|(index, item)| Some(index) != editing_index && item.shortcut.to_lowercase() == folded)
    {
        return Err("Phím tắt này đã tồn tại".into());
    }
    Ok(())
}

/// Return the slash-command query when the complete composer text is eligible for suggestions.
pub fn composer_query(text: &str) -> Option<&str> {
    let query = text.strip_prefix('/')?;
    if query.chars().count() > 20
        || query
            .chars()
            .any(|ch| ch == '/' || ch.is_whitespace() || ch.is_control())
    {
        return None;
    }
    Some(query)
}

/// Return prefix matches in their saved order, paired with their settings index.
pub fn matching_items<'a>(
    items: &'a [QuickMessage],
    query: &str,
) -> Vec<(usize, &'a QuickMessage)> {
    let query = query.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.shortcut.to_lowercase().starts_with(&query))
        .collect()
}

/// Draft values produced by selecting a quick message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerInsertion {
    /// Text that replaces the slash query in the composer.
    pub text: String,
    /// Optional managed image to add to the composer attachment strip.
    pub image_path: Option<String>,
}

/// Build composer state without sending anything to the network worker.
pub fn composer_insertion(item: &QuickMessage) -> ComposerInsertion {
    ComposerInsertion {
        text: item.content.clone(),
        image_path: item.image_path.clone(),
    }
}

/// Move the keyboard selection within the current result bounds.
pub fn move_selection(current: usize, len: usize, down: bool, up: bool) -> usize {
    if len == 0 {
        return 0;
    }
    let current = current.min(len - 1);
    if down {
        (current + 1).min(len - 1)
    } else if up {
        current.saturating_sub(1)
    } else {
        current
    }
}

/// Bound and validate quick messages loaded from the user-editable settings file.
pub fn sanitize_items(items: Vec<QuickMessage>) -> Vec<QuickMessage> {
    let mut sanitized = Vec::with_capacity(items.len().min(MAX_ITEMS));
    for mut item in items {
        item.shortcut = item.shortcut.trim().to_string();
        if validate_shortcut(&item.shortcut, &sanitized, None).is_err() {
            continue;
        }
        item.content = item
            .content
            .trim()
            .chars()
            .take(MAX_CONTENT_CHARS)
            .collect();
        item.image_path = item
            .image_path
            .filter(|path| managed_image_path(Path::new("."), path).is_ok());
        if item.content.is_empty() && item.image_path.is_none() {
            continue;
        }
        sanitized.push(item);
        if sanitized.len() == MAX_ITEMS {
            break;
        }
    }
    sanitized
}

/// Resolve a stored quick-message image path inside the app-owned image directory.
///
/// # Errors
/// Returns an error for absolute paths, traversal, nested paths, or a different directory.
pub fn managed_image_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = Path::new(relative);
    if relative.is_absolute() {
        bail!("Quick-message image path must be relative");
    }
    let mut components = relative.components();
    let directory = components.next();
    let file = components.next();
    if components.next().is_some()
        || directory != Some(Component::Normal(IMAGE_DIRECTORY.as_ref()))
        || !matches!(file, Some(Component::Normal(_)))
    {
        bail!("Invalid quick-message image path");
    }
    Ok(root.join(relative))
}

fn reject_symlink(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("Managed quick-message image path cannot be a symbolic link")
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Validate and copy one image into bounded private app storage.
///
/// The returned path is app-data-relative and safe to persist in settings.
///
/// # Errors
/// Returns an error for unreadable, oversized, unsupported, malformed, or overlarge images,
/// and for private-storage failures.
pub fn import_image(root: &Path, source: &Path) -> Result<String> {
    let metadata = std::fs::metadata(source).context("Không đọc được thông tin ảnh")?;
    if !metadata.is_file() {
        bail!("Đường dẫn đã chọn không phải là một tệp ảnh");
    }
    if metadata.len() > MAX_IMAGE_BYTES {
        bail!("Ảnh đính kèm không được lớn hơn 20 MiB");
    }

    let mut bytes = Vec::with_capacity(metadata.len().min(MAX_IMAGE_BYTES) as usize);
    std::fs::File::open(source)
        .context("Không mở được ảnh đã chọn")?
        .take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("Không đọc được ảnh đã chọn")?;
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        bail!("Ảnh đính kèm không được lớn hơn 20 MiB");
    }

    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .context("Không nhận diện được định dạng ảnh")?;
    let extension = match reader.format() {
        Some(image::ImageFormat::Png) => "png",
        Some(image::ImageFormat::Jpeg) => "jpg",
        Some(image::ImageFormat::Gif) => "gif",
        _ => bail!("Chỉ hỗ trợ ảnh PNG, JPEG hoặc GIF"),
    };
    let (width, height) = reader
        .into_dimensions()
        .context("Tệp đã chọn không phải là ảnh hợp lệ")?;
    if u64::from(width).saturating_mul(u64::from(height)) > MAX_IMAGE_PIXELS {
        bail!("Ảnh đính kèm có kích thước điểm ảnh quá lớn");
    }

    let digest = format!("{:x}", Sha256::digest(&bytes));
    let relative = format!("{IMAGE_DIRECTORY}/{digest}.{extension}");
    let destination = managed_image_path(root, &relative)?;
    let directory = root.join(IMAGE_DIRECTORY);
    reject_symlink(&directory)?;
    crate::data_dir::ensure_private_dir(&directory)
        .context("Không tạo được thư mục ảnh tin nhắn nhanh")?;
    reject_symlink(&destination)?;
    if !destination.is_file() {
        crate::data_dir::write_private_atomic(&destination, &bytes)
            .context("Không lưu được ảnh tin nhắn nhanh")?;
    } else {
        crate::data_dir::ensure_private_file(&destination)?;
    }
    Ok(relative)
}

/// Remove one managed image without following symbolic links outside app storage.
///
/// # Errors
/// Returns an error for unsafe paths, symbolic links, unexpected file types, or filesystem
/// failures other than an already-missing image.
pub fn delete_image(root: &Path, relative: &str) -> Result<()> {
    let target = managed_image_path(root, relative)?;
    reject_symlink(&root.join(IMAGE_DIRECTORY))?;
    reject_symlink(&target)?;
    match std::fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.is_file() => {
            std::fs::remove_file(target).context("Không xóa được ảnh tin nhắn nhanh")
        }
        Ok(_) => bail!("Managed quick-message image is not a regular file"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_validation_rejects_prefix_whitespace_length_and_duplicates() {
        assert!(validate_shortcut("github", &[], None).is_ok());
        assert!(validate_shortcut("", &[], None).is_err());
        assert!(validate_shortcut("/github", &[], None).is_err());
        assert!(validate_shortcut("git hub", &[], None).is_err());
        assert!(validate_shortcut("123456789012345678901", &[], None).is_err());

        let items = vec![QuickMessage {
            shortcut: "GitHub".into(),
            content: "Hồ sơ".into(),
            image_path: None,
        }];
        assert!(validate_shortcut("github", &items, None).is_err());
        assert!(validate_shortcut("github", &items, Some(0)).is_ok());
    }

    #[test]
    fn composer_query_only_accepts_a_leading_unbroken_slash_command() {
        assert_eq!(composer_query("/"), Some(""));
        assert_eq!(composer_query("/GiT"), Some("GiT"));
        assert_eq!(composer_query("hello /git"), None);
        assert_eq!(composer_query("/git hub"), None);
        assert_eq!(composer_query("//git"), None);
        assert_eq!(composer_query("/123456789012345678901"), None);
    }

    #[test]
    fn matching_items_are_case_insensitive_and_keep_settings_order() {
        let items = vec![
            QuickMessage {
                shortcut: "gioithieu".into(),
                content: "Xin chào".into(),
                image_path: None,
            },
            QuickMessage {
                shortcut: "github".into(),
                content: "Hồ sơ GitHub".into(),
                image_path: Some("quick-message-images/github.png".into()),
            },
            QuickMessage {
                shortcut: "camon".into(),
                content: "Cảm ơn".into(),
                image_path: None,
            },
        ];

        let matches = matching_items(&items, "GI");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].1.shortcut, "gioithieu");
        assert_eq!(matches[1].1.shortcut, "github");
    }

    #[test]
    fn saved_item_roundtrips_with_an_optional_image() {
        let item = QuickMessage {
            shortcut: "github".into(),
            content: "Hồ sơ GitHub".into(),
            image_path: Some("quick-message-images/github.png".into()),
        };
        let restored: QuickMessage =
            serde_json::from_str(&serde_json::to_string(&item).unwrap()).unwrap();
        assert_eq!(restored, item);
    }

    #[test]
    fn image_import_copies_a_valid_image_into_private_managed_storage() {
        let temp = crate::data_dir::PrivateTempDir::new("quick-message-image-test").unwrap();
        let source = temp.path().join("source.png");
        image::RgbImage::from_pixel(2, 2, image::Rgb([12, 34, 56]))
            .save(&source)
            .unwrap();

        let relative = import_image(temp.path(), &source).unwrap();
        assert!(relative.starts_with("quick-message-images/"));
        let managed = managed_image_path(temp.path(), &relative).unwrap();
        assert!(managed.is_file());
        assert_eq!(image::image_dimensions(&managed).unwrap(), (2, 2));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(managed).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn managed_image_path_rejects_absolute_and_traversal_paths() {
        let root = std::path::Path::new("/tmp/app-data");
        assert!(managed_image_path(root, "quick-message-images/a.png").is_ok());
        assert!(managed_image_path(root, "../outside.png").is_err());
        assert!(managed_image_path(root, "/tmp/outside.png").is_err());
        assert!(managed_image_path(root, "quick-message-images/nested/a.png").is_err());
        assert!(managed_image_path(root, "other/a.png").is_err());
    }

    #[test]
    fn image_import_rejects_non_images_without_creating_managed_files() {
        let temp = crate::data_dir::PrivateTempDir::new("quick-message-image-reject").unwrap();
        let source = temp.path().join("not-an-image.png");
        std::fs::write(&source, b"not an image").unwrap();

        assert!(import_image(temp.path(), &source).is_err());
        assert!(!temp.path().join("quick-message-images").exists());
    }

    #[test]
    fn keyboard_selection_clamps_to_the_available_results() {
        assert_eq!(move_selection(0, 3, true, false), 1);
        assert_eq!(move_selection(2, 3, true, false), 2);
        assert_eq!(move_selection(2, 3, false, true), 1);
        assert_eq!(move_selection(0, 3, false, true), 0);
        assert_eq!(move_selection(9, 0, true, false), 0);
    }

    #[test]
    fn composer_insertion_returns_draft_content_and_attachment_without_sending() {
        let item = QuickMessage {
            shortcut: "github".into(),
            content: "Hồ sơ GitHub".into(),
            image_path: Some("quick-message-images/github.png".into()),
        };
        let insertion = composer_insertion(&item);
        assert_eq!(insertion.text, "Hồ sơ GitHub");
        assert_eq!(
            insertion.image_path.as_deref(),
            Some("quick-message-images/github.png")
        );
    }

    #[cfg(unix)]
    #[test]
    fn managed_storage_rejects_symlinked_image_directories_for_import_and_delete() {
        use std::os::unix::fs::symlink;

        let temp = crate::data_dir::PrivateTempDir::new("quick-message-symlink-test").unwrap();
        let outside = temp.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, temp.path().join("quick-message-images")).unwrap();

        let source = temp.path().join("source.png");
        image::RgbImage::from_pixel(2, 2, image::Rgb([1, 2, 3]))
            .save(&source)
            .unwrap();
        assert!(import_image(temp.path(), &source).is_err());
        assert!(std::fs::read_dir(&outside).unwrap().next().is_none());

        let outside_file = outside.join("victim.png");
        std::fs::write(&outside_file, b"keep").unwrap();
        assert!(delete_image(temp.path(), "quick-message-images/victim.png").is_err());
        assert!(outside_file.is_file());
    }

    #[test]
    fn sanitizing_saved_items_bounds_counts_content_and_invalid_paths() {
        let mut items: Vec<QuickMessage> = (0..105)
            .map(|index| QuickMessage {
                shortcut: format!("m{index}"),
                content: "x".repeat(5_010),
                image_path: None,
            })
            .collect();
        items.insert(
            0,
            QuickMessage {
                shortcut: "bad shortcut".into(),
                content: "invalid".into(),
                image_path: None,
            },
        );
        items.insert(
            1,
            QuickMessage {
                shortcut: "empty".into(),
                content: String::new(),
                image_path: Some("../outside.png".into()),
            },
        );

        let sanitized = sanitize_items(items);
        assert_eq!(sanitized.len(), MAX_ITEMS);
        assert_eq!(sanitized[0].shortcut, "m0");
        assert_eq!(sanitized[0].content.chars().count(), MAX_CONTENT_CHARS);
        assert!(sanitized.iter().all(|item| item.shortcut != "empty"));
    }
}
