# Screenshot pipeline optimization

Selection cropping and display preparation run off the UI thread. The shell
checks the capture generation before accepting the result, and closing or
replacing the capture cancels the row processing. Repeated confirmation cannot
enqueue duplicate pins. Multi-monitor rectangle detection prioritizes the
display containing the cursor.

A prepared pin stores one BGRA pixel allocation, shared with its render image.
An exclusively owned RGBA input is converted in place; a shared input is copied
without modifying its owner. Export and OCR read a checked, borrowed pixel view
and convert only the requested crop during canvas rendering. Straight alpha and
hidden RGB values are preserved. Saving a new pin temporarily retains its RGBA
input until the persistence operation and its completion event are released.

Pins can appear before persistence finishes. Their pending operation identity
handles both early completion (before the window opens) and normal completion.
Export requests wait for that identity to resolve, and disk failure leaves the
displayed image available for Save or Copy. Placement adjustments are persisted
after creation. PNG encoding streams to the newly created file; flush and sync
still precede record publication, and failure removes only the new image.

Capture jobs carry a deadline and cancellation flag. The coordinator checks
cancellation while waiting, and queued expired jobs skip native capture. Image
detection checks cancellation between stages and within connected-component
traversal. OCR has a separate bounded queue with one running inference; cancelled
queued requests release their images and capacity without waiting for active
inference. Cancellation does not forcibly interrupt an OS capture call or an
already-running native inference. Existing generation/revision checks still
reject stale results, and the OCR idle reaper still releases loaded models.

## Reproducible measurements and checks

```powershell
cargo run -p rotor-desktop --example capture_pipeline_bench --release --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p rotor-desktop --example ocr_smoke --release --locked -- --wait-idle
cargo run -p rotor-screenshot --example profile_roundtrip --locked -- target/capture-profile-roundtrip-NEW
```

The roundtrip destination must not already exist. All images and profiles used
by these checks are synthetic. The OCR smoke test also checks the BGRA-to-canvas
path against the original pixels before recognizing generated Chinese/Latin text.

The benchmark alternates the previous two-buffer preparation and the current
exclusively owned preparation, discards two warmups per path, and reports ten
samples for each resolution. Fixture generation is outside the measured interval.
It verifies that the current implementation reuses the original pixel allocation.
Reported memory is retained CPU pixel bytes, not process working set or GPU usage.
The ownership speedup applies to paths such as restored images; fresh captures
still need separate persistence and display buffers while saving is in progress.

On the Windows development machine on 2026-09-16, one release run of this
synthetic benchmark produced:

| Resolution | Previous P50 / P95 | Owned single-buffer P50 / P95 | Retained pixel bytes before / after |
| --- | ---: | ---: | ---: |
| 1920 x 1080 | 3.151 / 3.285 ms | 0.796 / 0.975 ms | 15.82 / 7.91 MiB |
| 3840 x 2160 | 13.207 / 13.264 ms | 4.084 / 4.565 ms | 63.28 / 31.64 MiB |
| 7680 x 4320 | 47.697 / 50.525 ms | 12.857 / 15.926 ms | 253.12 / 126.56 MiB |

The baseline is the previous clone-and-swap preparation algorithm included in
the same executable, not a comparison of two complete application builds.
Timings vary with hardware and load; the one-buffer byte reduction is structural.

These checks do not establish shortcut-to-visible-frame latency, display-driver
behavior, physical multi-monitor/DPI interactions, macOS runtime acceptance, or
installation acceptance. The existing `rotor_capture_latency` logs now include
`selection_submitted` and `selection_prepared` to help measure UI selection work
separately from native capture and painting.
