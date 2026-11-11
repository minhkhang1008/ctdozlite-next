# 39 — GIF search and send (Giphy via Zalo)

Status: **capture-verified 2026-11-11** on chat.zalo.me (owner's account, BrowserSkill). One
GIF was sent to the owner's own "My Documents" thread.

## Evidence

- Network capture (BrowserSkill debug run): opening the GIF tab of the sticker box, typing a
  keyword, then clicking a result produced these requests:
  - `GET https://tt-sticker-wpa.chat.zalo.me/api/message/giphy/trending?zpw_ver=695&zpw_type=30&params=<AES>`
  - `GET https://tt-sticker-wpa.chat.zalo.me/api/message/giphy/search?zpw_ver=695&zpw_type=30&params=<AES>`
  - `POST https://tt-files-wpa.chat.zalo.me/api/message/gif?zpw_ver=695&zpw_type=30&params=<AES>`
    with an **empty body** (params travel in the query string). Response:
    `{"error_code":0,"data":"<AES>"}`.
  - GIF images load from `https://zgif-v2.zdn.vn/<hash>.png|.gif?z-cors=no` (trusted `*.zdn.vn`).
- Source: lazy chunk `https://zalo-chat-static.zadn.vn/v1/lazy/1.c72c0957b455ab70b73c.js` (the
  initiator of the search request). It is 16 MB, so it is kept only locally as
  `web-bundle/lazy-1.c72c0957b455ab70b73c.js` (git-ignored); re-download it from that URL to
  re-check. Search for `static sendGif(`, `searchGif(`, `getGifTrending(` and `SGG_SRC_`.

## Wire format (from the chunk)

| call | host | params JSON (AES, session key) |
| :--- | :--- | :--- |
| `getGifTrending` (cmd 11296) | `apiGifDomain` = sticker service | `{"offset","limit","imei"}` |
| `searchGif` (cmd 11297) | sticker service | `{"keyword","offset","limit"}` (literal key order) |
| `sendGif` 1-1 (cmd 11292) | file service, `/api/message/gif` | see below |
| `sendGif` group (cmd 11194) | file service, `/api/group/gif` | see below |

Send params, in object order: `thumb` (normal URL), `width`, `height` (original size), `msg`
(`""`), `type` (`0`), `href` (original URL), `contentId` (Giphy item id), `visibility` (group
only), `src` (source code), then `toid` (1-1) or `grid` (group, without the prefix) and
`clientId` (the client message id, **as a JSON string**). `buildParams("sendGif")` adds
nothing for this path (`transformParams` only runs when `src` is truthy in its own options).

Response `data` (decrypted) carries `data_list: [GifItem]`, where
`GifItem = {id, original:{url,width,height}, normal:{url,width,height}}`.

`src` codes (`SGG_SRC_*`): `-1` unknown, `1` chat-bar GIF box, `2` suggestion, `3` right menu,
`4` event, `6` search right menu, `7` composer `@gif` command (`SGG_SRC_SEARCH_INPUTBOX`).

Composer command: the official client treats `@gif <keyword>` in the input box as a GIF
search (`{cmd:"@gif", len:4, placeholder:"STR_SEARCH_GIF_2"}`). With an empty keyword it shows
trending. The default limits from config are `search_by_kw_limit: 50` and
`get_trending_limit: 50`.

## Implementation

`chat::gif_trending`, `chat::gif_search`, `chat::send_gif_with_client_id`; GUI `@gif` popover
in the composer, gated by `AppSettings::enable_gif_search`.
