# 38 — Group @mention tagging (outgoing)

Status: **implemented, NOT capture-verified**. Needs one manual send test (below).

## Problem

Picking a member in the composer's @ popover only inserted `@Name` as plain text. Every send
path sent `mentionInfo` empty (`""` / `"[]"`) or omitted it, so recipients never saw a tag.

## Wire format used

`POST {domains.group}/api/group/mention?{common_params}`, `params=` encrypted as usual. Fields:

| field | value |
| :--- | :--- |
| `grid` | group id without the `group_`/`g` prefix |
| `message` | clean text (after `extract_zalo_rtf_text_properties`) |
| `mentionInfo` | JSON **string** of `[{"pos","uid","len","type"}]` |
| `clientId` | client message id |
| `imei`, `ttl` (0), `visibility` (0) | as in `group/sendmsg` |
| `textProperties` | only when Markdown styling is present |

- `pos` and `len` are UTF-16 offsets into `message`. `len` covers `@Name`, including the `@`.
- `type` is `1` for @All (`uid == "-1"`), otherwise `0`.

## Evidence

- No capture yet. The official Zalo PC isn't installed on the dev machine.
- The shape matches the open-source zca-js `sendMessage` implementation (group messages with
  mentions go to `/api/group/mention`, `mentionInfo = JSON.stringify([{pos, uid, len, type}])`).
- It also matches what this app already parses from incoming messages: the `mentions[]`
  entries have `uid`, and `-1` means all (`src/unread.rs`, citing the original shared-worker).
- `vnuh05/ReZalo` README (an API list for this codebase) documents only the empty
  `mentionInfo` defaults in `message/link` and `group/sendlink`. It adds no new evidence.

## Safety net

If `group/mention` returns an error other than session expiry, the same text is resent
through `group/sendmsg`. The message arrives untagged rather than being lost. The failure is
logged as `chat::send_group_mention` (visible in `--dev`).

## Not covered

- Group text containing a URL **and** tags skips `sendlink`: it is sent as tagged text with no
  link card.
- 1-1 chats send no tags.

## Quote replies with tags (added 2026-11-11)

Double-clicking a group message enters reply mode and prefills `@Sender ` as a picked tag.
`group/quote` now carries the same `mentionInfo` JSON string when the reply text has resolvable
tags. Without tags the request is byte-for-byte unchanged.

- Evidence: zca-js `sendMessage` puts `mentionInfo` in the quote params too (its quote branch
  shares the mention handling with plain group text). As with `group/mention`, this is NOT
  capture-verified.
- Fallback: if `group/quote` fails, a group reply is resent through
  `send_group_mention_message_with_client_id`. That is `group/mention` with tags, or plain
  `sendmsg` without, so the text still arrives tagged but without the quote.
- 1-1 quotes (`message/quote`) are unchanged.
- Manual check: reply to a member's message in a test group with the auto `@Name` kept. On the
  phone, the reply must show the quote **and** a tag, and the member must get a mention
  notification.

## Manual verification

1. In a test group, type `@`, pick a member, then send `@Name thử tag`.
2. On the phone or official PC client, the name must render as a tag and the member must get a
   mention notification.
3. In `--dev`, the request must go to `/api/group/mention` with `error_code 0` and no
   `chat::send_group_mention` error.
