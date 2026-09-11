# Native GPUI platform adapter

Derived from gpui-pre-platform 0.3.3, snapshot of Zed commit
5b055fa789a8b8d38ac951a6e0cde272f66b4495. Apache-2.0; see LICENSE-APACHE.

Rotor supports Windows and macOS. This local patch keeps those platform
constructors and tests unchanged and excludes Linux/FreeBSD and web backends.
The x11/wayland feature names remain empty because gpui-kit requests them
unconditionally. No supported-platform notification backend is replaced.

The upstream chain is gpui-kit -> gpui-pre-platform -> gpui-pre-linux ->
notify-rust -> tauri-winrt-notification. The last edge is a Windows dependency
of a Linux-only dependency: it is absent from the Windows build but remains
in the cross-platform lockfile. Upstream has no feature to omit gpui_linux.
This patch removes the unneeded backend dependency before Cargo resolves it.
Re-evaluate when upgrading GPUI; do not edit registry cache or Cargo.lock by hand.
