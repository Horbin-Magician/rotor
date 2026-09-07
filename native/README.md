# Native development packages

The root `workspace.package.version` is authoritative. `package.json` must mirror
it while the legacy Tauri build remains available; `xtask version` checks this.

```powershell
cargo run -p xtask -- version
cargo run -p xtask -- build
cargo run -p xtask -- stage target/native-stage
cargo run -p xtask -- verify target/native-stage
$env:NSIS_MAKENSIS = 'C:\path\to\NSIS\makensis.exe'
cargo run -p xtask -- package target/native-stage target/native-package
```

Stage and package destinations must not already exist. No command installs,
launches, publishes, signs, or deletes a previous build. Staging copies the release
executable, native dynamic libraries, all model/font/icon assets, native metadata,
and the update public key. `resources.json` records every staged file's size and
SHA-256; packaging verifies it before reading the files. This checksum manifest is
an integrity check, not a signature or trust root.

Windows packages use a separate `Rotor GPUI Development` install directory,
Start menu entry, and uninstall key. The existing `Rotor` installation and user
profile remain separate. Release executables use the GUI subsystem; automation
can still collect exit status and explicitly redirected `--check-config` output.
Windows icon/version are embedded at build time. The pinned GPUI dependency
embeds the per-monitor DPI awareness and as-invoker manifest; do not embed a
second manifest in the application. The application retains its optional elevation
handoff; installer elevation does not change application startup policy.

On macOS arm64, the same commands stage an app bundle and generate `.app.tar.gz`
and `.dmg` files using the host's `tar` and `hdiutil`. Minimum macOS is 15.0.
These development artifacts are not Developer ID signed or notarized. macOS
compilation, quarantine behavior, signing/notarization, and upgrade installation
remain acceptance gates before release use.

The native preview feed is `gpui-latest/gpui-latest.json`. It has not been
published. Keep the existing Tauri `latest.json` channel until upgrade/rollback
and both platform acceptance gates pass. The native verifier accepts the same
public key and nested-base64 minisign signatures as the existing Tauri updater.

The Windows update UI re-verifies a downloaded installer while holding it against
write/delete sharing, starts it through the native elevation API, and quits only
after successful launch. NSIS waits up to 30 seconds for the original process to
exit. The finish page can restart with the same data directory and development
shortcut/index/elevation flags. The custom resource-directory override is not
carried into a newly installed package; packaged resource discovery applies.
Actual UAC cancellation, installation, restart, and rollback remain untested.

## Offline profile copies

Close the source application before copying. Import into a new directory, keeping
the original and a separate backup; the command never replaces an existing
profile or backup. Record the source version, or `unknown` if it is unavailable.

```powershell
cargo run -p xtask -- import-profile C:/Users/me/.rotor target/profile-copy target/profile-backup 2.6.0
cargo run -p xtask -- verify-profile target/profile-backup
cargo run -p rotor-desktop -- --data-dir target/profile-copy
```

Only `config.toml` and the complete `shotter` directory are copied, including
other workspaces and unknown fields. Search indexes rebuild. The importer checks
hashes before and after copying, rejects symlinks/nested destinations, retains
backups on validation failure, and publishes the new directory after verification.
Checksums detect changes; they are not a substitute for closing the source app.
`migration-receipt.json` records version/time and file hashes without config
values. Verify immutable backups or a freshly imported copy; ordinary app writes
will legitimately change the active copy's hashes. Rollback imports the selected
backup into another new directory, keeping current data and both backups.

```powershell
cargo run -p rotor-screenshot --example profile_roundtrip -- target/profile-roundtrip
```

This retained synthetic fixture exercises native writes and the legacy Rust
record reader without screenshots or windows. It is not the installed Tauri
client's full upgrade/rollback acceptance test.

The macOS update path prepares a verified app in a private sibling staging
directory before the current app quits. A helper waits for process exit, retains
the previous app, and requires a startup acknowledgement from the replacement.
Startup failure restores and reopens the previous app; failed bundles and update
receipts are retained. It requires write access to the app's parent directory.
This code has portable fixture coverage and Windows type checks; macOS process
control, bundle launch, signing, and real rollback still require Mac validation.
