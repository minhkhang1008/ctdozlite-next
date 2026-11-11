//! Bounded release downloads and transactional installation, independent of GUI and processes.

use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Resource limits for one release asset.
#[derive(Clone, Copy)]
pub struct DownloadLimits {
    /// Smallest accepted artifact.
    pub min_bytes: u64,
    /// Maximum buffered artifact size.
    pub max_bytes: u64,
    /// Deadline covering headers and the complete body.
    pub timeout: Duration,
}

impl Default for DownloadLimits {
    fn default() -> Self {
        Self {
            min_bytes: 512 * 1024,
            max_bytes: 512 * 1024 * 1024,
            timeout: Duration::from_secs(240),
        }
    }
}

/// Release metadata supplied by the release discovery layer.
pub struct DownloadRequest<'a> {
    /// HTTP asset location, without session cookies.
    pub url: &'a str,
    /// A single filename, never a path.
    pub asset_name: &'a str,
    /// Optional published SHA-256 hex digest.
    pub expected_sha256: Option<&'a str>,
    /// Optional size published by the release API.
    pub expected_size: Option<u64>,
    /// Resource bounds for this transfer.
    pub limits: DownloadLimits,
}

/// Explicit destination selected by the platform adapter, not remote metadata.
pub enum InstallTarget {
    /// Replace a portable executable and preserve its permissions.
    Portable(PathBuf),
    /// Save an installer for the platform launcher.
    Installer(PathBuf),
}

/// Reject absolute paths, traversal and cross-platform path separators.
///
/// # Errors
/// Returns an error unless the input is a single ordinary filename.
pub fn validate_asset_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':', '\0'])
        || name.chars().any(char::is_control)
    {
        return Err("Tên gói cập nhật không hợp lệ".into());
    }
    Ok(())
}

/// Download and validate an asset before any installation destination is touched.
/// The caller supplies a dedicated, cookie-free client and a progress callback.
///
/// # Errors
/// Rejects malformed metadata, HTTP failures, incomplete bodies, resource limits,
/// unsupported executable signatures, and checksum mismatches.
pub async fn download_release(
    client: &reqwest::Client,
    request: &DownloadRequest<'_>,
    mut progress: impl FnMut(u64, u64),
) -> Result<Vec<u8>, String> {
    validate_asset_name(request.asset_name)?;
    let limits = request.limits;
    if limits.min_bytes > limits.max_bytes || limits.max_bytes > usize::MAX as u64 {
        return Err("Giới hạn dung lượng cập nhật không hợp lệ".into());
    }
    if let Some(hash) = request.expected_sha256 {
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Mã SHA-256 của gói cập nhật không hợp lệ".into());
        }
    }
    if request
        .expected_size
        .is_some_and(|size| size > limits.max_bytes)
    {
        return Err("Gói cập nhật vượt giới hạn dung lượng".into());
    }
    let transfer = async {
        let mut response = client
            .get(request.url)
            .timeout(limits.timeout)
            .send()
            .await
            .map_err(|e| format!("Tải gói cập nhật thất bại: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Máy chủ cập nhật trả về lỗi: {e}"))?;
        let length = response.content_length();
        if length.is_some_and(|size| size > limits.max_bytes) {
            return Err("Gói cập nhật vượt giới hạn dung lượng".into());
        }
        if let (Some(expected), Some(actual)) = (request.expected_size, length) {
            if expected != actual {
                return Err("Dung lượng gói cập nhật không khớp metadata".into());
            }
        }
        let total = request.expected_size.or(length).unwrap_or(0);
        let mut bytes = Vec::new();
        let mut hasher = Sha256::new();
        let mut last_progress = std::time::Instant::now();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("Ngắt kết nối khi đang tải cập nhật: {e}"))?
        {
            if (chunk.len() as u64) > limits.max_bytes.saturating_sub(bytes.len() as u64) {
                return Err("Gói cập nhật vượt giới hạn dung lượng".into());
            }
            hasher.update(&chunk);
            bytes.extend_from_slice(&chunk);
            if last_progress.elapsed() >= Duration::from_millis(80) {
                progress(bytes.len() as u64, total);
                last_progress = std::time::Instant::now();
            }
        }
        let actual = bytes.len() as u64;
        if actual < limits.min_bytes {
            return Err("Gói cập nhật tải về quá nhỏ".into());
        }
        if length.is_some_and(|size| size != actual)
            || request.expected_size.is_some_and(|size| size != actual)
        {
            return Err("Gói cập nhật tải về chưa hoàn tất".into());
        }
        if let Some(expected) = request.expected_sha256 {
            let actual = format!("{:x}", hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected) {
                return Err("Mã kiểm tra toàn vẹn SHA-256 không khớp".into());
            }
        }
        let name = request.asset_name.to_ascii_lowercase();
        if name.ends_with(".exe") && !bytes.starts_with(b"MZ") {
            return Err("File cập nhật không có chữ ký PE hợp lệ".into());
        }
        if name.ends_with(".appimage") && !bytes.starts_with(b"\x7fELF") {
            return Err("File cập nhật không có chữ ký ELF hợp lệ".into());
        }
        progress(actual, total.max(actual));
        Ok(bytes)
    };
    tokio::time::timeout(limits.timeout, transfer)
        .await
        .map_err(|_| "Tải gói cập nhật quá thời gian cho phép".to_string())?
}

static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

fn staging_file(destination: &Path) -> Result<(PathBuf, File), String> {
    let parent = destination
        .parent()
        .ok_or("Đường dẫn cập nhật thiếu thư mục")?;
    for _ in 0..100 {
        let id = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".ctdo-update-{}-{id}", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Tạo file cập nhật tạm thất bại: {error}")),
        }
    }
    Err("Không tạo được file cập nhật tạm riêng biệt".into())
}

fn remove_staging(path: &Path) {
    // Cleanup must not hide the primary download/install failure.
    let _ = fs::remove_file(path);
}

/// Portable installation staged without replacing or launching the running executable.
/// Dropping an uncommitted transaction preserves the original executable.
pub struct DeferredPortableInstall {
    prepared: PreparedInstall,
}

impl DeferredPortableInstall {
    /// Replace the executable and launch only after the caller has joined its worker.
    ///
    /// # Errors
    /// Returns replacement, launch or explicit rollback errors.
    pub fn launch_and_finish(
        self,
        launch: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        self.prepared.finish(launch)
    }

    /// Discard the staged update and preserve the original executable.
    ///
    /// # Errors
    /// Returns a rollback error if replacement happened before cancellation.
    pub fn abort(mut self) -> Result<(), String> {
        self.prepared.rollback()
    }
}

/// Stage validated portable bytes without replacing or launching any executable.
///
/// # Errors
/// Returns filesystem errors while preparing private sibling files.
pub fn stage_portable_release(
    bytes: &[u8],
    destination: &Path,
) -> Result<DeferredPortableInstall, String> {
    Ok(DeferredPortableInstall {
        prepared: prepare_install(bytes, destination, true)?,
    })
}

struct PreparedInstall {
    destination: PathBuf,
    stage: PathBuf,
    backup: Option<PathBuf>,
    installed: bool,
    committed: bool,
}

impl PreparedInstall {
    fn finish(mut self, launch: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
        fs::rename(&self.stage, &self.destination)
            .map_err(|error| format!("Thay thế gói cập nhật thất bại: {error}"))?;
        self.installed = true;
        if let Err(error) = launch(&self.destination) {
            self.rollback().map_err(|rollback| {
                format!("Khởi chạy thất bại: {error}; hoàn tác thất bại: {rollback}")
            })?;
            return Err(format!("Khởi chạy thất bại, đã hoàn tác cập nhật: {error}"));
        }
        self.committed = true;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), String> {
        if self.installed && !self.committed {
            match &self.backup {
                Some(backup) => fs::rename(backup, &self.destination),
                None => fs::remove_file(&self.destination),
            }
            .map_err(|error| format!("Khôi phục gói cập nhật cũ thất bại: {error}"))?;
            self.installed = false;
        }
        Ok(())
    }
}

impl Drop for PreparedInstall {
    fn drop(&mut self) {
        // A retained backup is the recovery source if rollback itself fails.
        if self.rollback().is_err() {
            return;
        }
        remove_staging(&self.stage);
        if let Some(backup) = &self.backup {
            remove_staging(backup);
        }
    }
}

fn prepare_install(
    bytes: &[u8],
    destination: &Path,
    portable: bool,
) -> Result<PreparedInstall, String> {
    let metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_file() => Some(metadata),
        Ok(_) => return Err("Đích cập nhật không phải file thông thường".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !portable => None,
        Err(error) => return Err(format!("Đọc đích cập nhật thất bại: {error}")),
    };
    let (stage, mut file) = staging_file(destination)?;
    let prepared = (|| {
        file.write_all(bytes)?;
        if portable {
            if let Some(metadata) = &metadata {
                file.set_permissions(metadata.permissions())?;
            }
        }
        file.sync_all()
    })();
    drop(file);
    if let Err(error) = prepared {
        remove_staging(&stage);
        return Err(format!("Ghi file cập nhật thất bại: {error}"));
    }
    let backup = if metadata.is_some() {
        let (path, file) = match staging_file(destination) {
            Ok(value) => value,
            Err(error) => {
                remove_staging(&stage);
                return Err(error);
            }
        };
        drop(file);
        // A private sibling copy keeps the original destination present until atomic replacement.
        if let Err(error) = fs::copy(destination, &path) {
            remove_staging(&stage);
            remove_staging(&path);
            return Err(format!("Sao lưu bản cập nhật cũ thất bại: {error}"));
        }
        Some(path)
    } else {
        None
    };
    Ok(PreparedInstall {
        destination: destination.to_path_buf(),
        stage,
        backup,
        installed: false,
        committed: false,
    })
}

/// Atomically install already-validated bytes and invoke the injected launcher.
/// Existing bytes are restored if replacement or launch fails. A rollback failure
/// is returned explicitly and the backup is retained for recovery.
///
/// # Errors
/// Returns filesystem or launcher errors; never falls back to another destination.
pub fn install_release(
    bytes: &[u8],
    target: &InstallTarget,
    launch: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let (destination, portable) = match target {
        InstallTarget::Portable(path) => (path, true),
        InstallTarget::Installer(path) => (path, false),
    };
    prepare_install(bytes, destination, portable)?.finish(launch)
}

/// Metadata parsed from GitHub Releases REST API (`/repos/{owner}/{repo}/releases/latest`)
/// or the static `latest.json` release manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubReleaseMetadata {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_title: String,
    pub release_notes: String,
    pub html_url: String,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: u64,
    pub expected_sha256: Option<String>,
    pub published_at: String,
    pub sha256sums_url: Option<String>,
}

/// Compare semantic versions (`"v0.0.4"` > `"0.0.3"`).
pub fn is_newer_semver(remote: &str, current: &str) -> bool {
    let parse = |s: &str| -> (u64, u64, u64, bool) {
        let trimmed = s.trim().trim_start_matches(['v', 'V']);
        let (core, rest) = trimmed
            .split_once(['-', '+'])
            .map_or((trimmed, ""), |(c, r)| (c, r));
        let is_stable = !trimmed.contains('-') || rest.is_empty();
        let mut parts = core.split('.').filter_map(|p| p.trim().parse::<u64>().ok());
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            is_stable,
        )
    };
    parse(remote) > parse(current)
}

/// Score a release asset filename for a target `(os, arch)` pair.
/// Returns `<= 0` when the asset is incompatible with the platform.
pub fn select_platform_asset_score(name: &str, target_os: &str, target_arch: &str) -> i32 {
    let n_low = name.to_ascii_lowercase();
    if n_low.is_empty()
        || n_low.ends_with(".txt")
        || n_low.ends_with(".json")
        || n_low.ends_with(".sig")
        || n_low.ends_with(".asc")
    {
        return -1;
    }
    match target_os {
        "windows" => {
            if n_low.ends_with(".exe") && n_low.contains("portable") {
                110
            } else if n_low.ends_with(".exe") && n_low.contains("setup") {
                90
            } else if n_low.ends_with(".exe") {
                80
            } else {
                0
            }
        }
        "macos" => {
            if !n_low.ends_with(".dmg") {
                return 0;
            }
            if target_arch == "aarch64" && (n_low.contains("arm64") || n_low.contains("aarch64")) {
                110
            } else if target_arch == "x86_64"
                && (n_low.contains("x86_64") || n_low.contains("intel"))
            {
                110
            } else if !n_low.contains("arm64")
                && !n_low.contains("aarch64")
                && !n_low.contains("x86_64")
                && !n_low.contains("intel")
            {
                100
            } else {
                50
            }
        }
        _ => {
            if n_low.ends_with(".appimage") {
                100
            } else if n_low.ends_with(".deb") && !n_low.contains("legacy") {
                90
            } else if n_low.ends_with(".tar.gz") && !n_low.contains("legacy") {
                80
            } else if n_low.ends_with(".rpm") {
                70
            } else {
                0
            }
        }
    }
}

/// Extract a 64-character lowercase SHA-256 hex digest from `SHA256SUMS.txt` content for `asset_name`.
pub fn parse_sha256sums_for_asset(sha256sums_text: &str, asset_name: &str) -> Option<String> {
    for line in sha256sums_text.lines() {
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else { continue };
        let Some(fname) = parts.next() else { continue };
        let clean_fname = fname.trim_start_matches('*').trim_start_matches("./");
        if clean_fname == asset_name
            && hash.len() == 64
            && hash.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

/// Parse either a GitHub Releases API JSON object (`/releases/latest`) or a `latest.json`
/// manifest object into [`GitHubReleaseMetadata`].
pub fn parse_github_release_value(
    rel: &serde_json::Value,
    current_ver: &str,
    repo_slug: &str,
    target_os: &str,
    target_arch: &str,
) -> GitHubReleaseMetadata {
    let raw_tag = rel
        .get("tag_name")
        .or_else(|| rel.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or(current_ver);
    let latest_ver = raw_tag.trim().trim_start_matches(['v', 'V']).to_string();
    let has_update = is_newer_semver(&latest_ver, current_ver);

    let release_title = rel
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("ctdozlite v{latest_ver}"));

    let release_notes = rel
        .get("body")
        .or_else(|| rel.get("notes"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let html_url = rel
        .get("html_url")
        .or_else(|| rel.get("release_url"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://github.com/{repo_slug}/releases/tag/v{latest_ver}"));

    let published_at = rel
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .split('T')
        .next()
        .unwrap_or("")
        .to_string();

    let mut best_url: Option<String> = None;
    let mut best_name: Option<String> = None;
    let mut best_size: u64 = 0;
    let mut best_sha256: Option<String> = None;
    let mut best_score: i32 = 0;
    let mut sha256sums_url: Option<String> = None;

    if let Some(assets) = rel.get("assets").and_then(|v| v.as_array()) {
        for a in assets {
            let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let dl = a
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let sz = a.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
            if name.eq_ignore_ascii_case("SHA256SUMS.txt") && !dl.is_empty() {
                sha256sums_url = Some(dl.to_string());
                continue;
            }
            if name.is_empty() || dl.is_empty() {
                continue;
            }
            let score = select_platform_asset_score(name, target_os, target_arch);
            if score > best_score {
                best_score = score;
                best_url = Some(dl.to_string());
                best_name = Some(name.to_string());
                best_size = sz;
                // Extract SHA-256 from `latest.json` (`sha256`) or GitHub API asset (`digest: "sha256:..."`)
                best_sha256 = a
                    .get("sha256")
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        a.get("digest")
                            .and_then(|v| v.as_str())
                            .and_then(|d| d.strip_prefix("sha256:"))
                    })
                    .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
                    .map(|h| h.to_ascii_lowercase());
            }
        }
    }

    // Legacy single-asset `latest.json` fallback when `assets` array is absent
    if best_url.is_none() && rel.get("assets").is_none() {
        let fallback_name = match target_os {
            "windows" => format!("ctdozlite-{latest_ver}-portable-windows-x86_64.exe"),
            "macos" => match target_arch {
                "aarch64" => format!("ctdozlite-{latest_ver}-macos-arm64.dmg"),
                "x86_64" => format!("ctdozlite-{latest_ver}-macos-x86_64.dmg"),
                _ => format!("ctdozlite-{latest_ver}-macos.dmg"),
            },
            _ => format!("ctdozlite-{latest_ver}-x86_64.AppImage"),
        };
        best_url = Some(format!(
            "https://github.com/{repo_slug}/releases/download/v{latest_ver}/{fallback_name}"
        ));
        best_name = Some(fallback_name);
        best_sha256 = rel
            .get("sha256")
            .and_then(|v| v.as_str())
            .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(|h| h.to_ascii_lowercase());
    }

    GitHubReleaseMetadata {
        current_version: current_ver.to_string(),
        latest_version: latest_ver,
        has_update,
        release_title,
        release_notes,
        html_url,
        download_url: best_url,
        asset_name: best_name,
        asset_size: best_size,
        expected_sha256: best_sha256,
        published_at,
        sha256sums_url,
    }
}

/// The `.app` bundle this process runs from (`X.app/Contents/MacOS/<exe>`), if any.
#[cfg(target_os = "macos")]
pub fn current_app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let macos_dir = exe.parent()?;
    let contents = macos_dir.parent()?;
    let bundle = contents.parent()?;
    let is_bundle = macos_dir.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app";
    is_bundle.then(|| bundle.to_path_buf())
}

/// Detaches a mounted disk image when dropped, so every error path cleans up.
#[cfg(target_os = "macos")]
struct MountedDmg(PathBuf);

#[cfg(target_os = "macos")]
impl Drop for MountedDmg {
    fn drop(&mut self) {
        let _ = std::process::Command::new("/usr/bin/hdiutil")
            .args(["detach", "-quiet", "-force"])
            .arg(&self.0)
            .status();
    }
}

#[cfg(target_os = "macos")]
fn run_tool(program: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let status = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("Không chạy được {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} thất bại ({status})"))
    }
}

/// Replace the running `.app` bundle with the one inside a verified release `.dmg`.
///
/// The caller must have checked the DMG's SHA-256. Steps: mount read-only, verify the new app's
/// code signature, copy it next to the current bundle with `ditto`, then swap by renaming
/// (current → backup, new → current). Any failure before the swap leaves the installed app
/// untouched; a failed second rename restores the backup.
#[cfg(target_os = "macos")]
pub fn install_macos_app_from_dmg(dmg_bytes: &[u8], bundle: &Path) -> Result<(), String> {
    use std::ffi::OsStr;
    let parent = bundle
        .parent()
        .ok_or_else(|| "Không xác định được thư mục chứa ứng dụng".to_string())?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work =
        std::env::temp_dir().join(format!("ctdozlite-update-{}-{nonce}", std::process::id()));
    crate::data_dir::ensure_private_dir(&work).map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        let dmg = work.join("update.dmg");
        std::fs::write(&dmg, dmg_bytes).map_err(|e| format!("Không ghi được tệp DMG: {e}"))?;
        let mount = work.join("mnt");
        std::fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
        run_tool(
            "/usr/bin/hdiutil",
            &[
                OsStr::new("attach"),
                OsStr::new("-nobrowse"),
                OsStr::new("-noautoopen"),
                OsStr::new("-readonly"),
                OsStr::new("-mountpoint"),
                mount.as_os_str(),
                dmg.as_os_str(),
            ],
        )?;
        let _mounted = MountedDmg(mount.clone());
        let new_app = std::fs::read_dir(&mount)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.extension().is_some_and(|x| x == "app") && p.join("Contents/Info.plist").is_file()
            })
            .ok_or_else(|| "Không tìm thấy ứng dụng .app trong DMG".to_string())?;
        run_tool(
            "/usr/bin/codesign",
            &[
                OsStr::new("--verify"),
                OsStr::new("--deep"),
                OsStr::new("--strict"),
                new_app.as_os_str(),
            ],
        )
        .map_err(|e| format!("Chữ ký ứng dụng mới không hợp lệ: {e}"))?;

        let staging = parent.join(".ctdozlite-update-staging.app");
        let backup = parent.join(".ctdozlite-update-backup.app");
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_dir_all(&backup);
        run_tool(
            "/usr/bin/ditto",
            &[new_app.as_os_str(), staging.as_os_str()],
        )
        .map_err(|e| format!("Không chép được ứng dụng mới (thiếu quyền ghi?): {e}"))?;
        if let Err(e) = std::fs::rename(bundle, &backup) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("Không thay được ứng dụng hiện tại: {e}"));
        }
        if let Err(e) = std::fs::rename(&staging, bundle) {
            let _ = std::fs::rename(&backup, bundle);
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("Không cài được ứng dụng mới: {e}"));
        }
        let _ = std::fs::remove_dir_all(&backup);
        let _ = run_tool(
            "/usr/bin/xattr",
            &[
                OsStr::new("-dr"),
                OsStr::new("com.apple.quarantine"),
                bundle.as_os_str(),
            ],
        );
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// Re-open `bundle` once this process has exited (so two instances never share the data dir).
#[cfg(target_os = "macos")]
pub fn relaunch_app_bundle_after_exit(bundle: &Path) -> Result<(), String> {
    let pid = std::process::id().to_string();
    std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "while /bin/kill -0 \"$1\" 2>/dev/null; do /bin/sleep 0.2; done; /usr/bin/open \"$2\"",
            "ctdozlite-relaunch",
            &pid,
        ])
        .arg(bundle)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Không khởi động lại được ứng dụng: {e}"))
}

/// Title and body of the "new release available" OS notification.
pub fn new_update_notification(latest_version: &str, current_version: &str) -> (String, String) {
    (
        format!("{} — có bản cập nhật v{latest_version}", crate::APP_DISPLAY_NAME),
        format!(
            "Phiên bản mới v{latest_version} đã sẵn sàng (đang dùng v{current_version}). Mở Cài đặt để cập nhật."
        ),
    )
}

#[cfg(test)]
mod github_api_tests {
    use super::*;

    #[test]
    fn test_is_newer_semver_comparisons() {
        assert!(is_newer_semver("0.0.4", "0.0.3"));
        assert!(is_newer_semver("v0.0.4", "0.0.3"));
        assert!(is_newer_semver("v0.1.0", "v0.0.9"));
        assert!(!is_newer_semver("0.0.3", "0.0.3"));
        assert!(!is_newer_semver("0.0.2", "0.0.3"));
        assert!(is_newer_semver("0.0.4", "0.0.4-rc.1"));
    }

    #[test]
    fn test_parse_github_release_api_payload_with_native_digest() {
        let api_json = serde_json::json!({
            "tag_name": "v0.0.4",
            "name": "ctdozlite v0.0.4",
            "body": "- Sửa lỗi Windows Defender\n- Tự động thông báo cập nhật qua GitHub API",
            "html_url": "https://github.com/ctdoteam1110/ctdozlite/releases/tag/v0.0.4",
            "published_at": "2026-11-02T03:00:00Z",
            "assets": [
                {
                    "name": "SHA256SUMS.txt",
                    "size": 512,
                    "browser_download_url": "https://github.com/ctdoteam1110/ctdozlite/releases/download/v0.0.4/SHA256SUMS.txt"
                },
                {
                    "name": "ctdozlite-0.0.4-setup-windows-x86_64.exe",
                    "size": 9500000,
                    "digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                    "browser_download_url": "https://github.com/ctdoteam1110/ctdozlite/releases/download/v0.0.4/ctdozlite-0.0.4-setup-windows-x86_64.exe"
                },
                {
                    "name": "ctdozlite-0.0.4-portable-windows-x86_64.exe",
                    "size": 8400000,
                    "digest": "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
                    "browser_download_url": "https://github.com/ctdoteam1110/ctdozlite/releases/download/v0.0.4/ctdozlite-0.0.4-portable-windows-x86_64.exe"
                },
                {
                    "name": "ctdozlite-0.0.4-macos-arm64.dmg",
                    "size": 10200000,
                    "digest": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                    "browser_download_url": "https://github.com/ctdoteam1110/ctdozlite/releases/download/v0.0.4/ctdozlite-0.0.4-macos-arm64.dmg"
                }
            ]
        });

        let win_meta = parse_github_release_value(
            &api_json,
            "0.0.3",
            "ctdoteam1110/ctdozlite",
            "windows",
            "x86_64",
        );
        assert!(win_meta.has_update);
        assert_eq!(win_meta.latest_version, "0.0.4");
        assert_eq!(
            win_meta.asset_name.as_deref(),
            Some("ctdozlite-0.0.4-portable-windows-x86_64.exe")
        );
        assert_eq!(
            win_meta.expected_sha256.as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
        );
        assert_eq!(win_meta.published_at, "2026-11-02");

        let mac_meta = parse_github_release_value(
            &api_json,
            "0.0.3",
            "ctdoteam1110/ctdozlite",
            "macos",
            "aarch64",
        );
        assert_eq!(
            mac_meta.asset_name.as_deref(),
            Some("ctdozlite-0.0.4-macos-arm64.dmg")
        );
        assert_eq!(
            mac_meta.expected_sha256.as_deref(),
            Some("2222222222222222222222222222222222222222222222222222222222222222")
        );
    }

    #[test]
    fn test_parse_sha256sums_for_asset() {
        let sums = "\
abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789  ctdozlite-0.0.4-portable-windows-x86_64.exe\n\
2222222222222222222222222222222222222222222222222222222222222222 *./ctdozlite-0.0.4-macos-arm64.dmg\n";
        assert_eq!(
            parse_sha256sums_for_asset(sums, "ctdozlite-0.0.4-macos-arm64.dmg").as_deref(),
            Some("2222222222222222222222222222222222222222222222222222222222222222")
        );
    }
}
