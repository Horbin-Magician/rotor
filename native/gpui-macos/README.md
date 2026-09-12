# Native GPUI macOS backend

Derived from gpui-pre-macos 0.3.3, snapshot of Zed commit
5b055fa789a8b8d38ac951a6e0cde272f66b4495. Apache-2.0; see LICENSE-APACHE.
Source files and the normalized manifest are retained from the published crate;
publishing this local copy is disabled.

Rotor patches `MacWindow::drop` to release `accesskit_adapter` before closing
the native window. The adapter retains the window's content view, which owns
the GPUIView subview. That subview owns an Arc back to MacWindowState through
its Objective-C instance variable. Without explicitly releasing the adapter,
this cycle retains the Metal renderer, image textures and IOSurfaces after
the GPUI window and CPU images have been dropped.

Accessibility remains enabled. The adapter is released while the NSWindow
still owns its content view, allowing normal view and window destruction to
finish releasing the state. No rendering, input or platform APIs are changed.

Re-evaluate this patch when upgrading GPUI. Preserve the original license;
do not edit the Cargo registry cache. Validate repeated native window closure
with accessibility enabled when replacing or removing the patch.

## Foundation compatibility

The backend uses objc2 Foundation objects and geometry instead of deprecated
Cocoa Foundation bindings. `objc_bridge.rs` keeps raw handles at the remaining
objc 0.2 AppKit boundaries and wraps objc2 geometry transparently for callback
registration with the legacy runtime. Its encodings come from objc2; it does
not duplicate the Foundation layouts. Autoreleased strings, arrays and data
retain their existing ownership contracts. Deprecated API use is denied in
this local backend, including when Cargo checks it as a dependency.

From the repository root, run:

```sh
rustfmt --edition 2024 --check native/gpui-macos/src/gpui_macos.rs
cargo test -p gpui-pre-macos -p rotor-desktop --lib --features gpui-platform/test-support --locked
```

Select rotor-desktop as well so Cargo can enable the dependency's test-support
feature without making the vendored backend a workspace member. The native
tests verify geometry across both runtimes and Foundation ownership; the
pasteboard tests use unique pasteboards and require access to the macOS
pasteboard service. These tests do not replace visual window/input validation.
