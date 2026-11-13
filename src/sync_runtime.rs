//! Provision the public Sync V2 runtime locally; never redistribute it or attach credentials.
use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::atomic::Ordering, time::Duration};

const DIGEST: &str = "398a2016ede72d1e604a6ac13bc7c62f2c44a43f0c1d7b996e968427380fc739";
const LIMIT: usize = 2 * 1024 * 1024;
/// App-data-relative private cache, resolved after the app chooses its data directory.
pub(crate) fn cached_path() -> PathBuf {
    PathBuf::from(".sync-runtime/libzproto_wasm_bg.5ea0fb2c9af4a45bc7882b71307b6f83.wasm")
}
fn validate(bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() <= LIMIT && bytes.starts_with(b"\0asm"),
        "Gói đồng bộ không hợp lệ."
    );
    ensure!(
        format!("{:x}", Sha256::digest(bytes)) == DIGEST,
        "Gói đồng bộ không khớp SHA-256."
    );
    Ok(())
}
fn cancelled() -> bool {
    crate::history_sync::PHONE_SYNC_CANCEL.load(Ordering::Relaxed)
}

/// Prepare before sending any phone request; preserve explicit runtime overrides.
pub(crate) async fn ensure_ready(mut progress: impl FnMut(f32, String)) -> Result<()> {
    progress(0.03, "Đang kiểm tra công cụ đồng bộ điện thoại…".into());
    let ready = tokio::task::spawn_blocking(|| {
        if let Ok(override_path) = std::env::var("CTDOZLITE_ZPROTO_WASM") {
            ensure!(
                std::path::Path::new(&override_path).is_file(),
                "Đường dẫn công cụ đồng bộ đã cấu hình không tồn tại."
            );
            crate::zproto_host::ZprotoHost::load_from(std::path::Path::new(&override_path))?
                .generate_keypair()?;
            return Ok::<_, anyhow::Error>(true);
        }
        if let Some(path) = crate::zproto_host::find_wasm_path() {
            let valid = std::fs::metadata(&path)
                .map(|m| m.len() <= LIMIT as u64)
                .unwrap_or(false)
                && std::fs::read(&path).is_ok_and(|bytes| validate(&bytes).is_ok());
            if valid {
                crate::zproto_host::ZprotoHost::load_from(&path)?.generate_keypair()?;
                return Ok(true);
            }
            // Leave unrelated Zalo installations untouched; replace only our own invalid cache.
            if path == cached_path() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(false)
    })
    .await??;
    if ready {
        return Ok(());
    }
    ensure!(!cancelled(), "Đã dừng chuẩn bị đồng bộ.");
    progress(
        0.05,
        "Đang tải công cụ Sync V2 từ Zalo cho lần sử dụng đầu…".into(),
    );
    // A dedicated credential-free client prevents leaking Zalo cookies to public asset requests.
    let client = reqwest::Client::builder()
        .user_agent(concat!("ctdozlite-next/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(60))
        .build()?;
    let mut response = client
        .get(crate::endpoints::ZPROTO_RUNTIME)
        .header("Referer", crate::endpoints::ZPROTO_PUBLIC_ORIGIN)
        .send()
        .await?
        .error_for_status()?;
    ensure!(
        response.content_length().unwrap_or(0) <= LIMIT as u64,
        "Gói đồng bộ quá lớn."
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(!cancelled(), "Đã dừng chuẩn bị đồng bộ.");
        ensure!(
            bytes.len().saturating_add(chunk.len()) <= LIMIT,
            "Gói đồng bộ quá lớn."
        );
        bytes.extend_from_slice(&chunk);
    }
    tokio::task::spawn_blocking(move || -> Result<()> {
        activate(&bytes, &cached_path())?;
        Ok(())
    })
    .await??;
    Ok(())
}
fn activate(bytes: &[u8], path: &std::path::Path) -> Result<()> {
    validate(&bytes)?;
    ensure!(!cancelled(), "Đã dừng chuẩn bị đồng bộ.");
    crate::data_dir::ensure_private_dir(
        path.parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid runtime cache"))?,
    )?;
    crate::data_dir::write_private_atomic(&path, &bytes)?;
    if let Err(error) =
        crate::zproto_host::ZprotoHost::load_from(&path).and_then(|host| host.generate_keypair())
    {
        let _ = std::fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn runtime_rejects_invalid_and_modified_modules() {
        assert!(super::validate(b"not wasm").is_err());
        assert!(super::validate(b"\0asmcorrupted").is_err());
        if let Ok(path) = std::env::var("CTDOZLITE_ZPROTO_WASM") {
            let bytes = std::fs::read(path).unwrap();
            super::validate(&bytes).unwrap();
            let directory = crate::data_dir::PrivateTempDir::new("sync-runtime-fixture").unwrap();
            let destination = directory.path().join("runtime.wasm");
            super::activate(&bytes, &destination).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&destination)
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    0o600
                );
            }
            assert!(super::activate(b"corrupted", &destination).is_err());
            assert_eq!(std::fs::read(destination).unwrap(), bytes);
        }
    }
}
