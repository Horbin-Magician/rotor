# Release, mirror and channel operations

Complete the platform-specific gates in `doc/validation-status.md` before merging,
tagging or publishing. Keep the draft unpublished while selected platforms lack
acceptance evidence. 3.0.0 defaults to Windows x64 and macOS arm64. A deliberate
single-platform release must select it consistently through build and validation.

1. Review and merge the changes, including `doc/releases/<version>.md`, and run
   required CI. From the clean release branch, run one command (example next version):

   ```sh
   python3 scripts/bump-version.py 3.0.1
   ```

   On Windows use `python`. Python 3, Git and the pinned Rust toolchain are
   required; Node/Yarn are not. The script uses `xtask set-version` to update
   Cargo.toml and Cargo.lock, commits `chore: release v3.0.1`, creates `v3.0.1`,
   and atomically pushes the current branch and only that tag to `origin`.
   `--remote <name>` selects another configured remote. `--dry-run` checks and
   previews without editing files or refs (Cargo may populate its build cache);
   `--no-push` creates only the local commit and tag, without contacting a remote.
   Commit release notes before invoking the script. Dirty worktrees, detached
   HEAD, unchanged versions and existing tags are rejected. A failed push keeps
   the local release commit and tag and prints the exact push command to retry;
   do not rerun the bump or force-push over an existing release.
   The version tag must match Cargo.toml. For an already prepared version such
   as the initial 3.0.0, create and push its matching tag directly.
2. Wait for `native-publish-draft`. Review version notes, inventories, receipts,
   signatures and `native-update.json`. Verify its platform set and fixed version
   URLs. Test downloaded installers on the intended platforms and record results.
3. Publish the reviewed draft. `sync-to-gitee` checks the exact asset set and
   rewrites only native JSON artifact URLs to the Gitee version release. Binaries
   and signatures remain unchanged. Verify mirror downloads before promotion.
   The mirror job rejects an existing destination release; reconcile partial
   uploads before retrying rather than silently overwriting them.
   Normal releases require no manual workflow dispatch. `native-publish-draft`
   still accepts an existing tag for recovery or deliberate single-platform
   builds; `sync-to-gitee` accepts an exact published tag for recovery. Configure
   the signing secrets described in `native/signing.md` and the Gitee secrets
   `GITEE_OWNER`, `GITEE_REPO`, `GITEE_REPO_URL`, `GITEE_ACCESS_TOKEN`, and
   `SSH_PRIVATE_KEY` before releasing. Publishing a version and syncing it to
   Gitee completes the release flow; update-channel promotion below is separate.
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
