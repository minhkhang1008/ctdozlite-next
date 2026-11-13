//! Discover FFmpeg and install video-call tools outside the rendering thread.
//!
//! macOS uses pinned, SHA-256-verified static executables in the app data directory;
//! Windows/Linux use the system package manager with fixed argument lists.

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Duration;

static FFMPEG: Mutex<Option<PathBuf>> = Mutex::new(None);
/// Set only while a worker is installing video tools.
pub static INSTALLING: AtomicBool = AtomicBool::new(false);
/// Cancellation is checked while downloading and before activating an executable.
pub static CANCEL_INSTALL: AtomicBool = AtomicBool::new(false);
const MAX_ARCHIVE_BYTES: usize = 128 * 1024 * 1024;
const MAX_EXECUTABLE_BYTES: usize = 192 * 1024 * 1024;
const ARM_SHA256: &str = "6deb2860c3dc0c08e0f96bab119196c024103046fa3cc7ef72e3a77797d8553c";
const INTEL_SHA256: &str = "a45b462cf91ed89148ae218c4577e30896485d7a6792c3673bcf5f823fa01b63";

/// Read cached readiness without launching processes or accessing disk.
pub fn available() -> bool {
    FFMPEG.lock().map(|p| p.is_some()).unwrap_or(false)
}

/// Build every media subprocess with the same discovered FFmpeg executable.
pub fn ffmpeg_command() -> Command {
    let path = FFMPEG
        .lock()
        .ok()
        .and_then(|p| p.clone())
        .unwrap_or_else(|| PathBuf::from("ffmpeg"));
    Command::new(path)
}

fn candidate_paths(data_dir: &Path) -> Vec<PathBuf> {
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let mut candidates = vec![data_dir.join("tools").join(name)];
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(
            std::env::split_paths(&paths)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(name)),
        );
    }
    // Finder does not inherit the user's interactive shell PATH.
    if cfg!(target_os = "macos") {
        candidates.extend(
            [
                "/opt/homebrew/bin/ffmpeg",
                "/usr/local/bin/ffmpeg",
                "/usr/bin/ffmpeg",
            ]
            .map(PathBuf::from),
        );
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("Microsoft/WinGet/Links/ffmpeg.exe"));
    }
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|path| seen.insert(path.clone()));
    candidates
}

async fn probe(path: &Path) -> bool {
    let mut cmd = tokio::process::Command::new(path);
    cmd.args(["-version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    matches!(tokio::time::timeout(Duration::from_secs(8), cmd.status()).await, Ok(Ok(status)) if status.success())
}

/// Re-detect tools after installation or a user-requested refresh.
pub async fn refresh(data_dir: &Path) -> bool {
    let mut found = None;
    for path in candidate_paths(data_dir) {
        if probe(&path).await {
            found = Some(path);
            break;
        }
    }
    if let Ok(mut cached) = FFMPEG.lock() {
        *cached = found;
    }
    available()
}

fn extract_verified(bytes: &[u8], expected_sha256: &str) -> Result<Vec<u8>> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        bail!("Gói FFmpeg vượt giới hạn tải.");
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    // Read exactly one known file; archive paths never become filesystem paths.
    let file = archive.by_name("ffmpeg")?;
    if file.size() > MAX_EXECUTABLE_BYTES as u64 {
        bail!("Tệp FFmpeg vượt giới hạn giải nén.");
    }
    let mut executable = Vec::new();
    file.take(MAX_EXECUTABLE_BYTES as u64 + 1)
        .read_to_end(&mut executable)?;
    if executable.len() > MAX_EXECUTABLE_BYTES {
        bail!("Tệp FFmpeg vượt giới hạn giải nén.");
    }
    let actual = format!("{:x}", Sha256::digest(&executable));
    if actual != expected_sha256 {
        bail!("SHA-256 của FFmpeg không khớp. Chưa cài đặt.");
    }
    Ok(executable)
}

fn check_cancelled() -> Result<()> {
    if CANCEL_INSTALL.load(Ordering::Relaxed) {
        bail!("Đã dừng cài công cụ gọi video.");
    }
    Ok(())
}

/// Install FFmpeg without the Zalo HTTP client or credentials.
///
/// # Errors
/// Returns an error for unsupported package managers, download/hash failures,
/// cancellation, or a non-working executable. Existing tools remain available.
pub async fn install<F>(data_dir: &Path, progress: F) -> Result<()>
where
    F: Fn(f32, String) + Send + Sync,
{
    if cfg!(target_os = "macos") {
        install_macos(data_dir, &progress).await?;
    } else {
        install_package(&progress).await?;
    }
    check_cancelled()?;
    progress(0.95, "Đang kiểm tra công cụ gọi video…".into());
    if !refresh(data_dir).await {
        bail!("Đã chạy trình cài nhưng chưa tìm thấy FFmpeg. Hãy bấm Kiểm tra lại.");
    }
    Ok(())
}

async fn cancellable_download<T>(
    future: impl std::future::Future<Output = reqwest::Result<T>>,
    cancel: &AtomicBool,
) -> Result<T> {
    tokio::select! {
        result = future => Ok(result?),
        _ = async {
            loop {
                if cancel.load(Ordering::Relaxed) { break; }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        } => anyhow::bail!("Đã dừng cài công cụ gọi video."),
    }
}

fn trusted_download_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url
            .host_str()
            .is_some_and(|host| crate::endpoints::FFMPEG_DOWNLOAD_HOSTS.contains(&host))
        && (url.port_or_known_default() == Some(443)
            || (url.host_str() == Some("e.deolaha.ca") && url.port() == Some(4242)))
}

async fn install_macos<F>(data_dir: &Path, progress: &F) -> Result<()>
where
    F: Fn(f32, String) + Send + Sync,
{
    let (url, hash) = if cfg!(target_arch = "aarch64") {
        (crate::endpoints::FFMPEG_MAC_ARM64, ARM_SHA256)
    } else if cfg!(target_arch = "x86_64") {
        (crate::endpoints::FFMPEG_MAC_INTEL, INTEL_SHA256)
    } else {
        bail!("Chưa hỗ trợ cài FFmpeg cho kiến trúc này.");
    };
    let http = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 3 && trusted_download_url(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("Untrusted video tools redirect")
            }
        }))
        .user_agent(concat!("ctdozlite-next/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(180))
        .build()?;
    progress(0.05, "Đang tải công cụ gọi video…".into());
    let mut response = cancellable_download(http.get(url).send(), &CANCEL_INSTALL)
        .await?
        .error_for_status()?;
    let total = response
        .content_length()
        .unwrap_or(MAX_ARCHIVE_BYTES as u64);
    if total > MAX_ARCHIVE_BYTES as u64 {
        bail!("Gói FFmpeg vượt giới hạn tải.");
    }
    let mut bytes = Vec::new();
    let mut last_progress = std::time::Instant::now();
    while let Some(chunk) = cancellable_download(response.chunk(), &CANCEL_INSTALL).await? {
        check_cancelled()?;
        if bytes.len().saturating_add(chunk.len()) > MAX_ARCHIVE_BYTES {
            bail!("Gói FFmpeg vượt giới hạn tải.");
        }
        bytes.extend_from_slice(&chunk);
        if last_progress.elapsed() >= Duration::from_millis(150) {
            progress(
                0.05 + 0.65 * (bytes.len() as f32 / total.max(1) as f32).min(1.0),
                format!(
                    "Đang tải công cụ gọi video: {} MB…",
                    bytes.len() / (1024 * 1024)
                ),
            );
            last_progress = std::time::Instant::now();
        }
    }
    progress(0.75, "Đang xác minh và cài FFmpeg…".into());
    install_archive(data_dir, bytes, hash).await
}

async fn install_archive(data_dir: &Path, bytes: Vec<u8>, hash: &str) -> Result<()> {
    let tools = data_dir.join("tools");
    let staging = tools.join(".ffmpeg-install");
    let stage_write = staging.clone();
    let hash = hash.to_string();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let executable = extract_verified(&bytes, &hash)?;
        crate::data_dir::ensure_private_dir(&tools)?;
        crate::data_dir::write_private_atomic(&stage_write, &executable)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stage_write, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    })
    .await??;
    let result = async {
        check_cancelled()?;
        if !probe(&staging).await {
            bail!("FFmpeg tải về chưa chạy được trên máy này.");
        }
        check_cancelled()?;
        tokio::fs::rename(&staging, data_dir.join("tools/ffmpeg")).await?;
        Ok(())
    }
    .await;
    // Only our own staging file is removed, including after failed validation.
    if result.is_err() {
        let _ = tokio::fs::remove_file(staging).await;
    }
    result
}

async fn install_package<F>(progress: &F) -> Result<()>
where
    F: Fn(f32, String) + Send + Sync,
{
    let (program, args): (&str, &[&str]) = if cfg!(windows) {
        (
            "winget",
            &[
                "install",
                "--id",
                "Gyan.FFmpeg",
                "--exact",
                "--source",
                "winget",
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
        )
    } else if cfg!(target_os = "linux") {
        ("pkexec", &["/usr/bin/apt-get", "install", "-y", "ffmpeg"])
    } else {
        bail!("Chưa hỗ trợ cài FFmpeg tự động trên hệ điều hành này.");
    };
    progress(
        0.1,
        "Đang cài FFmpeg qua trình quản lý gói; hệ điều hành có thể yêu cầu quyền cài đặt…".into(),
    );
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd
        .spawn()
        .context("Không mở được trình quản lý gói FFmpeg")?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
    loop {
        if CANCEL_INSTALL.load(Ordering::Relaxed) || tokio::time::Instant::now() >= deadline {
            let _ = child.kill().await;
            bail!("Đã dừng cài đặt. Trình quản lý gói có thể cần hoàn tất thao tác đang chạy.");
        }
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                bail!(
                    "Chưa cài được FFmpeg (mã {}). Kiểm tra quyền cài đặt và kết nối rồi thử lại.",
                    status.code().unwrap_or(-1)
                );
            }
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive(name: &str, data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data).unwrap();
        zip.finish().unwrap().into_inner()
    }
    #[tokio::test]
    async fn cancel_stops_a_download_even_when_no_response_arrives() {
        let cancel = AtomicBool::new(true);
        let pending = std::future::pending::<reqwest::Result<()>>();
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            cancellable_download(pending, &cancel),
        )
        .await;
        assert!(result.unwrap().is_err());
    }

    #[test]
    fn download_redirects_only_follow_exact_https_publishers() {
        for url in [
            "https://evermeet.cx/ffmpeg/a.zip",
            "https://e.deolaha.ca:4242/pub/ffmpeg/a.zip",
        ] {
            assert!(trusted_download_url(&reqwest::Url::parse(url).unwrap()));
        }
        for url in [
            "http://evermeet.cx/a.zip",
            "https://evermeet.cx.evil.test/a.zip",
            "https://127.0.0.1/a.zip",
            "https://evermeet.cx:8000/a.zip",
        ] {
            assert!(!trusted_download_url(&reqwest::Url::parse(url).unwrap()));
        }
    }

    #[test]
    fn archive_checks_hash_and_never_extracts_paths() {
        let data = b"fake binary for offline tests";
        let hash = format!("{:x}", Sha256::digest(data));
        assert_eq!(
            extract_verified(&archive("ffmpeg", data), &hash).unwrap(),
            data
        );
        assert!(extract_verified(&archive("ffmpeg", data), &"0".repeat(64)).is_err());
        assert!(extract_verified(&archive("../ffmpeg", data), &hash).is_err());
        assert!(extract_verified(b"not a zip", &hash).is_err());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn installation_verifies_before_activation_and_preserves_existing_binary_on_error() {
        use std::os::unix::fs::PermissionsExt;
        let root = crate::data_dir::PrivateTempDir::new("video-install-test").unwrap();
        let fake = b"#!/bin/sh\nexit 0\n";
        let hash = format!("{:x}", Sha256::digest(fake));
        // Hash is constant test data; use an owned lifetime for this asynchronous boundary.
        let hash: &str = Box::leak(hash.into_boxed_str());
        install_archive(root.path(), archive("ffmpeg", fake), &hash)
            .await
            .unwrap();
        let installed = root.path().join("tools/ffmpeg");
        assert!(probe(&installed).await);
        assert_eq!(
            std::fs::metadata(&installed).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(
            install_archive(root.path(), archive("ffmpeg", b"bad binary"), &hash)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&installed).unwrap(), fake);
    }

    #[test]
    fn finder_discovery_includes_local_install_and_homebrew() {
        let paths = candidate_paths(Path::new("/tmp/app-data"));
        let name = if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        };
        assert_eq!(paths[0], Path::new("/tmp/app-data/tools").join(name));
        if cfg!(target_os = "macos") {
            assert!(paths.contains(&PathBuf::from("/opt/homebrew/bin/ffmpeg")));
        }
    }
}
