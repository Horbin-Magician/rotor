# Release, mirror and channel operations

Complete the platform-specific gates in `doc/validation-status.md` before merging,
tagging or publishing. Keep the draft unpublished while selected platforms lack
acceptance evidence. 3.0.0 defaults to Windows x64 and macOS arm64. A deliberate
single-platform release must select it consistently through build and validation.

1. Review the PR against master, run required CI, merge the reviewed commit, then
   create and push `v3.0.0` on that exact commit. The tag must match Cargo.toml.
2. Wait for `native-publish-draft`. Review version notes, inventories, receipts,
   signatures and `native-update.json`. Verify its platform set and fixed version
   URLs. Test downloaded installers on the intended platforms and record results.
3. Publish the reviewed draft. `sync-to-gitee` checks the exact asset set and
   rewrites only native JSON artifact URLs to the Gitee version release. Binaries
   and signatures remain unchanged. Verify mirror downloads before promotion.
   The mirror job rejects an existing destination release; reconcile partial
   uploads before retrying rather than silently overwriting them.
4. Retain the existing channel manifest outside the source tree for withdrawal.
   Compare the new manifest against the reviewed version release.
5. Production stable uses release `native-stable`, attachment `native-stable.json`.
   Development preview uses `native-preview`, attachment `native-preview.json`,
   referencing development artifacts published at `native-dev-v<version>`.
   Stable must never advertise a prerelease. Create channel releases once as
   marked prereleases so maintenance does not become the default latest release.
6. Rename the reviewed version's `native-update.json` to the channel filename.
   For GitHub, use `gh release upload native-stable native-stable.json --clobber`
   (or preview). Fixed artifact URLs remain unchanged. For Gitee, use its verified
   rewritten manifest and replace the same named attachment in its channel
   release. The version mirror workflow does not update existing channel releases.
   Verify both channel URLs and signed artifacts before announcing updates.
7. To withdraw, restore the previous verified manifest on both channels, or remove
   the channel attachment when no accepted predecessor exists. Retain version
   artifacts and install backups for diagnosis. Clients reject downgrades;
   withdrawal prevents new offers. Publish a higher fixed version for clients
   that already installed a problematic release.

Channel addresses in `native/app.toml` are tested against updater constants.
No operation writes the historical generic feed. Signing setup is documented in
`native/signing.md`; synthetic tests do not prove hosted key access or pairing.
