# History sync entry points and initial cursor

Date: 2026-11-12

## Evidence

- The owner's screenshot shows `/api/cm/getoldv2` returning server error `-69`
  (`Stop retry`); the displayed error also included the encrypted request query.
  This establishes a rejection, not its precise server-side cause.
- In the checked-in `web-bundle/lazy-1.c72c0957b455ab70b73c.js`, search for
  `static getCM(e,t="0",n=0,a=50`: the defaults are cursor `"0"` and recent mode.
  It sends `globalMsgId:t`, and chooses command `12507` / `/api/cm/getoldv2` only
  when `n` is truthy; otherwise it uses `12506` / `/api/cm/getrecentv2`.
- The previous Rust history loop always selected old mode, even when SQLite had
  no cursor. `get_cloud_messages` replaced an empty or zero cursor with
  `9223372036854775808`, a sentinel absent from that bundle's `getCM` flow.
- In this checkout the rail and Settings phone-sync buttons already sent the
  same `SyncPhoneMessages { send_phone_prompt: true }` command. A different
  command was therefore not demonstrated as the cause of the reported rail issue.

## Change

An empty/zero cursor now starts with recent messages at `"0"`. Once a real
message ID is available, paging switches to old mode. Existing stored cursors
still resume old mode. Request signing, encryption, cookies and host trust checks
remain on the existing session path.

Both phone-sync buttons call one UI helper. The rail opens Settings → message
sync to make progress, cancellation and the completion result visible. The
Settings button remains in place. Server failures are shown in Vietnamese with
their code, without request URLs; a run with errors says it is incomplete.
The worker pauses 400 ms between threads, including failed ones: previously a
failure before the page sleep imposed no delay between threads; now there are at
most 2.5 thread starts/second from this loop, excluding request time. This is a
configured bound, not a measured live throughput benchmark.

## Verification boundary

Offline tests cover initial/reused cursors, unchanged real cursor values and
redacted `-69` reporting. A real phone/server run was not performed. The cursor
correction does not prove that every server-side `-69` rejection is resolved.

Manual check with a logged-in account: click the rail button and observe the same
progress/result as the Settings phone-sync button; then run Group & Cloud sync
both with a new conversation and with cached history. Verify that recent data is
stored and a second run pages backward. If Zalo still rejects a request, retain
the error code and endpoint path only, without cookies or query parameters.
