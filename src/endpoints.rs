//! Working endpoint table, taken from a live Zalo Web session (2026-10-21).
//!
//! Replaces guesswork with observed values. Every entry below answered `error_code 0`
//! during a real QR-authenticated session.

/// Service map as returned by `zpw_service_map_v3` on `chat.zalo.me`.
///
/// Note the domains are `*.chat.zalo.me`, **not** `*.zaloapp.com` — the PC bundle's
/// default table is stale for the web/QR session flavour.
pub const SERVICE_MAP_V3: &[(&str, &str)] = &[
    ("profile", "https://tt-profile-wpa.chat.zalo.me"),
    ("friend", "https://tt-friend-wpa.chat.zalo.me"),
    ("conversation", "https://tt-convers-wpa.chat.zalo.me"),
    ("chat", "https://tt-chat3-wpa.chat.zalo.me"),
    ("group", "https://tt-group-wpa.chat.zalo.me"),
    ("sticker", "https://tt-sticker-wpa.chat.zalo.me"),
    ("alias", "https://tt-alias-wpa.chat.zalo.me"),
    ("label", "https://label-wpa.chat.zalo.me"),
    ("reaction", "https://reaction.chat.zalo.me"),
    ("emoji_status", "https://emoji-status.chat.zalo.me"),
    ("zcloud", "https://zcld.chat.zalo.me"),
    ("file", "https://tt-files-wpa.chat.zalo.me"),
    ("voice_call", "https://voicecall-wpa.chat.zalo.me"),
];

/// `zpw_ctrs_new[0]` — conversation control plane.
pub const CONVERS_CTL: &str = "https://p5-ctl-chat.zaloapp.com";

/// `zpw_chts_new[0]` — message plane.
pub const MSG_CHAT: &str = "https://p5-msg-chat.zaloapp.com";

/// Auth host for `reqqr` / password login.
pub const AUTH: &str = "https://wpa.zaloapp.com";

/// Base host that serves `getServerInfo` for an authenticated web session.
pub const WPA_CHAT: &str = "https://wpa.chat.zalo.me";

/// Group board (pinned messages / notes / reminders). Zalo PC resolves this through
/// `apiGroupBoard = https://groupboard-wpa.api.${domain}`; web sessions answer on the
/// `chat.zalo.me` flavour, which is what the live capture came from (`friendboard-wpa.chat.zalo.me`).
pub const GROUP_BOARD: &str = "https://groupboard-wpa.chat.zalo.me";

/// Friend board (pinned messages in a 1-1 chat). Confirmed by a live Zalo Web capture:
/// `GET https://friendboard-wpa.chat.zalo.me/api/friendboard/create?...&params=<AES>`.
pub const FRIEND_BOARD: &str = "https://friendboard-wpa.chat.zalo.me";

/// Legacy board host (`apiBoardDomain`); kept as a fallback for `/api/board/pin`.
pub const BOARD: &str = "https://board-wpa.chat.zalo.me";

/// Pinned FFmpeg 9.0.2 static downloads, never used with Zalo credentials.
pub const FFMPEG_MAC_ARM64: &str = "https://www.osxexperts.net/ffmpeg902arm.zip";
/// Intel build from the macOS distributor linked by ffmpeg.org.
pub const FFMPEG_MAC_INTEL: &str = "https://evermeet.cx/ffmpeg/ffmpeg-9.0.2.zip";

/// Look up a service domain by key.
pub fn service(key: &str) -> Option<&'static str> {
    SERVICE_MAP_V3
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}

/// Exact publisher/mirror hosts allowed for pinned video-tool downloads.
pub const FFMPEG_DOWNLOAD_HOSTS: &[&str] = &["www.osxexperts.net", "evermeet.cx", "e.deolaha.ca"];

/// Public zproto runtime, fetched without session credentials and pinned by digest.
pub const ZPROTO_RUNTIME: &str =
    "https://chat.zalo.me/libs/libzproto_wasm_bg.5ea0fb2c9af4a45bc7882b71307b6f83.wasm";

/// Public asset origin used as the runtime request referer, without credentials.
pub const ZPROTO_PUBLIC_ORIGIN: &str = "https://chat.zalo.me/";
