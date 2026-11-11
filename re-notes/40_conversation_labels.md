# 40 — Conversation labels ("Phân loại"), read-only

Status: **implemented from source; request shape not capture-verified** (read-only GET).

## Evidence

- Zalo web lazy chunk `https://zalo-chat-static.zadn.vn/v1/lazy/1.c72c0957b455ab70b73c.js` (see
  re-notes/39 for how to fetch it):
  - `static getConversationLabel(e=0){let t={imei:…getZaloClientID()},
    n=p.b.getLabelDomain()+"/api/convlabel/get?"+this._getCommonParams()+"&params="+this._encodeParams(t);
    return this._get(n,null,12161)}`
  - `_encodeParams(e)` = `encodeURIComponent(encodeAES(JSON.stringify(e)))`, the same AES params as
    every other API.
  - The response handler destructures `{version, lastUpdateTime, labelData}`. `convertData`
    accepts `labelData` as a JSON string, an array or an object of items, keeps `offset` order,
    and uses `text` or the localised `textKey`.
- On chat.zalo.me (2026-11-12, owner's account) the share dialog's "Phân loại" dropdown lists
  the account labels (gia đình, bạn bè, …) with a colour tag and a checkbox each.
- zca-js `getLabels` documents the same item shape:
  `{id, text, textKey, conversations: string[], color, offset, emoji, createTime}`, where
  `conversations` holds `"<uid>"` for 1-1 and `"g<groupId>"` for groups.

## Implementation

`chat::fetch_conv_labels` → `GET {endpoints::service("label")}/api/convlabel/get?{common}&params=AES({imei})`.
`chat::parse_conv_labels` handles the string, array and object forms. `ConvLabel::contains`
requires the `g` prefix for groups (app group ids are bare numbers). The labels are only used to
filter the share dialog list; nothing is written back (`convlabel/update` is not used).

## Manual check

Open "Chia sẻ" on any message, then "Phân loại": your Zalo labels should appear with their
colours, and ticking one should leave only the conversations carrying that label.
