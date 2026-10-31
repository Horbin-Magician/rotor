# Native update signing

The sole public key source is `native/update-public.key`, in plain minisign
public-key text. Its key material is unchanged; only the outer base64 wrapper
has been removed. Signatures are plain minisign text using the prehashed mode.
No outer base64 wrapper or legacy non-prehashed signature is accepted.
Native manifests require `schema_version: 1`.

Repository administrators must configure these GitHub Actions secrets before
running a signed candidate or release:

- `ROTOR_SIGNING_PRIVATE_KEY`: the full encrypted minisign secret-key text.
- `ROTOR_SIGNING_PRIVATE_KEY_PASSWORD`: its password.

Convert the previously wrapped secret in a private environment and save the
result directly into the new secret. Remove the obsolete framework-named
secrets from repository/environment settings. Workflows have no fallback.
Do not print, commit or attach private key material. For local signing the key
variable may name a private key file instead. `xtask sign` verifies every
signature against the checked-in public key before writing it, so a wrong key
pair fails. This change does not configure hosted secrets or prove that a
release signing environment has the matching private key.

```powershell
cargo run -p xtask -- sign Rotor_3.0.0_x64-setup.exe
```

## Apple Developer ID distribution

Apple code signing and notarization are separate from the updater's minisign
signatures above. macOS production packaging requires both
`APPLE_SIGNING_IDENTITY` and `APPLE_NOTARY_PROFILE`; it fails if either is absent.
Development packages remain unsigned unless both variables are configured.

Create a **Developer ID Application** certificate with its private key on the
signing Mac, using Xcode Settings > Accounts > Manage Certificates, or the
[Apple Developer website](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/).
An Apple Development or Apple Distribution certificate is not a substitute.
If importing an existing identity, import the password-protected `.p12` containing
both certificate and private key into Keychain Access. Never store it in this repo.
Check the installed identity name with `security find-identity -v -p codesigning`.

Store notarization credentials interactively, outside chat and shell history:

```sh
xcrun notarytool store-credentials rotor-release
```

Use the enrolled team's Team ID and an Apple Account app-specific password when
prompted. Use the Team ID from the developer account, not an inferred identifier
from a development certificate. An App Store Connect API key is also supported by
notarytool. See [Apple's notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow).

Build and package on Apple Silicon with new destination directories:

```sh
export APPLE_SIGNING_IDENTITY='Developer ID Application: YOUR NAME (TEAM_ID)'
export APPLE_NOTARY_PROFILE=rotor-release
cargo run -p xtask --locked -- build --production
cargo run -p xtask --locked -- stage --production target/apple-release-stage
cargo run -p xtask --locked -- package target/apple-release-stage target/apple-release-package
```

Packaging verifies the original stage and signs a temporary copy, preserving the
stage's byte-integrity receipts. It signs bundled dylibs and the application with
Hardened Runtime and a secure timestamp, verifies the bundle, submits a ZIP to
Apple, requires `Accepted`, staples and validates the ticket, and checks Gatekeeper.
Both the updater archive and DMG contain that stapled application. The DMG is then
signed, notarized, stapled and assessed as well. Only after success is the final
package inventory generated. No relaxed Hardened Runtime entitlements are enabled.
An unsuccessful command leaves incomplete output for diagnosis; do not distribute
it or sign it with the updater key. Retry using a new output directory.

Apple submission may take time, particularly for a team's first submission. If
rejected, retrieve the submission log using the ID in the error:

```sh
xcrun notarytool log SUBMISSION_ID --keychain-profile rotor-release
```

After packaging succeeds, apply the existing `xtask sign` minisign process to the
final DMG and `.app.tar.gz`, then regenerate their inventory. Do not modify or
restaple packages after signing them for the updater.

### GitHub Actions

Production macOS jobs in `native-candidate.yml` (also called by `publish.yml`)
require these repository/environment secrets:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE_P12_BASE64` | Base64 of an exported Developer ID Application `.p12`, including its private key |
| `APPLE_CERTIFICATE_PASSWORD` | Password protecting that `.p12` |
| `APPLE_SIGNING_IDENTITY` | Full `Developer ID Application: ... (TEAM_ID)` identity name |
| `APPLE_ID` | Enrolled Apple Account email |
| `APPLE_TEAM_ID` | Developer Program Team ID |
| `APPLE_APP_SPECIFIC_PASSWORD` | App-specific password for notarization |

The runner imports the certificate into a temporary keychain, validates/stores
notarization credentials there, and deletes the keychain in an always-run cleanup
step. These secrets do not replace `ROTOR_SIGNING_*`. Configure secrets using the
GitHub settings UI or private local input; never paste them into chat or commit
certificate/key files. Local credential configuration does not configure GitHub.

Automated tests cover configuration rejection. They do not establish successful
Apple notarization, Gatekeeper acceptance, or signed-app behavior. Before release,
exercise screenshots, OCR, global shortcuts, startup, and native updates on a Mac
with the signed build, including a quarantined download on a separate Mac.
