//! Paged history sync for groups, "Cloud của tôi", and mobile message sync (`Đồng bộ tin nhắn từ điện thoại`, #19, #23, #68).
//!
//! - Group & Cloud history walks `/api/cm/getoldv2` backwards 50 messages at a time from the oldest
//!   message already in `zalo_history.db`, so a second run resumes where the first stopped.
//! - Phone message sync selects the Android RSA backup path or an iPhone Sync V2 session
//!   (`cmd 590`, released with `cmd 591`), without mixing both requests,
//!   drains `lp_chat` multi-batch queues (`getChatFromPolling`), batch-queries `/api/preloadconvers/get-last-msgs`
//!   across all 1-1 and group threads in 25-thread chunks, and syncs recent cloud/group history into `zalo_history.db`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;

use crate::chat::{self, Thread};
use crate::session::Session;
use crate::store::{self, MessageStore};

mod android;
pub use android::import_mobile_backup_container;
pub(crate) use android::{receive_android_backup, record_android_control};

const DB_PATH: &str = "zalo_history.db";
const PAGE: u32 = 50;
/// Pages per thread per run (1 000 messages); run again to go further back.
const MAX_PAGES: usize = 20;
const PAUSE: Duration = Duration::from_millis(400);
const MAX_SYNC_DOWNLOAD_BYTES: usize = 256 * 1024 * 1024;
const MAX_DECOMPRESSED_SYNC_BYTES: usize = 512 * 1024 * 1024;

async fn read_response_limited(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        anyhow::bail!("sync response exceeds the {limit}-byte download limit");
    }
    let mut output = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let next_len = output
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| anyhow::anyhow!("sync response size overflow"))?;
        if next_len > limit {
            anyhow::bail!("sync response exceeds the {limit}-byte download limit");
        }
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

struct LimitedVecWriter {
    bytes: Vec<u8>,
    limit: usize,
}

impl LimitedVecWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
}

impl std::io::Write for LimitedVecWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let next_len = self
            .bytes
            .len()
            .checked_add(buf.len())
            .ok_or_else(|| std::io::Error::other("decompression limit exceeded"))?;
        if next_len > self.limit {
            return Err(std::io::Error::other("decompression limit exceeded"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn decompress_xz_limited(input: &[u8], limit: usize) -> Result<Vec<u8>> {
    let mut cursor = std::io::Cursor::new(input);
    let mut output = LimitedVecWriter::new(limit);
    lzma_rs::xz_decompress(&mut cursor, &mut output)
        .map_err(|error| anyhow::anyhow!("XZ decompression failed: {error}"))?;
    Ok(output.bytes)
}

/// Set by the GUI's "Dừng" button; checked between pages.
pub static CANCEL: AtomicBool = AtomicBool::new(false);
/// True while a Cloud/Group sync runs, so a second click doesn't start a parallel one.
pub static RUNNING: AtomicBool = AtomicBool::new(false);

/// Set by the GUI's "Dừng đồng bộ điện thoại" button (#68).
pub static PHONE_SYNC_CANCEL: AtomicBool = AtomicBool::new(false);
/// True while a phone message sync (`sync_phone_messages`) is active (#68).
pub static PHONE_SYNC_RUNNING: AtomicBool = AtomicBool::new(false);

/// Automatic refreshes never own the manual phone-sync state.
pub(crate) static REFRESH_RUNNING: AtomicBool = AtomicBool::new(false);
pub(crate) static REFRESH_CANCEL: AtomicBool = AtomicBool::new(false);

/// Phone transfer implementation selected explicitly in persisted settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PhonePlatform {
    /// Native RSA/AES/XZ backup import; no WASM runtime required.
    Android,
    /// Sync V2 transfer using the zproto runtime.
    Ios,
}

/// Fetch older pages for `t` until the server has nothing older, `MAX_PAGES` is hit, or the user
/// cancels. Each page is committed immediately so a crash or cancel loses nothing. SQLite work
/// runs on the blocking pool; no connection is held across an await.
/// Returns how many messages were new to the database.
pub async fn sync_thread(s: &Session, t: &mut Thread) -> Result<usize> {
    let id = t.id.clone();
    let mut cursor =
        tokio::task::spawn_blocking(move || MessageStore::open(DB_PATH).ok()?.oldest_msg_id(&id))
            .await?
            .unwrap_or_default();
    let mut added = 0;
    for _ in 0..MAX_PAGES {
        if CANCEL.load(Ordering::Relaxed) {
            break;
        }
        let (request_cursor, is_old) = history_page_request(&cursor);
        let resp = match chat::get_cloud_messages(s, &t.id, request_cursor, is_old, PAGE).await {
            Ok(resp) => resp,
            // Zalo PC caches -69 per (cursor, old, conversation) as "do not repeat this request"
            // and its message search reads -69 as "no more"; an older page ending this way means
            // the server has nothing further for this thread, not that the whole sync failed.
            Err(error) if is_end_of_history(&error, is_old) => break,
            Err(error) => return Err(error),
        };
        let before = t.messages.len();
        chat::merge_group_history(t, &resp);
        if t.messages.len() == before {
            break; // nothing older on the server
        }
        let snapshot = t.clone();
        added += tokio::task::spawn_blocking(move || {
            store::persist_threads(
                &mut MessageStore::open(DB_PATH)?,
                std::slice::from_ref(&snapshot),
            )
        })
        .await??;
        match next_cursor(t, &cursor) {
            Some(c) => cursor = c,
            None => break,
        }
        tokio::time::sleep(PAUSE).await;
    }
    Ok(added)
}

/// True when a failed page request means "no older history", not a real failure.
fn is_end_of_history(error: &anyhow::Error, is_old: bool) -> bool {
    is_old
        && error
            .downcast_ref::<crate::session::SessionError>()
            .is_some_and(|server| server.code == -69)
}

fn history_page_request(cursor: &str) -> (&str, bool) {
    // getCM starts at "0" on getrecentv2; getoldv2 requires an existing message cursor.
    // Evidence: re-notes/42_history_sync_entrypoints.md.
    if cursor.is_empty() || cursor == "0" {
        ("0", false)
    } else {
        (cursor, true)
    }
}

/// Describe a history failure without exposing request URLs or encrypted parameters.
pub(crate) fn history_sync_error_message(error: &anyhow::Error) -> String {
    if let Some(server) = error.downcast_ref::<crate::session::SessionError>() {
        if server.code == -69 {
            "Máy chủ Zalo từ chối tải lịch sử (mã -69). Thử lại sau hoặc đồng bộ từ điện thoại."
                .into()
        } else {
            format!("Máy chủ Zalo từ chối tải lịch sử (mã {}).", server.code)
        }
    } else {
        "Không tải được lịch sử. Kiểm tra kết nối và thử lại.".into()
    }
}

/// The oldest server message id after a merge, or `None` when the page didn't move the cursor
/// back (the server returned only messages we already had); continuing would loop forever.
/// Local echoes (`out_*`) are not valid cursors, hence the digits-only filter.
fn next_cursor(t: &Thread, current: &str) -> Option<String> {
    let oldest = t
        .messages
        .iter()
        .map(|m| m.id.as_str())
        .find(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))?;
    (oldest != current).then(|| oldest.to_string())
}

#[derive(Default, Clone)]
pub(crate) struct V2Transfer {
    pub sync_id: String,
    pub partitions: std::collections::HashMap<String, String>,
    pub pending: usize,
    pub saw_last: bool,
    pub imported: usize,
    pub failure: Option<String>,
}

impl V2Transfer {
    fn complete_batch(&mut self, result: Result<usize>, is_last: bool) {
        self.pending = self.pending.saturating_sub(1);
        match result {
            Ok(count) => self.imported += count,
            Err(_) => {
                self.failure = Some(
                    "Không giải mã được gói Sync V2. Phiên đồng bộ đã dừng; hãy thử lại.".into(),
                )
            }
        }
        self.saw_last |= is_last;
    }
}

static V2_TRANSFER: std::sync::Mutex<Option<V2Transfer>> = std::sync::Mutex::new(None);
static PHONE_COMPLETE: AtomicBool = AtomicBool::new(false);
static PHONE_IMPORTED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Serialize decoding and persistence, after metadata is ready.
/// Keep account-file changes behind completion/cancellation of the phone worker.
pub(crate) static PHONE_SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
pub(crate) static V2_BATCH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn active_v2_id() -> Result<String> {
    V2_TRANSFER
        .lock()
        .ok()
        .and_then(|s| s.as_ref().map(|s| s.sync_id.clone()))
        .ok_or_else(|| anyhow::anyhow!("Không có yêu cầu Sync V2 đang hoạt động."))
}

pub(crate) fn begin_v2_transfer(
    sync_id: &str,
    partitions: std::collections::HashMap<String, String>,
) -> Result<()> {
    let mut state = V2_TRANSFER
        .lock()
        .map_err(|_| anyhow::anyhow!("Sync state unavailable"))?;
    if state.is_some() {
        anyhow::bail!("Một phiên Sync V2 khác đang chạy.");
    }
    PHONE_IMPORTED.store(0, Ordering::Relaxed);
    PHONE_COMPLETE.store(false, Ordering::Relaxed);
    *state = Some(V2Transfer {
        sync_id: sync_id.into(),
        partitions,
        ..Default::default()
    });
    Ok(())
}

pub(crate) fn v2_transfer(sync_id: &str) -> Option<V2Transfer> {
    V2_TRANSFER
        .lock()
        .ok()
        .and_then(|state| state.as_ref().filter(|s| s.sync_id == sync_id).cloned())
}

pub(crate) fn enqueue_v2_batch(sync_id: &str) -> bool {
    if let Ok(mut state) = V2_TRANSFER.lock() {
        if let Some(state) = state.as_mut().filter(|s| s.sync_id == sync_id) {
            state.pending += 1;
            return true;
        }
    }
    false
}

pub(crate) fn finish_v2_batch(sync_id: &str, result: Result<usize>, is_last: bool) {
    if let Ok(mut state) = V2_TRANSFER.lock() {
        if let Some(state) = state.as_mut().filter(|s| s.sync_id == sync_id) {
            state.complete_batch(result, is_last);
            PHONE_IMPORTED.store(state.imported, Ordering::Relaxed);
            PHONE_COMPLETE.store(
                state.saw_last && state.pending == 0 && state.failure.is_none(),
                Ordering::Relaxed,
            );
        }
    }
}

/// Number of phone messages actually inserted during the latest request.
pub fn last_phone_imported_count() -> usize {
    PHONE_IMPORTED.load(Ordering::Relaxed)
}

/// True only after the final phone batch and every pending import have completed.
pub fn last_phone_sync_complete() -> bool {
    PHONE_COMPLETE.load(Ordering::Relaxed)
}

pub(crate) struct V2SyncLease(pub String);
impl Drop for V2SyncLease {
    fn drop(&mut self) {
        // Session release is best effort when the transport is already disconnected.
        let reason = if PHONE_SYNC_CANCEL.load(Ordering::Relaxed) {
            "PC_USER_CANCELS_SYNC"
        } else if last_phone_sync_complete() {
            "SYNC_FULL_COMPLETED"
        } else {
            "BATCH_TIMEOUT"
        };
        let _ = crate::socket::send_sync_ack_delete(&self.0, reason);
        if let Ok(mut state) = V2_TRANSFER.lock() {
            *state = None;
        }
        if let Ok(mut keys) = CURRENT_V2_KEYS.lock() {
            *keys = None;
        }
        if let Ok(mut session) = CURRENT_V2_SESSION.lock() {
            *session = None;
        }
        if let Ok(mut registry) = SYNC_V2_SESSIONS.lock() {
            registry.retain(|(id, _)| id != &self.0);
        }
    }
}

static LATEST_MOBILE_TEMP_KEY: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Record a fresh `temp_key` received from a `transfer_after_login` control event.
pub fn set_latest_mobile_temp_key(temp_key: &str) {
    if let Ok(mut g) = LATEST_MOBILE_TEMP_KEY.lock() {
        *g = temp_key.trim().to_string();
    }
}

/// Return the most recent `temp_key` issued by the user's phone on login (if any).
pub fn latest_mobile_temp_key() -> String {
    LATEST_MOBILE_TEMP_KEY
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}
static LAST_MOBILE_SYNC_STATUS: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Record the latest status returned by the user's phone (`WaitingConfirmOnPhone`, `MobileBusyRestoring`, etc.).
pub fn set_last_mobile_sync_status(status: Option<String>) {
    if let Ok(mut g) = LAST_MOBILE_SYNC_STATUS.lock() {
        *g = status;
    }
}

/// Read the latest status returned by the user's phone during `sync_phone_messages`.
pub fn last_mobile_sync_status() -> Option<String> {
    LAST_MOBILE_SYNC_STATUS.lock().ok().and_then(|g| g.clone())
}

static CURRENT_SYNC_KEYS: std::sync::Mutex<Option<(rsa::RsaPrivateKey, String)>> =
    std::sync::Mutex::new(None);

/// Retrieve the active RSA 2048 keys used for mobile sync decryption without consuming them.
pub fn get_sync_keys() -> Option<(rsa::RsaPrivateKey, String)> {
    CURRENT_SYNC_KEYS.lock().ok().and_then(|g| g.clone())
}

/// Retrieve the active RSA 2048 public key.
pub fn get_sync_public_key() -> Option<String> {
    CURRENT_SYNC_KEYS
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|(_, pub_k)| pub_k.clone()))
}
/// Clear the active sync keys once sync is finalized.
pub fn clear_sync_keys() {
    if let Ok(mut g) = CURRENT_SYNC_KEYS.lock() {
        *g = None;
    }
}

/// Generate a genuine 2048-bit RSA SubjectPublicKeyInfo (`SPKI`) DER Base64 key
/// (`294 bytes` -> `392 chars` Base64 starting with `MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA...`),
/// matching `sync-message-key-generator` (`main-startup.js:598226`) for `pullMobileMsg` (`cmd 12000`).
pub fn generate_sync_public_key() -> String {
    use base64::Engine;
    use rsa::pkcs8::EncodePublicKey;
    use rsa::{RsaPrivateKey, RsaPublicKey};

    let mut rng = rsa::rand_core::OsRng;
    if let Ok(priv_key) = RsaPrivateKey::new(&mut rng, 2048) {
        let pub_key = RsaPublicKey::from(&priv_key);
        if let Ok(der) = pub_key.to_public_key_der() {
            let b64 = base64::engine::general_purpose::STANDARD.encode(der.as_bytes());
            if let Ok(mut g) = CURRENT_SYNC_KEYS.lock() {
                *g = Some((priv_key, b64.clone()));
            }
            return b64;
        }
    }
    String::new()
}
/// Synchronize 1-1 and group messages from the user's phone and Zalo cloud (`Đồng bộ tin nhắn từ điện thoại`, #68).
///
/// Workflow:
/// 1. Opens one manual Android backup request. The platform-aware entry point also supports
///    iPhone Sync V2 (`cmd 590`, released with `cmd 591`).
/// 2. Drains `lp_chat` (`get_chat_from_polling`) across multiple batches to collect queued 1-1 & group messages.
/// 3. Queries `/api/preloadconvers/get-last-msgs` (`get_last_msgs`) in 25-thread batches across all
///    1-1 and group conversations and merges messages into `threads`.
/// 4. Fetches recent & older pages via `/api/cm/getrecentv2` and `/api/cm/getoldv2` for Cloud & groups.
/// 5. Persists all merged messages to `zalo_history.db` and `threads_cache.json`.
///
/// Returns `(added_messages, synced_threads_count)`.
pub async fn sync_phone_messages<F>(
    s: &Session,
    poll_state: &tokio::sync::Mutex<chat::PollState>,
    threads: &mut Vec<Thread>,
    on_progress: F,
) -> Result<(usize, usize)>
where
    F: FnMut(f32, String),
{
    sync_phone_messages_opts(
        s,
        poll_state,
        threads,
        Some(PhonePlatform::Android),
        on_progress,
    )
    .await
}

/// Synchronize messages with explicit control over whether to send a wake-up/pull prompt
/// to the selected phone platform (`Some`) or only drain server/cloud queues (`None`).
pub async fn sync_phone_messages_opts<F>(
    s: &Session,
    poll_state: &tokio::sync::Mutex<chat::PollState>,
    threads: &mut Vec<Thread>,
    platform: Option<PhonePlatform>,
    mut on_progress: F,
) -> Result<(usize, usize)>
where
    F: FnMut(f32, String),
{
    if s.login.is_none() {
        anyhow::bail!("chưa đăng nhập");
    }
    let send_phone_prompt = platform.is_some();
    let cancel = if send_phone_prompt {
        &PHONE_SYNC_CANCEL
    } else {
        &REFRESH_CANCEL
    };
    if send_phone_prompt {
        PHONE_IMPORTED.store(0, Ordering::Relaxed);
        PHONE_COMPLETE.store(false, Ordering::Relaxed);
    }
    let before_total: usize = threads.iter().map(|t| t.messages.len()).sum();
    let mut phone_lease = None;
    let mut phone_error = None;
    if platform == Some(PhonePlatform::Android) {
        set_last_mobile_sync_status(None);
        android::sync(s, &mut on_progress).await?;
    } else if platform == Some(PhonePlatform::Ios) {
        crate::sync_runtime::ensure_ready(&mut on_progress).await?;
        set_last_mobile_sync_status(None);
        on_progress(
            0.10,
            "Đang gửi yêu cầu Sync V2; hãy mở Zalo trên điện thoại để xác nhận…".into(),
        );
        let sync_id = chat::start_phone_sync_v2(s, threads).await?;
        phone_lease = Some(V2SyncLease(sync_id.clone()));
        let deadline = std::time::Instant::now() + Duration::from_secs(210);
        loop {
            if PHONE_SYNC_CANCEL.load(Ordering::Relaxed) {
                break;
            }
            if let Some(state) = v2_transfer(&sync_id) {
                if let Some(error) = state.failure {
                    phone_error = Some(error);
                    break;
                }
                if state.saw_last && state.pending == 0 {
                    set_last_mobile_sync_status(Some(format!(
                        "Đã tải {} tin nhắn từ điện thoại.",
                        state.imported
                    )));
                    break;
                }
            }
            if let Some(note) = last_mobile_sync_status() {
                if note.contains("từ chối") {
                    PHONE_SYNC_CANCEL.store(true, Ordering::Relaxed);
                    break;
                }
                on_progress(0.45, note);
            }
            if std::time::Instant::now() >= deadline {
                phone_error =
                    Some("Điện thoại chưa hoàn tất Sync V2. Hãy mở Zalo và thử lại.".into());
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
    // Release the phone session before the independent server/cloud refresh.
    if phone_lease.is_some() {
        // Wait for a decoder already writing to SQLite before releasing its session.
        let _inflight = V2_BATCH_LOCK.lock().await;
        drop(phone_lease.take());
    }
    if let Some(error) = phone_error {
        anyhow::bail!(error);
    }
    let phone_imported = if send_phone_prompt {
        last_phone_imported_count()
    } else {
        0
    };

    if send_phone_prompt && last_phone_sync_complete() {
        let mut snapshot = threads.clone();
        *threads = tokio::task::spawn_blocking(move || -> Result<Vec<Thread>> {
            MessageStore::open(DB_PATH)?.hydrate_threads(&mut snapshot, 500)?;
            Ok(snapshot)
        })
        .await??;
    }

    // Phase 1b: Synchronize pinned conversations from server (/api/pinconvers/list, #87)
    on_progress(
        0.18,
        "📌 Đang đồng bộ danh sách hội thoại đã ghim từ máy chủ Zalo…".to_string(),
    );
    if let Ok(pinned_val) = chat::get_pinned(s).await {
        let pinned_ids = chat::parse_pinned_conversations(&pinned_val);
        chat::apply_pinned_threads(threads, &pinned_ids);
    }

    if cancel.load(Ordering::Relaxed) {
        return Ok((phone_imported, 0));
    }

    // Phase 2: Drain `lp_chat` multi-batch action queues (`getChatFromPolling`)
    on_progress(
        0.22,
        "📥 Đang nhận hàng đợi tin nhắn 1-1 & nhóm từ điện thoại…".to_string(),
    );
    for batch_idx in 0..5 {
        if cancel.load(Ordering::Relaxed) {
            return Ok((phone_imported, 0));
        }
        let lp_res = {
            let mut ps = poll_state.lock().await;
            chat::get_chat_from_polling(s, &mut ps).await
        };
        let Ok(lp_resp) = lp_res else {
            break;
        };
        chat::merge_previews(threads, &lp_resp);
        if let Some(data_node) = lp_resp.get("data") {
            crate::unread::merge_read_points(threads, data_node);
        }
        let data_node = lp_resp.get("data").unwrap_or(&lp_resp);
        let more = data_node
            .get("more")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        let has_reset = data_node
            .get("resetLastActionId")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|r| !r.is_empty() && r != "0");
        on_progress(
            0.22 + (batch_idx as f32 + 1.0) * 0.04,
            format!("📥 Đã nhận lô hàng đợi #{} từ điện thoại…", batch_idx + 1),
        );
        if more == 0 && !has_reset {
            break;
        }
    }

    // Phase 3: Batch-query `/api/preloadconvers/get-last-msgs` across all 1-1 and group conversations
    on_progress(
        0.45,
        "🔄 Đang đồng bộ tin nhắn các cuộc trò chuyện 1-1 & nhóm…".to_string(),
    );
    if let Ok(preview_resp) = chat::get_last_msgs(s, &[]).await {
        chat::merge_previews(threads, &preview_resp);
    }
    let known_ids: Vec<String> = threads
        .iter()
        .filter(|t| !t.id.is_empty())
        .map(|t| {
            if t.is_group && !t.id.starts_with('g') && !t.id.starts_with("group_") {
                format!("g{}", t.id)
            } else {
                t.id.clone()
            }
        })
        .collect();
    let chunks: Vec<&[String]> = known_ids.chunks(25).collect();
    let total_chunks = chunks.len().max(1);
    for (idx, chunk) in chunks.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if let Ok(preview_resp) = chat::get_last_msgs(s, chunk).await {
            chat::merge_previews(threads, &preview_resp);
        }
        let frac = 0.45 + 0.25 * ((idx + 1) as f32 / total_chunks as f32);
        on_progress(
            frac,
            format!(
                "🔄 Đang đồng bộ hội thoại 1-1 & nhóm ({}/{total_chunks} nhóm lô)…",
                idx + 1
            ),
        );
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    // Phase 4: Sync Cloud & group conversations via `/api/cm/getrecentv2` + `/api/cm/getoldv2`
    let send2me_id = s
        .login
        .as_ref()
        .map(|l| l.send2me_id.clone())
        .unwrap_or_default();
    let cm_targets: Vec<usize> = threads
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            !t.id.is_empty()
                && (t.is_group
                    || t.name.contains("Cloud của tôi")
                    || (!send2me_id.is_empty() && t.id == send2me_id))
        })
        .map(|(i, _)| i)
        .collect();
    let total_cm = cm_targets.len().max(1);
    for (pos, tidx) in cm_targets.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let (tid, tname) = {
            let t = &threads[tidx];
            (t.id.clone(), t.name.clone())
        };
        let frac = 0.70 + 0.25 * ((pos + 1) as f32 / total_cm as f32);
        on_progress(
            frac,
            format!("☁️ Đang đồng bộ lịch sử: {tname} ({}/{total_cm})…", pos + 1),
        );
        if let Ok(cm_resp) = chat::get_cloud_messages(s, &tid, "0", false, PAGE).await {
            chat::merge_group_history(&mut threads[tidx], &cm_resp);
        }
        if let Some(oldest) = next_cursor(&threads[tidx], "") {
            if let Ok(old_resp) = chat::get_cloud_messages(s, &tid, &oldest, true, PAGE).await {
                chat::merge_group_history(&mut threads[tidx], &old_resp);
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    // Phase 5: Resolve contact/group display names, deduplicate, and persist to SQLite (`zalo_history.db`)
    on_progress(
        0.96,
        "💾 Đang lưu tin nhắn đã đồng bộ vào cơ sở dữ liệu zalo_history.db…".to_string(),
    );
    chat::resolve_thread_names(s, threads).await;
    chat::deduplicate_threads(threads, None);

    let snapshot = threads.clone();
    let uin_str = s.login.as_ref().map(|l| l.uin.clone()).unwrap_or_default();
    let db_path = store::db_path_for_account(&uin_str);
    let db_path_clone = db_path.clone();
    let (db_added, hydrated) =
        tokio::task::spawn_blocking(move || -> Result<(usize, Vec<Thread>)> {
            let mut st = MessageStore::open(&db_path_clone)?;
            let n = store::persist_threads(&mut st, &snapshot)?;
            let mut hydrated = snapshot;
            st.hydrate_threads(&mut hydrated, 500)?;
            chat::save_threads_cache(&hydrated);
            Ok((n, hydrated))
        })
        .await??;
    *threads = hydrated;
    let after_total: usize = threads.iter().map(|t| t.messages.len()).sum();
    let added = (db_added + phone_imported).max(after_total.saturating_sub(before_total));
    let active_threads = threads.iter().filter(|t| !t.messages.is_empty()).count();
    let final_msg = if send_phone_prompt && last_phone_sync_complete() {
        format!("Đã lưu {phone_imported} tin từ điện thoại; đang hoàn tất làm mới hội thoại.")
    } else {
        format!("Đã làm mới {active_threads} hội thoại từ máy chủ.")
    };
    on_progress(1.0, final_msg);
    Ok((added, active_threads))
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SyncV2Keys {
    pub ik_pub: String,
    pub ik_priv: String,
    pub ek_pub: String,
    pub ek_priv: String,
}

#[derive(Debug, Clone)]
struct SyncV2KeysRaw {
    ik_pub: Vec<u8>,
    ik_priv: Vec<u8>,
    ek_priv: Vec<u8>,
}

/// Khóa Sync V2 ed25519 của phiên hiện tại (raw bytes, giữ trong bộ nhớ).
static CURRENT_V2_KEYS: std::sync::Mutex<Option<SyncV2KeysRaw>> = std::sync::Mutex::new(None);

pub fn generate_sync_v2_keys() -> Result<SyncV2Keys> {
    // wasmtime host (không cần Node.js). scripts/sync_v2_helper.js chỉ còn
    // là tài liệu tham khảo cho luồng JS gốc (deprecated path).
    let host = crate::zproto_host::ZprotoHost::load()?;
    let (ik_pub, ik_priv) =
        load_or_create_identity_key(std::path::Path::new("sync_identity.json"), || {
            host.generate_keypair()
        })?;
    let (ek_pub, ek_priv) = host.generate_keypair()?;
    if ik_pub == ek_pub {
        anyhow::bail!("Khóa phiên Sync V2 trùng khóa định danh; đã dừng đồng bộ.");
    }
    use base64::Engine;
    let keys = SyncV2Keys {
        ik_pub: base64::engine::general_purpose::STANDARD.encode(&ik_pub),
        ik_priv: hex_encode(&ik_priv),
        ek_pub: base64::engine::general_purpose::STANDARD.encode(&ek_pub),
        ek_priv: hex_encode(&ek_priv),
    };
    if let Ok(mut g) = CURRENT_V2_KEYS.lock() {
        *g = Some(SyncV2KeysRaw {
            ik_pub,
            ik_priv,
            ek_priv,
        });
    }
    Ok(keys)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredSyncIdentity {
    public_key: String,
    private_key: String,
}

fn load_or_create_identity_key<F>(path: &std::path::Path, generate: F) -> Result<(Vec<u8>, Vec<u8>)>
where
    F: FnOnce() -> Result<(Vec<u8>, Vec<u8>)>,
{
    use base64::{engine::general_purpose::STANDARD, Engine};
    let keys = match std::fs::File::open(path) {
        Ok(file) => {
            use std::io::Read;
            crate::data_dir::ensure_private_file(path)?;
            let mut raw = Vec::new();
            file.take(4097).read_to_end(&mut raw)?;
            if raw.len() > 4096 {
                anyhow::bail!("Sync identity file is too large");
            }
            let saved: StoredSyncIdentity = serde_json::from_slice(&raw)
                .map_err(|_| anyhow::anyhow!("Không đọc được khóa định danh Sync V2 đã lưu."))?;
            (
                STANDARD.decode(saved.public_key)?,
                STANDARD.decode(saved.private_key)?,
            )
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let (public, private) = generate()?;
            validate_identity_key(&public, &private)?;
            let json = serde_json::to_vec(&StoredSyncIdentity {
                public_key: STANDARD.encode(&public),
                private_key: STANDARD.encode(&private),
            })?;
            crate::data_dir::write_private_atomic(path, &json)?;
            (public, private)
        }
        Err(error) => return Err(error.into()),
    };
    validate_identity_key(&keys.0, &keys.1)?;
    Ok(keys)
}

fn validate_identity_key(public: &[u8], private: &[u8]) -> Result<()> {
    if public.len() != 33
        || public.first() != Some(&5)
        || private.len() != 32
        || private.iter().all(|b| *b == 0)
    {
        anyhow::bail!("Khóa định danh Sync V2 không hợp lệ.");
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 15) as usize] as char);
    }
    s
}
/// Kết quả giải mã metadata Sync V2: session id để đối chiếu với event batch.
#[derive(Debug, Default)]
pub struct SyncV2Meta {
    pub session_id: String,
    pub sender_identity: String,
}

pub async fn import_sync_v2_meta(s: &Session, url: &str) -> Result<SyncV2Meta> {
    if !crate::session::is_trusted_zalo_url(url) {
        anyhow::bail!("Refusing authenticated history download from an untrusted Zalo URL");
    }
    let request_id = active_v2_id()?;
    let keys = current_v2_keys()?;
    let uin = s.login.as_ref().map(|l| l.uin.clone()).unwrap_or_default();
    let sek = s.zpw_sek.clone().unwrap_or_default();
    let cookie_hdr = format!("zpw_sek={sek}; zpw_uin={uin}");

    let resp = s
        .http
        .get(url)
        .header("Cookie", cookie_hdr)
        .header("User-Agent", "ZaloPC-win32-26.10.11")
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("failed to download v2 meta: HTTP {}", resp.status());
    }

    let bytes = read_response_limited(resp, MAX_SYNC_DOWNLOAD_BYTES).await?;
    // wasmtime host (không cần Node.js). Keys phải tồn tại trong bộ nhớ tiến
    // trình (generate_sync_v2_keys đã chạy trước khi request sync).
    tokio::task::spawn_blocking(move || -> Result<SyncV2Meta> {
        let host = crate::zproto_host::ZprotoHost::load()?;
        let mut session_record: Option<Vec<u8>> = None;
        let mut last: Option<crate::zproto_host::DecryptedMeta> = None;
        for chunk in crate::zproto_host::split_frames(&bytes)? {
            let m = host.decrypt_metadata(
                &chunk,
                &keys.ik_pub,
                &keys.ik_priv,
                &keys.ek_priv,
                session_record.as_deref(),
            )?;
            session_record = Some(m.session_record.clone());
            last = Some(m);
        }
        let m = last.ok_or_else(|| anyhow::anyhow!("metadata rỗng"))?;
        {
            let state = V2_TRANSFER
                .lock()
                .map_err(|_| anyhow::anyhow!("Sync state unavailable"))?;
            if !state.as_ref().is_some_and(|s| s.sync_id == request_id) {
                anyhow::bail!("Phiên Sync V2 đã kết thúc.");
            }
            store_v2_session(&m.session_record);
        }
        Ok(SyncV2Meta {
            session_id: m.session_id,
            sender_identity: m.sender_identity,
        })
    })
    .await?
}

fn current_v2_keys() -> Result<SyncV2KeysRaw> {
    CURRENT_V2_KEYS
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .ok_or_else(|| anyhow::anyhow!("chưa có khóa Sync V2 — gọi generate_sync_v2_keys trước"))
}

/// Session record của metadata gần nhất (raw bytes, giữ trong bộ nhớ).
static CURRENT_V2_SESSION: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);

fn store_v2_session(session_record: &[u8]) {
    if let Ok(mut g) = CURRENT_V2_SESSION.lock() {
        *g = Some(session_record.to_vec());
    }
}

fn current_v2_session() -> Result<Vec<u8>> {
    CURRENT_V2_SESSION
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .ok_or_else(|| anyhow::anyhow!("chưa giải mã metadata — gọi import_sync_v2_meta trước"))
}
/// Đăng ký session Sync V2 đã giải mã xong metadata, key theo sync_id của event.
/// Batch message chỉ được giải mã khi sync_id của nó đã có metadata — tránh
/// dùng nhầm session của phiên cũ (#116).
static SYNC_V2_SESSIONS: std::sync::Mutex<Vec<(String, String)>> =
    std::sync::Mutex::new(Vec::new());

pub fn record_sync_v2_session(sync_id: &str, session_id: &str) {
    if let Ok(mut m) = SYNC_V2_SESSIONS.lock() {
        m.retain(|(k, _)| k != sync_id);
        m.push((sync_id.to_string(), session_id.to_string()));
    }
}

pub fn has_sync_v2_session(sync_id: &str) -> bool {
    SYNC_V2_SESSIONS
        .lock()
        .map(|m| m.iter().any(|(k, _)| k == sync_id))
        .unwrap_or(false)
}

#[derive(Debug, Default)]
pub struct ParsedProtoMsg {
    pub sender_id: String,
    pub client_id: u64,
    pub msg_status: i32,
    pub msg_type: i32,
    pub timestamp: u64,
    pub global_id: String,
    pub content: String,
}

#[derive(Debug, Default)]
pub struct ParsedProtoPartition {
    pub id: String,
    pub messages: Vec<ParsedProtoMsg>,
}

fn parse_vli(buf: &[u8], mut offset: usize) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    while offset < buf.len() {
        let b = buf[offset];
        offset += 1;
        if shift == 63 && b > 1 {
            return None;
        }
        result |= ((b & 0x7F) as u64) << shift;
        if (b & 0x80) == 0 {
            return Some((result, offset));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    None
}

fn parse_message(buf: &[u8]) -> Option<ParsedProtoMsg> {
    let mut msg = ParsedProtoMsg::default();
    let mut offset = 0;
    while offset < buf.len() {
        let (tag, next) = parse_vli(buf, offset)?;
        offset = next;
        let field_num = tag >> 3;
        let wire_type = tag & 0x07;
        match wire_type {
            0 => {
                let (val, next) = parse_vli(buf, offset)?;
                offset = next;
                match field_num {
                    2 => msg.client_id = val,
                    3 => msg.msg_status = val as i32,
                    4 => msg.msg_type = val as i32,
                    5 => msg.timestamp = val,
                    _ => {}
                }
            }
            2 => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next;
                let len = usize::try_from(len).ok()?;
                if offset.checked_add(len)? > buf.len() {
                    return None;
                }
                let bytes = &buf[offset..offset + len];
                offset += len;
                match field_num {
                    1 => msg.sender_id = String::from_utf8_lossy(bytes).into_owned(),
                    6 => msg.global_id = String::from_utf8_lossy(bytes).into_owned(),
                    7 => msg.content = String::from_utf8_lossy(bytes).into_owned(),
                    _ => {}
                }
            }
            1 => {
                offset = offset.checked_add(8)?;
                if offset > buf.len() {
                    return None;
                }
            }
            5 => {
                offset = offset.checked_add(4)?;
                if offset > buf.len() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(msg)
}

fn parse_partition(buf: &[u8]) -> Option<ParsedProtoPartition> {
    let mut part = ParsedProtoPartition::default();
    let mut offset = 0;
    while offset < buf.len() {
        let (tag, next) = parse_vli(buf, offset)?;
        offset = next;
        let field_num = tag >> 3;
        let wire_type = tag & 0x07;
        match (field_num, wire_type) {
            (1, 2) => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next;
                let len = usize::try_from(len).ok()?;
                if offset.checked_add(len)? > buf.len() {
                    return None;
                }
                part.id = String::from_utf8_lossy(&buf[offset..offset + len]).into_owned();
                offset += len;
            }
            (2, 2) => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next;
                let len = usize::try_from(len).ok()?;
                if offset.checked_add(len)? > buf.len() {
                    return None;
                }
                if let Some(m) = parse_message(&buf[offset..offset + len]) {
                    part.messages.push(m);
                }
                offset += len;
            }
            (_, 0) => {
                let (_, next) = parse_vli(buf, offset)?;
                offset = next;
            }
            (_, 2) => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next.checked_add(usize::try_from(len).ok()?)?;
                if offset > buf.len() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(part)
}

/// Giải nén payload Sync V2 tự nhận dạng: zstd (28 b5 2f fd) / gzip (1f 8b) /
/// zlib (78 xx) / raw với giới hạn bộ nhớ đầu ra.
pub fn decompress_sync_payload(buf: &[u8]) -> Result<Vec<u8>> {
    decompress_sync_payload_with_limit(buf, MAX_DECOMPRESSED_SYNC_BYTES)
}

fn decompress_sync_payload_with_limit(buf: &[u8], limit: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    fn read_limited(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        reader.take(limit as u64 + 1).read_to_end(&mut output)?;
        if output.len() > limit {
            anyhow::bail!("decompression limit exceeded ({limit} bytes)");
        }
        Ok(output)
    }

    if buf.len() >= 4 && buf[0] == 0x28 && buf[1] == 0xb5 && buf[2] == 0x2f && buf[3] == 0xfd {
        let decoder = zstd::stream::read::Decoder::new(std::io::Cursor::new(buf))?;
        return read_limited(decoder, limit);
    }
    if buf.len() >= 2 && buf[0] == 0x1f && buf[1] == 0x8b {
        return read_limited(flate2::read::GzDecoder::new(buf), limit);
    }
    if buf.len() >= 2 && buf[0] == 0x78 {
        return read_limited(flate2::read::ZlibDecoder::new(buf), limit);
    }
    if buf.len() > limit {
        anyhow::bail!("decompression limit exceeded ({limit} bytes)");
    }
    Ok(buf.to_vec())
}

pub fn parse_sync_chunk(buf: &[u8]) -> Option<Vec<ParsedProtoPartition>> {
    let mut parts = Vec::new();
    let mut offset = 0;
    while offset < buf.len() {
        let (tag, next) = parse_vli(buf, offset)?;
        offset = next;
        let field_num = tag >> 3;
        let wire_type = tag & 0x07;
        match (field_num, wire_type) {
            (2, 2) => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next;
                let len = usize::try_from(len).ok()?;
                if offset.checked_add(len)? > buf.len() {
                    return None;
                }
                if let Some(p) = parse_partition(&buf[offset..offset + len]) {
                    parts.push(p);
                }
                offset += len;
            }
            (_, 0) => {
                let (_, next) = parse_vli(buf, offset)?;
                offset = next;
            }
            (_, 2) => {
                let (len, next) = parse_vli(buf, offset)?;
                offset = next.checked_add(usize::try_from(len).ok()?)?;
                if offset > buf.len() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(parts)
}

fn resolve_v2_partition(
    mapping: &std::collections::HashMap<String, String>,
    partition: &str,
) -> Result<String> {
    mapping
        .get(partition)
        .or_else(|| mapping.get(&format!("oneone/{partition}")))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Gói Sync V2 chứa hội thoại không được yêu cầu."))
}

fn persist_v2_partitions(
    store: &mut MessageStore,
    partitions: Vec<ParsedProtoPartition>,
    mapping: &std::collections::HashMap<String, String>,
    uin: &str,
    my_name: &str,
) -> Result<usize> {
    store.conn.execute_batch("SAVEPOINT sync_v2_batch")?;
    let result = (|| -> Result<usize> {
        let mut imported = 0;
        for part in partitions {
            let thread_id = resolve_v2_partition(mapping, &part.id)?;
            for message in part.messages {
                let is_outgoing = message.sender_id == uin;
                let msg_id = if !message.global_id.is_empty() && message.global_id != "0" {
                    message.global_id
                } else {
                    message.client_id.to_string()
                };
                if msg_id == "0" {
                    anyhow::bail!("Gói Sync V2 thiếu định danh tin nhắn.");
                }
                let stored = crate::store::StoredMessage {
                    msg_id,
                    thread_id: thread_id.clone(),
                    sender_id: if is_outgoing {
                        uin.into()
                    } else {
                        thread_id.clone()
                    },
                    sender_name: if is_outgoing {
                        my_name.into()
                    } else {
                        String::new()
                    },
                    text: message.content,
                    msg_type: "webchat".into(),
                    ts: message.timestamp as i64,
                    is_outgoing,
                    raw_json: String::new(),
                };
                if store.upsert_message(&stored)? {
                    imported += 1;
                }
            }
        }
        Ok(imported)
    })();
    match result {
        Ok(count) => {
            store.conn.execute_batch("RELEASE sync_v2_batch")?;
            Ok(count)
        }
        Err(error) => {
            store
                .conn
                .execute_batch("ROLLBACK TO sync_v2_batch; RELEASE sync_v2_batch")?;
            Err(error)
        }
    }
}

/// Format phone-sync failures without authenticated request URLs or payloads.
pub(crate) fn phone_sync_error_message(error: &anyhow::Error) -> String {
    if error
        .downcast_ref::<crate::session::SessionError>()
        .is_some()
    {
        history_sync_error_message(error)
    } else if error.downcast_ref::<reqwest::Error>().is_some() {
        "Không kết nối được máy chủ đồng bộ. Kiểm tra mạng và thử lại.".into()
    } else {
        error.to_string()
    }
}

pub async fn import_sync_v2_messages(s: &Session, url: &str) -> Result<usize> {
    if !crate::session::is_trusted_zalo_url(url) {
        anyhow::bail!("Refusing authenticated history download from an untrusted Zalo URL");
    }
    let request_id = active_v2_id()?;
    let transfer =
        v2_transfer(&request_id).ok_or_else(|| anyhow::anyhow!("Phiên Sync V2 đã kết thúc."))?;
    let session_record = current_v2_session()?;
    let metadata_session = SYNC_V2_SESSIONS
        .lock()
        .ok()
        .and_then(|registry| {
            registry
                .iter()
                .find(|(id, _)| id == &request_id)
                .map(|(_, session)| session.clone())
        })
        .ok_or_else(|| anyhow::anyhow!("Chưa nhận metadata của phiên Sync V2."))?;
    let uin = s.login.as_ref().map(|l| l.uin.clone()).unwrap_or_default();
    let sek = s.zpw_sek.clone().unwrap_or_default();
    let cookie_hdr = format!("zpw_sek={sek}; zpw_uin={uin}");

    let resp = s
        .http
        .get(url)
        .header("Cookie", cookie_hdr)
        .header("User-Agent", "ZaloPC-win32-26.10.11")
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("failed to download v2 messages: HTTP {}", resp.status());
    }

    let bytes = read_response_limited(resp, MAX_SYNC_DOWNLOAD_BYTES).await?;
    // wasmtime host (không cần Node.js). Multi-chunk: decrypt + unpack từng
    // chunk, nối protobuf bytes lại (đúng luồng HistoryCodec mẫu).
    let my_name = s
        .login
        .as_ref()
        .map(|l| l.display_name.clone())
        .unwrap_or_else(|| "Tôi".into());
    tokio::task::spawn_blocking(move || -> Result<usize> {
        let host = crate::zproto_host::ZprotoHost::load()?;
        let mut parts: Vec<u8> = Vec::new();
        for chunk in crate::zproto_host::split_frames(&bytes)? {
            let res = host.decrypt_message(&chunk, &session_record)?;
            if res.session_id != metadata_session {
                anyhow::bail!("Gói tin thuộc phiên Sync V2 khác.");
            }
            let decoded = decompress_sync_payload(&res.plaintext)?;
            let combined_len = parts
                .len()
                .checked_add(decoded.len())
                .ok_or_else(|| anyhow::anyhow!("decompressed sync size overflow"))?;
            if combined_len > MAX_DECOMPRESSED_SYNC_BYTES {
                anyhow::bail!("decompression limit exceeded ({MAX_DECOMPRESSED_SYNC_BYTES} bytes)");
            }
            parts.extend_from_slice(&decoded);
        }
        let decrypted_bytes = parts;
        // Nếu batch trả raw còn nén thì Rust tự nhận dạng thêm
        // (zstd/zlib/gzip — Zalo dùng cả 3 theo HistoryCodec mẫu).
        let decompressed = decompress_sync_payload(&decrypted_bytes)?;

        let partitions = parse_sync_chunk(&decompressed)
            .ok_or_else(|| anyhow::anyhow!("Dữ liệu Sync V2 không đúng định dạng."))?;
        let db_path = crate::store::db_path_for_account(&uin);
        let mut store = crate::store::MessageStore::open(&db_path)?;
        if v2_transfer(&request_id).is_none() {
            anyhow::bail!("Phiên Sync V2 đã kết thúc.");
        }
        let imported =
            persist_v2_partitions(&mut store, partitions, &transfer.partitions, &uin, &my_name)?;

        let mut cached_threads = crate::chat::load_threads_cache();
        if store.hydrate_threads(&mut cached_threads, 50).is_ok() {
            crate::chat::save_threads_cache(&cached_threads);
        } else {
            // Cache refresh is best effort after the message transaction has committed.
            crate::logger::log_error("sync_v2", "Messages saved but thread cache refresh failed");
        }

        Ok(imported)
    })
    .await?
}
#[cfg(test)]
mod tests {

    #[test]
    fn imported_batches_are_atomic_and_idempotent_and_use_real_thread_ids() {
        fn part(id: &str) -> super::ParsedProtoPartition {
            super::ParsedProtoPartition {
                id: id.into(),
                messages: vec![super::ParsedProtoMsg {
                    global_id: "msg-1".into(),
                    sender_id: "sender-1".into(),
                    content: "hello".into(),
                    timestamp: 1234,
                    ..Default::default()
                }],
            }
        }
        let mut store = crate::store::MessageStore::open(":memory:").unwrap();
        let map = std::collections::HashMap::from([("oneone/noise-a".into(), "thread-1".into())]);
        assert!(super::persist_v2_partitions(
            &mut store,
            vec![part("noise-a"), part("unknown")],
            &map,
            "me",
            "Me"
        )
        .is_err());
        assert!(store.messages_for("thread-1", 10).unwrap().is_empty());
        assert_eq!(
            super::persist_v2_partitions(&mut store, vec![part("noise-a")], &map, "me", "Me")
                .unwrap(),
            1
        );
        // (INV-4) Re-delivery cannot create a second message or inflate the inserted count.
        assert_eq!(
            super::persist_v2_partitions(&mut store, vec![part("noise-a")], &map, "me", "Me")
                .unwrap(),
            0
        );
        assert_eq!(store.messages_for("thread-1", 10).unwrap()[0].text, "hello");
        assert!(store.messages_for("noise-a", 10).unwrap().is_empty());
    }

    #[test]
    fn sync_completion_waits_for_all_pending_batches() {
        let mut state = super::V2Transfer {
            pending: 2,
            ..Default::default()
        };
        state.complete_batch(Ok(3), true);
        assert!(state.saw_last);
        assert_eq!(state.pending, 1);
        state.complete_batch(Ok(4), false);
        assert_eq!(state.pending, 0);
        assert_eq!(state.imported, 7);
        assert!(state.failure.is_none());
        state.pending = 1;
        state.complete_batch(Err(anyhow::anyhow!("private-url")), true);
        assert!(!state.failure.unwrap().contains("private-url"));
    }

    #[test]
    fn sync_partitions_are_mapped_to_requested_threads_and_bad_lengths_fail() {
        let mapping = std::collections::HashMap::from([("oneone/noise-a".into(), "123".into())]);
        assert_eq!(
            super::resolve_v2_partition(&mapping, "oneone/noise-a").unwrap(),
            "123"
        );
        assert_eq!(
            super::resolve_v2_partition(&mapping, "noise-a").unwrap(),
            "123"
        );
        assert!(super::resolve_v2_partition(&mapping, "unknown").is_err());
        let bad = [vec![0x12], vec![0xff; 9], vec![0x7f]].concat();
        assert!(super::parse_sync_chunk(&bad).is_none());
        assert!(super::parse_vli(&[0xff; 12], 0).is_none());
    }

    #[test]
    fn sync_identity_is_private_stable_and_corruption_is_not_overwritten() {
        let temp = crate::data_dir::PrivateTempDir::new("sync-identity-test").unwrap();
        let path = temp.path().join("identity.json");
        let mut public = vec![7; 33];
        public[0] = 5;
        let private = vec![9; 32];
        let first = super::load_or_create_identity_key(&path, || Ok((public, private))).unwrap();
        let second =
            super::load_or_create_identity_key(&path, || panic!("identity must be reused"))
                .unwrap();
        assert_eq!(first, second);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::write(&path, b"broken").unwrap();
        assert!(super::load_or_create_identity_key(&path, || panic!(
            "do not overwrite a broken identity"
        ))
        .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
    }

    use super::*;
    use crate::chat::ChatMessage;

    #[test]
    fn history_starts_with_recent_and_resumes_with_old() {
        assert_eq!(history_page_request(""), ("0", false));
        assert_eq!(history_page_request("0"), ("0", false));
        assert_eq!(history_page_request("12345"), ("12345", true));
    }

    #[test]
    fn history_errors_explain_rejection_without_request_url() {
        let stop: anyhow::Error = crate::session::SessionError::new(-69, "Stop retry").into();
        assert!(is_end_of_history(&stop, true));
        assert!(
            !is_end_of_history(&stop, false),
            "first page -69 stays an error"
        );
        let other: anyhow::Error = crate::session::SessionError::new(-1, "x").into();
        assert!(!is_end_of_history(&other, true));
        let error: anyhow::Error = crate::session::SessionError::new(-69, "Stop retry")
            .at("https://example.invalid/api/cm/getoldv2?params=private-payload")
            .into();
        let text = history_sync_error_message(&error);
        assert!(!phone_sync_error_message(&error).contains("private-payload"));
        let network = reqwest::Client::new().get("http://[").build().unwrap_err();
        assert!(!phone_sync_error_message(&network.into()).contains("http://"));
        assert!(text.contains("-69"));
        assert!(text.contains("đồng bộ từ điện thoại"));
        assert!(!text.contains("https://"));
        assert!(!text.contains("private-payload"));
        assert!(!history_sync_error_message(&anyhow::anyhow!("secret-url")).contains("secret-url"));
    }

    fn thread_with(ids: &[&str]) -> Thread {
        Thread {
            id: "g1".into(),
            name: "Nhóm".into(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            is_group: true,
            messages: ids
                .iter()
                .map(|id| ChatMessage {
                    id: (*id).into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn cursor_moves_to_the_oldest_server_message() {
        assert_eq!(
            next_cursor(&thread_with(&["5", "9"]), "9").as_deref(),
            Some("5")
        );
        assert_eq!(
            next_cursor(&thread_with(&["out_1", "7", "9"]), "9").as_deref(),
            Some("7")
        );
    }

    #[test]
    fn cursor_stops_when_page_brought_nothing_older() {
        assert_eq!(next_cursor(&thread_with(&["5", "9"]), "5"), None);
        assert_eq!(next_cursor(&thread_with(&[]), "5"), None);
        assert_eq!(next_cursor(&thread_with(&["", "out_2"]), "5"), None);
    }

    #[test]
    fn generate_sync_public_key_produces_valid_rsa2048_spki_der_base64() {
        use base64::Engine;
        let k1 = generate_sync_public_key();
        let k2 = generate_sync_public_key();
        assert_ne!(k1, k2);
        assert!(
            k1.starts_with("MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA"),
            "must have valid ASN.1 RSA-2048 SPKI DER prefix: {k1}"
        );
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&k1)
            .expect("valid base64");
        assert_eq!(raw.len(), 294);
        assert_eq!(&raw[raw.len() - 5..], &[0x02, 0x03, 0x01, 0x00, 0x01]);
    }

    #[test]
    fn extract_mobile_sync_controls_parses_v1_and_v2_events() {
        let payload = serde_json::json!({
            "controls": [
                {
                    "content": {
                        "act_type": "transfer_sync2",
                        "act": "transfer_status",
                        "data": "{\"syncId\":\"abc123\",\"fromDevice\":0,\"status\":3}"
                    }
                },
                {
                    "content": {
                        "act_type": "syncmsgmb",
                        "act": "user_confirm",
                        "data": "{\"user_action\":2,\"pc_name\":\"ADMIN-PC\"}"
                    }
                },
                {
                    "content": {
                        "act_type": "transfer_sync2",
                        "act": "upload_batch",
                        "data": "{\"syncId\":\"abc123\",\"err\":0,\"msgUrl\":\"https://f.zalo.me/batch1\"}"
                    }
                }
            ]
        });
        let evs = crate::chat::extract_mobile_sync_controls(&payload);
        assert_eq!(
            evs,
            vec![
                crate::chat::MobileSyncControlEvent::WaitingConfirmOnPhone {
                    sync_id: "abc123".into()
                },
                crate::chat::MobileSyncControlEvent::MobileBusyRestoring,
                crate::chat::MobileSyncControlEvent::SyncV2Batch {
                    url: "https://f.zalo.me/batch1".into(),
                    sync_id: "abc123".into(),
                    batch_type: 0,
                    is_last: false,
                },
            ]
        );
    }

    #[test]
    fn decompress_sync_payload_detects_zlib_gzip_and_raw() {
        use std::io::Write;
        let raw = b"hello sync";
        // zlib
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(raw).unwrap();
        let zlib_bytes = enc.finish().unwrap();
        assert_eq!(decompress_sync_payload(&zlib_bytes).unwrap(), raw);
        // gzip
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(raw).unwrap();
        let gzip_bytes = enc.finish().unwrap();
        assert_eq!(decompress_sync_payload(&gzip_bytes).unwrap(), raw);
        // raw passthrough
        assert_eq!(decompress_sync_payload(raw).unwrap(), raw);
        // zstd (encode bằng crate zstd đã có)
        let zstd_bytes = zstd::encode_all(std::io::Cursor::new(raw), 1).unwrap();
        assert_eq!(decompress_sync_payload(&zstd_bytes).unwrap(), raw);
    }

    #[test]
    fn decompression_stops_when_output_exceeds_limit() {
        use std::io::Write;
        let raw = vec![b'x'; 128];
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&raw).unwrap();
        let compressed = enc.finish().unwrap();
        let err = decompress_sync_payload_with_limit(&compressed, 32)
            .unwrap_err()
            .to_string();
        assert!(err.contains("decompression limit"), "{err}");

        let mut xz = Vec::new();
        lzma_rs::xz_compress(&mut std::io::Cursor::new(&raw), &mut xz).unwrap();
        let err = decompress_xz_limited(&xz, 32).unwrap_err().to_string();
        assert!(err.contains("decompression limit"), "{err}");
    }

    #[test]
    fn sync_v2_session_registry_gates_batches() {
        assert!(!has_sync_v2_session("nope-sync"));
        record_sync_v2_session("sync-1", "sess-aaa");
        assert!(has_sync_v2_session("sync-1"));
        assert!(!has_sync_v2_session("sync-2"));
        // Ghi đè cùng sync_id không trùng lặp.
        record_sync_v2_session("sync-1", "sess-bbb");
        assert!(has_sync_v2_session("sync-1"));
    }
}
