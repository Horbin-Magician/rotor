# AGENTS.md

## Project and migration state

Rotor is a Rust/GPUI native desktop toolbox. The root Cargo workspace is the
application workspace; `rotor-desktop` is the default target. All application and
shared crates live in `crates/`, models/fonts/icons in `assets/`, packaging in
`native/`, and release tooling in `xtask/`.

The user explicitly waived further visual UI tests and deferred macOS validation.
Continue Windows engineering and nonvisual checks. Record skipped tests as skipped,
never passed. Current work and evidence are in `doc/gpui-migration/remaining-tasks.md`.

Legacy frontend/Tauri sources are no longer workspace members or native CI inputs.
Their physical deletion was blocked by automatic approval review; use the explicit
removal plan and do not retry deletion through an alternative method without new
user authorization. Retained legacy code is historical, not a development target.

## Constraints

- Do not enable browser debugging.
- Keep shared business crates free of Tauri, GPUI and WebView dependencies.
- Platform code may accept wrapped native handles; it must not depend on UI Entity types.
- Keep window/lifecycle code in rotor-desktop and rendering/interaction in rotor-ui.
- Do not move old integration adapters into shared crates.
- Preserve unknown config/record fields, accepted write ordering and request identities.
- Use isolated synthetic profiles for tests; avoid real credentials and user data.
- Do not edit generated/dependency output such as node_modules, dist, target or retained
  src-tauri target/gen directories unless explicitly required.
- Preserve model/font bytes and their licenses. `.github/scripts/check-annotation-font.ps1`
  verifies the font and OFL checksums.

## Commands

```powershell
cargo run -p rotor-desktop -- --no-elevate --data-dir target/dev-profile
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p xtask -- build
cargo run -p xtask -- stage target/native-stage
cargo run -p xtask -- package target/native-stage target/native-package
```

Stage/package destinations must be new. NSIS 3.11 is required on Windows; set
`NSIS_MAKENSIS` when it is not on PATH. Node/Yarn are not native build prerequisites.

The root workspace version is authoritative. `xtask set-version <semver> --dry-run`
previews a change; actual editing requires a clean worktree and updates Cargo.lock.
It never commits, tags or pushes. `publish.yml` creates native release drafts;
update-feed promotion is separate. Existing updater secret names beginning with
`TAURI_SIGNING_` remain for key compatibility, not as a Tauri runtime dependency.

## Module map

- rotor-desktop: app/window lifecycle, tray, hotkeys, capture/pin placement, logging.
- rotor-ui: settings, search, translation, screenshot masks, pins/annotations/OCR UI.
- rotor-canvas: GUI-independent document, geometry and offscreen composition.
- rotor-common: config transactions, paths, resources, identities, profile migration.
- rotor-platform: filesystem/index helpers, clipboard/selection, startup, installation,
  elevation, window geometry and OS integration.
- rotor-runtime: asynchronous services, event identities, bounded queues, cancellation,
  settings/hotkey coordination, pins and updates.
- rotor-searcher: indexing, exclusions, ranking, pagination and release lifecycle.
- rotor-screenshot: capture, legacy-compatible pin records, image helpers and OCR.
- rotor-translator: Google, DeepSeek streaming and custom HTTP engines.
- rotor-updater: manifests, signatures, download and platform handoff/recovery.

## Profiles and shortcuts

Development uses `.rotor-gpui`; production identity uses `.rotor`. `--data-dir` or
`ROTOR_DATA_DIR` selects an explicit profile. `--build-info` and `--check-resources`
return before windows or profile initialization; `--check-config` needs an explicit
synthetic data directory during tests.

Stored Windows defaults are Ctrl+Shift+F/S/D/W for search/capture/selection/input
translation. Development adds Alt and provides Ctrl+Alt+Shift+G for Settings.
Pin-local defaults are S/Enter/H/Escape. IME/text editing owns its own key handling.

## Validation and distribution

Keep implementation, automated checks, skipped visual tests and actual installation
results distinct. Windows silent checks use `.github/scripts/test-windows-install.ps1`
with development identity, an unused registry namespace and a new target subdirectory.
They preserve synthetic user files and backups and clean their installation entries.

The independent P0 workspace remains in `experiments/gpui-probe`. Historical paths
and screenshots under migration evidence describe their recorded commits; prefer
current crate paths for ongoing edits. macOS code remains, but Windows results do
not prove macOS build, runtime or packaging acceptance.
