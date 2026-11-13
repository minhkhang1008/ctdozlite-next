# Integration progress

- [x] Narrow-window clamps and bundled WASM discovery — module tests pass
- [x] Passive incoming ringing — module tests pass; live call not verified
- [x] FFmpeg discovery and in-app installation — offline installer/layout tests and real ARM64 codec probe pass
- [x] Hardened Sync V2 corrections — real WASM and offline protocol/storage tests pass; phone confirmation/import not verified live
- [x] Separate automatic refresh from manual sync; restore explicitly selected native Android backup path — synthetic tests pass
- [ ] Verify real Android request/confirmation, identity mapping and old-message import
- [x] Drafts, Saved, local reminders, advanced search — worker-backed UI, account-local persistence, bound SQL and exact old-message lookup
- [x] Bounded document preview, voice AAC/audio, Markdown — explicit attachment fetch, Office expansion caps, isolated PDF helper, native capture and extended renderer

- [x] Standalone Sync V2 runtime preparation — official credential-free fetch, pinned SHA-256, private activation and actual WASM validation
- [ ] Verify iPhone approval/import, actual Android backup schema, and real voice/video send/receive
- [ ] Native GUI relaunch, microphone permission/device cases, and installed/release package checks

All implementation groups in the existing plan are now present in source. The unchecked
items require real devices or a runnable packaged build; offline tests are not substituted
for these checks. No commit, push, release or installed-app replacement performed.
