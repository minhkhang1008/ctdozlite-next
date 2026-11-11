# Agent Work Log

Shared, append-only log for every AI agent (Claude, Codex, ...) working in this repo.
**Read the latest entries before starting. Append a new entry after every task.**
Newest entry at the bottom. Keep entries short and factual.

Entry template:

```
## YYYY-MM-DD — <short title>  (agent: <name/model>)
- Request: what the user asked
- Context / root cause: what was found (with evidence)
- Changes: files + what changed
- Verification: what was run and the result; what is NOT verified
- Status: done / partial / blocked — and any open follow-ups
```

---

## 2026-11-11 — Conversation sidebar (danh sách hội thoại) cannot be resized  (agent: Claude Code, claude-opus-5-5)

- Request: Sidebar still could not be resized after an earlier agent's fix, when running
  `cargo run --release -- --gui --dev --data-dir ~/.ctdozlite-personal`.
- Earlier attempt (uncommitted on `personal-hardened`): added `conversation_sidebar_width` state,
  fixed `width_range` to 200..=520, `ui.set_width(ui.available_width())` in the panel, and removed
  the tab counter / "Đã ghim (n)" label so the header could not force the panel wider. That did not
  address the real cause.
- Root cause: egui 0.29.1 `SidePanel` stores the *content* rect as its width every frame
  (`PanelState { rect }`, egui `containers/panel.rs`). In `src/gui/workspace.rs` the thread-list
  row width was measured **outside** the `ScrollArea` as `available_width() - 8`, but the app uses
  `ScrollStyle::solid()` (`src/gui/theme.rs`: bar_width 6 + inner margin 4 = 10px reserved). Rows
  were therefore 2px wider than the viewport; a vertical-only `ScrollArea` with
  `auto_shrink([false,false])` grows to fit, so the panel re-stored a width +2px larger on every
  repaint. After a drag, the sidebar crept back toward the 520px max, so resizing looked broken.
- Evidence: headless egui 0.29.1 repro (scratchpad, not in repo) — with the old measurement the
  stored width went 280 → 287 with no input and drifted 242 → 254 after a drag to 240; with the
  fix it stays at 280 and holds at 240 after the drag.
- Changes: `src/gui/workspace.rs` — `row_target_w` is now computed inside the
  `threads_list_scroll` `ScrollArea` closure (used by thread rows and message-search hit rows).
- Verification: `cargo check` passes. Headless repro confirms the behaviour. NOT yet verified by
  the user in the real GUI.
- Status: done, pending user test. Rule for future edits: inside a resizable `SidePanel`, never
  allocate widgets wider than the space measured in their own `ui` (especially inside a solid
  `ScrollArea`), or the panel will fight manual resizing.

## 2026-11-11 — Replace self-drawn new-message popup with native macOS notifications  (agent: Claude Code, claude-opus-5-5)

- Request: Disable the self-drawn in-app new-message notification; send every new incoming message
  as a native macOS notification (title = sender / group, body = preview) via
  `src/native_notifications.rs`; DailyNotifier helper in `.app`, `osascript` fallback when the helper
  binary is missing (e.g. `cargo run`), text passed safely; no notification for outgoing messages;
  request permission at startup.
- Context found:
  - The "toast" for new messages was `incoming_popup` (`IncomingPopupState`), a separate egui
    corner viewport with quick-reply, drawn in `render_toast_overlay` (`src/gui/modals.rs`).
    `show_toast` / `toast_notice` is a different thing: short action feedback ("Đã tải thêm…",
    "Đã gửi tự động…"). **It was kept on purpose**: it isn't a message notification, and sending it
    to the OS would flood Notification Center.
  - `native_notifications.rs` previously did authorization only (no delivery). `DailyNotifier.swift`
    `--notify` existed but only accepted the generic payload `"Zalo"/"You have a new message."`
    (privacy design). It now accepts sender/preview text because the owner asked for it.
- Changes:
  - `src/native_notifications.rs`: new `pub async fn notify(title, body)`. It flattens control
    characters, caps title at 120 / body at 240 chars, and falls back to "ctdozlite" / "Tin nhắn mới"
    when empty. If the helper binary exists → `--notify` with JSON on stdin (opaque random sha256 id,
    `sound:false` because the GUI plays its own chime); `notAuthorized` → `NotificationError::NotAuthorized`,
    no osascript bypass. Else → `/usr/bin/osascript -e 'on run argv' -e 'display notification (item 2 of argv)
    with title (item 1 of argv)' -e 'end run' -- <title> <body>`, so the text is argv, never code.
    The spawn/collect helpers were refactored to support stdin. Five new tests were added.
  - `src/gui/worker.rs`: `WorkerCmd::PostNativeNotification { title, body }`. Delivery is
    fire-and-forget on tokio; failures go to `logger::log_error` (dev mode only, variant name only).
  - `src/gui.rs`: where the popup used to be created (incoming, not muted, not outgoing, and thread ≠ active or
    window unfocused), it now sends `PostNativeNotification` if `settings.enable_notifications`.
    `IncomingPopupState` and the `incoming_popup` field were removed. A one-shot
    `request_notification_permission_at_startup()` (macOS only) sends `RequestNotificationAuthorization`
    on the first frame after the worker is up.
  - `src/gui/modals.rs`: the corner popup viewport and `get_desktop_bottom_right_pos` were removed from
    `render_toast_overlay` (only the action-feedback toast remains). The Settings note text was updated.
  - `native/macos/DailyNotifier.swift`: `Payload.validate()` now accepts non-empty single-line text
    (title ≤120, body ≤240 graphemes; rejects Unicode `Cc` control chars, allows emoji ZWJ) plus the
    64-hex id. The self-test was updated.
- Verification: `cargo check --message-format=short` OK; `cargo test --lib` 247 passed / 0 failed.
  The helper was rebuilt into the scratchpad (not `native/macos/build`) and `--self-test` PASSES. NOT verified:
  real notification delivery/permission prompt in the GUI (osascript path under `cargo run`, and the
  helper path inside a packaged `.app`); the user must test.
- Status: done, pending user GUI test. Follow-ups / caveats:
  - Under `cargo run`, notifications come from `osascript` (attributed to Script Editor); macOS may
    need Script Editor allowed in System Settings › Notifications. The startup permission request
    returns `HelperUnavailable` in that mode (no helper) — expected.
  - Brand-new threads (first message from a contact not yet in the list) still do not notify,
    same as the old popup did.
  - Removing the popup also removed its quick-reply box.
  - Packaged builds must rebuild DailyNotifier (`native/macos/build-native-helpers.sh`) to pick up the
    relaxed validation; an old helper will reject the payload (exit 2 → `InvalidResponse`).
  - `cargo fmt --check` still reports pre-existing diffs in untouched code (build.rs, call_probe.rs,
    gui.rs, modals.rs); I didn't change them.

## 2026-11-11 — Message bubble alignment, bare-URL links, link-share copy, large emoji  (agent: Claude Code, claude-opus-5-5)

- Request: (1) align incoming bubbles without avatar; (2) blue, hover-underlined, clickable URL when
  there is no preview; (3) "Copy tin nhắn" copies the original URL for link shares; (4) emoji-only
  messages rendered large / in colour.
- Root causes:
  1. In egui 0.29, `allocate_exact_size` (avatar) adds `item_spacing.x` after the widget, but
     `add_space(30.0)` does not. Follow-up bubbles therefore sat `item_spacing.x` further left.
  2. Every message containing `http(s)://` becomes a link card. When there was no thumb/title/desc,
     the card drew the URL as a bold title in normal text colour.
  3. The context menu copied `msg.text`, which for attached link shares holds title/desc preview text.
  4. egui ships only a monochrome emoji font. The app bundles only the 6 reaction PNGs (`assets/emojis`).
- Changes (`src/gui/messages.rs` only):
  - Placeholder is now `add_space(30.0 + item_spacing.x)`.
  - Link card: if it has no thumb, title (≠url/domain) or desc, it renders the URL as a label in
    `url_link_color` (#388BFD dark / #1E88E5 light). The label underlines on hover (hover state kept in
    egui temp data, one frame behind), opens the URL on click, and drops the domain line.
  - `render_markdown_inline_rich`: bare URLs (`http(s)://`, `www.`, `docs.google.com/`,
    `drive.google.com/` at a word boundary) are blue, underlined on hover and clickable. Scheme-less
    URLs get `https://`.
  - New `message_copy_text`: for messages with `link_url` (not photo/file/sticker) it copies the
    URL, or "comment\nurl". Otherwise it copies `msg.text` unchanged.
  - New `emoji_only_clusters` (ZWJ, skin-tone, VS16, flag aware, ≤8 clusters) and `render_emoji_only`
    at 36px. It uses the bundled colour PNG when the emoji is one of the 6 reactions, else the glyph.
    The width pre-measure mirrors this. `render_message_body` gained an `emoji_textures` param.
  - 3 new unit tests.
- Verification: `cargo check --message-format=short` clean; `cargo test --lib` 250 passed / 0 failed.
  NOT verified in the real GUI.
- Caveats:
  - Emoji other than the 6 reactions (e.g. 🐧) are still monochrome glyphs (now 36px). Full colour
    needs a colour emoji atlas (e.g. Twemoji PNGs) or CoreText rasterisation, which isn't done.
  - **Mistake:** I ran `cargo fmt -- src/gui/messages.rs`, which formats the WHOLE crate. Files that were
    clean before the task now carry formatting-only diffs. My attempt to `git checkout` them was blocked
    by the permission classifier; the owner should decide. Files that were already dirty
    (gui.rs, modals.rs, theme.rs, worker.rs, workspace.rs, store.rs, api_mcp.rs,
    native_notifications.rs, zproto_host.rs) were also reformatted on top of the owner's
    uncommitted edits. The changes are semantically neutral, but they make diffs noisy.
    Use `rustfmt --edition 2021 <file>` for single-file formatting.

## 2026-11-11 — Colour emoji from the system font (follow-up)  (agent: Claude Code, claude-opus-5-5)

- Request: finish colour emoji (e.g. 🐧, 💌); owner asked why emoji can't render as plain text.
- Why text can't do it: egui/epaint 0.29 rasterises glyphs via ab_glyph into a single-channel
  coverage atlas tinted by the text colour. Colour glyph tables (Apple `sbix`, Noto `CBDT`,
  Windows `COLR`) are never read. So egui text can only show monochrome emoji; colour emoji must
  be drawn as textures.
- Changes:
  - Deps: `rustybuzz 0.20` (shaping, shares the existing `ttf-parser 0.25`) and `memmap2 0.9`
    (already in the lock tree).
  - New `src/emoji_raster.rs`: memory-maps `/System/Library/Fonts/Apple Color Emoji.ttc`
    (Linux Noto paths as fallback; Windows unsupported → None). It shapes the cluster with
    rustybuzz so ZWJ sequences, flags and skin tones resolve to the combined glyph, takes the
    PNG strike via `glyph_raster_image(gid, 96)`, pads it to a square and caps it at 160px.
    No network and no bundled assets. Unsafe `Mmap::map` has a SAFETY note.
  - `WorkerCmd::RasterizeEmoji { key, cluster }` runs on `spawn_blocking` (INV-3) and replies with
    the existing `StickerImageReady` / `StickerImageFailed` events. Textures live in
    `sticker_textures` under `emoji_<hex codepoints>`. In `gui.rs`, a failed `emoji_` key stays in
    `loading_stickers` as a negative cache, so it is not re-requested every frame.
  - `messages.rs`: new `scan_emoji_clusters` (byte ranges); `emoji_only_clusters` is rebuilt on it.
    Emoji-only messages use system texture → bundled reaction PNG → glyph, at 36px.
    For inline text, `render_markdown_inline_rich` / `render_chat_markdown_block` now take
    `textures` + `loading`. `split_emoji_sections` turns each ready emoji into a transparent
    placeholder section (first codepoint only). The galley is laid out manually and
    `paint_emoji_marks` overdraws the texture at each placeholder glyph. Only Emoji_Presentation
    / ≥U+1F000 / VS16 clusters are converted, so `©`, `→`, `✓` stay text.
  - Tests: 4 in `emoji_raster` (real font: 🐧 💌 ❤️ 👍🏽 🇻🇳 👨‍👩‍👧, combined ≠ component) and 1 for
    section splitting.
- Verification: `cargo check` clean; `cargo test --lib` 255 passed / 0 failed. NOT verified visually
  in the GUI.
- Caveats: the first frame shows the monochrome glyph until the texture arrives. Selecting and copying
  label text yields only the first codepoint of a ZWJ sequence (the "Copy tin nhắn" menu still copies
  the full `msg.text`). Thread list previews, composer, quote snippets and reactions still use glyphs.
  Formatting this time used `rustfmt` on the touched files only.

## 2026-11-11 — @mention popover regressions + real group tagging  (agent: Claude Code, claude-opus-5-5)

- Request: (1) the @ features stopped working (seen with `--release --gui --dev --data-dir
  ~/.ctdozlite-personal`); (2) picking a member only inserted plain text, with no real tag.
- Root causes found (in code; NOT reproduced in the live GUI, no runtime logs available):
  - `extract_active_mention_query` stopped suggesting at more than 3 words, so typing a full
    Vietnamese name ("@Nguyễn Hoàng Hải Đăng", 4 words) closed the popover mid-name.
  - `GroupMembersLoaded` with an empty list (failed `getmg-v2`/`group/members`) was cached in
    `group_members_map` forever and never refetched. The popover then offered only "@All".
  - No send path ever carried `mentionInfo` (always `""` / `"[]"` / absent) → tags impossible.
  - `suggest_mentions` is `true` in the owner's `app_settings.json`, so settings were not the cause.
- Changes:
  - `chat.rs`: `MentionPick`, `build_mention_info` (UTF-16 pos/len incl. `@`, type 1 for `-1`,
    longest-name-first, word boundary, drops picks no longer in text), and
    `send_group_mention_message_with_client_id` → `/api/group/mention`. On a non-expiry error it
    falls back to `sendmsg`. Evidence and caveats are in `re-notes/38_group_mentions.md`
    (zca-js shape + our incoming `mentions` parser; NO capture).
  - `worker.rs`: `WorkerCmd::SendMessage` gained `mentions`. Group texts with tags skip
    `sendlink` and go through the mention sender. Callers in `modals.rs` pass `Vec::new()`.
  - `workspace.rs`: query limit raised to 6 words (`MENTION_QUERY_MAX_WORDS`).
    `mention_name_matches` helper added. An empty member list re-fetches every 20 s at most
    (`mention_member_retry_at`). Fallback candidates come from incoming senders' `uidFrom`
    (`mention_fallback_from_messages`). `insert_composer_mention` records the pick, sets the
    caret at the end, and dismisses the "Name " query so Enter sends instead of re-inserting.
    Picks are sent via `composer_mentions` and cleared when the draft is empty. The popover now
    returns `MentionCandidate`.
  - `gui.rs`: two new state fields.
  - Tests: 2 in `chat.rs`, 2 in `workspace.rs`.
- Verification: `cargo check` clean; `cargo test --lib` 259 passed / 0 failed. NOT verified: live
  send to Zalo (endpoint and response unconfirmed), popover behaviour in the GUI, and whether the
  owner's group member fetch actually fails at runtime.
- Formatting: `rustfmt` on touched leaf files only (chat.rs, workspace.rs, worker.rs, modals.rs,
  messages.rs). The crate-wide `cargo fmt` churn from earlier today is still in the tree.

## 2026-11-11 — Blue @tag text + @ popover keyboard navigation  (agent: Claude Code, claude-opus-5-5)

- Request: (1) render @tags blue in group messages (incoming and outgoing), using `mentions`
  metadata and falling back to text parsing; (2) ↑/↓/Tab/Enter navigation in the @ popover:
  Enter/Tab pick without sending or moving focus, clear highlight, caret after the name.
  (Owner confirmed group tag sending via `/api/group/mention` works → re-notes/38 evidence.)
- Root causes:
  - `render_markdown_inline_rich` only checked `@` when it sat exactly at the start of `rest`.
    `@` was not one of the "next marker" positions, so mid-text `@Name` was bulk-appended as
    plain text.
  - Popover keys were read AFTER the TextEdit had run: ↑/↓ moved the caret, and Tab moved focus
    (egui `Focus::begin_pass` reads Tab from raw input at frame start; TextEdit's filter has
    `tab: false`). Escape clears focus at frame start, so the old focus-gated Esc check failed.
- Changes:
  - `messages.rs`: `message_mention_texts` maps wire `mentions` / archive `meta.mentionsList`
    UTF-16 `pos/len` onto `msg.text` → exact `@Name` strings. It returns `Some(empty)` when the
    message is known to have no tags, and `None` → heuristic when entries are unmappable.
    `find_mention_start` (word-boundary `@`) was added to the marker scan. Metadata spans match
    longest-first. The old `@All`/`@Name` guess runs only without metadata. Style:
    `mention_link_color` (#0068FF light / #4DA3FF dark), no chip background or underline. The
    renderers gained a `mentions: Option<&[String]>` param.
  - `GuiEvent::MessageSent` gained `mentions`. The SendMessage worker path fills it with
    `build_mention_info`, and gui.rs stores it in the local outgoing message's
    `wire_ref.fields["mentions"]` so our own bubble highlights exactly. Other emitters
    (worker ×3, call.rs ×5) pass `Vec::new()`.
  - `workspace.rs`: keys are consumed before the TextEdit with `consume_key` (↓/↑ clamp,
    Tab/Enter pick, Enter's `\n` text event dropped). The TextEdits use
    `.lock_focus(popover open)` so Tab doesn't leave. Esc is handled without a focus gate and
    restores focus. Selection resets to 0 when the query changes (`mention_last_query`). The
    selected row gets a primary tint plus a left accent bar, and is scrolled into view.
  - Tests: +2 in messages.rs.
- Verification: `cargo check --message-format=short` clean; `cargo test --lib` 261 passed /
  0 failed. NOT verified in the live GUI. Caveat: if `group/mention` falls back to `sendmsg`, our
  local bubble still shows blue tags even though recipients get plain text.

## 2026-11-11 — Redesign GUI to match Zalo PC (chat.zalo.me) look  (agent: Claude Code, claude-opus-5-5)

- Request: inspect chat.zalo.me with BrowserSkill (owner's logged-in account, permission given to
  open chats) and make ctdozlite look like it.
- Evidence gathered (scratchpad only, nothing committed): computed CSS custom properties for dark
  mode, the `:root .light` token block from Zalo's public stylesheet, and DOM measurements of nav
  rail, conversation rows, header, bubbles and composer. Only colour values and metrics were reused;
  no Zalo CSS, icons, fonts or images were copied. The Lucide icon set was kept.
- Changes:
  - `theme.rs`: dark/light palettes replaced with Zalo tokens (dark: rail #121416, panels #22262B,
    in-bubble #282D33, out-bubble/selected #1F344D, text #DFE2E7/#9FACBC, accent #0068FF/#66A6FF;
    light: blue rail #005AE0, white panels, canvas #E5E7EB, out-bubble/selected #E5F1FF, text
    #081B3A/#5A6981). New tokens `nav_icon`, `nav_icon_active`, `nav_selected_bg`, `nav_hover_bg`
    (the first two can be overridden in theme.json). Widget rounding 8 → 6.
  - `workspace.rs`: nav rail 64px wide with 48px tiles, 6px corners, tinted selection tile (pill
    removed); sidebar search is a borderless 32px well with 5px corners and "Tìm kiếm" hint; tabs
    14px; conversation rows 70px card in a 78px slot, 48px avatar, 16/14/12px text, no left bar, no
    group icon prefix; chat header 68px with 48px avatar and 18px title; composer input 15px.
  - `messages.rs`: bubbles borderless with a 1px soft shadow (`bubble_shadow`), 6px corners, 12px
    padding, 15px body text, 12px time, 13px secondary-colour sender name, 40px incoming avatar.
  - `gui.rs`: default sidebar width 280 → 344.
- Verification: `cargo check` clean; `cargo test --lib` 261 passed / 0 failed. rustfmt run on the
  three touched gui files only. NOT verified visually in the running GUI (not launched).
- Status: done, pending the owner's visual check. Not done: Zalo's right-hand group info panel
  layout, the pinned-message bar and the separate "Đã nhận" status pill under the last bubble.
  Text truncation still uses character counts, so very wide glyphs may need tuning after the
  larger fonts.

## 2026-11-11 — Blue @tags inside the composer + first-use @ popover keys dead  (agent: Claude Code, claude-opus-5-5)

- Request: (1) picked @tags in the composer should be blue like in sent bubbles (they were white);
  (2) the first @ pick in a session only worked with the mouse; keyboard worked afterwards.
- Root cause (2), from code (not reproduced live): `gui.rs` IME pre-pass set `ime_active = true`
  on `ImeEvent::Enabled`, which macOS/winit sends when a TextEdit gains focus. Enabled has no
  preedit, so the flag stayed true until the first IME commit. While it was true, `is_ime_busy`
  skipped the popover's ↑/↓/Tab/Enter handling and the pre-pass stripped every Enter key. After
  the first commit (e.g. after a mouse pick) the flag cleared, which matches "works from the
  second time".
- Changes:
  - `gui.rs`: `ImeEvent::Enabled` no longer marks composing; only a non-empty Preedit does.
  - `workspace.rs`: `composer_mention_spans` (longest-first, word boundary, no overlap) and
    `composer_mention_layout_job`; both composer TextEdits use a `.layouter` that paints
    `composer_mentions` names in `mention_link_color`. `COMPOSER_FONT_SIZE` = 15. +1 test.
  - `messages.rs`: `mention_link_color` made `pub(super)`.
- Verification: `cargo check` clean; `cargo test --lib` 262 passed / 0 failed. NOT verified in the
  live GUI (Telex IME behaviour in particular). If keys still fail while a Telex preedit is open,
  that is intended: Enter then commits the IME text first.

## 2026-11-11 — Paste images from the clipboard on macOS (Cmd+V)  (agent: Claude Code, claude-opus-5-5)

- Request: allow pasting images straight from the clipboard. Upstream only had it working on Windows.
- Root cause: egui-winit 0.29.1 `on_keyboard_input` swallows `Cmd+V` (`is_paste_command`) and
  pushes `Event::Paste` only when the clipboard has non-empty text. With an image-only clipboard
  egui saw no input at all, so `is_clipboard_paste_triggered` never fired on macOS; the old
  osascript `«class PNGf»` reader was never reached. That reader also missed TIFF-only images and
  would have pasted the file *icon* for files copied in Finder.
- Changes:
  - `Cargo.toml`: macOS-only deps `objc2 0.5`, `block2 0.5`, `objc2-foundation 0.2`,
    `objc2-app-kit 0.2`. These are the same versions winit already pulls in, so no new crates
    (Cargo.lock only gained dependency-list entries).
  - New `src/gui/macos_clipboard.rs`: a local `NSEvent` key-down monitor (no Accessibility or
    Input Monitoring permission needed) records `Cmd+V` (no Ctrl/Option, not a repeat) with a
    timestamp and requests a repaint. `take_cmd_v()` consumes it once, with a 1.5 s staleness
    window. `read_image()` reads NSPasteboard in-process: (1) Finder file URLs with image
    extensions, ≤64 MB, never the icon TIFF; (2) `public.png`; (3) `public.tiff`. Formats the
    `image` crate can't decode (TIFF/HEIC/BMP/WebP) are re-encoded to PNG through
    `NSBitmapImageRep`. 2 unit tests.
  - `src/gui/messages.rs`: on macOS, `is_clipboard_paste_triggered` installs the monitor (once)
    and ORs in `take_cmd_v()`. The macOS branch of `read_clipboard_image_to_png` now calls
    `macos_clipboard::read_image()`. The osascript script and its test were removed, so there is
    no longer a subprocess on every text paste.
  - `src/gui.rs`: `#[cfg(target_os = "macos")] mod macos_clipboard;`.
- Verification: `cargo check` clean; clippy shows no warnings in the touched code;
  `cargo test --lib` 263 passed. A temporary live test (removed afterwards) put a PNG, a TIFF and
  a Finder file copy on the real pasteboard: all decoded to 37×21. A text-only clipboard returned
  None. NOT verified: the Cmd+V key monitor in the running GUI (needs a manual try in a chat).
- Status: done, pending the owner's GUI test. UI hint strings still say "Ctrl+V".

## 2026-11-11 — Zalo-style "who reacted" tooltip and dialog  (agent: Claude Code, claude-opus-5-5)

- Request: rework the UI/UX for viewing who reacted. The owner sent two Zalo PC screenshots as
  references: a blue hover tooltip naming the reactor ("Trí"), and a "Biểu cảm" modal with emoji
  tabs on the left and reactors on the right.
- Before: hovering the pill showed only emoji + counts. Clicking opened an `egui::Window`
  "Cảm xúc" (340×380) with horizontal tab buttons.
- Changes:
  - `messages.rs`: `render_chat_bubble` / `render_reactions` take
    `reactor_name: &dyn Fn(&str) -> String`. New `reaction_tooltip_lines` builds the tooltip text
    from the ledger only (`reactor_ids`, deduplicated, most-used emoji first, max 8 names, then
    "và N người khác", which also absorbs unexplained counts). With no ledger it shows
    "N lượt thả cảm xúc". New `paint_reaction_tooltip` paints the solid blue card (#1A3F94 dark /
    #0052CC light), 14px white text and a down arrow centred on the pill, clamped to the screen,
    on the Tooltip layer. +2 tests.
  - `workspace.rs`: the caller builds the resolver from the existing per-frame
    `member_name_by_uid`, mapping self to "Bạn" and a 1:1 peer to `thread_name`.
  - `reactions_dialog.rs`: rewritten as a modal: dimmed backdrop (click outside or Esc closes) and
    a 440×350 card (clamped to the window) with a 52px header "Biểu cảm" + X button and a divider.
    The 120px left column has 36px tab rows ("Tất cả N", then emoji N), the selected one tinted.
    The right column scrolls 56px rows: 40px avatar, ellipsized 15px name, and emoji + count
    right-aligned. Data and row-building logic are unchanged.
- Verification: `cargo check` clean; clippy reports nothing new in the touched code (one existing
  `sort_by` lint at reactions_dialog.rs:108 remains); `cargo test --lib` 265 passed. rustfmt was
  run on reactions_dialog.rs and messages.rs only. NOT verified visually in the running GUI.
- Status: done, pending the owner's visual check.

## 2026-11-11 — Double-click to reply with an auto @tag; Esc leaves reply mode  (agent: Claude Code, claude-opus-5-5)

- Request: double-clicking a message enters reply mode and tags the sender at the start of the
  composer as a visible `@Name` the user can delete. Esc exits reply mode.
- Context: double-click → `BubbleAction::Reply` already existed, but it only set `replying_to`.
  There was no Esc handling for reply mode. `SendQuote` sent no tags at all
  (re-notes/38 "Not covered").
- Changes:
  - `workspace.rs`:
    - `reply_mention_for` resolves the sender uid from `sidebar_sender_id`, a numeric `sender`,
      or a unique member-name match. The name comes from `member_name_by_uid`. It never tags
      us. +1 test.
    - `start_reply` drops any previous auto tag and prefixes `@Name ` (only in groups and only
      for others' messages). The tag is added to `composer_mentions`, so it renders blue and is
      sent as a real tag. It also dismisses the @ popover for that query and puts the caret at
      the end.
    - `cancel_reply` / `remove_reply_auto_mention` strip the auto tag only if it is still
      untouched at the start.
    - Used by: double-click/menu Reply, the banner X, thread switch, and Esc. Esc is consumed
      after the @ popover's Esc and skipped while IME is composing or any modal is open
      (image viewer, reactions, forward, settings, schedule, phone lookup, profile menu).
    - Sending clears `reply_auto_mention` and passes `composer_mentions` to `SendQuote`.
  - `gui.rs`: `reply_auto_mention` state. `QuoteSent` carries `mentions` and stores them in
    `wire_ref.fields["mentions"]` so our own reply bubble highlights the tag.
  - `worker.rs`: `SendQuote { mentions }` calls the new `chat::send_quote_message_with_mentions`.
  - `chat.rs`: `send_quote_message_with_mentions`; `send_quote_message_with_client_id` delegates
    with `&[]`, so the TUI and old callers are unchanged. A group quote adds `mentionInfo` only
    when tags resolve. The group fallback now goes through
    `send_group_mention_message_with_client_id`, which equals plain sendmsg when there are no tags.
  - `re-notes/38_group_mentions.md`: documents quote + mentionInfo (zca-js evidence, NOT
    capture-verified) and the manual check.
- Verification: `cargo check` clean; `cargo test --lib` 266 passed; clippy reports nothing new
  in the touched code. NOT verified live: the GUI behaviour, or whether Zalo honours
  `mentionInfo` on `group/quote`.
- Status: done, pending the owner's live test (see the re-notes/38 manual check).

## 2026-11-11 — Drag & drop files: "Gửi nhanh" vs "Xem trước khi gửi" zones  (agent: Claude Code, claude-opus-5-5)

- Request (with a Zalo web screenshot as reference): while dragging a file over the app, show
  two clearly drawn drop zones and highlight the one under the cursor. Dropping on the message
  area sends immediately; dropping on the composer attaches the file to the draft for review.
- Before: any drop anywhere sent every file at once (handler inside the composer frame), with
  no hover UI.
- Key finding: during an AppKit drag session the window receives no mouse-move events, and
  winit 0.30 only forwards `HoveredFile` with no position, so egui's pointer stays frozen. New
  `src/gui/macos_drag.rs::cursor_in_window` reads `NSEvent.mouseLocation` and maps it into
  egui points: it flips the y axis using the primary `NSScreen` height, subtracts the
  viewport's `inner_rect`, and corrects for egui zoom (`pixels_per_point / native`). Other
  OSes use `pointer.latest_pos()`. Cargo features added: foundation `NSGeometry`, app-kit
  `NSScreen` (same crate versions).
- Changes (`workspace.rs`, `gui.rs`):
  - After the composer, a `4b` block: while `hovered_files` is non-empty,
    `drop_zone_rects(messages inner_rect, composer rect)` gives the quick zone over the message
    area and the preview zone over the composer (min 96px). `paint_drop_zones` draws on the
    Foreground layer: a covered background, a dashed border, title + hint. The active zone gets
    a primary tint, primary border and primary title. Repaint runs every 16ms while hovering.
    `drop_zone_last` is the fallback when no position is known.
  - `drop_zone_at`: only the message area means QuickSend; anywhere else (composer, sidebar,
    unknown) means Preview, so an imprecise drop never sends.
  - `handle_dropped_files`: directories are skipped with a toast. QuickSend sends one
    `PickAndSendAttachment` per file. Preview pushes `PendingDropFile` entries (dedup, max 30);
    size and a 112px thumbnail (png/jpg/gif ≤40 MB) are read on a `std::thread` and returned
    via the `drop_thumb_tx/rx` mpsc channel.
  - `render_pending_drop_files` strip (above the scheduled banner): a header showing the count
    plus "Hủy tất cả", and horizontally scrolling 210×52 cards (thumbnail or icon, ellipsized
    name, size / "Đang đọc…", ✕). `picker_reserve` +96 while the strip is non-empty.
  - Send: `can_send` also counts pending drops. Enter/send sends the pasted image first, then
    the dropped files; the typed text is the caption of the first file only.
  - +1 test (zone split / default).
- Verification: `cargo check` clean; `cargo test --lib` 267 passed; clippy reports nothing new
  in the touched code. rustfmt was run on workspace.rs and macos_drag.rs. NOT verified live:
  hover highlighting during a real Finder drag (especially the coordinate mapping on
  multi-monitor setups or with a non-1.0 UI zoom), and the GUI look.
- Status: done, pending the owner's live test.

## 2026-11-11 — Investigation: "Tìm kiếm GIF" setting does nothing  (agent: Claude Code, claude-opus-5-5)

- Request: Settings → "Gợi ý @" lists name suggestions, GIF search and sticker search, but in
  practice only @-tagging works. Check GIF search (wanted as a real feature).
- Findings:
  - `AppSettings::enable_gif_search` is only stored and drawn as a toggle
    (`modals.rs` ~2594). No code reads it, and the app has no GIF search, picker or send path.
  - `enable_sticker_keyword_search` ("Tìm sticker") only AND-gates the contextual sticker
    suggestions together with `suggest_stickers_while_typing` (`stickers.rs:1809`). There is
    no "@sticker"-style command.
  - Protocol evidence available offline: Zalo config has
    `apiGifDomain = https://sticker-wpa.<domain>`,
    `gif:{search_by_kw_limit:50, get_trending_limit:50, chat_box_gif_trending_limit:50}`,
    `maxGifSize: 3` (MB, overridable by `settings.sharefile.max_size_gif`), message types
    `MSG_GIF` / `CLI_MSG_TYPE_GIF` / `MSG_SUBTYPE_GIF` (photo-like `params` with
    width/height), and the `chat.gif` msgType (23), which the store already parses.
    `chat.rs` sticker suggest sends `gif:1` but only reads `sugg_sticker`.
  - The captured `web-bundle/` does NOT contain the GIF search or GIF send request code
    (lazy chunk not captured): no `/api/...gif...` paths at all.
- Changes: none (investigation only).
- Status: blocked on protocol evidence. Implementing search/send would mean guessing endpoints,
  which AGENTS.md forbids. Proposed next step (needs owner OK): capture on chat.zalo.me the
  request made when searching GIFs, and the one made when sending a GIF (e.g. to My
  Documents); record them in re-notes/39; then implement.

## 2026-11-11 — History lost after restart / re-login; phone sync "has no effect"  (agent: Claude Code, claude-opus-5-5)

- Request: after quitting, reopening and logging in again, all old messages were gone, unlike
  Zalo web; data sync seemed to do nothing. (Also: owner approved capturing the GIF protocol on
  chat.zalo.me; see Status.)
- Root cause (code + on-disk evidence): history was split between two databases.
  - Writes: live messages go to `zalo_history.db` (`PersistThreads`, group/cloud sync).
  - Reads: startup (`main.rs`), the worker reloads after login, workspace refresh, and phone
    sync (`history_sync.rs:472/1193`, writes too) all used `store::db_path_for_account(uin)`,
    i.e. the parked `accounts/<uin>_history.db`, which is refreshed only by
    `save_current_account_snapshot`.
  - Logout snapshots the live DB, then deletes it. Login never restored the parked copy, so a
    fresh, empty live DB was created, and the next snapshot overwrote the good parked copy.
    Phone-synced messages written to the parked DB were also clobbered by the next snapshot.
  - On disk: `~/Library/Application Support/ctdozlite` live 1909 msgs vs parked 1824 (47 threads,
    2026-11-02..11). `~/.ctdozlite-personal` live 422 vs parked 421 (22 threads); that data dir
    was only created at 21:44 today, so the older history lives in the default dir, not the
    personal one.
- Changes:
  - `store.rs`: `db_path_for_account` always returns `zalo_history.db` (the active account's DB).
  - `account_manager.rs`: `activate_account_data(uin)` with an `active_account` owner marker.
    Live files owned by another account are parked first. A missing live DB is restored from
    `accounts/<uin>_*`. Legacy layout (no marker, both exist): the parked DB is merged into
    live via `import_history_from_path` (INSERT OR IGNORE, so no loss). `switch_to_account`
    writes the marker. +3 tests (temp dirs).
  - `main.rs`: `activate_logged_in_account` runs before the threads cache / DB are read (GUI and
    TUI paths). `worker.rs`: activation runs before the snapshot on QR and password login.
- Caveat: the one-time legacy merge upserts thread rows from the older parked copy, and
  `last_read_msg_id` may move back, so a few threads can show stale unread badges once.
- Verification: `cargo test --locked` all pass (270 lib); clippy reports nothing new in the
  touched code. rustfmt was run on main.rs and account_manager.rs. NOT verified live: a real
  logout → login cycle, and phone sync (`sync_phone_messages`) end to end (needs the phone).
- GIF (still blocked): no browser automation was available in this session (no Claude in Chrome /
  built-in browser tools), so nothing was captured. Asked the owner to save the lazy JS chunk
  that loads when opening the GIF panel on chat.zalo.me into `web-bundle/`.

## 2026-11-11 — GIF search & send via the composer `@gif` command  (agent: Claude Code, claude-opus-5-5)

- Request: make Settings → "Tìm kiếm GIF" real; the owner approved capturing the protocol on
  chat.zalo.me with BrowserSkill (Main Chrome) and sending one test GIF to "My Documents".
- Evidence (re-notes/39_gif_search_send.md, capture-verified):
  - `GET {sticker}/api/message/giphy/trending` params `{offset,limit,imei}`.
  - `GET {sticker}/api/message/giphy/search` params `{keyword,offset,limit}`.
  - `POST {file}/api/{message|group}/gif` with an EMPTY body; params in the query string:
    `{thumb,width,height,msg:"",type:0,href,contentId,[visibility],src,toid|grid,clientId:"<str>"}`.
  - Results arrive as `data.data_list[{id, original{url,w,h}, normal{url,w,h}}]` on
    `zgif-v2.zdn.vn`. src 7 = composer `@gif` command.
  - The test send returned error_code 0 and showed "Bạn: GIF" in My Documents. The source chunk
    was saved as `web-bundle/lazy-1.c72c0957b455ab70b73c.js` (16 MB).
- Changes:
  - `chat.rs`: `GifItem`, `parse_gif_list` (drops untrusted hosts / id-less items),
    `gif_trending`, `gif_search`, `gif_send_body`, `send_gif_with_client_id` (local echo is a
    photo-style `ChatMessage`, text `[GIF]`). Hosts come from `s.domains.*` with an
    `endpoints::service` fallback. +2 tests.
  - `worker.rs`: `WorkerCmd::SearchGifs{query,seq}` (empty → trending, limit 30) and
    `WorkerCmd::SendGif` (→ `AttachmentSent` / `SendFailed`). `gui.rs`: `GuiEvent::GifResults`
    (stale seq ignored) plus gif_* state.
  - `workspace.rs`: `gif_command_query` (`@gif` + whitespace, case-insensitive), 350 ms
    debounced `update_gif_search`, a `render_gif_popover` 4-column grid above the composer
    (thumbnails via `FetchStickerImage`, key `thumb_<md5>`), keys ←→↑↓ / Enter (Enter
    swallowed while there are no results, so "@gif…" is never sent as text) / Esc (dismiss for
    that text). Click sends. Mention suggestions are off in GIF mode. Gated by
    `settings.enable_gif_search`. +2 tests.
  - `messages.rs`: `[GIF]` is not shown as a photo caption. `modals.rs`: setting hint now says
    "Gõ @gif + từ khóa…".
- Verification: `cargo test --locked` all pass (274 lib); clippy reports nothing new in the
  touched code. A read-only live probe (temporary example, deleted afterwards) could NOT run:
  both saved sessions (`~/.ctdozlite-personal` and Application Support) return 600 even for
  get_friends, i.e. they are expired. So search/send from ctdozlite itself is NOT verified live;
  only the wire format is (from the web capture plus the source).
- Status: done, pending a live test after re-login.

## 2026-11-11 — Turn off Claude co-author attribution in commits/PRs  (agent: Claude Code, claude-opus-5-5)

- Request: Stop adding "Co-Authored-By: Claude" to GitHub commits.
- Changes: user-global `~/.claude/settings.json` (outside the repo) — added
  `"attribution": {"commit": "", "pr": "", "sessionUrl": false}`. No repo files changed besides this log.
- Verification: `jq -e '.attribution'` parses the file and shows the block. Commits already in
  history are NOT rewritten (only new ones are affected).
- Status: done.

## 2026-11-11 — Release all fork work to `main`, delete `personal-hardened`  (agent: Claude Code, claude-opus-5-5)

- Request: commit and push everything to `main` of github.com/minhkhang1008/ctdozlite, delete
  the `personal-hardened` branch, and make the docs professional.
- Pre-commit audit: scanned the diff and the new files for UIN/IMEI/tokens/emails/personal paths;
  none found. A captured Giphy id in a test was swapped for a dummy. The local-only files are
  now git-ignored: `.claude/`, `.agents/skills/` (machine symlinks), `active_account` (holds the
  UIN), and `web-bundle/lazy-*.js` (16 MB Zalo JS, referenced by URL in re-notes/39 and not
  redistributed, per the README disclaimer). `.agents/WORKLOG.md` is tracked as AGENTS.md
  requires.
- Docs:
  - CHANGELOG `[Unreleased]` (Security/Added/Changed/Fixed).
  - README fork notice and module tree (new files; updater and REST/MCP marked disabled).
  - TINH_NANG matrix: ⌘V paste ✅; GIF, mentions, reply-by-double-click, reactions viewer,
    drag & drop zones; updater and REST/MCP 🚫; the Cyrillic "Авто-Reply" typo fixed.
  - HUONG_DAN_SU_DUNG §2.2 interactions. re-notes/39 now says where the chunk lives.
- Commits: chore(harness) worklog/ignore → feat(ui) code batch → docs. `main` is
  fast-forwarded from `personal-hardened` (which carries the security commit 1c691ae).
- Verification: `cargo build --locked` and `cargo test --locked` pass. Not split per feature:
  the features share workspace.rs / gui.rs / worker.rs hunks, so per-feature commits would
  not build on their own.

## 2026-11-11 — Rebrand docs as ctdozlite-next; drop upstream-team material  (agent: Claude Code, claude-opus-5-5)

- Request: README and other .md files belong to the original team; ctdozlite-next is the owner's
  personal build and needs simple docs. Owner has enabled GitHub Actions.
- Removed (upstream-only): `ctdozlite_landing/` (site, contributors, issue feeds); `docs/i18n/`;
  `docs/KE_HOACH_PRODUCTION.md`; `docs/DA_BIET.md` (auto-generated from upstream issues);
  the mdBook site (`docs/README.md`, `docs/SUMMARY.md`, `book.toml`, `.github/workflows/docs.yml`,
  `scripts/build-docs.sh`, `scripts/gen-known-issues.sh`); `.github/ISSUE_TEMPLATE/`; and
  `docs/assets/demo-windows.png`.
- Rewritten short: README (fork intro, differences, install, quick use, attribution + licence
  kept), `docs/CAI_DAT.md` (macOS install, local build, release-by-tag steps),
  `docs/TINH_NANG.md` (no upstream issue column), `docs/KHAC_PHUC_SU_CO.md`.
- Edited: HUONG_DAN (binary name `zalo-rust` → `ctdozlite`); AGENTS.md (fork repo URL, correct
  `--bin ctdozlite`); CHANGELOG (note that ≤0.1.0 is upstream history); ADR 0002 marked
  superseded; issue-tracker standard now points at the fork; DESIGN.md landing reference
  removed. `Cargo.toml` homepage/repository → fork; the Settings "Tìm hiểu" link → fork docs.
- Kept on purpose: upstream copyright in `build.rs` Windows metadata and the licences; the
  engineering docs (standards, ADRs, re-notes, KIEN_TRUC, GIAO_THUC, DESIGN, GLOSSARY,
  SESSION_SECURITY); the binary/bundle name `ctdozlite` (renaming it would move user data dirs).
- CI: the release step's hardcoded v0.0.7 title/body (upstream links, auto-update claims) was
  replaced by a generic `ctdozlite-next vX.Y.Z` with links built from `github.repository`.
  YAML validated with Ruby.
- Verification: `cargo build --locked` and `cargo test --locked` pass.

## 2026-11-12 — Release ctdozlite-next v0.2.0  (agent: Claude Code, claude-opus-5-5)

- Request: do the release (the owner enabled Actions and pushed the docs/CI commits).
- Steps: a dry run of CI on `main` (2879f98) built every platform. Two Ubuntu jobs first failed
  with apt "Temporary failure resolving" (runner network, 31 min at 1.3 KB/s); a rerun of only
  the failed jobs passed. Then `chore(release): v0.2.0` (7d66bee): version 0.2.0 in
  Cargo.toml/lock, CHANGELOG `[0.2.0] - 2026-11-12`. Tag `v0.2.0` was pushed. Run 43064924907
  passed every job and published "ctdozlite-next v0.2.0" with 14 assets.
- Verification: downloaded `ctdozlite-0.2.0-macos-arm64.dmg`. SHA256SUMS OK;
  `codesign --verify --deep --strict` OK (ad-hoc); Info.plist 0.2.0; arm64 Mach-O;
  `--version` prints v0.2.0. The release body links resolve to the renamed repo.
- Note: the owner renamed the GitHub repo to `minhkhang1008/ctdozlite-next` (GitHub redirects the
  old name). Local `origin` still uses the old URL (changing it was blocked, so it is left for the
  owner). Docs/Cargo/in-app links still say `minhkhang1008/ctdozlite`; update them in a follow-up.

## 2026-11-12 — Rename to ctdozlite-next, re-enable updater, hover bar, share dialog, reply layout, darker theme  (agent: Claude Code, claude-opus-5-5)

- Requests: (1) header "ctdozlite" → "ctdozlite-next"; (2) reply banner pushes the composer
  off-screen; (3) re-enable auto update and rename "Zalo Lite"; (4) Zalo hover bar (Trả lời /
  Chia sẻ / "…" menu as in the owner's screenshot); (5) Zalo-style share dialog; (6) darker
  background, gray bubbles. Plus a question about release turnaround.
- BrowserSkill: the chat.zalo.me agent tab kept losing "active tab" to the owner's own Zalo tab
  ("Bạn đang mở Zalo trên một Tab khác"). Stopped rather than fight the owner's session; worked
  from the owner's screenshots of the "…" menu and the share dialog.
- Changes:
  - `lib.rs` `APP_DISPLAY_NAME = "ctdozlite-next"` is used for the window/notification titles, the
    header, the Settings card, the macOS notification fallback and `--version`. Info.plist
    CFBundleName/DisplayName follow it. The binary, bundle id, data dir and eframe storage key
    stay `ctdozlite`, so data and notification permissions carry over. The integration test
    expectation was updated.
  - Updater: removed `AUTO_UPDATER_ENABLED`; `GITHUB_REPO = minhkhang1008/ctdozlite-next`.
    Restored the startup check (`gui.rs`) and the 30-min periodic check (`worker.rs`); the update
    notification goes through `PostNativeNotification` (`updater::new_update_notification`).
    New macOS in-place install in `updater.rs` (`current_app_bundle`,
    `install_macos_app_from_dmg`, `relaunch_app_bundle_after_exit`): it runs only when running
    from a .app and the DMG SHA-256 was supplied/verified. Steps: hdiutil attach -readonly
    (detached on drop) → codesign --verify --deep --strict → ditto to a staging .app → rename swap
    with a backup and rollback → strip quarantine. Relaunch waits for this PID to exit. Any
    failure falls back to the old "open the DMG" path. AGENTS.md rule #1 was rewritten: REST/MCP
    stays off; the updater may only use the fork repo.
  - Reply banner: `msg_area_height` now uses the measured height of everything below the list
    (`chat_bottom_stack_h`, measured from the scroll inner bottom to the composer bottom on the
    previous frame, with a repaint on change) instead of the 48px estimate. The reply preview is
    flattened to one truncated line.
  - `messages.rs` `hover_action_bar`: 28px circles (quote, forward, three dots), left of outgoing
    and right of incoming bubbles, bottom-aligned. Shown when hovering the bubble or bar (same
    layer, inside the clip) or while its menu is open. "…" opens the existing menu in
    `compact` mode (no emoji row, no reply/share; separator after Copy; "Tuỳ chọn khác ›" flyout
    holding reminder / open photo / save file via the new `message_extra_menu_items`). The
    outside-click close ignores the flyout and the "…" toggle. Shared `menu_frame` /
    `style_menu_ui`. Right-click keeps the full menu.
  - Share dialog (`modals.rs::render_forward_dialog`) rewritten: modal card with "Chia sẻ" + ✕,
    accent-insensitive search, tabs Gần đây / Nhóm trò chuyện / Bạn bè, a checkbox list with
    avatars (posting-restricted groups greyed), a preview box (link / photo / file / sticker /
    text), an optional note, Hủy / Chia sẻ (n). `WorkerCmd::Forward` gained `note`, sent after a
    successful forward in the same task (keeps order) and echoed as `MessageSent`.
  - Theme (dark): canvas #121417, sidebar #1B1E22, input #101215, border #2E3237, in-bubble gray
    #30343A, out-bubble blue-gray #343D4A.
  - Docs: README, TINH_NANG, CAI_DAT (auto update), ADR 0002 re-activated, CHANGELOG
    [Unreleased]; every `minhkhang1008/ctdozlite` link → `-next` (repo was renamed).
- Verification: `cargo build --locked` and `cargo test --locked` all pass; clippy reports nothing
  new in the touched code (the remaining warnings predate this work). NOT verified in the
  running GUI (no live session available here): hover bar, menus, share dialog, reply layout,
  colours. The macOS in-place update is untested end to end; it needs a newer release (v0.2.1+)
  installed over v0.2.x.

## 2026-11-12 — Faster CI; BrowserSkill pass: multi-select, message details, share "Phân loại", "Copy link"  (agent: Claude Code, claude-opus-5-5)

- Request: apply the CI speed-up, re-run BrowserSkill and do what was skipped last time.
- CI (`ci-release.yml`): the four installer jobs now run only for `v*` tags, workflow_dispatch,
  or PRs labelled `ci:full-package`. A push to main runs just `test`. `paths-ignore`
  (`**.md`, `docs/**`, `.agents/**`, `re-notes/**`, `LICENSE-*`) skips docs-only pushes and PRs
  (path filters do not apply to tags). YAML validated.
- BrowserSkill (My Documents only, nothing sent or deleted): this time the tab activated. Observed:
  - Hover bar = quote / forward / "Thêm" (bottom-aligned, left of own bubble), plus a reaction
    row.
  - The "…" menu on an own link message in My Documents = Copy link / Đánh dấu tin nhắn /
    Chọn nhiều tin nhắn / Xóa. It opens upward near the bottom. In groups it also has Ghim,
    Xem chi tiết and Tuỳ chọn khác (owner's screenshot).
  - "Chọn nhiều tin nhắn" shows a bar above the composer: "[n] Đã chọn · Sao chép · Chia sẻ ·
    Xóa (red) · Hủy". Cancelled afterwards.
  - The share dialog's "Phân loại" is a checkbox dropdown of the account's conversation labels
    with colour tags. Closed with Hủy.
- Code:
  - `chat.rs`: `ConvLabel`, `parse_conv_labels`, `fetch_conv_labels`
    (`GET label-wpa/api/convlabel/get`, read-only; source evidence in re-notes/40). +1 test.
  - `worker.rs` `FetchConvLabels` → `GuiEvent::ConvLabelsLoaded`, fetched lazily when the share
    dialog first opens.
  - Share dialog: a "Phân loại ▾" dropdown filter (ARGB/RGB colours parsed safely);
    `forward_more` lets one share carry several messages (the note only follows the last
    message per target).
  - `messages.rs`: `BubbleAction::{StartMultiSelect, ToggleSelect, ShowDetails}`;
    `multi_select_overlay` (temp-data keys `MULTI_SELECT_ACTIVE/KEYS` set by the workspace before
    the list; when active, each bubble is a toggle with tint, border and check badge, and
    hover/menus/double-click are skipped). Menus gained "Chọn nhiều tin nhắn" and
    "Xem chi tiết". Copy is labelled "Copy link" for link messages.
  - `workspace.rs`: `open_share_dialog`, `multi_selected_messages`, `render_multi_select_bar`
    (Sao chép joins `message_copy_text`, Chia sẻ opens the dialog with all selected, Xóa sends
    DeleteForMe for each, Hủy). Esc exits; switching thread clears the selection.
    `modals.rs::render_message_details`: sender, local full time, type, delivery state,
    reactions, id + copy.
  - Docs: CHANGELOG, TINH_NANG, HUONG_DAN, CAI_DAT (CI timing), re-notes/40.
- Verification: `cargo test --locked` all pass (275 lib); no clippy warnings inside the new
  functions. rustfmt was run on messages/modals/workspace/chat only. NOT verified in the running
  GUI. `convlabel/get` is not live-tested (no valid local session); the parser follows the
  source and zca-js.

## 2026-11-12 — Light-mode chrome, QR auto-refresh + help, no phone login, start maximized, sidebar labels, drop .mailmap  (agent: Claude Code, claude-opus-5-5)

- Requests: (1) remove phone login; (2) QR: phone sometimes says "không có kết nối với máy chủ";
  guide the user, and auto-refresh/expire like Zalo PC; (3) header still says "ctdozlite"; (4) is
  .mailmap needed?; (5) light mode: title bar text/icons, version pill and info panel unreadable;
  (6) open full screen; (7) implement "Phân loại" fully.
- Findings:
  - (3) The owner's screenshots show "Native v0.1.0", an old binary. Current code renders
    `APP_DISPLAY_NAME` and the version from Cargo.toml (0.2.0). Verified by launching a fresh build
    with a temp data dir: the title shows "ctdozlite-next v0.2.0". A second Zalo icon in the Dock
    was the owner's running old copy.
  - (5) Light `bg_app` is Zalo blue, but the title bar used `text_*` colours; the drawer footer
    used `bg_app` as a panel fill.
  - (2) `wait_for_scan` / `confirm` errors were silently dropped, so an expired QR stayed on
    screen forever.
  - (6) `ViewportBuilder::with_maximized(true)` with `decorations(false)` made the window never
    appear on macOS (reproduced: process running, no window). Fixed by sending
    `ViewportCommand::Maximized(true)` on the first frame (verified: window opens maximized).
- Changes:
  - theme.rs: chrome tokens (`chrome_fg`, `chrome_fg_muted`, `chrome_hover`, `chrome_pill_bg`,
    `chrome_pill_fg`) are white-on-blue in light and match the old look in dark. The title bar
    uses them; the pill now reads `v0.2.0`. The drawer footer uses `bg_hover`.
    `label_color()` helper (RGB/ARGB).
  - gui.rs: the login screen is QR only (LoginTab/phone/password state removed; the worker
    `LoginPassword` and the CLI `--phone/--pass` remain). QR countdown from
    `qr_login.qr_timeout` (minus 5 s), auto `request_new_qr` on expiry, `GuiEvent::QrExpired`
    (from worker scan/confirm errors) triggers auto refresh, capped at 3 failures within 20 s
    each. Collapsible help: quit/reopen Zalo, refresh QR, check network/VPN, confirm in time.
  - Sidebar labels: server labels replace the hard-coded local categories. Colour tag on rows,
    "Phân loại ▾" filter next to the tabs (refetches on open), and a read-only label list in the
    right-click menu. `SetLabel` action removed; `settings.thread_labels` is left unused for
    settings compatibility. Labels are fetched at startup (if logged in) and on LoginSuccess.
  - Removed `.mailmap` (only merges upstream contributor identities in git log/shortlog).
  - Docs: TINH_NANG, HUONG_DAN (QR help), CHANGELOG, re-notes/40 (why there is no write path).
- Verification: `cargo test --locked` all pass; no clippy warnings in the new code. Launched a
  debug build in light mode with a temp data dir: the title bar is readable, there is no phone
  tab, the countdown and help link render, and the window starts maximized. NOT verified: a
  logged-in session (sidebar labels, the drawer in light mode) and real QR expiry auto-refresh
  end to end.

## 2026-11-12 — Update dialog shows the actual changes, not just links  (agent: Claude Code, claude-opus-5-5)

- Request: the in-app "Cập nhật phần mềm" card listed only CHANGELOG/CAI_DAT links; show a short
  list of the changes instead.
- Cause: the CI release body was a fixed pointer text, and the app lists every `- ` bullet of the
  release body (`extract_concise_changelog`).
- Changes:
  - New `scripts/release_notes.py <ver> [CHANGELOG] [owner/repo]`: takes the `## [ver]` section
    and emits one short line per bullet (the bold title, minus "(`src/…`)" refs), with headings
    mapped to Vietnamese and a non-bullet "Chi tiết / Cài đặt" line at the end.
  - `ci-release.yml` release job: sparse-checks out CHANGELOG + script, builds
    `release_notes.md`, uses `body_path` with `generate_release_notes: false` (GitHub's commit
    list leaked "by @user in https://…" bullets into the dialog).
  - `modals.rs`: `short_changelog_title` trims long "**Title:** text" bullets. The extractor
    skips link-only bullets ("Có gì mới:", "Cài đặt:") and GitHub auto commit lines. +1 test.
  - The published v0.2.0 release body was regenerated from the tagged CHANGELOG and updated
    with `gh release edit`, so the installed app shows the list now.
- Verification: the script output was checked for 0.2.0; workflow YAML validated; the new test
  passes along with the full `cargo test --locked`.
