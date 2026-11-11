//! Process-level storage checks without touching a real session or HOME.
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ctdozlite-cli-test-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        let canon = path.canonicalize().unwrap();
        #[cfg(windows)]
        let canon = PathBuf::from(
            canon
                .to_str()
                .unwrap_or_default()
                .strip_prefix(r"\\?\")
                .unwrap_or(canon.to_str().unwrap_or_default()),
        );
        Self(canon)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn explicit_storage_is_stable_and_invalid_directory_fails_before_network() {
    let fixture = Fixture::new();
    let data = fixture.0.join("explicit-data");
    let result = Command::new(env!("CARGO_BIN_EXE_ctdozlite"))
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--print-data-dir",
            "--logout",
        ])
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        data.to_str().unwrap()
    );
    std::fs::write(data.join("keep.fixture"), b"retained fixture").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ctdozlite"))
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--print-data-dir",
            "--logout",
        ])
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(
        std::fs::read(data.join("keep.fixture")).unwrap(),
        b"retained fixture"
    );
    let bad = Command::new(env!("CARGO_BIN_EXE_ctdozlite"))
        .args([
            "--data-dir",
            data.join("keep.fixture").to_str().unwrap(),
            "--print-data-dir",
            "--logout",
        ])
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(bad.stdout.is_empty());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("thư mục dữ liệu"));
    assert_eq!(
        std::fs::read(data.join("keep.fixture")).unwrap(),
        b"retained fixture"
    );
}

#[test]
#[cfg(target_os = "macos")]
fn finder_bundle_with_root_cwd_selects_private_application_support() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let bundle = fixture.0.join("Fixture.app/Contents/MacOS");
    std::fs::create_dir_all(&bundle).unwrap();
    let executable = bundle.join("ctdozlite");
    std::fs::copy(env!("CARGO_BIN_EXE_ctdozlite"), &executable).unwrap();
    let home = fixture.0.join("home");
    std::fs::create_dir(&home).unwrap();
    let expected = home.join("Library/Application Support/ctdozlite");
    for _ in 0..2 {
        let result = Command::new(&executable)
            .args(["--print-data-dir", "--logout"])
            .env("HOME", &home)
            .current_dir("/")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout).trim(),
            expected.to_str().unwrap()
        );
        assert_eq!(
            std::fs::metadata(&expected).unwrap().permissions().mode() & 0o777,
            0o700
        );
        std::fs::write(expected.join("session.fixture"), b"retained fixture").unwrap();
    }
    assert_eq!(
        std::fs::read(expected.join("session.fixture")).unwrap(),
        b"retained fixture"
    );
    assert!(!std::fs::read_dir(&expected).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("storage-probe")));
}

#[test]
fn legacy_cli_keeps_cwd_and_version_does_not_prepare_storage() {
    eprintln!(
        "CARGO_BIN_EXE_ctdozlite={}",
        env!("CARGO_BIN_EXE_ctdozlite")
    );
    let fixture = Fixture::new();
    let result = Command::new(env!("CARGO_BIN_EXE_ctdozlite"))
        .args(["--print-data-dir", "--logout"])
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        fixture.0.to_str().unwrap()
    );
    let data = fixture.0.join("not-created");
    let result = Command::new(env!("CARGO_BIN_EXE_ctdozlite"))
        .args(["--version", "--data-dir", data.to_str().unwrap()])
        .env_remove("HOME")
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        format!("ctdozlite-next v{}", env!("CARGO_PKG_VERSION"))
    );
    assert!(!data.exists());
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
}

#[test]
fn malformed_storage_options_fail_before_startup() {
    let fixture = Fixture::new();
    for args in [
        vec!["--data-dir"],
        vec!["--data-dir="],
        vec!["--data-dir=a", "--data-dir=b"],
        vec!["--data-dir", "--logout"],
        vec!["--data-dir", "-d"],
        vec!["--data-dir", "-v"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_ctdozlite"))
            .args(&args)
            .arg("--logout")
            .env_remove("HOME")
            .current_dir(&fixture.0)
            .output()
            .unwrap();
        assert!(!result.status.success(), "accepted {args:?}");
        assert!(String::from_utf8_lossy(&result.stderr).contains("--data-dir"));
    }
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
}

#[test]
#[cfg(target_os = "macos")]
fn bundled_version_does_not_create_application_support_or_require_home() {
    let fixture = Fixture::new();
    let bundle = fixture.0.join("Fixture.app/Contents/MacOS");
    std::fs::create_dir_all(&bundle).unwrap();
    let executable = bundle.join("ctdozlite");
    std::fs::copy(env!("CARGO_BIN_EXE_ctdozlite"), &executable).unwrap();
    let home = fixture.0.join("home-not-created");
    let result = Command::new(&executable)
        .arg("--version")
        .env("HOME", &home)
        .current_dir("/")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(!home.exists(), "--version created Application Support");
    let result = Command::new(&executable)
        .arg("--version")
        .env_remove("HOME")
        .current_dir("/")
        .output()
        .unwrap();
    assert!(result.status.success());
}

#[test]
fn relative_storage_is_private_and_unrelated_cwd_state_is_untouched() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("session.json"), b"existing fixture").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ctdozlite"))
        .args(["--data-dir=relative-data", "--print-data-dir", "--logout"])
        .env_remove("HOME")
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        fixture.0.join("relative-data").to_str().unwrap()
    );
    assert_eq!(
        std::fs::read(fixture.0.join("session.json")).unwrap(),
        b"existing fixture"
    );
    assert_eq!(
        std::fs::read_dir(fixture.0.join("relative-data"))
            .unwrap()
            .count(),
        0
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(fixture.0.join("relative-data"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
