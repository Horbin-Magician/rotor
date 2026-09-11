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
