//! Read-only topology evidence with the same GPUI initialization as the shell.
//! No windows, profile, clipboard or screenshot capture are created.
use std::{cell::Cell, process::ExitCode, rc::Rc};

fn main() -> ExitCode {
    let failed = Rc::new(Cell::new(false));
    let result = failed.clone();
    gpui_kit::application().run(move |cx| {
        match rotor_runtime::current_monitor_configs() {
            Ok(monitors) => {
                let displays = cx.displays();
                println!(
                    "id\tx\ty\twidth_px\theight_px\tscale\tgpui_x\tgpui_y\tgpui_width\tgpui_height"
                );
                for monitor in monitors {
                    let display = displays
                        .iter()
                        .find(|display| u64::from(display.id()) as u32 == monitor.id);
                    if let Some(display) = display {
                        let bounds = display.bounds();
                        println!(
                            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                            monitor.id,
                            monitor.x,
                            monitor.y,
                            monitor.width,
                            monitor.height,
                            monitor.scale_factor,
                            bounds.origin.x.as_f32(),
                            bounds.origin.y.as_f32(),
                            bounds.size.width.as_f32(),
                            bounds.size.height.as_f32()
                        );
                    } else {
                        eprintln!(
                            "Captured monitor {} has no matching GPUI display",
                            monitor.id
                        );
                        failed.set(true);
                    }
                }
            }
            Err(error) => {
                eprintln!("Cannot read monitor topology: {error}");
                failed.set(true);
            }
        }
        cx.quit();
    });
    if result.get() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
