# AGENTS.md

Operating guide for AI agents and humans working in this repository.

> **AGENT SIGNAL — READ FIRST:** Before doing anything, read the latest entries in
> [`.agents/WORKLOG.md`](.agents/WORKLOG.md) to learn the current context (recent fixes, root
> causes, what is and isn't verified). After finishing **any** task, append an entry there using
> the template at the top of that file. This is required by the repository owner.

## Project

`ctdozlite-next` (crate `ctdozlite`, library `zalo_rust`) is a native Zalo desktop client written in Rust: `egui` GUI, `ratatui` TUI, a Tokio worker for network I/O, and a local SQLite message store. No Node.js, no Electron, no Chromium.

- Repository: <https://github.com/minhkhang1008/ctdozlite-next> (personal fork of <https://github.com/ctdoteam1110/ctdozlite>)
- Rust 1.75+ / edition 2021
- Product docs are Vietnamese (`docs/`, `README.md`); engineering standards are English (`docs/standards/`)

## Commands

| Task | Command |
| :--- | :--- |
| Build (debug) | `cargo build --locked` |
| Build (release binary) | `cargo build --release --locked --bin ctdozlite` |
| Test | `cargo test --locked` |
| Format | `cargo fmt --all` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Run GUI | `cargo run -- --gui` |
| Run TUI | `cargo run -- --tui --qr` |

`CMAKE_POLICY_VERSION_MINIMUM=3.5` is required when building against CMake 4.x (see `docs/KHAC_PHUC_SU_CO.md`).

## Architecture Map

| Path | Responsibility |
| :--- | :--- |
| `src/main.rs` | CLI entry; selects GUI or TUI mode |
| `src/lib.rs` | Library surface shared by binaries and tests |
| `src/gui.rs`, `src/gui/` | egui app shell, worker bridge, messages, modals, theme, stickers, workspace |
| `src/gui/worker.rs` | Tokio runtime; `WorkerCmd` / `WorkerEvent` boundary between UI and network |
| `src/tui.rs`, `src/ui.rs` | ratatui terminal client |
| `src/chat.rs` | Message and thread operations against the Zalo chat API |
| `src/session.rs` | `session.json`, cookies, service map, request signing and params |
| `src/login.rs`, `src/qr.rs` | QR and password login flows |
| `src/socket.rs` | WebSocket transport, frame codec, push/pull command IDs |
| `src/param_cipher.rs` | `ParamCipher v2` request encoding |
| `src/crypto.rs` | AES-CBC/GCM helpers, MD5 `signkey` |
| `src/media.rs` | Attachments, images, stickers, voice payloads |
| `src/call.rs`, `src/audio.rs` | VoIP signalling and audio capture/playback |
| `src/store.rs` | SQLite persistence (`messages`, `threads`) |
| `src/endpoints.rs` | Single source of truth for service hosts |
| `src/logger.rs`, `src/voip_log.rs` | Diagnostic logging and call tracing |

## Non-Negotiables

- **Never commit secrets.** `session.json`, `*.db`, cookies, `zpw_sek`, `zpw_enk`, IMEI and capture dumps stay out of Git (`.gitignore`, `SESSION_SECURITY.md`).
- **No blocking work on the UI thread.** Network, disk and crypto go through `WorkerCmd`; the UI only renders and sends commands.
- **All hosts live in `src/endpoints.rs`.** Never hardcode a Zalo host elsewhere.
- **Every outbound request goes through the standard path:** params → `ParamCipher v2` → `signkey` → session cookie attachment.
- **Comments in English, user-facing strings in Vietnamese.** See the comment taxonomy below.
- **Do not silently change protocol behaviour.** Any change to params, signing, frame IDs or cipher flow requires evidence (capture, note in `re-notes/`) and a test or documented manual verification.

## Engineering Standards

MUST read the corresponding standard file under `docs/standards/` before modifying related code or tests:

- **Issue tracking & tickets** → `docs/standards/issue-tracker.md`
- **Domain glossary & ADRs** → `docs/standards/domain-docs.md`
- **Comments & docstrings** → `docs/standards/code-comment-taxonomy.md`
- **API calls, IPC, and error handling** → `docs/standards/api-design-and-error-handling.md`
- **SQLite schema and migrations** → `docs/standards/database-and-migrations.md`
- **Threads, channels, locks, and races** → `docs/standards/concurrency-and-locking.md`
- **Tests, fixtures, and benchmarks** → `docs/standards/testing-and-fixtures.md`
- **Secrets, crypto, and session handling** → `docs/standards/security-and-cryptography.md`
- **Branches, commits, and PRs** → `docs/standards/git-flow-and-pr-matrix.md`

---

# Hướng dẫn dành cho Codex Agent (Bản cá nhân ctdozlite trên macOS)

## 1. Nguyên tắc bảo mật bất khả xâm phạm (ĐÃ VÁ - TUYỆT ĐỐI KHÔNG GỠ BỎ)
1. **Không REST/MCP; Auto-Updater chỉ trỏ về fork:** REST/MCP server (`api_mcp.rs`) bị vô hiệu hóa chủ đích, không kích hoạt lại. Auto-updater được chủ repo bật lại (2026-11-12) và **chỉ** được kiểm tra/tải từ `github.com/minhkhang1008/ctdozlite-next` (`GITHUB_REPO` trong `src/gui/worker.rs`); luôn xác minh SHA-256 trước khi cài, không bao giờ gửi cookie/token Zalo tới GitHub, không thêm nguồn cập nhật khác.
2. **Domain Allowlist bắt buộc:** Mọi kết nối gửi kèm `zpw_sek`, `zpw_uin` hoặc session cookie (HTTP, WebSocket, History Sync, Multipart Upload) đều phải đi qua hàm kiểm tra `is_trusted_zalo_url` (`*.zalo.me`, `*.zaloapp.com`, `*.zadn.vn`, `*.zdn.vn`).
3. **Quyền riêng tư mạng:** Không tự động gửi HTTP GET tới các URL của tin nhắn đến để tạo Link Preview.
4. **Quyền file & Bộ nhớ:** Mọi file nhạy cảm (`session.json`, `zalo_history.db`, file DB tạm khi giải mã) trên macOS/Unix phải giữ quyền `0o600` (thư mục `0o700`). Luồng giải nén trong `history_sync.rs` phải giữ nguyên trần dung lượng RAM.

## 2. Bản đồ kiến trúc để khoanh vùng khi sửa lỗi
- `src/gui/` (`mod.rs`, `workspace.rs`, `messages.rs`, `modals.rs`, `worker.rs`): Toàn bộ giao diện `egui`, xử lý sự kiện UI, hiển thị tin nhắn, bộ gõ, clipboard và luồng worker nền.
- `src/chat.rs` & `src/socket.rs`: Luồng WebSocket (`zpw_ws`), HTTP polling, giải mã tin nhắn đến, gửi tin nhắn, reaction, quote, sticker, trạng thái đã đọc.
- `src/store.rs`: Lưu trữ SQLite (`zalo_history.db`), truy vấn lịch sử, tìm kiếm tin nhắn, cache hội thoại (`threads_cache.json`).
- `src/history_sync.rs`: Đồng bộ và giải mã lịch sử từ `.db.crypt` (Android) và Sync V2 (iOS).
- `src/call.rs`, `src/media.rs`, `src/audio.rs`: Gọi thoại/video, xử lý thiết bị âm thanh (`cpal`/`opus`) và giải mã video (`ffmpeg`).

## 3. Quy tắc viết code khi sửa lỗi
- Không dùng `.unwrap()` hoặc `.expect()` trên dữ liệu từ mạng, JSON hoặc SQLite.
- Không chạy tác vụ I/O, SQLite nặng hoặc request mạng trực tiếp trên luồng vẽ UI (`egui` frame update); luôn đẩy qua `worker.rs`.
- Sau mỗi lần sửa, chỉ chạy `cargo check --message-format=short` hoặc `cargo test` trên module liên quan để xác nhận nhanh.
