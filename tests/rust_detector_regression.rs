use shakespot::core::Detector;

#[path = "support/detector_v040.rs"]
mod reference;

#[test]
fn optimized_detector_matches_reference_at_every_sample() {
    let mut total_triggers = 0;
    for sensitivity in 1..=5 {
        for step in [1.0, 4.0, 8.0, 16.0, 25.0] {
            for degrees in (0..360).step_by(30) {
                let (sin, cos) = f64::from(degrees).to_radians().sin_cos();
                for amplitude in [24.0, 32.0, 85.0, 220.0] {
                    let mut before = reference::Detector::default();
                    let mut after = Detector::default();
                    before.set_sensitivity(sensitivity);
                    after.set_sensitivity(sensitivity);
                    for i in 0..600 {
                        let time = f64::from(i) * step;
                        let u = amplitude * (time / 48.0).sin();
                        let v = 2.0 * (time / 11.0).sin();
                        let (x, y) = (1200.0 + u * cos - v * sin, -800.0 + u * sin + v * cos);
                        let expected = before.add(time, x, y, false);
                        let actual = after.add(time, x, y, false);
                        assert_eq!(
                            actual, expected,
                            "sensitivity={sensitivity} step={step} angle={degrees} amplitude={amplitude} i={i}"
                        );
                        total_triggers += usize::from(actual);
                    }
                }
            }
        }
    }
    assert!(total_triggers > 1000);
}

#[test]
fn rolling_window_and_reset_boundaries_match_reference() {
    let mut before = reference::Detector::default();
    let mut after = Detector::default();
    let mut seed = 0x93a0_c97bu32;
    let (mut time, mut x, mut y) = (0.0, 0.0, 0.0);
    for i in 0..200_000 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        time += [0.125, 1.0, 8.0, 25.0, 129.0][i % 5];
        x += f64::from((seed & 255) as i32 - 128) * 0.4;
        y += f64::from(((seed >> 8) & 255) as i32 - 128) * 0.1;
        match i % 997 {
            100 => time += 131.0,
            200 => time -= 300.0,
            300 => x += 901.0,
            400 => {
                let sensitivity = (i % 5 + 1) as i32;
                before.set_sensitivity(sensitivity);
                after.set_sensitivity(sensitivity);
            }
            _ => {}
        }
        let down = i % 113 == 0;
        assert_eq!(
            after.add(time, x, y, down),
            before.add(time, x, y, down),
            "i={i}"
        );
    }
    // 长期累计后停止、恢复与非法输入都不能留下旧路程。
    for _ in 0..500 {
        time += 8.0;
        assert_eq!(after.add(time, x, y, false), before.add(time, x, y, false));
    }
    for invalid in [f64::NAN, f64::INFINITY] {
        assert_eq!(
            after.add(time, invalid, y, false),
            before.add(time, invalid, y, false)
        );
    }
    for i in 0..600 {
        let t = f64::from(i * 8);
        let wave = 85.0 * (t / 48.0).sin();
        assert_eq!(
            after.add(t, wave, 0.0, false),
            before.add(t, wave, 0.0, false)
        );
    }
}

#[test]
fn narrow_thresholds_and_long_untriggered_motion_match_reference() {
    for amplitude in [23.999999, 24.0, 24.000001, 26.0, 30.0, 32.0] {
        let mut before = reference::Detector::default();
        let mut after = Detector::default();
        for i in 0..40_000 {
            let t = f64::from(i * 8);
            let (x, y) = if i < 20_000 {
                (t * 0.18, t * 0.08)
            } else {
                (amplitude * (t / 40.0).sin(), 0.0)
            };
            assert_eq!(
                after.add(t, x, y, false),
                before.add(t, x, y, false),
                "amplitude={amplitude} i={i}"
            );
        }
    }
}
