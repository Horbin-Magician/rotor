use rotor_ui::PinBounds;

/// Evaluate the unsnapped drag position every time, so pulling away releases
/// the edge without accumulating corrections in the pointer anchor.
pub(super) fn snap(bounds: PinBounds, targets: &[PinBounds], threshold: i64) -> PinBounds {
    let (x, y) = (i64::from(bounds.x), i64::from(bounds.y));
    let (w, h) = (i64::from(bounds.width), i64::from(bounds.height));
    let mut best = None;
    for target in targets {
        let (tx, ty) = (i64::from(target.x), i64::from(target.y));
        let (tw, th) = (i64::from(target.width), i64::from(target.height));
        // Opposing edges attach windows. Parallel edges align their corners
        // only when a pair is already close enough to attach.
        for (horizontal, edge) in [
            (true, tx - w),
            (true, tx + tw),
            (false, ty - h),
            (false, ty + th),
        ] {
            let (origin, cross, length, start, end) = if horizontal {
                (x, y, h, ty, ty + th)
            } else {
                (y, x, w, tx, tx + tw)
            };
            if (edge - origin).abs() > threshold {
                continue;
            }
            let aligned = [start, end - length]
                .into_iter()
                .filter(|value| (*value - cross).abs() <= threshold)
                .min_by_key(|value| ((*value - cross).abs(), *value))
                .unwrap_or(cross);
            if aligned + length < start || aligned > end {
                continue;
            }
            let (nx, ny) = if horizontal {
                (edge, aligned)
            } else {
                (aligned, edge)
            };
            let (Ok(nx), Ok(ny)) = (i32::try_from(nx), i32::try_from(ny)) else {
                continue;
            };
            let score = (edge - origin).abs();
            // Stable ties keep HashMap enumeration order from causing jitter.
            let candidate = (score, (aligned - cross).abs(), nx, ny);
            if best.is_none_or(|previous| candidate < previous) {
                best = Some(candidate);
            }
        }
    }
    if let Some((_, _, x, y)) = best {
        PinBounds { x, y, ..bounds }
    } else {
        bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32) -> PinBounds {
        PinBounds {
            x,
            y,
            width: 100,
            height: 80,
        }
    }
    fn position(bounds: PinBounds) -> (i32, i32) {
        (bounds.x, bounds.y)
    }

    #[test]
    fn attaches_all_four_edges_and_aligns_nearby_corners() {
        let target = rect(200, 200);
        for (input, expected) in [
            ((108, 203), (100, 200)),
            ((293, 197), (300, 200)),
            ((204, 128), (200, 120)),
            ((196, 272), (200, 280)),
            ((108, 230), (100, 230)),
        ] {
            assert_eq!(
                position(snap(rect(input.0, input.1), &[target], 10)),
                expected
            );
        }
    }

    #[test]
    fn releases_beyond_threshold_and_ignores_distant_edges() {
        for input in [rect(111, 200), rect(108, 400), rect(200, 200)] {
            assert_eq!(
                position(snap(input, &[rect(200, 200)], 10)),
                position(input)
            );
        }
        assert_eq!(position(snap(rect(108, 200), &[], 10)), (108, 200));
    }

    #[test]
    fn handles_negative_coordinates_scaled_thresholds_and_stable_ties() {
        assert_eq!(
            position(snap(rect(-115, -200), &[rect(0, -200)], 20)),
            (-100, -200)
        );
        let a = rect(200, 200);
        let b = rect(-10, 200);
        assert_eq!(
            position(snap(rect(95, 200), &[a, b], 10)),
            position(snap(rect(95, 200), &[b, a], 10))
        );
        // Extended arithmetic must not wrap at desktop coordinate limits.
        assert_eq!(
            position(snap(rect(i32::MAX, 0), &[rect(i32::MIN, 0)], 10)),
            (i32::MAX, 0)
        );
    }
}
