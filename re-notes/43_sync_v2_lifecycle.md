# Sync V2 lifecycle and host corrections

Date: 2026-11-12. Sources: upstream `7ed3408`, including `ea7e36f`,
`149ff00`, `4032f96`, `659b5f5` and the metadata ABI correction in `4b5f4fd`.
These are source observations, not new authenticated traffic captures.

## Reproduced host defect

With the WASM from upstream's Git object extracted only into `/tmp`, the old
host returned the same public key on two consecutive `generate_keypair` calls.
The new uniqueness assertion failed before the fix. The port models ArrayBuffer
and typed-array views distinctly and writes OS randomness through those views.
Two key generations and the symmetric key-agreement test now pass. TypedArray
copies are bounded by their destination rather than growing an owned buffer.

`zprotoSync2DecryptMetadata` in this snapshot takes eleven arguments. The old
host supplied thirteen. The test now reaches the crypto decoder with an invalid
synthetic message instead of failing the function-call signature check.

## Manual request

- Query `/api/znoise` through the existing profile-service request path with
  `fids`/`gids`. The checked-in bundle's `getNoisedUids` uses command 12390.
  Upstream's `sync_probe` demonstrates the ordered `fids` response and the
  `oneone/<noised-id>` partition format. The correspondence is covered by
  synthetic offline response tests; a live response has not been checked here.
- Generate a base62 request ID using OS entropy, not the old time-seeded hex ID.
- Reuse one identity key from `sync_identity.json`; write atomically with private
  directory/file permissions. A malformed existing identity is rejected without
  silently replacing it. Ephemeral keys remain fresh per request.
- Send one command 590 with the current hostname, empty manual `tempKey`, explicit
  conversation partitions and bounded per-partition requests. Do not send V1
  `pull_mobile_msg` or an initial 592 alongside it.
- Receive metadata (batch type 2), then message data (type 1). Data tasks wait
  for metadata rather than discarding an early batch. Both receiver paths share
  the same handler; decode/persistence is serialized and runs off the UI thread.
- Accept only the active request ID and decrypted metadata session. Map decoded
  partition names back to the requested real thread IDs before persistence.
- Commit each message batch atomically using a SQLite savepoint. An unknown
  partition rolls back the batch; replayed IDs add zero rows. Count only committed
  insertions. The final-batch marker alone cannot finish a run while earlier
  enqueued tasks are still pending.
- Release with command 591 on success/cancellation/error/timeout, then clear
  ephemeral keys and session state. Account switching/logout cancel and wait for
  the phone worker and any current decoder before changing live account files.
- Rehydrate the final thread snapshot from SQLite; the UI reports observed phone
  insertions separately from the independent Group/Cloud refresh.

The authenticated URL allowlist and download/decompression caps remain intact.
No raw request payload/key logging from upstream was copied. The legacy RSA
persistent-key file/test that deleted keys from the current directory was not
ported: the manual flow uses V2 exclusively.

## Verification boundary

Offline tests cover entropy/key agreement using the actual WASM module, eleven-
argument metadata ABI, private identity-file persistence, malformed protobuf
lengths, request construction, partition mapping, transactional rollback,
idempotency, completion ordering and truthful completion text. No secrets,
phone data or WASM binary were added to Git.

Not verified: approval on a real phone, live `/api/znoise` responses, historical
message contents from a real phone, or calls to other users. The module still
needs an installed Zalo PC WASM file or `CTDOZLITE_ZPROTO_WASM`; locating bundled
resources does not establish that a released app package includes that module.
Group/Cloud server paging remains a separate source; no claim that every server
`-69` rejection has been resolved is made.

## 2026-11-13 correction: select the phone path explicitly

The owner reported Android and no phone prompt, followed by a missing-WASM error.
The earlier V2-only manual selection was inappropriate for exercising the existing
Android backup path. That source change was not proof of real Android compatibility.
Settings now persist an explicit Android/iPhone selection (Android by default).
Only one selected transfer protocol runs; Android uses the existing native backup
import, while iPhone retains V2 and its existing runtime requirement.
See `44_android_phone_sync.md` for the restored path and verification boundary.

## Completion follow-up, 2026-11-13

The GUI now requires an explicit Android/iPhone choice when unset. Standalone
Sync V2 runtime provisioning is implemented and checked using an actual module;
see `45_remaining_integration.md`. Earlier missing-runtime limitations above
describe the preceding slice. Real phone transfer remains unverified.
