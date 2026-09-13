# Release and update operations

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
   HEAD and existing tags are rejected. A failed push keeps
   the local release commit and tag and prints the exact push command to retry;
   do not rerun the bump or force-push over an existing release.
   The version tag must match Cargo.toml. For an already prepared version such
   as the initial 3.0.0, use the same script with `3.0.0`: it checks Cargo.lock
   with `--locked`, skips version editing and the extra commit, then tags and
   pushes the current commit. An inconsistent lockfile must be fixed and
   committed first.
2. Wait for `native-publish-draft`. Review version notes, inventories, receipts,
   signatures and `native-update.json`. Verify its platform set and fixed version
   URLs. Test downloaded installers on the intended platforms and record results.
3. Publish the reviewed GitHub draft. Normal releases require no manual workflow
   dispatch. `native-publish-draft` still accepts an existing tag for recovery or
   deliberate single-platform builds. Configure the signing secrets described in
   `native/signing.md` before releasing. Publishing the version completes the
   release flow. Mark the reviewed production release as GitHub's latest release
   to make it available to the updater.
4. Both application identities read the single update endpoint:
   `https://github.com/Horbin-Magician/rotor/releases/latest/download/native-update.json`.
   The published latest release must include the reviewed `native-update.json`
   and its signed production artifacts. No separate channel release, renamed
   manifest or promotion upload is required. Verify the endpoint after publishing.
5. To withdraw an update, select the previous accepted release as GitHub's latest
   release. Retain version artifacts and install backups for diagnosis. Clients
   reject downgrades; withdrawal prevents new offers. Publish a higher fixed
   version for clients that already installed a problematic release.

The endpoint in `native/app.toml` is tested against the updater constant.
Development and production retain separate application identities and profiles;
sharing update metadata does not bypass installation identity checks.
The native manifest remains separate from the historical `latest.json` format.
Signing setup is documented in `native/signing.md`; synthetic tests do not prove
hosted key access or pairing.
