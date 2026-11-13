# Android manual phone sync and independent server refresh

Date: 2026-11-13. This is source inspection and synthetic verification, not a new
phone traffic capture. The owner identified their phone as Android.

## Evidence and request selection

- Before this correction, automatic startup/transfer refresh and manual phone sync
  used the same worker command, running/cancel flags, progress and completion event.
  A manual click could observe an automatic server refresh and send no phone request.
- The preceding integration made every manual attempt V2-only. Missing local WASM
  therefore aborted the attempt even though the repository already contained the
  native RSA backup request/import implementation.
- `web-bundle/lazy-1.c72c0957b455ab70b73c.js`, `static pullMobileMsg`, sends GET command
  12000 on the file service with pc_name/public_key/from_seq_id/is_retry/min_seq_id/
  temp_key/imei. Manual requests use an empty temp_key. The correction keeps that
  payload and sends a single GET; a rejection does not trigger a second POST prompt.
- The same bundle's `static getNoisedUids` uses GET `/api/znoise` on the profile
  service (12390) with fids/gids. Native import now uses this existing standard
  request path instead of a literal host and POST. Senders are included in the
  identity mapping so the hardcoded development account number can be removed.
- RSA PKCS#1 v1.5, the uppercase-hex AES key derivation, zero-IV CBC per 65536-byte
  chunk, ZDB4.0 header, XZ payload and ChatContent fields retain the existing
  importer contract. Compatibility with current Android Zalo is not yet measured.

## Lifecycle and persistence

`RefreshMessageQueues` has separate running/cancel flags and an account-scoped
result; it never emits phone progress or completion. A manual request interrupts
an automatic refresh at the next cancellation check and waits on the existing
account-operation lock before starting. Settings and the rail share this action.

Android waits up to 210 seconds for a backup to be imported. Approval alone is not
completion. Rejection/busy controls arrive as typed events from both transports.
A backup must belong to the active account and decrypt with the active request
key; replayed socket/poll events are accepted once. Cancel/retry/account changes
wait for the decoder before key cleanup or account-file changes. Failure to send,
a rejection, invalid input and timeout cannot become successful cloud completion.

RSA generation/decryption, CBC/XZ decoding, temporary SQLite reads and local
persistence run in the blocking pool. Inputs retain the trusted Zalo URL rule,
256 MiB download cap, 512 MiB decompression cap, private temporary directories/files,
and bounded archive names/file count/lengths. No WASM is used on this Android path.
No third-party runtime assets are downloaded or redistributed by this correction.

Mapped sender IDs identify outgoing messages for the logged-in account. Imported
messages retain their actual content; the previous invented Maps sentence is
removed. SQLite errors propagate; a savepoint rolls back the entire backup on
failure, and duplicate message IDs add zero rows. Thread records are created so
new conversations can be hydrated into the final UI snapshot. Success counts
reflect committed phone insertions separately from server refresh messages.

## Offline verification and manual acceptance

Regression: incomplete phone sync previously included “Đã hoàn tất”; a new
assertion failed before changing this to “Đồng bộ chưa đầy đủ”.

Synthetic tests cover encrypted CBC/XZ/SQLite input, malformed key/header/lengths,
sender/account mapping, new-thread hydration, idempotent retry, rollback, duplicate
receipt, account gating and typed rejection/busy controls. GUI tests cover the
persisted platform selection and truthful completion message.

Run a build containing this change, select Android in Settings, keep mobile Zalo
online, and use either sync button. Confirm the phone receives the request (and
approve if prompted); verify a known old message appears in the correct conversation
and survives reopening. Then test cancel/retry and refusal on the phone. Record the
actual outcome without tokens, request URLs or message contents in shared logs.

Still unverified: a real Android confirmation, the live identity mapping and backup
schema, full historical/media coverage, installed-app replacement and phone transfer
across account changes. An iPhone attempt still requires the existing zproto WASM
runtime; this correction does not claim standalone V2 provisioning is complete.

## Platform choice correction

The owner also distributes the app to relatives using iPhones. The GUI therefore
has no preselected platform for an unset preference: the first sync opens Settings
and requires Android or iPhone before sending a request. Either explicit choice is
persisted; existing explicit Android/Ios settings still deserialize correctly.
This is a local installation preference and can be changed in Settings.

## Completion follow-up, 2026-11-13

The GUI now requires an explicit Android/iPhone choice when unset. Standalone
Sync V2 runtime provisioning is implemented and checked using an actual module;
see `45_remaining_integration.md`. Earlier missing-runtime limitations above
describe the preceding slice. Real phone transfer remains unverified.
