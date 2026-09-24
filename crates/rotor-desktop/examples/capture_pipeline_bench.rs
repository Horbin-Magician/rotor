//! Synthetic CPU pixel preparation benchmark; no windows, desktop pixels or profiles.
//! Run with --release. This does not measure GPU upload or shortcut-to-window latency.
use gpui_kit::RenderImage;
use image::{Frame, Rgba, RgbaImage};
use std::{
    hint::black_box,
    io::Write,
    sync::Arc,
    time::{Duration, Instant},
};

fn fixture(width: u32, height: u32) -> Arc<RgbaImage> {
    Arc::new(RgbaImage::from_fn(width, height, |x, y| {
        Rgba([(x % 251) as u8, (y % 239) as u8, ((x ^ y) % 233) as u8, 255])
    }))
}

fn summarize(label: &str, samples: &[Duration], bytes: usize) -> serde_json::Value {
    let raw: Vec<_> = samples
        .iter()
        .map(|sample| sample.as_secs_f64() * 1000.)
        .collect();
    let mut samples = samples.to_vec();
    samples.sort_unstable();
    let percentile =
        |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1].as_secs_f64() * 1000.;
    println!(
        "{label}: p50={:.3}ms p95={:.3}ms retained_cpu_pixels={:.2}MiB",
        percentile(50),
        percentile(95),
        bytes as f64 / (1024. * 1024.)
    );
    serde_json::json!({
        "path": label, "samples_ms": raw, "p50_ms": percentile(50),
        "p95_ms": percentile(95), "retained_cpu_pixel_bytes": bytes,
        "failures": 0,
    })
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut output = match args.as_slice() {
        [] => None,
        [flag, path] if flag == "--output" => Some(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|error| error.to_string())?,
        ),
        _ => return Err("Usage: capture_pipeline_bench [--output NEW_JSON_PATH]".into()),
    };
    let mut cases = Vec::new();
    for (width, height) in [(1920, 1080), (3840, 2160), (7680, 2160)] {
        let mut old_samples = Vec::new();
        let mut new_samples = Vec::new();
        // Alternate order, exclude fixture allocation and discard warmup samples.
        for iteration in 0..32 {
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
        println!("{width}x{height}, 30 samples per path");
        let bytes = width as usize * height as usize * 4;
        cases.push(serde_json::json!({
            "width": width, "height": height,
            "paths": [summarize("previous two-buffer preparation", &old_samples, bytes * 2),
                summarize("owned single-buffer preparation", &new_samples, bytes)],
        }));
    }
    if let Some(output) = &mut output {
        let report = serde_json::json!({
            "schema_version": 1, "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "build": rotor_common::native_app::build_info(), "debug_assertions": cfg!(debug_assertions),
            "scope": "synthetic CPU pixel preparation; excludes fixture allocation",
            "limitations": "Not native capture, save, OCR, GPU upload, process peak memory or visible latency. Dual 4K uses one equal-area buffer, not two monitor windows.",
            "cases": cases,
        });
        writeln!(
            output,
            "{}",
            serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}
