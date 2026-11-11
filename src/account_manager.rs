//! Multi-account management for ctdozlite.
//!
//! Stores isolated account sessions, thread caches, and history databases in `accounts/<uin>.*`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountProfile {
    pub uin: String,
    pub display_name: String,
    pub avatar: Option<String>,
    pub last_active_ts: i64,
    pub is_current: bool,
}

pub fn accounts_dir() -> PathBuf {
    PathBuf::from("accounts")
}

/// List all saved accounts in the `accounts/` directory.
pub fn list_saved_accounts(current_uin: &str) -> Vec<AccountProfile> {
    let dir = accounts_dir();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if file_stem.ends_with("_threads") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(login) = val.get("login") {
                        let uin = login
                            .get("uin")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        if !uin.is_empty() {
                            let display_name = login
                                .get("display_name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Người dùng Zalo")
                                .to_string();
                            let avatar = login
                                .get("avatar")
                                .and_then(|v| v.as_str())
                                .map(str::to_string);
                            let is_current = uin == current_uin;
                            let last_active_ts = entry
                                .metadata()
                                .and_then(|m| m.modified())
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs() as i64)
                                .unwrap_or(0);
                            out.push(AccountProfile {
                                uin,
                                display_name,
                                avatar,
                                last_active_ts,
                                is_current,
                            });
                        }
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| b.last_active_ts.cmp(&a.last_active_ts));
    out
}

/// Save the current session and caches into `accounts/<uin>.*`
pub fn save_current_account_snapshot(uin: &str) -> Result<()> {
    if uin.is_empty() {
        return Ok(());
    }
    let dir = accounts_dir();
    crate::data_dir::ensure_private_dir(&dir)?;
    if Path::new("session.json").exists() {
        crate::data_dir::ensure_private_file(Path::new("session.json"))?;
        crate::data_dir::copy_private(Path::new("session.json"), &dir.join(format!("{uin}.json")))?;
    }
    if Path::new("threads_cache.json").exists() {
        crate::data_dir::copy_private(
            Path::new("threads_cache.json"),
            &dir.join(format!("{uin}_threads.json")),
        )?;
    }
    if Path::new("zalo_history.db").exists() {
        crate::data_dir::copy_private(
            Path::new("zalo_history.db"),
            &dir.join(format!("{uin}_history.db")),
        )?;
    }
    Ok(())
}

/// Switch to a target account:
/// 1. Saves current account snapshot to accounts/<current_uin>.*
/// 2. Restores accounts/<target_uin>.* into session.json, threads_cache.json, zalo_history.db
pub fn switch_to_account(current_uin: &str, target_uin: &str) -> Result<()> {
    if target_uin.is_empty() {
        anyhow::bail!("Target UIN is empty");
    }
    let dir = accounts_dir();
    if !current_uin.is_empty() {
        let _ = save_current_account_snapshot(current_uin);
    }
    let _ = std::fs::remove_file("session.json");
    let _ = std::fs::remove_file("threads_cache.json");
    let _ = std::fs::remove_file("zalo_history.db");

    let target_session = dir.join(format!("{target_uin}.json"));
    if !target_session.exists() {
        anyhow::bail!("Không tìm thấy dữ liệu phiên của tài khoản {target_uin}");
    }
    crate::data_dir::ensure_private_file(&target_session)?;
    crate::data_dir::copy_private(&target_session, Path::new("session.json"))?;

    let target_threads = dir.join(format!("{target_uin}_threads.json"));
    if target_threads.exists() {
        crate::data_dir::copy_private(&target_threads, Path::new("threads_cache.json"))?;
    }

    let target_db = dir.join(format!("{target_uin}_history.db"));
    if target_db.exists() {
        crate::data_dir::copy_private(&target_db, Path::new("zalo_history.db"))?;
    }
    crate::data_dir::write_private_file(Path::new(ACTIVE_ACCOUNT_MARKER), target_uin.as_bytes())?;
    Ok(())
}

/// Delete a saved account from `accounts/`
pub fn delete_saved_account(uin: &str) -> Result<()> {
    if uin.is_empty() {
        return Ok(());
    }
    let dir = accounts_dir();
    let _ = std::fs::remove_file(dir.join(format!("{uin}.json")));
    let _ = std::fs::remove_file(dir.join(format!("{uin}_threads.json")));
    let _ = std::fs::remove_file(dir.join(format!("{uin}_history.db")));
    Ok(())
}

/// Records which account owns the live `zalo_history.db` / `threads_cache.json`.
const ACTIVE_ACCOUNT_MARKER: &str = "active_account";
const LIVE_DB: &str = "zalo_history.db";
const LIVE_THREADS: &str = "threads_cache.json";

/// What `activate_account_data` did, for logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationOutcome {
    /// The live files already belonged to this account; nothing changed.
    AlreadyActive,
    /// The parked `accounts/<uin>_*` copy was restored as the live files.
    Restored,
    /// Legacy layout (no owner marker): the parked database was merged into the live one.
    Merged { messages: usize },
    /// Neither live nor parked history exists yet.
    Fresh,
}

/// Make the live history files belong to `uin` after login or at startup.
///
/// The active account always reads and writes `zalo_history.db`; `accounts/<uin>_*` only parks
/// accounts that are not active. Before this existed, logout parked and deleted the live DB but
/// login never restored it, so a re-login started from an empty history (and the next snapshot
/// overwrote the good parked copy).
pub fn activate_account_data(uin: &str) -> Result<ActivationOutcome> {
    activate_account_data_in(Path::new("."), uin)
}

fn activate_account_data_in(base: &Path, uin: &str) -> Result<ActivationOutcome> {
    if uin.is_empty() {
        return Ok(ActivationOutcome::Fresh);
    }
    let live_db = base.join(LIVE_DB);
    let live_threads = base.join(LIVE_THREADS);
    let marker = base.join(ACTIVE_ACCOUNT_MARKER);
    let dir = base.join(accounts_dir());
    let parked_db = dir.join(format!("{uin}_history.db"));
    let parked_threads = dir.join(format!("{uin}_threads.json"));
    let owner = std::fs::read_to_string(&marker)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if live_db.exists() && owner == uin {
        return Ok(ActivationOutcome::AlreadyActive);
    }

    // Live files of another account: park them (history only; that account's session file
    // was saved when it was active) before restoring ours.
    if live_db.exists() && !owner.is_empty() && owner != uin {
        crate::data_dir::ensure_private_dir(&dir)?;
        crate::data_dir::copy_private(&live_db, &dir.join(format!("{owner}_history.db")))?;
        if live_threads.exists() {
            crate::data_dir::copy_private(
                &live_threads,
                &dir.join(format!("{owner}_threads.json")),
            )?;
        }
        std::fs::remove_file(&live_db)?;
        let _ = std::fs::remove_file(&live_threads);
    }

    let outcome = if !live_db.exists() {
        if parked_db.exists() {
            crate::data_dir::copy_private(&parked_db, &live_db)?;
            if parked_threads.exists() && !live_threads.exists() {
                crate::data_dir::copy_private(&parked_threads, &live_threads)?;
            }
            ActivationOutcome::Restored
        } else {
            ActivationOutcome::Fresh
        }
    } else if parked_db.exists() {
        // No owner marker: data written by builds that split reads (parked DB) from writes
        // (live DB). Merge so neither side's messages are lost; inserts are idempotent.
        let report =
            crate::store::import_history_from_path(&parked_db, live_db.to_string_lossy().as_ref())?;
        ActivationOutcome::Merged {
            messages: report.messages_imported,
        }
    } else {
        ActivationOutcome::AlreadyActive
    };
    crate::data_dir::write_private_file(&marker, uin.as_bytes())?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_base(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ctdozlite-acct-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(p.join("accounts")).unwrap();
        p
    }

    #[test]
    fn relogin_restores_parked_history_instead_of_starting_empty() {
        let base = temp_base("restore");
        std::fs::write(base.join("accounts/111_history.db"), b"parked").unwrap();
        std::fs::write(base.join("accounts/111_threads.json"), b"[]").unwrap();
        // Logged out: no live files.
        assert_eq!(
            activate_account_data_in(&base, "111").unwrap(),
            ActivationOutcome::Restored
        );
        assert_eq!(std::fs::read(base.join(LIVE_DB)).unwrap(), b"parked");
        assert_eq!(
            activate_account_data_in(&base, "111").unwrap(),
            ActivationOutcome::AlreadyActive
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn other_accounts_live_history_is_parked_before_switching() {
        let base = temp_base("park");
        std::fs::write(base.join(LIVE_DB), b"alice-live").unwrap();
        std::fs::write(base.join(ACTIVE_ACCOUNT_MARKER), b"aaa").unwrap();
        std::fs::write(base.join("accounts/bbb_history.db"), b"bob-parked").unwrap();
        assert_eq!(
            activate_account_data_in(&base, "bbb").unwrap(),
            ActivationOutcome::Restored
        );
        assert_eq!(
            std::fs::read(base.join("accounts/aaa_history.db")).unwrap(),
            b"alice-live"
        );
        assert_eq!(std::fs::read(base.join(LIVE_DB)).unwrap(), b"bob-parked");
        assert_eq!(
            std::fs::read_to_string(base.join(ACTIVE_ACCOUNT_MARKER)).unwrap(),
            "bbb"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn legacy_split_databases_are_merged_without_loss() {
        let base = temp_base("merge");
        let live = base.join(LIVE_DB);
        let parked = base.join("accounts/111_history.db");
        let msg = |id: &str| crate::store::StoredMessage {
            msg_id: id.into(),
            thread_id: "t1".into(),
            sender_id: "1".into(),
            sender_name: "A".into(),
            text: id.into(),
            msg_type: "webchat".into(),
            ts: 1,
            is_outgoing: false,
            raw_json: String::new(),
        };
        {
            let mut s = crate::store::MessageStore::open(live.to_str().unwrap()).unwrap();
            s.upsert_message(&msg("live-only")).unwrap();
            s.upsert_message(&msg("both")).unwrap();
        }
        {
            let mut s = crate::store::MessageStore::open(parked.to_str().unwrap()).unwrap();
            s.upsert_message(&msg("parked-only")).unwrap();
            s.upsert_message(&msg("both")).unwrap();
        }
        assert_eq!(
            activate_account_data_in(&base, "111").unwrap(),
            ActivationOutcome::Merged { messages: 1 }
        );
        let s = crate::store::MessageStore::open(live.to_str().unwrap()).unwrap();
        let n: i64 = s
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 3);
        let _ = std::fs::remove_dir_all(&base);
    }
}
