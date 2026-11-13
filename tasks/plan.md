# Upstream integration and video tools

Implement sequentially, preserving existing uncommitted sync/theme work and fork security.
No commit, push or release without the owner's explicit approval.

1. Port narrow-window clamp fixes and macOS bundle WASM discovery; module tests.
2. Port passive incoming ringing (efe708d), review answer/cancel/timeout races; call tests.
3. Add FFmpeg discovery and in-app installation with worker progress, bounded processes,
   no UI I/O, trusted package sources, and re-detection without restarting.
4. Integrate selected Sync V2 corrections with private key storage, isolated fixtures,
   existing URL trust/download/decompression limits and recent-first history retained.
5. Integrate drafts, Saved, reminders and advanced search in separate verified slices.
6. Integrate bounded document preview, voice AAC/audio fixes and Markdown rendering.

Each slice must pass relevant cargo tests and cargo check before the next slice.
Live Zalo/hardware verification must remain distinct from offline tests.

## Completion pass requested 2026-11-13

Continue existing scope; do not replace already implemented slices.
1. Standalone Sync V2 runtime: fetch pinned public Zalo WASM without session cookies,
   bounded download, SHA-256 + actual host validation, private atomic cache, cancellation.
2. Local productivity: persist per-thread drafts; Saved and reminders; advanced search
   operators with bound SQL and account-scoped worker results, native UI entry points.
3. Explicit document preview: bounded network and Office decompression, PDF/text/Office
   extraction off UI, no automatic attachment or link fetching.
4. Voice recording: native microphone/resampling and AAC ADTS encoding, worker-only,
   real voice parsing and safe temporary files; maintain call audio behavior.
5. Markdown: fenced code/tables/lists with copy/plaintext toggle, preserve mentions/emoji.
6. Focused module checks after each slice, final source review and updated user docs.
   Real phone/call verification requires human device interaction and is tracked separately.

Acceptance: every item has an accessible UI path and a terminal worker result; local data
survives restart and remains account-isolated; no required dependency must be manually
installed for the macOS workflows. No commit, push, release or installed-app replacement.

## Implementation checkpoint

All six source implementation groups are delivered. Acceptance evidence and UI entry
points are recorded in docs/HOAN_THIEN_PLAN_2026-11-13.md and the agent worklog.
Real-phone/call/native-package acceptance remains explicitly open in tasks/todo.md.
