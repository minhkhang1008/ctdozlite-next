//! Select writable storage before opening sessions, caches, logs, or databases.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

/// Resolve paths without changing process state, including in parallel tests.
///
/// # Errors
/// Returns an error for invalid storage options or a missing/relative bundle HOME.
pub fn resolve(
    args: &[String],
    cwd: &Path,
    executable: &Path,
    home: Option<&Path>,
    macos: bool,
) -> Result<(PathBuf, bool)> {
    let mut explicit = None;
    let mut profile_name = None;
    let mut i = 0;
    while i < args.len() {
        let value = if args[i] == "--data-dir" {
            i += 1;
            Some(
                args.get(i)
                    .filter(|value| !value.starts_with('-'))
                    .context("--data-dir yêu cầu đường dẫn")?
                    .as_str(),
            )
        } else {
            args[i].strip_prefix("--data-dir=")
        };
        if let Some(value) = value {
            if value.is_empty() {
                bail!("--data-dir yêu cầu đường dẫn không rỗng");
            }
            if explicit.is_some() {
                bail!("--data-dir chỉ được chỉ định một lần");
            }
            explicit = Some(PathBuf::from(value));
            i += 1;
            continue;
        }
        let prof_val = if args[i] == "--profile" {
            i += 1;
            Some(
                args.get(i)
                    .filter(|value| !value.starts_with('-'))
                    .context("--profile yêu cầu tên hồ sơ tài khoản")?
                    .as_str(),
            )
        } else {
            args[i].strip_prefix("--profile=")
        };
        if let Some(prof) = prof_val {
            let safe: String = prof
                .trim()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if safe.is_empty() {
                bail!("--profile chỉ chấp nhận ký tự chữ, số, '-' hoặc '_'");
            }
            profile_name = Some(safe);
        }
        i += 1;
    }
    if let Some(path) = explicit {
        return Ok((
            if path.is_absolute() || path.to_str().is_some_and(|s| s.starts_with('/')) {
                path
            } else {
                cwd.join(path)
            },
            true,
        ));
    }
    let bundled = executable
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "MacOS"))
        .and_then(Path::parent)
        .filter(|p| p.file_name().is_some_and(|n| n == "Contents"))
        .and_then(Path::parent)
        .is_some_and(|p| p.extension().is_some_and(|e| e == "app"));
    let (base, is_private) = if macos && bundled {
        let home = home.context("Cần HOME để xác định Application Support")?;
        if !home.is_absolute() && !home.to_str().is_some_and(|s| s.starts_with('/')) {
            bail!("HOME phải là đường dẫn tuyệt đối");
        }
        (home.join("Library/Application Support/ctdozlite"), true)
    } else {
        (cwd.to_path_buf(), false)
    };
    if let Some(prof) = profile_name {
        return Ok((base.join("profiles").join(prof), true));
    }
    Ok((base, is_private))
}

/// Validate storage before any persistent state is opened.
///
/// # Errors
/// Returns an error if creation, permissions, or the writable probe fails.
pub fn prepare(path: &Path, private: bool) -> Result<()> {
    if private {
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            let mut builder = std::fs::DirBuilder::new();
            builder
                .recursive(true)
                .mode(0o700)
                .create(path)
                .with_context(|| format!("Không tạo được thư mục dữ liệu {}", path.display()))?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).with_context(
                || format!("Không đặt được quyền thư mục dữ liệu {}", path.display()),
            )?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(path)
            .with_context(|| format!("Không tạo được thư mục dữ liệu {}", path.display()))?;
    }
    let (probe_path, probe) = create_private_file(path, ".storage-probe")
        .with_context(|| format!("Thư mục dữ liệu không ghi được: {}", path.display()))?;
    drop(probe);
    std::fs::remove_file(&probe_path).context("Không xóa được tệp kiểm tra quyền ghi")?;
    Ok(())
}

/// Select storage once before workers or persistent state are opened.
///
/// # Errors
/// Returns an error when process paths or writable storage cannot be selected.
pub fn initialize(args: &[String]) -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("Không xác định được thư mục làm việc")?;
    let executable =
        std::env::current_exe().context("Không xác định được đường dẫn chương trình")?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let (path, private) = resolve(
        args,
        &cwd,
        &executable,
        home.as_deref(),
        cfg!(target_os = "macos"),
    )?;
    prepare(&path, private)?;
    // Legacy storage callers use relative paths; switch once before creating workers.
    std::env::set_current_dir(&path)
        .with_context(|| format!("Không chọn được thư mục dữ liệu {}", path.display()))?;
    Ok(path)
}

fn create_private_file(parent: &Path, prefix: &str) -> std::io::Result<(PathBuf, std::fs::File)> {
    for _ in 0..100 {
        let path = parent.join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "Không tạo được tệp kiểm tra quyền ghi",
    ))
}

/// Create a directory if needed and enforce owner-only access on Unix.
pub fn ensure_private_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(0o700).create(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(path)?;
    Ok(())
}

/// Enforce owner read/write permissions for an existing file on Unix.
pub fn ensure_private_file(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Create or truncate a file with owner-only permissions and write its complete contents.
pub fn write_private_file(path: &Path, contents: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        bail!("Parent directory does not exist: {}", parent.display());
    }
    ensure_private_dir(parent)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    ensure_private_file(path)?;
    file.write_all(contents)?;
    file.sync_all()?;
    Ok(())
}

/// Copy a sensitive file without inheriting permissive destination permissions.
pub fn copy_private(source: &Path, destination: &Path) -> Result<u64> {
    use std::io::Write;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    ensure_private_dir(parent)?;
    let mut input = std::fs::File::open(source)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(destination)?;
    ensure_private_file(destination)?;
    let copied = std::io::copy(&mut input, &mut output)?;
    output.flush()?;
    output.sync_all()?;
    Ok(copied)
}

/// Owner-only temporary directory that removes all contents when dropped.
pub struct PrivateTempDir {
    path: PathBuf,
}

impl PrivateTempDir {
    /// Create a uniquely named private directory under the system temporary directory.
    pub fn new(prefix: &str) -> Result<Self> {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "{prefix}-{}-{}",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => {
                    ensure_private_dir(&path)?;
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        bail!("Cannot create a unique private temporary directory")
    }

    /// Return the private directory path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Replace a session only after the complete private temporary file is flushed.
///
/// # Errors
/// Returns an error if creating, writing, flushing, or replacing the file fails.
pub fn write_private_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        bail!("Session directory does not exist: {}", parent.display());
    }
    ensure_private_dir(parent)?;
    let (temporary, mut file) = create_private_file(parent, ".session-tmp")
        .with_context(|| format!("Cannot create temporary session in {}", parent.display()))?;
    let result = (|| -> Result<()> {
        file.write_all(contents).context("Cannot write session")?;
        file.sync_all().context("Cannot flush session")?;
        drop(file);
        std::fs::rename(&temporary, path)
            .with_context(|| format!("Cannot replace session {}", path.display()))?;
        ensure_private_file(path)?;
        Ok(())
    })();
    if result.is_err() {
        // Preserve the original failure if best-effort temporary cleanup also fails.
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_helpers_enforce_modes_and_cleanup_temp_data() {
        use std::os::unix::fs::PermissionsExt;
        let root = fixture();
        let private_dir = root.join("private");
        ensure_private_dir(&private_dir).unwrap();
        let file = private_dir.join("secret.bin");
        write_private_file(&file, b"secret").unwrap();
        assert_eq!(std::fs::metadata(&private_dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);

        let temp_path;
        {
            let temp = PrivateTempDir::new("ctdozlite-private-test").unwrap();
            temp_path = temp.path().to_path_buf();
            write_private_file(&temp_path.join("decrypted.db"), b"sqlite").unwrap();
            assert_eq!(std::fs::metadata(&temp_path).unwrap().permissions().mode() & 0o777, 0o700);
        }
        assert!(!temp_path.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    fn fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ctdozlite-storage-test-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn resolves_bundle_cli_and_explicit_paths() {
        let cwd = Path::new("/work");
        let exe = Path::new("/Applications/Zalo.app/Contents/MacOS/ctdozlite");
        assert_eq!(
            resolve(&[], cwd, exe, Some(Path::new("/Users/fixture")), true).unwrap(),
            (
                PathBuf::from("/Users/fixture/Library/Application Support/ctdozlite"),
                true
            )
        );
        assert_eq!(
            resolve(&[], cwd, Path::new("/bin/ctdozlite"), None, true).unwrap(),
            (cwd.to_path_buf(), false)
        );
        assert_eq!(
            resolve(&[], cwd, exe, None, false).unwrap(),
            (cwd.to_path_buf(), false)
        );
        assert_eq!(
            resolve(
                &["--data-dir".into(), "fixture".into()],
                cwd,
                exe,
                None,
                true
            )
            .unwrap(),
            (cwd.join("fixture"), true)
        );
        assert!(resolve(&[], cwd, exe, None, true).is_err());
        assert!(resolve(&["--data-dir".into()], cwd, exe, None, true).is_err());
        assert!(resolve(&["--data-dir=".into()], cwd, exe, None, true).is_err());
        assert!(resolve(
            &["--data-dir=a".into(), "--data-dir=b".into()],
            cwd,
            exe,
            None,
            true
        )
        .is_err());
        assert_eq!(
            resolve(
                &["--profile".into(), "acc_2".into()],
                cwd,
                Path::new("/bin/ctdozlite"),
                None,
                true
            )
            .unwrap(),
            (cwd.join("profiles").join("acc_2"), true)
        );
    }

    #[test]
    fn preparation_retains_existing_data_and_secures_directory() {
        let root = fixture();
        let path = root.join("storage");
        prepare(&path, true).unwrap();
        std::fs::write(path.join("session.json"), b"existing fixture").unwrap();
        prepare(&path, true).unwrap();
        assert_eq!(
            std::fs::read(path.join("session.json")).unwrap(),
            b"existing fixture"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 1);
        assert!(prepare(&path.join("session.json"), true).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn explicit_path_overrides_bundle_and_relative_home_is_rejected() {
        let cwd = Path::new("/work");
        let exe = Path::new("/Applications/Fixture.app/Contents/MacOS/ctdozlite");
        assert_eq!(
            resolve(&["--data-dir=/fixture-data".into()], cwd, exe, None, true).unwrap(),
            (PathBuf::from("/fixture-data"), true)
        );
        assert!(resolve(&[], cwd, exe, Some(Path::new("relative-home")), true).is_err());
        assert_eq!(
            resolve(
                &[],
                cwd,
                Path::new("/Fixture.app/Contents/MacOS-extra/ctdozlite"),
                None,
                true
            )
            .unwrap(),
            (cwd.to_path_buf(), false)
        );
    }

    #[test]
    fn legacy_preparation_keeps_existing_permissions_and_probe_files() {
        let root = fixture();
        let sentinel = root.join(format!(
            ".storage-probe-{}-{}",
            std::process::id(),
            NEXT_FILE.load(Ordering::Relaxed)
        ));
        std::fs::write(&sentinel, b"retained fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        prepare(&root, false).unwrap();
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"retained fixture");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn spaced_storage_option_rejects_flags_but_explicit_dash_paths_are_valid() {
        let cwd = Path::new("/work");
        let exe = Path::new("/bin/ctdozlite");
        for flag in ["-d", "-v", "-V", "--logout"] {
            assert!(
                resolve(&["--data-dir".into(), flag.into()], cwd, exe, None, true).is_err(),
                "accepted {flag}"
            );
        }
        for args in [
            vec!["--data-dir=-d".into()],
            vec!["--data-dir".into(), "./-d".into()],
        ] {
            let (path, private) = resolve(&args, cwd, exe, None, true).unwrap();
            assert_eq!(path, cwd.join("-d"));
            assert!(private);
        }
    }
    #[test]
    fn atomic_write_is_private_and_failed_replace_retains_destination() {
        let root = fixture();
        let path = root.join("session.json");
        write_private_atomic(&path, b"first fixture").unwrap();
        write_private_atomic(&path, b"second fixture").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let occupied = root.join("occupied");
        std::fs::create_dir(&occupied).unwrap();
        std::fs::write(occupied.join("keep"), b"retained").unwrap();
        assert!(write_private_atomic(&occupied, b"replacement").is_err());
        assert_eq!(std::fs::read(occupied.join("keep")).unwrap(), b"retained");
        assert!(write_private_atomic(&root.join("missing/session.json"), b"fixture").is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }
}
