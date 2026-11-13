//! Local drafts, Saved, reminders and bound-parameter message search.
use crate::store::StoredMessage;
use anyhow::{ensure, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Draft owned by one conversation in the active account database.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Draft {
    pub thread_id: String,
    pub text: String,
    pub reply: Option<crate::chat::ChatMessage>,
    #[serde(default)]
    pub auto_mention: Option<(String, String)>,
    pub mentions: Vec<(String, String)>,
}
/// A locally saved message with an editable note.
#[derive(Debug, Clone)]
pub struct Saved {
    pub message: StoredMessage,
    pub note: String,
}
/// Local reminder; due_at uses Unix seconds, message timestamps remain milliseconds.
#[derive(Debug, Clone)]
pub struct Reminder {
    pub id: i64,
    pub thread_id: String,
    pub msg_id: String,
    pub due_at: i64,
    pub preview: String,
}
/// Complete local snapshot, hydrated only by the worker.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub drafts: Vec<Draft>,
    pub saved: Vec<Saved>,
    pub reminders: Vec<Reminder>,
}

/// Additive, repeatable migration; existing messages and read points are preserved.
pub(crate) fn init_schema(conn: &Connection) -> Result<()> {
    if conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? >= 2 {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS drafts(thread_id TEXT PRIMARY KEY,payload TEXT NOT NULL); CREATE TABLE IF NOT EXISTS saved_items(msg_id TEXT PRIMARY KEY,thread_id TEXT NOT NULL,note TEXT NOT NULL DEFAULT ''); CREATE TABLE IF NOT EXISTS message_reminders(id INTEGER PRIMARY KEY,thread_id TEXT NOT NULL,msg_id TEXT NOT NULL,due_at INTEGER NOT NULL,preview TEXT NOT NULL); CREATE INDEX IF NOT EXISTS idx_reminder_due ON message_reminders(due_at);")?;
    let columns = |table: &str| -> Result<Vec<String>> {
        let mut statement = tx.prepare(&format!("PRAGMA table_info({table})"))?;
        let mapped = statement.query_map([], |r| r.get(1))?;
        Ok(mapped.collect::<rusqlite::Result<Vec<_>>>()?)
    };
    let drafts = columns("drafts")?;
    if !drafts.iter().any(|c| c == "payload") {
        tx.execute_batch("ALTER TABLE drafts ADD COLUMN payload TEXT NOT NULL DEFAULT '{}';")?;
        if drafts.iter().any(|c| c == "text") {
            tx.execute_batch("UPDATE drafts SET payload=json_object('thread_id',thread_id,'text',text,'reply',NULL,'mentions',json('[]'));")?;
        }
    }
    let reminders = columns("message_reminders")?;
    if !reminders.iter().any(|c| c == "preview") {
        tx.execute_batch(
            "ALTER TABLE message_reminders ADD COLUMN preview TEXT NOT NULL DEFAULT '';",
        )?;
        if reminders.iter().any(|c| c == "text_preview") {
            tx.execute_batch("UPDATE message_reminders SET preview=text_preview;")?;
        }
    }
    tx.execute_batch("INSERT OR IGNORE INTO saved_items(msg_id,thread_id,note) SELECT msg_id,thread_id,'' FROM messages WHERE CASE WHEN json_valid(raw_json) THEN COALESCE(json_extract(raw_json,'$.is_starred'),0) ELSE 0 END=1; PRAGMA user_version=2;")?;
    tx.commit()?;
    Ok(())
}
/// Persist even whitespace drafts faithfully; an empty draft removes its row.
pub fn save_draft(conn: &Connection, draft: &Draft) -> Result<()> {
    ensure!(draft.text.len() <= 1024 * 1024, "Bản nháp quá dài.");
    if draft.text.is_empty() && draft.reply.is_none() {
        conn.execute("DELETE FROM drafts WHERE thread_id=?1", [&draft.thread_id])?;
    } else {
        conn.execute("INSERT INTO drafts(thread_id,payload) VALUES(?1,?2) ON CONFLICT(thread_id) DO UPDATE SET payload=excluded.payload", params![draft.thread_id, serde_json::to_string(draft)?])?;
    }
    Ok(())
}
/// Save or remove a message; the source is the active account's committed history.
pub fn save_item(conn: &Connection, thread: &str, msg: &str, note: Option<&str>) -> Result<()> {
    let transaction = conn.unchecked_transaction()?;
    if let Some(note) = note {
        ensure!(note.len() <= 16384, "Ghi chú quá dài.");
        let exists = conn
            .query_row(
                "SELECT 1 FROM messages WHERE msg_id=?1 AND thread_id=?2",
                params![msg, thread],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        ensure!(exists, "Tin nhắn chưa được lưu trong lịch sử cục bộ.");
        conn.execute("INSERT INTO saved_items(msg_id,thread_id,note) VALUES(?1,?2,?3) ON CONFLICT(msg_id) DO UPDATE SET note=excluded.note", params![msg,thread,note])?;
    } else {
        conn.execute(
            "DELETE FROM saved_items WHERE msg_id=?1 AND thread_id=?2",
            params![msg, thread],
        )?;
    }
    conn.execute("UPDATE messages SET raw_json=json_set(CASE WHEN json_valid(raw_json) THEN raw_json ELSE '{}' END,'$.is_starred',json(?1)) WHERE msg_id=?2 AND thread_id=?3", params![if note.is_some() { "true" } else { "false" }, msg, thread])?;
    transaction.commit()?;
    Ok(())
}
/// Create a future reminder without sending any message to other users.
pub fn add_reminder(
    conn: &Connection,
    thread: &str,
    msg: &str,
    due: i64,
    preview: &str,
    now: i64,
) -> Result<()> {
    ensure!(
        !thread.is_empty() && !msg.is_empty() && due > now,
        "Chọn thời gian nhắc trong tương lai."
    );
    let preview: String = preview.chars().take(140).collect();
    conn.execute(
        "INSERT INTO message_reminders(thread_id,msg_id,due_at,preview) VALUES(?1,?2,?3,?4)",
        params![thread, msg, due, preview],
    )?;
    Ok(())
}
/// Claim due reminders atomically; restart or the next tick cannot deliver them again.
pub fn claim_due(conn: &mut Connection, now: i64) -> Result<Vec<Reminder>> {
    let tx = conn.transaction()?;
    let due = {
        let mut stmt = tx.prepare("SELECT id,thread_id,msg_id,due_at,preview FROM message_reminders WHERE due_at<=?1 ORDER BY due_at LIMIT 100")?;
        let mapped = stmt.query_map([now], read_reminder)?;
        mapped.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for item in &due {
        tx.execute("DELETE FROM message_reminders WHERE id=?1", [item.id])?;
    }
    tx.commit()?;
    Ok(due)
}
fn read_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredMessage> {
    Ok(StoredMessage {
        msg_id: row.get(0)?,
        thread_id: row.get(1)?,
        sender_id: row.get(2)?,
        sender_name: row.get(3)?,
        text: row.get(4)?,
        msg_type: row.get(5)?,
        ts: row.get(6)?,
        is_outgoing: row.get::<_, i64>(7)? != 0,
        raw_json: row.get(8)?,
    })
}
fn read_reminder(row: &rusqlite::Row<'_>) -> rusqlite::Result<Reminder> {
    Ok(Reminder {
        id: row.get(0)?,
        thread_id: row.get(1)?,
        msg_id: row.get(2)?,
        due_at: row.get(3)?,
        preview: row.get(4)?,
    })
}
/// Load account-local state without touching another account's parked files.
pub fn snapshot(conn: &Connection) -> Result<Snapshot> {
    let mut drafts = Vec::new();
    let mut stmt = conn.prepare("SELECT payload FROM drafts")?;
    for value in stmt.query_map([], |r| r.get::<_, String>(0))? {
        drafts.push(serde_json::from_str(&value?)?);
    }
    let mut stmt = conn.prepare("SELECT m.msg_id,m.thread_id,m.sender_id,m.sender_name,m.text,m.msg_type,m.ts,m.is_outgoing,m.raw_json,s.note FROM saved_items s JOIN messages m ON m.msg_id=s.msg_id AND m.thread_id=s.thread_id ORDER BY m.ts DESC LIMIT 1000")?;
    let saved = stmt
        .query_map([], |r| {
            Ok(Saved {
                message: read_message(r)?,
                note: r.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut stmt = conn.prepare("SELECT id,thread_id,msg_id,due_at,preview FROM message_reminders ORDER BY due_at LIMIT 1000")?;
    let reminders = stmt
        .query_map([], read_reminder)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Snapshot {
        drafts,
        saved,
        reminders,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Attachment-kind operator accepted by advanced local search.
pub enum SearchHas {
    Link,
    File,
    Image,
    Voice,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
/// Parsed operators and literal terms; all values become bound SQL parameters.
pub struct SearchQuery {
    pub from: Option<String>,
    pub in_thread: Option<String>,
    pub has: Vec<SearchHas>,
    pub before_ms: Option<i64>,
    pub after_ms: Option<i64>,
    pub is_starred: bool,
    pub is_pinned: bool,
    pub phrases: Vec<String>,
    pub hashtags: Vec<String>,
    pub words: Vec<String>,
}

fn search_day_bound_ms(day: &str, end: bool) -> Option<i64> {
    use chrono::TimeZone;
    let mut it = day.split('-');
    let (y, mo, d): (i32, u32, u32) = (
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    );
    if it.next().is_some() {
        return None;
    }
    let date = chrono::NaiveDate::from_ymd_opt(y, mo, d)?;
    let dt = if end {
        date.and_hms_opt(23, 59, 59)?
    } else {
        date.and_hms_opt(0, 0, 0)?
    };
    let local = chrono::Local.from_local_datetime(&dt).single()?;
    Some(local.timestamp_millis() + if end { 999 } else { 0 })
}

/// Parse quoted phrases and from/in/has/before/after/is/hashtag operators.
pub fn parse_search_query(raw: &str) -> SearchQuery {
    let mut q = SearchQuery::default();
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    for ch in raw.chars() {
        if ch == '"' {
            in_q = !in_q;
            cur.push(ch);
        } else if ch.is_whitespace() && !in_q {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    for tok in tokens {
        if tok.len() >= 2 && tok.starts_with('"') && tok.ends_with('"') {
            let p = tok[1..tok.len() - 1].trim();
            if !p.is_empty() {
                q.phrases.push(p.to_string());
            }
            continue;
        }
        if let Some(v) = tok.strip_prefix("from:") {
            let v = v.trim().trim_matches('"');
            if !v.is_empty() {
                q.from = Some(v.to_string());
            }
            continue;
        }
        if let Some(v) = tok.strip_prefix("in:") {
            let v = v.trim().trim_matches('"');
            if !v.is_empty() {
                q.in_thread = Some(v.to_string());
            }
            continue;
        }
        if let Some(v) = tok.strip_prefix("has:") {
            match v.trim().to_lowercase().as_str() {
                "link" => q.has.push(SearchHas::Link),
                "file" => q.has.push(SearchHas::File),
                "image" | "img" | "photo" => q.has.push(SearchHas::Image),
                "voice" | "audio" => q.has.push(SearchHas::Voice),
                _ => q.words.push(tok.clone()),
            }
            continue;
        }
        if let Some(v) = tok.strip_prefix("before:") {
            if let Some(ms) = search_day_bound_ms(v.trim(), true) {
                q.before_ms = Some(ms);
            } else {
                q.words.push(tok.clone());
            }
            continue;
        }
        if let Some(v) = tok.strip_prefix("after:") {
            if let Some(ms) = search_day_bound_ms(v.trim(), false) {
                q.after_ms = Some(ms);
            } else {
                q.words.push(tok.clone());
            }
            continue;
        }
        if let Some(v) = tok.strip_prefix("is:") {
            match v.trim().to_lowercase().as_str() {
                "starred" | "star" | "saved" => q.is_starred = true,
                "pinned" | "pin" | "ghim" => q.is_pinned = true,
                _ => q.words.push(tok.clone()),
            }
            continue;
        }
        if let Some(tag) = tok.strip_prefix('#') {
            let t = tag.trim_matches(|c: char| ".,;:!?()[]".contains(c));
            if !t.is_empty()
                && t.chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            {
                q.hashtags.push(t.to_string());
            } else {
                q.words.push(tok.clone());
            }
            continue;
        }
        q.words.push(tok);
    }
    q
}

/// Search local history with SQL parameters; operator values never become SQL syntax.
pub fn search(conn: &Connection, raw: &str) -> Result<Vec<StoredMessage>> {
    use rusqlite::types::Value;
    let q = parse_search_query(raw);
    let mut sql = "SELECT m.msg_id,m.thread_id,m.sender_id,m.sender_name,m.text,m.msg_type,m.ts,m.is_outgoing,m.raw_json FROM messages m LEFT JOIN threads t ON t.thread_id=m.thread_id WHERE 1=1".to_owned();
    let mut args = Vec::<Value>::new();
    let like = |s: &str| {
        format!(
            "%{}%",
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    };
    if let Some(v) = q.from {
        sql.push_str(" AND (m.sender_id=? OR m.sender_name LIKE ? ESCAPE '\\')");
        args.push(v.clone().into());
        args.push(like(&v).into());
    }
    if let Some(v) = q.in_thread {
        sql.push_str(" AND (m.thread_id=? OR t.name LIKE ? ESCAPE '\\')");
        args.push(v.clone().into());
        args.push(like(&v).into());
    }
    for h in q.has {
        sql.push_str(match h { SearchHas::Image => " AND m.msg_type LIKE '%photo%'", SearchHas::File => " AND m.msg_type LIKE '%file%'", SearchHas::Voice => " AND (m.msg_type LIKE '%voice%' OR m.msg_type LIKE '%audio%' OR m.msg_type LIKE '%mp3%')", SearchHas::Link => " AND (m.msg_type LIKE '%link%' OR m.text LIKE '%https://%' OR m.text LIKE '%http://%')" });
    }
    if let Some(v) = q.before_ms {
        sql.push_str(" AND m.ts<=?");
        args.push(v.into());
    }
    if let Some(v) = q.after_ms {
        sql.push_str(" AND m.ts>=?");
        args.push(v.into());
    }
    if q.is_starred {
        sql.push_str(" AND (EXISTS(SELECT 1 FROM saved_items s WHERE s.msg_id=m.msg_id) OR CASE WHEN json_valid(m.raw_json) THEN COALESCE(json_extract(m.raw_json,'$.is_starred'),0) ELSE 0 END=1)");
    }
    if q.is_pinned {
        sql.push_str(" AND CASE WHEN json_valid(m.raw_json) THEN COALESCE(json_extract(m.raw_json,'$.is_pinned'),0) ELSE 0 END=1");
    }
    for word in q
        .words
        .into_iter()
        .chain(q.phrases)
        .chain(q.hashtags.into_iter().map(|t| format!("#{t}")))
    {
        sql.push_str(" AND m.text LIKE ? ESCAPE '\\'");
        args.push(like(&word).into());
    }
    sql.push_str(" ORDER BY m.ts DESC LIMIT 200");
    let mut stmt = conn.prepare(&sql)?;
    let found = stmt
        .query_map(rusqlite::params_from_iter(args), read_message)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(found)
}

/// Retrieve a target and bounded context from SQLite, including history outside the startup window.
fn locate(c: &Connection, thread: &str, msg: &str) -> Result<Update> {
    let target=c.query_row("SELECT msg_id,thread_id,sender_id,sender_name,text,msg_type,ts,is_outgoing,raw_json FROM messages WHERE msg_id=?1 AND thread_id=?2",params![msg,thread],read_message)?;
    let mut statement=c.prepare("SELECT msg_id,thread_id,sender_id,sender_name,text,msg_type,ts,is_outgoing,raw_json FROM messages WHERE thread_id=?1 ORDER BY ABS(CAST(ts AS REAL)-?2) LIMIT 50")?;
    let mut rows = statement
        .query_map(params![thread, target.ts], read_message)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !rows.iter().any(|r| r.msg_id == msg) {
        rows.push(target);
    }
    rows.sort_by_key(|r| r.ts);
    let mut messages = Vec::new();
    for row in rows {
        let mut message =
            serde_json::from_str::<crate::chat::ChatMessage>(&row.raw_json).unwrap_or_default();
        message.id = row.msg_id;
        message.text = row.text;
        message.sender = row.sender_name;
        message.ts = row.ts;
        message.time = crate::clock::message_time(row.ts);
        message.is_outgoing = row.is_outgoing;
        messages.push(message);
    }
    let metadata = c
        .query_row(
            "SELECT name,is_group FROM threads WHERE thread_id=?1",
            [thread],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()?;
    let (name, group) = metadata.unwrap_or_else(|| (thread.to_owned(), 0));
    Ok(Update::Located {
        thread: crate::chat::Thread {
            id: thread.to_owned(),
            name,
            is_group: group != 0,
            messages,
            ..Default::default()
        },
        reference: msg.to_owned(),
    })
}

/// Worker request; each variant describes one local user intent.
#[derive(Debug, Clone)]
pub enum Action {
    Load,
    Locate {
        thread: String,
        msg: String,
    },
    Draft(Draft),
    Save {
        thread: String,
        msg: String,
        note: Option<String>,
    },
    Search {
        generation: u64,
        query: String,
    },
    Remind {
        thread: String,
        msg: String,
        due: i64,
        preview: String,
    },
    RemoveReminder(i64),
    Tick,
    Quit(Draft),
}
/// Account-scoped terminal result; background ticks do not replace composer state.
#[derive(Debug, Clone)]
pub enum Update {
    Located {
        thread: crate::chat::Thread,
        reference: String,
    },
    Snapshot(Snapshot),
    Found {
        generation: u64,
        messages: Vec<StoredMessage>,
    },
    Due(Vec<Reminder>),
    Ack,
    QuitReady,
}
/// Execute on the worker's blocking pool, in command order with account changes.
pub fn execute(mut store: crate::store::MessageStore, action: Action, now: i64) -> Result<Update> {
    let c = &mut store.conn;
    match action {
        Action::Load => return Ok(Update::Snapshot(snapshot(c)?)),
        Action::Locate { thread, msg } => return locate(c, &thread, &msg),
        Action::Draft(d) => {
            save_draft(c, &d)?;
            return Ok(Update::Ack);
        }
        Action::Quit(d) => {
            save_draft(c, &d)?;
            return Ok(Update::QuitReady);
        }
        Action::Save { thread, msg, note } => save_item(c, &thread, &msg, note.as_deref())?,
        Action::Search { generation, query } => {
            return Ok(Update::Found {
                generation,
                messages: search(c, &query)?,
            })
        }
        Action::Remind {
            thread,
            msg,
            due,
            preview,
        } => add_reminder(c, &thread, &msg, due, &preview, now)?,
        Action::RemoveReminder(id) => {
            c.execute("DELETE FROM message_reminders WHERE id=?1", [id])?;
        }
        Action::Tick => return Ok(Update::Due(claim_due(c, now)?)),
    }
    Ok(Update::Snapshot(snapshot(c)?))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn seed() -> crate::store::MessageStore {
        let mut s = crate::store::MessageStore::open(":memory:").unwrap();
        s.upsert_message(&StoredMessage {
            msg_id: "m1".into(),
            thread_id: "t1".into(),
            sender_id: "u1".into(),
            sender_name: "Test sender".into(),
            text: "hello 100% #todo".into(),
            msg_type: "chat.photo".into(),
            ts: 1000,
            is_outgoing: false,
            raw_json: r#"{"is_starred":false}"#.into(),
        })
        .unwrap();
        s
    }
    #[test]
    fn before_date_includes_the_last_millisecond_of_the_day() {
        let beginning = search_day_bound_ms("2026-01-02", false).unwrap();
        let end = search_day_bound_ms("2026-01-02", true).unwrap();
        assert_eq!(end - beginning, 86_399_999);
    }

    #[test]
    fn quit_ack_follows_durable_draft_write_and_reopen_preserves_metadata() {
        let directory =
            crate::data_dir::PrivateTempDir::new("productivity-reopen-fixture").unwrap();
        let path = directory.path().join("history.db");
        let store = crate::store::MessageStore::open(path.to_str().unwrap()).unwrap();
        let draft = Draft {
            thread_id: "thread-fixture".into(),
            text: "Unsent fixture".into(),
            reply: Some(crate::chat::ChatMessage {
                id: "quoted-fixture".into(),
                ..Default::default()
            }),
            auto_mention: Some(("fake-user".into(), "Test sender".into())),
            mentions: vec![("fake-user".into(), "Test sender".into())],
        };
        assert!(matches!(
            execute(store, Action::Quit(draft), 100).unwrap(),
            Update::QuitReady
        ));
        let reopened = crate::store::MessageStore::open(path.to_str().unwrap()).unwrap();
        let state = snapshot(&reopened.conn).unwrap();
        assert_eq!(state.drafts[0].text, "Unsent fixture");
        assert_eq!(state.drafts[0].reply.as_ref().unwrap().id, "quoted-fixture");
        assert_eq!(
            state.drafts[0].auto_mention.as_ref().unwrap().0,
            "fake-user"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn saved_mutation_rolls_back_if_metadata_write_fails() {
        let store = seed();
        store.conn.execute_batch("CREATE TRIGGER reject_star BEFORE UPDATE ON messages BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert!(save_item(&store.conn, "t1", "m1", Some("note")).is_err());
        assert!(snapshot(&store.conn).unwrap().saved.is_empty());
    }

    #[test]
    fn jump_loads_an_exact_old_message_with_bounded_context() {
        let mut store = seed();
        for i in 2..200 {
            store
                .upsert_message(&StoredMessage {
                    msg_id: format!("m{i}"),
                    thread_id: "t1".into(),
                    sender_id: "u1".into(),
                    sender_name: "Test sender".into(),
                    text: format!("newer {i}"),
                    msg_type: "webchat".into(),
                    ts: i * 1000,
                    is_outgoing: false,
                    raw_json: String::new(),
                })
                .unwrap();
        }
        let Update::Located { thread, reference } = execute(
            store,
            Action::Locate {
                thread: "t1".into(),
                msg: "m1".into(),
            },
            1,
        )
        .unwrap() else {
            panic!("expected located context")
        };
        assert_eq!(reference, "m1");
        assert!(thread.messages.len() <= 51);
        assert!(thread.messages.iter().any(|m| m.id == "m1"));
    }

    #[test]
    fn additive_migration_preserves_upstream_drafts_saved_notes_and_reminders() {
        let s = seed();
        s.conn.execute_batch("DROP TABLE drafts; DROP TABLE saved_items; DROP TABLE message_reminders; PRAGMA user_version=1; CREATE TABLE drafts(thread_id TEXT PRIMARY KEY,text TEXT NOT NULL DEFAULT '',reply_ref TEXT NOT NULL DEFAULT '',updated_at INTEGER NOT NULL DEFAULT 0); INSERT INTO drafts VALUES('t1','legacy draft','m1',100); CREATE TABLE saved_items(msg_id TEXT PRIMARY KEY,thread_id TEXT NOT NULL,note TEXT NOT NULL DEFAULT '',due_at INTEGER NOT NULL DEFAULT 0,created_at INTEGER NOT NULL DEFAULT 0); INSERT INTO saved_items VALUES('m1','t1','legacy note',0,0); CREATE TABLE message_reminders(id INTEGER PRIMARY KEY,thread_id TEXT NOT NULL,msg_id TEXT NOT NULL,due_at INTEGER NOT NULL,text_preview TEXT NOT NULL DEFAULT '',created_at INTEGER NOT NULL DEFAULT 0); INSERT INTO message_reminders VALUES(1,'t1','m1',200,'legacy reminder',100);").unwrap();
        init_schema(&s.conn).unwrap();
        init_schema(&s.conn).unwrap();
        let state = snapshot(&s.conn).unwrap();
        assert_eq!(state.drafts[0].text, "legacy draft");
        assert_eq!(state.saved[0].note, "legacy note");
        assert_eq!(state.reminders[0].preview, "legacy reminder");
        save_draft(
            &s.conn,
            &Draft {
                thread_id: "t1".into(),
                text: "edited".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot(&s.conn).unwrap().drafts[0].text, "edited");
    }

    #[test]
    fn draft_saved_reminder_survive_reloading_and_are_isolated() {
        let mut a = seed();
        let b = seed();
        let draft = Draft {
            thread_id: "t1".into(),
            text: "unsent".into(),
            mentions: vec![("u1".into(), "Test sender".into())],
            ..Default::default()
        };
        save_draft(&a.conn, &draft).unwrap();
        save_item(&a.conn, "t1", "m1", Some("note")).unwrap();
        add_reminder(&a.conn, "t1", "m1", 200, "hello", 100).unwrap();
        let state = snapshot(&a.conn).unwrap();
        assert_eq!(state.drafts[0].text, "unsent");
        assert_eq!(state.drafts[0].mentions.len(), 1);
        assert_eq!(state.saved[0].note, "note");
        assert!(snapshot(&b.conn).unwrap().drafts.is_empty());
        assert!(snapshot(&b.conn).unwrap().saved.is_empty());
        assert!(claim_due(&mut a.conn, 199).unwrap().is_empty());
        assert_eq!(claim_due(&mut a.conn, 200).unwrap().len(), 1);
        assert!(claim_due(&mut a.conn, 201).unwrap().is_empty());
        save_draft(
            &a.conn,
            &Draft {
                thread_id: "t1".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(snapshot(&a.conn).unwrap().drafts.is_empty());
    }
    #[test]
    fn search_is_bound_literal_and_checks_boolean_values() {
        let s = seed();
        assert_eq!(
            search(&s.conn, "has:image from:u1 \"hello\" #todo")
                .unwrap()
                .len(),
            1
        );
        assert!(search(&s.conn, "is:starred").unwrap().is_empty());
        save_item(&s.conn, "t1", "m1", Some("")).unwrap();
        assert_eq!(search(&s.conn, "is:saved").unwrap().len(), 1);
        assert!(search(&s.conn, "x' OR 1=1 --").unwrap().is_empty());
        assert!(search(&s.conn, "100_").unwrap().is_empty());
        assert_eq!(search(&s.conn, "100%").unwrap().len(), 1);
        assert!(add_reminder(&s.conn, "t1", "m1", 100, "x", 100).is_err());
    }
}
