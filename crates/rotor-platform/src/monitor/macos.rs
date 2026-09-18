use super::MonitorConfig;
use core_graphics::display::{CGDisplay, CGGetActiveDisplayList};

pub(super) fn current_configs() -> Result<Vec<MonitorConfig>, String> {
    // Use the returned count on the second call: a display can disappear
    // between the count query and enumeration. Never interpret spare slots as IDs.
    let mut count = 0;
    let error = unsafe { CGGetActiveDisplayList(0, std::ptr::null_mut(), &mut count) };
    if error != 0 {
        return Err(format!("Could not count active displays: {error}"));
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut ids = vec![0; count as usize];
    let error = unsafe { CGGetActiveDisplayList(count, ids.as_mut_ptr(), &mut count) };
    if error != 0 {
        return Err(format!("Could not enumerate active displays: {error}"));
    }
    ids.truncate(count as usize);
    ids.into_iter().map(config).collect()
}

pub(super) fn config_for_capture(expected: &MonitorConfig) -> Result<MonitorConfig, String> {
    config(expected.id)
}

fn config(id: u32) -> Result<MonitorConfig, String> {
    let display = CGDisplay::new(id);
    if id == 0 || !display.is_active() {
        return Err("Captured display is no longer active".into());
    }
    let bounds = display.bounds();
    let mode = display
        .display_mode()
        .ok_or("Display mode is unavailable")?;
    config_from_bounds(
        id,
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
        mode.pixel_width(),
        mode.pixel_height(),
    )
}

fn config_from_bounds(
    id: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    pixel_width: u64,
    pixel_height: u64,
) -> Result<MonitorConfig, String> {
    if [x, y]
        .iter()
        .any(|v| !v.is_finite() || *v < i32::MIN as f64 || *v > i32::MAX as f64)
        || [width, height]
            .iter()
            .any(|v| !v.is_finite() || *v < 1. || *v > u32::MAX as f64)
    {
        return Err("Invalid display bounds".into());
    }
    let (width, height) = (width as u32, height as u32);
    // Keep Quartz desktop points separate from native capture pixels. Using
    // the mode's pixel width preserves Retina and scaled-display semantics.
    let scale_factor = pixel_width as f32 / width as f32;
    let (width, height) = pixel_dimensions(pixel_width, pixel_height, scale_factor)?;
    Ok(MonitorConfig {
        id,
        x: x as i32,
        y: y as i32,
        width,
        height,
        scale_factor,
    })
}

// The mode reports both pixel dimensions; use them directly instead of
// re-deriving the height from the width scale, which can round differently.
fn pixel_dimensions(width: u64, height: u64, scale: f32) -> Result<(u32, u32), String> {
    if !scale.is_finite() || scale <= 0. {
        return Err("Invalid display scale".into());
    }
    let convert = |pixels: u64| {
        u32::try_from(pixels)
            .ok()
            .filter(|pixels| *pixels >= 1)
            .ok_or_else(|| "Invalid capture dimensions".to_string())
    };
    Ok((convert(width)?, convert(height)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retina_and_external_displays_preserve_ids_and_point_origins() {
        let retina = config_from_bounds(7, -1512., -20., 1512., 982., 3024, 1964).unwrap();
        assert_eq!(
            retina,
            MonitorConfig {
                id: 7,
                x: -1512,
                y: -20,
                width: 3024,
                height: 1964,
                scale_factor: 2.,
            }
        );
        let external = config_from_bounds(9, 0., 0., 1920., 1080., 1920, 1080).unwrap();
        assert_eq!(
            (external.width, external.height, external.scale_factor),
            (1920, 1080, 1.)
        );
        // Scaled modes report their own pixel height; do not re-derive it.
        let scaled = config_from_bounds(3, 0., 0., 1680., 1050., 2880, 1801).unwrap();
        assert_eq!((scaled.width, scaled.height), (2880, 1801));
    }

    #[test]
    fn invalid_geometry_and_scales_are_rejected() {
        for scale in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(pixel_dimensions(3024, 1964, scale).is_err());
        }
        assert!(pixel_dimensions(0, 1964, 2.).is_err());
        assert!(pixel_dimensions(3024, 0, 2.).is_err());
        assert!(pixel_dimensions(u64::from(u32::MAX) + 1, 1964, 2.).is_err());
        assert!(config_from_bounds(1, f64::NAN, 0., 1., 1., 1, 1).is_err());
        assert!(config_from_bounds(1, 0., 0., f64::INFINITY, 1., 1, 1).is_err());
        assert!(config_from_bounds(1, 0., 0., 1., 1., 0, 1).is_err());
        assert!(config_from_bounds(1, 0., 0., 1., 1., 1, 0).is_err());
    }
}
