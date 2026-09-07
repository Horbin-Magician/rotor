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
