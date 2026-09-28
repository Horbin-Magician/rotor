# Installer artwork

The checked-in assets share Rotor's existing blue mark, pale blue surfaces and
slate text. They are inputs to `xtask package`; normal packaging does not require
Python packages, a logged-in Finder session or Automation permissions.

- `windows-sidebar.bmp`: 328 × 628, twice the MUI welcome/finish dimensions.
- `windows-header.bmp`: 300 × 114, twice the MUI header dimensions. MUI scales
  both bitmaps to its native controls; do not enable `NOSTRETCH`.
- `macos-background.tiff`: 720 × 300 points, with 1× and 2× representations.
- `macos-background.png`: the 1× artwork preview.
- `macos-*.dsstore`: Finder icon view, hidden toolbar/sidebar, fixed window size,
  80-point icons and locations for each identity from `native/app.toml`.
  These are copied as `.DS_Store` into the DMG root. Background aliases use the
  volume name and `.background/background.tiff`, without author-machine paths
  or file IDs. The `/Volumes/<product>` path is a resolution hint.

The DMG uses HFS+ so Finder can read the layout consistently. Artwork and layout
files live outside the app bundle; the updater's `.app.tar.gz` is unchanged.
Windows page text stays in `native/windows.nsi` as native localized labels.

## Regenerate

On macOS, from the repository root:

```sh
python3 -m venv /tmp/rotor-installer-art
/tmp/rotor-installer-art/bin/pip install -r native/installer/requirements.txt
/tmp/rotor-installer-art/bin/python native/installer/generate-assets.py
```

The authoring script uses the existing `assets/icons/icon.png`, installed Arial
and Arial Unicode fonts, Pillow, `ds_store`, `mac_alias`, and Apple's `tiffutil`.
Fonts are rasterized into artwork, not redistributed. Change the script to edit
artwork or icon positions, then regenerate all assets. Regenerate layouts when
product names change. Commit the generator and generated assets together.

## Visual acceptance

Build through xtask, then stage and package into new directories as described in
[the native packaging guide](../README.md). Rebuild after changing these inputs.

- macOS: open both development and production DMGs in Finder. Confirm the
  background, full bilingual instructions, app labels and Applications link;
  check Retina rendering and a non-default mount path. Dragging should use the
  real application icon. Eject test images after checking.
- Windows: with NSIS 3.11, inspect English and Simplified Chinese welcome,
  directory, progress, finish and uninstall pages at 100%, 150% and 200% display
  scaling. Check the longer development name, keyboard navigation and run box.
  Use an isolated development installation for installation/recovery tests.

Compiling NSIS on macOS verifies script/resource integration; it does not prove
Windows rendering, UAC behavior or installation/recovery acceptance.
