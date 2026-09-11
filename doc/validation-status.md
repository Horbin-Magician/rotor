# Rotor 3.0.0 validation status

Work in progress on Windows x64, 2026-09-11. No public release or channel promotion
has been performed by this task. Automated and actual acceptance results are separate.

## Completed checks so far

- Common identity tests in development and production modes.
- Platform single-instance and startup quoting tests after removing old adapters.
- Runtime, screenshot persistence and search tests; synthetic native profile roundtrip.
- Signature, incorrect-key, tampering, truncation and portable bundle recovery tests.
- Selected-platform manifest and channel/config agreement tests.
- Release metadata Python tests and workflow YAML parsing.
- Windows and macOS dependency graph checks; no Tauri packages in Cargo.lock.

## Acceptance still to run

- Final workspace fmt/check/test/clippy and OCR smoke after all changes.
- Package build/stage/verify/package, installed resource discovery and isolated
  Windows installation/recovery/uninstall.
- Real macOS build, notarization/Gatekeeper, install and update failure recovery.
- Interactive multidisplay/scaling, capture/pins/OCR, IME and shortcuts, translation
  cancellation, startup/tray lifecycle and current application screenshots.
- Remote PR CI, review, merge, tag, hosted key pairing, draft review and publishing.

Do not mark these gates passed using source inspection or simulated fixtures.
