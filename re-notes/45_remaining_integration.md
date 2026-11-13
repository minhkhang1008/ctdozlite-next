# Remaining integration slices — 2026-11-13

Sources are the fetched upstream v0.1.1 snapshot 7ed3408, wave-A 169d115,
voice/document work 2b200ed and 1889923, and Markdown 2961176. These are source
observations and synthetic fixtures, not authenticated captures from this run.

- Productivity is selectively implemented instead of copying upstream UI-thread
  SQLite or scanning all parked account files. Worker commands serialize snapshots,
  mutations and account switching; terminal events carry the active account. Schema
  migration 2 is transactional/additive and compatible with upstream draft/note/
  reminder columns. Search binds every value; boolean metadata is read as JSON,
  not presence-of-key substring checks. Locate loads at most 51 messages including
  the target and feeds the fork's existing exact-reference quote navigation.
- A new file-based temporary fixture verifies draft flush acknowledgement precedes
  durable persistence/reopen; all fixtures use synthetic data and private temporary
  directories, never session.json or the owner's history database.
- Runtime: the public chat.zalo.me/libs 5ea0... WASM is 287049 bytes and SHA-256
  398a2016ede72d1e604a6ac13bc7c62f2c44a43f0c1d7b996e968427380fc739. A bare urllib
  request returned 403 in this run; app User-Agent plus the public chat.zalo.me/
  Referer returned 200 and exactly that digest. Production uses those public headers
  on a separate cookie-free client. No cookies, IMEI or signing keys go in this fetch.
- Voice upload follows upstream send_voice_message's AAC contract. The checked-in
  lazy-1.c72c0957b455ab70b73c.js command table maps file-service commands 12060/12063
  to /api/message/voice/upload and /api/group/voice/upload. Payload details and
  chunkContent follow upstream; the old sendVoice implementation in that bundle
  still mentions legacy voicemp3/sendmp3. No claim of a new successful live upload.
  Keep the standard Session encoded_params/post_multipart signing and trust checks.
- Voice receives priority over generic fileSize metadata, preserves parameters and
  normalizes known millisecond duration to duration_seconds (including sub-3.6s
  recordings). Capture uses a dedicated native thread, correct advertised sample
  format, continuous phase resampling, a five-minute cap and private AAC files.
  Encoding uses managed FFmpeg, bounded process time, cleanup-on-drop and explicit
  send/cancel. Account/generation guards discard stale progress and results.
- Document preview is explicit click only and uses bounded, allowlisted downloads.
  Office validates ZIP counts/sizes and bounds actual aggregate reads. PDF uses a
  stdin/stdout child before session initialization, timeout and Unix resource limits.
  Adding quick-xml/pdf-extract is limited to text extraction; parser crate sources
  were inspected (lopdf stream decompression is unbounded in-process), motivating
  the separate PDF process. No automatic link preview was restored.
- Markdown ports pure block parsing/tests and extends the fork's renderer instead
  of replacing its mention/emoji/font/quote behavior. Settings persist render/plain
  and single-star preference. Malformed/unclosed fences remain visible text.

Not included because outside this plan: folders, recall-and-resend editing, Feed,
translation providers, scripting, LLM stubs. Live device/package checks are tracked
separately in tasks/todo.md; source completion does not mean deployed/phone-verified.
