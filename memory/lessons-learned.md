# Lessons learned

Short imperative rules learned the hard way. Add one when the same mistake
happens twice or a review finding would have been prevented by a rule.
Loaded into every Claude session: keep it short. Newest first.
Format: `- YYYY-MM-DD: <rule> (why: <one line>)`

- 2026-11-10: Quote a `replica/features.csv` cell that contains a comma (why: BF184, BF188).
- 2026-11-07: Show the `make check` (or `buzzdome-verify`) output before calling a task done (why: agents tend to declare success early).
- 2026-11-07: Commit the `app/src/bindings` files `cargo test --workspace` rewrote in the same commit as the Rust type change (why: CI's check job fails on stale bindings).
- 2026-11-07: Build the UI (`npm run build`) before the first Rust build, and set `SHERPA_ONNX_ARCHIVE_DIR` from `.github/scripts/sherpa-onnx-libs.sh` (why: Tauri embeds `app/dist`, and the voice crate otherwise downloads unchecked archives or fails behind a proxy).
- 2026-11-07: Be green locally before pushing, and push before a manual "Run workflow" of Build (why: Actions minutes are limited and a push cancels the branch's running manual Build).
- 2026-11-07: Never run the E2E suite on a Windows machine with real app data (why: the Windows E2E harness uses and wipes the real app-data folder; CI only).
- 2026-11-07: When the disk fills, delete `target/debug/{deps,build,incremental,.fingerprint}`, not the whole repo (why: a clean `target/` takes about 12 GB).
- 2026-11-07: Gate a replay-changing rule on `rules_version` instead of changing old behaviour (why: a resumed older show must play on exactly as it was played).
