//! Android backup request lifecycle, bounded decoding and transactional persistence.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::{
    clear_sync_keys, decompress_xz_limited, generate_sync_public_key, get_sync_keys,
    last_mobile_sync_status, last_phone_sync_complete, read_response_limited,
    set_last_mobile_sync_status, DB_PATH, MAX_DECOMPRESSED_SYNC_BYTES, MAX_SYNC_DOWNLOAD_BYTES,
    PHONE_COMPLETE, PHONE_IMPORTED, PHONE_SYNC_CANCEL, V2_BATCH_LOCK,
};
use crate::{chat, session::Session, store::MessageStore};
use anyhow::Result;

// Kept separate from the user-cancel flag so timeout remains an error in the UI.
static ANDROID_ABORT: AtomicBool = AtomicBool::new(false);
fn stopped() -> bool {
    PHONE_SYNC_CANCEL.load(Ordering::Relaxed) || ANDROID_ABORT.load(Ordering::Relaxed)
}

// The manual worker owns the transfer; socket/poll tasks only deliver one result.
struct AndroidTransfer {
    account: String,
    claimed: bool,
    result: Option<std::result::Result<usize, String>>,
}
impl AndroidTransfer {
    fn control(&mut self, account: &str, event: &chat::MobileSyncControlEvent) {
        if self.account != account || self.claimed || self.result.is_some() {
            return;
        }
        let error = match event {
            chat::MobileSyncControlEvent::UserRejected => {
                Some("Bạn đã từ chối yêu cầu đồng bộ trên điện thoại.")
            }
            chat::MobileSyncControlEvent::MobileBusyRestoring => {
                Some("Điện thoại đang bận khôi phục dữ liệu. Hãy thử lại sau.")
            }
            _ => None,
        };
        if let Some(error) = error {
            self.result = Some(Err(error.into()));
        }
    }
    fn claim(&mut self, account: &str) -> bool {
        if self.account != account || self.claimed || self.result.is_some() {
            return false;
        }
        self.claimed = true;
        true
    }
}
static ANDROID_TRANSFER: std::sync::Mutex<Option<AndroidTransfer>> = std::sync::Mutex::new(None);

/// Deliver typed control failures from either transport to the active Android request.
pub(crate) fn record_android_control(s: &Session, event: &chat::MobileSyncControlEvent) {
    let account = s.login.as_ref().map(|l| l.uin.as_str()).unwrap_or_default();
    if let Ok(mut state) = ANDROID_TRANSFER.lock() {
        if let Some(state) = state.as_mut() {
            state.control(account, event);
        }
    }
}

struct AndroidLease;
impl Drop for AndroidLease {
    fn drop(&mut self) {
        if let Ok(mut state) = ANDROID_TRANSFER.lock() {
            *state = None;
        }
        clear_sync_keys();
    }
}

/// Accept a backup only for the active manual Android request, once per request.
pub(crate) async fn receive_android_backup(s: &Session, url: &str, encrypted_key: &str) {
    let _decode = V2_BATCH_LOCK.lock().await;
    if stopped() {
        return;
    }
    let account = s.login.as_ref().map(|l| l.uin.as_str()).unwrap_or_default();
    let active = ANDROID_TRANSFER
        .lock()
        .map(|guard| {
            guard.as_ref().is_some_and(|state| {
                state.account == account && !state.claimed && state.result.is_none()
            })
        })
        .unwrap_or(false);
    if !active {
        return;
    }
    // A delayed backup from a previous request cannot fail the new request.
    let Some((private_key, _)) = get_sync_keys() else {
        return;
    };
    let encrypted = encrypted_key.to_owned();
    let matches_key = tokio::task::spawn_blocking(move || {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(encrypted)
            .ok()
            .and_then(|bytes| private_key.decrypt(rsa::Pkcs1v15Encrypt, &bytes).ok())
            .is_some_and(|key| key.len() >= 16)
    })
    .await
    .unwrap_or(false);
    if !matches_key || stopped() {
        return;
    }
    let claimed = ANDROID_TRANSFER
        .lock()
        .map(|mut guard| {
            if let Some(state) = guard.as_mut() {
                return state.claim(account);
            }
            false
        })
        .unwrap_or(false);
    if !claimed {
        return;
    }
    set_last_mobile_sync_status(Some(
        "Đang tải và lưu lịch sử từ điện thoại Android…".into(),
    ));
    let result = import_mobile_backup_container(s, url, encrypted_key).await;
    if let Ok(mut guard) = ANDROID_TRANSFER.lock() {
        if let Some(state) = guard.as_mut().filter(|state| state.account == account) {
            state.result = Some(match result {
                Ok(count) => {
                    PHONE_IMPORTED.store(count, Ordering::Relaxed);
                    PHONE_COMPLETE.store(true, Ordering::Relaxed);
                    Ok(count)
                }
                Err(error) => Err(android_failure_message(&error)),
            });
        }
    }
}

pub(super) async fn sync<F: FnMut(f32, String)>(s: &Session, on_progress: &mut F) -> Result<()> {
    ANDROID_ABORT.store(false, Ordering::Relaxed);
    let mut request_session = s.clone();
    request_session.computer_name = chat::sync_device_name(s).await;
    if stopped() {
        return Ok(());
    }
    let public_key = tokio::task::spawn_blocking(generate_sync_public_key).await?;
    anyhow::ensure!(
        !public_key.is_empty(),
        "Không thể tạo khóa đồng bộ Android."
    );
    let _lease = AndroidLease;
    {
        let account = s
            .login
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("chưa đăng nhập"))?
            .uin
            .clone();
        let mut state = ANDROID_TRANSFER
            .lock()
            .map_err(|_| anyhow::anyhow!("Không thể bắt đầu đồng bộ Android."))?;
        *state = Some(AndroidTransfer {
            account,
            claimed: false,
            result: None,
        });
    }
    if stopped() {
        return Ok(());
    }
    set_last_mobile_sync_status(None);
    on_progress(0.1, "Đang gửi yêu cầu đồng bộ Android…".into());
    let response =
        match chat::request_phone_sync_with_temp_key(&request_session, &public_key, "").await {
            Ok(response) => response,
            Err(error) => {
                let _ = chat::cancel_phone_sync(&request_session, &public_key).await;
                return Err(error);
            }
        };
    match response
        .get("error_code")
        .and_then(serde_json::Value::as_i64)
    {
        Some(0) => {}
        Some(code) => {
            let _ = chat::cancel_phone_sync(&request_session, &public_key).await;
            return Err(
                crate::session::SessionError::new(code, "Android sync request rejected").into(),
            );
        }
        None => {
            let _ = chat::cancel_phone_sync(&request_session, &public_key).await;
            anyhow::bail!("Phản hồi yêu cầu đồng bộ Android không hợp lệ.");
        }
    }
    on_progress(
        0.2,
        "Đã gửi yêu cầu. Hãy mở Zalo trên Android và xác nhận đồng bộ nếu được hỏi…".into(),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(210);
    let outcome = loop {
        if stopped() {
            break Ok(());
        }
        let result = ANDROID_TRANSFER
            .lock()
            .ok()
            .and_then(|state| state.as_ref().and_then(|state| state.result.clone()));
        if let Some(result) = result {
            break result.map(|_| ()).map_err(anyhow::Error::msg);
        }
        if let Some(note) = last_mobile_sync_status() {
            on_progress(0.45, note);
        }
        if std::time::Instant::now() >= deadline {
            break Err(anyhow::anyhow!("Chưa nhận được lịch sử từ Android. Hãy mở Zalo trên điện thoại và thử lại; tin nhắn cũ chưa được đồng bộ."));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    if outcome.is_err() || PHONE_SYNC_CANCEL.load(Ordering::Relaxed) {
        ANDROID_ABORT.store(true, Ordering::Relaxed);
    }
    // Account changes and retries wait until a decoder has stopped writing.
    let _decode = V2_BATCH_LOCK.lock().await;
    if !last_phone_sync_complete() {
        let _ = chat::cancel_phone_sync(&request_session, &public_key).await;
    }
    outcome
}

/// Download, decrypt, decompress and ingest a mobile backup container (.db.crypt)
/// into `zalo_history.db`. The manual worker hydrates the final UI snapshot.
pub async fn import_mobile_backup_container(
    s: &Session,
    url: &str,
    encrypted_key: &str,
) -> Result<usize> {
    use base64::Engine;
    anyhow::ensure!(
        crate::session::is_trusted_zalo_media_url(url),
        AndroidStage::Url
    );
    let (private_key, public_key) = get_sync_keys().ok_or(AndroidStage::Key)?;
    let encrypted_key = base64::engine::general_purpose::STANDARD
        .decode(encrypted_key)
        .map_err(|_| AndroidStage::Key)?;
    // Credentials only go to the core Zalo domains; media CDNs get a cookie-free client.
    let request = if crate::session::is_trusted_zalo_url(url) {
        s.http.get(url).header(
            "Cookie",
            format!(
                "zpw_sek={}; zpw_uin={}",
                s.zpw_sek.as_deref().unwrap_or_default(),
                s.login.as_ref().map(|l| l.uin.as_str()).unwrap_or_default()
            ),
        )
    } else {
        crate::media_resources::credential_free_client()
            .map_err(|_| AndroidStage::Download(None))?
            .get(url)
    };
    let started = std::time::Instant::now();
    let response = request
        .header("User-Agent", "ZaloPC-win32-26.10.11")
        .send()
        .await
        .map_err(|_| AndroidStage::Download(None))?;
    let status = response.status();
    // Dev panel shows host and path only: the query may carry a signed download token.
    crate::logger::log_request(
        "GET",
        &redacted_download_target(url),
        Some(status.as_u16()),
        started.elapsed().as_millis(),
        "android backup download",
        None,
        None,
    );
    if !status.is_success() {
        return Err(AndroidStage::Download(Some(status.as_u16())).into());
    }
    let encrypted = read_response_limited(response, MAX_SYNC_DOWNLOAD_BYTES)
        .await
        .map_err(|_| AndroidStage::Download(None))?;
    let rows = tokio::task::spawn_blocking(move || {
        let key = private_key
            .decrypt(rsa::Pkcs1v15Encrypt, &encrypted_key)
            .map_err(|_| AndroidStage::Key)?;
        decode_android_backup(&encrypted, &key).map_err(|_| AndroidStage::Decode)
    })
    .await
    .map_err(|_| AndroidStage::Decode)??;
    let mut users = std::collections::BTreeSet::new();
    let mut groups = std::collections::BTreeSet::new();
    for row in &rows {
        if row.group {
            groups.insert(row.thread);
        } else {
            users.insert(row.thread);
        }
        if row.sender > 0 {
            users.insert(row.sender);
        }
    }
    let mut mapping = std::collections::HashMap::new();
    // Include senders as well as conversation IDs: sender numbers are not account UIDs.
    for (group, ids) in [
        (false, users.into_iter().collect::<Vec<_>>()),
        (true, groups.into_iter().collect::<Vec<_>>()),
    ] {
        for chunk in ids.chunks(100) {
            anyhow::ensure!(!stopped(), "Đã dừng đồng bộ.");
            let body = if group {
                serde_json::json!({"fids": [], "gids": chunk})
            } else {
                serde_json::json!({"fids": chunk, "gids": []})
            };
            let params = s.encoded_params(&body).map_err(|_| AndroidStage::Mapping)?;
            let path = format!("/api/znoise?{}&params={params}", s.common_params());
            let response = s
                .get_endpoint("profile", &path)
                .await
                .map_err(|_| AndroidStage::Mapping)?;
            let decoded = s.maybe_decrypt(response, s.session_key_bytes()).await;
            let key = if group { "gids" } else { "fids" };
            let values = decoded
                .get("data")
                .and_then(|d| d.get(key))
                .and_then(serde_json::Value::as_array)
                .ok_or(AndroidStage::Mapping)?;
            anyhow::ensure!(values.len() == chunk.len(), AndroidStage::Mapping);
            for (id, value) in chunk.iter().zip(values) {
                let mapped = value
                    .as_str()
                    .filter(|id| !id.is_empty() && *id != "0")
                    .ok_or(AndroidStage::Mapping)?;
                mapping.insert((group, *id), mapped.to_owned());
            }
        }
    }
    let login = s
        .login
        .clone()
        .ok_or_else(|| anyhow::anyhow!("chưa đăng nhập"))?;
    let count = tokio::task::spawn_blocking(move || -> Result<usize> {
        let mut store = MessageStore::open(DB_PATH)?;
        persist_android_rows(
            &mut store,
            rows,
            &mapping,
            &login.uin,
            &login.send2me_id,
            &login.display_name,
        )
        .map_err(|_| AndroidStage::Store.into())
    })
    .await
    .map_err(|_| AndroidStage::Store)??;
    // Acknowledgement follows durable commit; failure must not erase the import count.
    let _ = chat::clean_sync(s, &public_key).await;
    Ok(count)
}

/// `host/path` of a download URL without query, fragment or credentials.
fn redacted_download_target(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .host_str()
                .map(|host| format!("{host}{}", parsed.path()))
        })
        .unwrap_or_else(|| "<invalid url>".into())
}

/// Failing step of an Android backup import; only these fixed labels reach the user so request
/// URLs, keys and message content never leak into status text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AndroidStage {
    Url,
    Key,
    Download(Option<u16>),
    Decode,
    Mapping,
    Store,
}

impl std::fmt::Display for AndroidStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Url => f.write_str("địa chỉ bản sao lưu không thuộc nguồn Zalo được tin cậy"),
            Self::Key => f.write_str("không giải mã được khóa chuyển dữ liệu"),
            Self::Download(Some(code)) => {
                write!(f, "máy chủ từ chối tải bản sao lưu (HTTP {code})")
            }
            Self::Download(None) => f.write_str("không tải được bản sao lưu"),
            Self::Decode => f.write_str("không giải mã hoặc đọc được định dạng bản sao lưu"),
            Self::Mapping => f.write_str("không ánh xạ được ID hội thoại với tài khoản này"),
            Self::Store => f.write_str("không ghi được vào cơ sở dữ liệu tin nhắn"),
        }
    }
}

impl std::error::Error for AndroidStage {}

fn android_failure_message(error: &anyhow::Error) -> String {
    match error.downcast_ref::<AndroidStage>() {
        Some(stage) => {
            format!("Đồng bộ Android thất bại ở bước: {stage}. Chưa hoàn tất đồng bộ điện thoại.")
        }
        None => {
            "Không thể tải hoặc nhập bản sao lưu Android. Chưa hoàn tất đồng bộ điện thoại.".into()
        }
    }
}

#[derive(Clone)]
struct AndroidRow {
    thread: i64,
    group: bool,
    sender: i64,
    client_id: i64,
    global_id: i64,
    kind: i64,
    timestamp: i64,
    content: String,
}

fn decode_android_backup(encrypted: &[u8], raw_key: &[u8]) -> Result<Vec<AndroidRow>> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit};
    anyhow::ensure!(raw_key.len() >= 16, "Android backup key is too short");
    anyhow::ensure!(
        encrypted.len() >= 48 && encrypted.len() <= MAX_SYNC_DOWNLOAD_BYTES,
        "Invalid Android backup size"
    );
    let hex: String = raw_key[..16].iter().map(|b| format!("{b:02X}")).collect();
    let mut decrypted = Vec::with_capacity(encrypted.len());
    for chunk in encrypted.chunks(65536) {
        let aligned = chunk.len() / 16 * 16;
        let mut block = chunk[..aligned].to_vec();
        cbc::Decryptor::<aes::Aes256>::new(hex.as_bytes().into(), (&[0u8; 16]).into())
            .decrypt_padded_mut::<cipher::block_padding::NoPadding>(&mut block)
            .map_err(|_| anyhow::anyhow!("Android backup decryption failed"))?;
        decrypted.extend_from_slice(&block);
        decrypted.extend_from_slice(&chunk[aligned..]);
    }
    parse_android_container(&decrypted)
}

fn parse_android_container(decrypted: &[u8]) -> Result<Vec<AndroidRow>> {
    fn take<'a>(bytes: &'a [u8], offset: &mut usize, len: usize) -> Result<&'a [u8]> {
        let end = offset
            .checked_add(len)
            .ok_or_else(|| anyhow::anyhow!("Android container size overflow"))?;
        let value = bytes
            .get(*offset..end)
            .ok_or_else(|| anyhow::anyhow!("Truncated Android container"))?;
        *offset = end;
        Ok(value)
    }
    fn number(bytes: &[u8], offset: &mut usize) -> Result<usize> {
        Ok(u32::from_be_bytes(take(bytes, offset, 4)?.try_into()?) as usize)
    }
    anyhow::ensure!(
        decrypted.get(..7) == Some(b"ZDB4.0\0"),
        "Invalid Android container magic"
    );
    let mut offset = 14;
    let files = number(decrypted, &mut offset)?;
    anyhow::ensure!(files <= 50000, "Too many Android backup files");
    let mut toc = Vec::new();
    let mut total: usize = 0;
    for _ in 0..files {
        let len = number(decrypted, &mut offset)?;
        anyhow::ensure!(len <= 255, "Android backup filename is too long");
        let name = std::str::from_utf8(take(decrypted, &mut offset, len)?)?.to_owned();
        let size = number(decrypted, &mut offset)?;
        total = total
            .checked_add(size)
            .ok_or_else(|| anyhow::anyhow!("Android backup size overflow"))?;
        anyhow::ensure!(
            total <= MAX_DECOMPRESSED_SYNC_BYTES,
            "Android backup exceeds decompression limit"
        );
        toc.push((name, size));
    }
    let compressed = decrypted
        .get(offset..)
        .ok_or_else(|| anyhow::anyhow!("Missing Android payload"))?;
    let end = compressed
        .windows(2)
        .rposition(|w| w == b"YZ")
        .ok_or_else(|| anyhow::anyhow!("Missing XZ footer"))?
        + 2;
    let payload = decompress_xz_limited(&compressed[..end], MAX_DECOMPRESSED_SYNC_BYTES)?;
    anyhow::ensure!(payload.len() == total, "Android payload length mismatch");
    let temporary = crate::data_dir::PrivateTempDir::new("ctdozlite-sync")?;
    let mut rows = Vec::new();
    let mut databases = 0;
    let mut offset = 0;
    for (index, (name, size)) in toc.into_iter().enumerate() {
        let bytes = take(&payload, &mut offset, size)?;
        let Some(stem) = name.strip_suffix(".db") else {
            continue;
        };
        let group = stem.starts_with("group_");
        let numeric = stem.strip_prefix("group_").unwrap_or(stem);
        let Ok(thread) = numeric.parse::<i64>() else {
            continue;
        };
        databases += 1;
        let path = temporary.path().join(format!("sync-{index}.db"));
        crate::data_dir::write_private_file(&path, bytes)?;
        let connection = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let mut statement = connection.prepare("SELECT SenderId, CliMsgId, GlbMsgId, MsgType, TimeStamp, MsgContent FROM ChatContent ORDER BY TimeStamp ASC")?;
        let entries = statement.query_map([], |r| {
            Ok(AndroidRow {
                thread,
                group,
                sender: r.get(0)?,
                client_id: r.get(1)?,
                global_id: r.get(2)?,
                kind: r.get(3)?,
                timestamp: r.get(4)?,
                content: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            })
        })?;
        for row in entries {
            rows.push(row?);
        }
    }
    anyhow::ensure!(databases > 0, "No supported Android message database");
    Ok(rows)
}

fn persist_android_rows(
    store: &mut MessageStore,
    rows: Vec<AndroidRow>,
    mapping: &std::collections::HashMap<(bool, i64), String>,
    uin: &str,
    self_id: &str,
    name: &str,
) -> Result<usize> {
    store.conn.execute_batch("SAVEPOINT android_sync")?;
    let result = (|| -> Result<usize> {
        let mut count = 0;
        for row in rows {
            anyhow::ensure!(!stopped(), "Đã dừng đồng bộ.");
            let thread = mapping
                .get(&(row.group, row.thread))
                .ok_or_else(|| anyhow::anyhow!("Unknown Android conversation"))?;
            let sender = if row.sender == 0 {
                "0"
            } else {
                mapping
                    .get(&(false, row.sender))
                    .map(String::as_str)
                    .ok_or_else(|| anyhow::anyhow!("Unknown Android sender"))?
            };
            let outgoing = sender == uin || (!self_id.is_empty() && sender == self_id);
            let id = if row.global_id > 0 {
                row.global_id
            } else {
                row.client_id
            };
            anyhow::ensure!(id > 0, "Missing Android message ID");
            let (kind, fallback) = match row.kind {
                10 => ("chat.sticker", "[Nhãn dán]"),
                3 => ("chat.photo", "[Hình ảnh]"),
                12 => ("chat.link", "[Liên kết]"),
                _ => ("webchat", ""),
            };
            let message = crate::store::StoredMessage {
                msg_id: id.to_string(),
                thread_id: thread.clone(),
                sender_id: if outgoing {
                    uin.to_owned()
                } else {
                    sender.to_owned()
                },
                sender_name: if outgoing {
                    name.to_owned()
                } else {
                    String::new()
                },
                text: if row.content.is_empty() {
                    fallback.into()
                } else {
                    row.content
                },
                msg_type: kind.into(),
                ts: row.timestamp,
                is_outgoing: outgoing,
                raw_json: String::new(),
            };
            store.upsert_thread(&crate::store::StoredThread {
                thread_id: thread.clone(),
                name: String::new(),
                is_group: row.group,
                last_ts: row.timestamp,
                last_read_msg_id: String::new(),
            })?;
            if store.upsert_message(&message)? {
                count += 1;
            }
        }
        Ok(count)
    })();
    match result {
        Ok(count) => {
            store.conn.execute_batch("RELEASE android_sync")?;
            Ok(count)
        }
        Err(error) => {
            store
                .conn
                .execute_batch("ROLLBACK TO android_sync; RELEASE android_sync")?;
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    fn android_fixture_rows() -> Vec<super::AndroidRow> {
        vec![
            super::AndroidRow {
                thread: 77,
                group: false,
                sender: 42,
                client_id: 1,
                global_id: 101,
                kind: 1,
                timestamp: 1000,
                content: "Tin đi thử nghiệm".into(),
            },
            super::AndroidRow {
                thread: 77,
                group: false,
                sender: 77,
                client_id: 2,
                global_id: 102,
                kind: 1,
                timestamp: 2000,
                content: "Tin đến thử nghiệm".into(),
            },
        ]
    }

    fn android_mapping() -> std::collections::HashMap<(bool, i64), String> {
        [
            ((false, 42), "self-session-id".into()),
            ((false, 77), "friend-session-id".into()),
        ]
        .into()
    }

    #[test]
    fn android_import_maps_real_senders_is_idempotent_and_hydrates_new_threads() {
        let mut store = crate::store::MessageStore::open(":memory:").unwrap();
        let mapping = android_mapping();
        assert_eq!(
            super::persist_android_rows(
                &mut store,
                android_fixture_rows(),
                &mapping,
                "self-session-id",
                "",
                "Tôi"
            )
            .unwrap(),
            2
        );
        assert_eq!(
            super::persist_android_rows(
                &mut store,
                android_fixture_rows(),
                &mapping,
                "self-session-id",
                "",
                "Tôi"
            )
            .unwrap(),
            0
        );
        let mut threads = vec![];
        store.hydrate_threads(&mut threads, 50).unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "friend-session-id");
        assert_eq!(threads[0].messages.len(), 2);
        assert!(
            threads[0]
                .messages
                .iter()
                .find(|m| m.id == "101")
                .unwrap()
                .is_outgoing
        );
        assert!(
            !threads[0]
                .messages
                .iter()
                .find(|m| m.id == "102")
                .unwrap()
                .is_outgoing
        );
        assert_eq!(
            threads[0]
                .messages
                .iter()
                .find(|m| m.id == "102")
                .unwrap()
                .text,
            "Tin đến thử nghiệm"
        );
    }

    #[test]
    fn android_import_rolls_back_if_any_sender_cannot_be_resolved() {
        let mut store = crate::store::MessageStore::open(":memory:").unwrap();
        let mut rows = android_fixture_rows();
        rows[1].sender = 999;
        assert!(super::persist_android_rows(
            &mut store,
            rows,
            &android_mapping(),
            "self-session-id",
            "",
            "Tôi"
        )
        .is_err());
        let count: i64 = store
            .conn
            .query_row("SELECT count(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        assert!(store.all_threads().unwrap().is_empty());
    }

    #[test]
    fn android_request_accepts_one_backup_only_for_the_active_account() {
        let mut state = super::AndroidTransfer {
            account: "account-a".into(),
            claimed: false,
            result: None,
        };
        assert!(!state.claim("account-b"));
        assert!(state.claim("account-a"));
        assert!(!state.claim("account-a"));
        state.claimed = false;
        state.result = Some(Ok(0));
        assert!(!state.claim("account-a"));
    }

    #[test]
    fn android_rejection_and_busy_controls_are_typed_and_account_scoped() {
        let mut state = super::AndroidTransfer {
            account: "a".into(),
            claimed: false,
            result: None,
        };
        state.control("b", &crate::chat::MobileSyncControlEvent::UserRejected);
        assert!(state.result.is_none());
        state.control("a", &crate::chat::MobileSyncControlEvent::UserConfirmed);
        assert!(state.result.is_none());
        state.control("a", &crate::chat::MobileSyncControlEvent::UserRejected);
        assert!(state
            .result
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap_err()
            .contains("từ chối"));
        state.result = None;
        state.control(
            "a",
            &crate::chat::MobileSyncControlEvent::MobileBusyRestoring,
        );
        assert!(state.result.as_ref().unwrap().is_err());
        state.result = Some(Ok(2));
        state.control("a", &crate::chat::MobileSyncControlEvent::UserRejected);
        assert_eq!(state.result, Some(Ok(2)));
    }

    #[test]
    fn download_target_log_drops_query_and_credentials() {
        let text = super::redacted_download_target(
            "https://user:pw@f1-zpc.zdn.vn/a/b.zdb?token=SECRET#frag",
        );
        assert_eq!(text, "f1-zpc.zdn.vn/a/b.zdb");
        assert_eq!(super::redacted_download_target("nonsense"), "<invalid url>");
    }

    #[test]
    fn android_failure_message_names_stage_without_leaking_detail() {
        let error: anyhow::Error = super::AndroidStage::Download(Some(403)).into();
        let text = super::android_failure_message(&error);
        assert!(text.contains("HTTP 403"), "{text}");
        let other = anyhow::anyhow!("https://secret.example/token");
        assert!(!super::android_failure_message(&other).contains("secret"));
    }

    #[test]
    fn android_decoder_rejects_short_keys_and_truncated_containers_without_panicking() {
        assert!(super::decode_android_backup(&[0; 48], &[0; 15]).is_err());
        assert!(super::parse_android_container(b"ZDB4.0\0").is_err());
        let mut container = b"ZDB4.0\0".to_vec();
        container.resize(14, 0);
        container.extend_from_slice(&1u32.to_be_bytes());
        container.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(super::parse_android_container(&container).is_err());
    }

    #[test]
    fn android_decoder_reads_encrypted_xz_sqlite_fixture() {
        use aes::cipher::{BlockEncryptMut, KeyIvInit};
        let directory = crate::data_dir::PrivateTempDir::new("android-sync-fixture").unwrap();
        let path = directory.path().join("fixture.db");
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE ChatContent(SenderId INTEGER, CliMsgId INTEGER, GlbMsgId INTEGER, MsgType INTEGER, TimeStamp INTEGER, MsgContent TEXT); INSERT INTO ChatContent VALUES (42, 1, 101, 1, 1000, 'Synthetic Android fixture');").unwrap();
        connection
            .execute("VACUUM INTO ?1", [path.to_str().unwrap()])
            .unwrap();
        let sqlite = std::fs::read(&path).unwrap();
        let mut compressed = vec![];
        lzma_rs::xz_compress(&mut std::io::Cursor::new(&sqlite), &mut compressed).unwrap();
        let mut container = b"ZDB4.0\0".to_vec();
        container.resize(14, 0);
        container.extend_from_slice(&1u32.to_be_bytes());
        container.extend_from_slice(&5u32.to_be_bytes());
        container.extend_from_slice(b"77.db");
        container.extend_from_slice(&(sqlite.len() as u32).to_be_bytes());
        container.extend_from_slice(&compressed);
        while container.len() % 16 != 0 {
            container.push(0);
        }
        let key = [0x12; 16];
        let hex = "12".repeat(16);
        for chunk in container.chunks_mut(65536) {
            let len = chunk.len();
            cbc::Encryptor::<aes::Aes256>::new(hex.as_bytes().into(), (&[0u8; 16]).into())
                .encrypt_padded_mut::<cipher::block_padding::NoPadding>(chunk, len)
                .unwrap();
        }
        let rows = super::decode_android_backup(&container, &key).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].sender, 42);
        assert_eq!(rows[0].thread, 77);
        assert_eq!(rows[0].content, "Synthetic Android fixture");
    }
}
