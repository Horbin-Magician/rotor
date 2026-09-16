//! Synthetic CPU pixel preparation benchmark; no windows, desktop pixels or profiles.
//! Run with --release. This does not measure GPU upload or shortcut-to-window latency.
use gpui_kit::RenderImage;
use image::{Frame, Rgba, RgbaImage};
use std::{
    hint::black_box,
    sync::Arc,
    time::{Duration, Instant},
};

fn fixture(width: u32, height: u32) -> Arc<RgbaImage> {
    Arc::new(RgbaImage::from_fn(width, height, |x, y| {
        Rgba([(x % 251) as u8, (y % 239) as u8, ((x ^ y) % 233) as u8, 255])
    }))
}

fn summarize(label: &str, mut samples: Vec<Duration>, bytes: usize) {
    samples.sort_unstable();
    let percentile =
        |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1].as_secs_f64() * 1000.;
    println!(
        "{label}: p50={:.3}ms p95={:.3}ms retained_cpu_pixels={:.2}MiB",
        percentile(50),
        percentile(95),
        bytes as f64 / (1024. * 1024.)
    );
}

fn main() -> Result<(), String> {
    for (width, height) in [(1920, 1080), (3840, 2160), (7680, 4320)] {
        let mut old_samples = Vec::new();
        let mut new_samples = Vec::new();
        // Alternate order, exclude fixture allocation and discard warmup samples.
        for iteration in 0..12 {
            for baseline in if iteration % 2 == 0 {
                [true, false]
            } else {
                [false, true]
            } {
                let source = fixture(width, height);
                let allocation = source.as_raw().as_ptr();
                let start = Instant::now();
                if baseline {
                    // Previous PreparedImage retained both of these allocations.
                    let mut bgra = source.as_ref().clone();
                    for pixel in bgra.pixels_mut() {
                        pixel.0.swap(0, 2);
                    }
                    let render = Arc::new(RenderImage::new(vec![Frame::new(bgra)]));
                    let elapsed = start.elapsed();
                    black_box((&source, &render));
                    if iteration >= 2 {
                        old_samples.push(elapsed);
                    }
                } else {
                    let prepared = rotor_ui::prepare_image(source)?;
                    let elapsed = start.elapsed();
                    assert_eq!(prepared.render.as_bytes(0).unwrap().as_ptr(), allocation);
                    black_box(&prepared);
                    if iteration >= 2 {
                        new_samples.push(elapsed);
                    }
                }
            }
        }
        println!("{width}x{height}, 10 samples per path");
        let bytes = width as usize * height as usize * 4;
        summarize("previous two-buffer preparation", old_samples, bytes * 2);
        summarize("owned single-buffer preparation", new_samples, bytes);
    }
    Ok(())
}
