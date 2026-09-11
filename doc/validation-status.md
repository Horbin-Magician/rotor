# Rotor 3.0.0 validation status

Windows validation completed on 2026-09-11 with Rust 1.97.0, MSVC and NSIS 3.11.
See [machine-readable evidence](validation/windows-3.0.0.json) for artifact hashes,
source receipts, identities, ignored tests and isolated installation results.
No version tag, public release or channel promotion has been performed by this task.

## Passed automated checks

- Workspace `cargo fmt --all -- --check`, `cargo check --workspace --locked`,
  `cargo test --workspace --locked` and strict all-target Clippy.
- Common identity tests in development and production modes.
- Platform single-instance and startup quoting tests after removing old adapters.
- Runtime, screenshot persistence and search tests; synthetic native profile roundtrip.
- Signature, incorrect-key, tampering, truncation and portable bundle recovery tests.
- Selected-platform manifest and channel/config agreement tests.
- Release metadata Python tests and workflow YAML parsing.
- Windows and macOS dependency graph checks; no Tauri packages in Cargo.lock.
- OCR smoke recognized Chinese and English and passed idle-resource release.
- Fresh native configuration/pin roundtrip passed with synthetic data.
- Both development and production build/stage/verify/package completed.
- Staged development and production resources were discovered from a working
  directory outside the checkout.

The two manual index-memory measurements remain ignored in the standard test run.
Portable macOS bundle tests do not establish execution on a Mac.

## Passed Windows installation checks

The development installer was exercised in a new synthetic directory with unused
registry/startup entries. Fresh installation, refusal of an unrelated nonempty
directory, rejection of an actively locked installation, same-version replacement,
installed resource discovery, corrupt-executable rollback, background restart and
uninstall passed. Synthetic profiles and user-added files survived as intended.

Fault injection initially exposed a blocking Windows loader dialog. The independent
recovery executable now suppresses OS loader error dialogs before attempting launch.
An observed directory-switch failure also led to bounded rename retries. The final
test run passed both recovery and locked-installation rejection after these fixes.

Local outputs are `target/native3-delivery-package/Rotor-Dev_3.0.0_x64-setup.exe`
and `target/native3-production-final-package/Rotor_3.0.0_x64-setup.exe`.
The production installer was built and verified, but was not installed by the
development-only acceptance script. Packages are unsigned local review artifacts;
synthetic signature tests do not prove access to the production signing key.

## Inspected UI

[Screenshots](screenshots/README.md) show general settings, a restored native pin
with a rectangle drawn through its toolbar, and actual search/translation views
hosted by the synthetic-profile gallery. These images do not replace full popup,
global-shortcut, provider, multi-display or IME acceptance.

## Outstanding acceptance and release work

- Real macOS build, notarization/Gatekeeper, install and update failure recovery.
- Production installation/UAC and the complete multi-display/scaling,
  capture/OCR, IME/global-shortcut, translation cancellation and login/tray matrix.
- Hosted signing-key pairing, remote CI/review, merge, tag, draft review and publishing.

The review branch `codex/native-3-release` is pushed. GitHub's connector returned
403 (PR creation not accessible to the integration), and the available in-app
browser is signed out. No PR was created. The branch includes the preceding native
migration commits; master is an ancestor, with no local merge conflict to resolve.

Do not mark these gates passed using source inspection or simulated fixtures.
