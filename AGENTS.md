# AGENTS.md

## Project

Rotor is a Rust/GPUI native desktop toolbox. The root Cargo workspace is the
application workspace; `rotor-desktop` is the default target. All application and
shared crates live in `crates/`, models/fonts/icons in `assets/`, packaging in
`native/`, and release tooling in `xtask/`.

Use `Cargo.toml`, `rust-toolchain.toml`, `.github/workflows/native-checks.yml`,
and the relevant source/scripts to verify commands and requirements. Keep this
file focused on durable working instructions, not historical test-pass claims.

## Constraints

- Keep shared business crates free of GPUI dependencies.
- Platform code may accept wrapped native handles; it must not depend on UI Entity types.
- Keep window/lifecycle code in rotor-desktop and rendering/interaction in rotor-ui.
- Workers publish typed runtime events; the desktop shell owns window lookup and
  generation checks. Preserve cancellation and stale-result rejection.
- Do not move old integration adapters into shared crates.
- Preserve unknown config/record fields, accepted write ordering and request identities.
- Use isolated synthetic profiles for tests; avoid real credentials and user data.
- Do not edit generated/dependency output under target unless explicitly required.
- Preserve model/font bytes and their licenses. `.github/scripts/check-annotation-font.ps1`
  verifies the font and OFL checksums.

## Commit messages

- Write commit subjects and bodies in English.
- Use Conventional Commits: `<type>(<scope>): <summary>`. Omit the scope when
  the change spans the repository or has no meaningful component scope.
- Choose from `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`,
  `chore`, and `revert`; use a concise scope such as `ui`, `desktop`, `runtime`,
  `search`, `capture`, `updater`, or `windows` when appropriate.
- Start the summary with a lowercase imperative verb, describe the concrete
  change, and omit the trailing period. Aim for 72 characters or fewer, but
  prioritize clarity. Avoid vague summaries such as "update" or "tiny fix".
- Add a body when needed to explain motivation, behavior changes, or validation
  limits. Separate it from the subject with a blank line.
- Mark breaking changes with `!` before the colon and explain the impact and
  migration in a `BREAKING CHANGE:` footer.
- Examples: `fix(capture): correct mouse bounds on scaled displays` and
  `docs: document native packaging requirements`.

## Commands

Run from the repository root. Use the toolchain pinned in `rust-toolchain.toml`
(currently Rust 1.97.0), with rustfmt and clippy. Windows requires MSVC C++ Build
Tools and a Windows SDK. PowerShell scripts use PowerShell 7.

```powershell
cargo run -p rotor-desktop -- --no-elevate --data-dir target/dev-profile
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
./.github/scripts/check-core-dependencies.ps1
./.github/scripts/check-annotation-font.ps1
```

Use checks appropriate to the change; the commands above are the workspace
baseline. CI also runs these nonvisual integration checks:

```powershell
cargo run -p rotor-desktop --example ocr_smoke --release --locked -- --wait-idle
cargo run -p rotor-screenshot --example profile_roundtrip --locked -- target/profile-roundtrip
```

The OCR example generates its fixture under `target/ocr-smoke`; profile roundtrip
requires a new destination. Release metadata changes also use
`python .github/scripts/test-prepare-gitee-release.py` and
`python .github/scripts/test-prepare-native-draft.py`.

## Packaging and releases

```powershell
cargo run -p xtask -- build
cargo run -p xtask -- stage target/native-stage
cargo run -p xtask -- verify target/native-stage
cargo run -p xtask -- package target/native-stage target/native-package
```

Stage/package destinations must be new. NSIS 3.11 is required on Windows; set
`NSIS_MAKENSIS` when it is not on PATH. Node/Yarn are not native build prerequisites.

Always build through xtask before staging: a bare `cargo build` does not create
the required binary snapshot. Build/stage must use the same identity; pass
`--production` to both for production packages. Stage/package reject stale
version, identity or source/asset receipts, so rebuild after input changes.
`resources.json` checks file integrity; it is not a signature or trust root.

The root workspace version is authoritative. `xtask set-version <semver> --dry-run`
previews a change; actual editing requires a clean worktree and updates Cargo.lock.
It never commits, tags or pushes. `publish.yml` creates native release drafts;
update-feed promotion is separate. `native-candidate.yml` produces review
artifacts without publishing a release and defaults to Windows. Keep native
preview feeds separate from the legacy `latest.json` channel. Existing updater secret names beginning with
`TAURI_SIGNING_` remain for key compatibility, not as a Tauri runtime dependency.

## Module map

- rotor-desktop: app/window lifecycle, tray, hotkeys, capture/pin placement, logging.
  `src/bin/rotor-recovery.rs` is the separate Windows startup recovery launcher.
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

Release optimization does not select production identity; the `production`
feature does. `--resource-dir` or `ROTOR_RESOURCE_DIR` overrides the assets root.
Development builds, including release builds, may fall back to checkout assets;
production requires deployed assets or an explicit override. Resource discovery
must remain independent of the working directory.

Invoke `--build-info` alone. Use `--no-index` and `--no-hotkeys` for isolated
runtime checks that do not need those services; they do not make normal startup
headless. Prefer the diagnostics and examples above when windows are unnecessary.

Stored Windows defaults are Ctrl+Shift+F/S/D/W for search/capture/selection/input
translation. Development adds Alt and provides Ctrl+Alt+Shift+G for Settings.
Pin-local defaults are S/Enter/H/Escape. IME/text editing owns its own key handling.

## Validation and distribution

Keep implementation, automated checks, skipped visual tests and actual installation
results distinct. Windows silent checks use `.github/scripts/test-windows-install.ps1`
with development identity, an unused registry namespace and a new target subdirectory.
They preserve synthetic user files and backups and clean their installation entries.

The installation script requires an already elevated PowerShell 7 session on
64-bit Windows. `-PreviousPackageDirectory` requires a verified, lower-version
development package; `-TestRecovery` and `-MeasureIdle` exercise only the dedicated
test installation. These do not establish production upgrade, UAC interaction,
legacy-client handoff or full performance acceptance.

Profile import/rollback uses `xtask import-profile` and `verify-profile`; consult
`native/README.md` before use. Close the source application, retain a separate
backup, and use new destinations. Tests must use synthetic data. The importer
copies `config.toml` and the complete `shotter` tree; search indexes rebuild.

macOS code remains, but Windows results do not prove macOS build, runtime or packaging acceptance.
