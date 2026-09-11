# Native update signing

The sole public key source is `native/update-public.key`, in plain minisign
public-key text. A fresh native release key pair was generated on 2026-09-11;
the encrypted private key and password are retained outside the repository.
Signatures are plain minisign text using the prehashed mode.
No outer base64 wrapper or legacy non-prehashed signature is accepted.
Native manifests require `schema_version: 1`.

Repository administrators must configure these GitHub Actions secrets before
running a signed candidate or release:

- `ROTOR_SIGNING_PRIVATE_KEY`: the full encrypted minisign secret-key text.
- `ROTOR_SIGNING_PRIVATE_KEY_PASSWORD`: its password.

Use the private key matching the current checked-in public key, without an outer
base64 wrapper. The previous signing key no longer matches this public key.
Remove obsolete framework-named secrets once their workflows are retired.
Workflows have no fallback.
Do not print, commit or attach private key material. For local signing the key
variable may name a private key file instead. `xtask sign` verifies every
signature against the checked-in public key before writing it, so a wrong key
pair fails. This change does not configure hosted secrets or prove that a
release signing environment has the matching private key.

Local review installers produced before this key rotation contain the previous
public key. Rebuild through xtask before signing or publishing new packages.

```powershell
cargo run -p xtask -- sign Rotor_3.0.0_x64-setup.exe
```
