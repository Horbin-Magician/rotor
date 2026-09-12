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
