use shakespot::{core::*, settings};

fn wave(t: f64) -> (f64, f64) {
    (85.0 * (t / 48.0).sin(), 0.0)
}
fn trace(
    path: impl Fn(f64) -> (f64, f64),
    count: usize,
    step: f64,
    sensitivity: i32,
    down: bool,
) -> Vec<f64> {
    let mut detector = Detector::default();
    detector.set_sensitivity(sensitivity);
    (0..count)
        .filter_map(|i| {
            let t = i as f64 * step;
            let (x, y) = path(t);
            detector.add(t, x, y, down).then_some(t)
        })
        .collect()
}
#[test]
fn horizontal_shake() {
    assert!(!trace(wave, 80, 16.0, 3, false).is_empty());
}
#[test]
fn vertical_shake() {
    assert!(!trace(|t| (0.0, wave(t).0), 80, 16.0, 3, false).is_empty());
}
#[test]
fn diagonal_shake() {
    assert!(!trace(|t| (wave(t).0, 60.0 * (t / 48.0).sin()), 80, 16.0, 3, false).is_empty());
}
#[test]
fn rotating_the_same_small_gesture_preserves_trigger_time() {
    let base = trace(|t| (32.0 * (t / 40.0).sin(), 0.0), 126, 8.0, 3, false);
    assert!(!base.is_empty());
    for degrees in (0..360).step_by(15) {
        let (sin, cos) = f64::from(degrees).to_radians().sin_cos();
        let rotated = trace(
            |t| {
                let value = 32.0 * (t / 40.0).sin();
                (1200.0 + value * cos, -800.0 + value * sin)
            },
            126,
            8.0,
            3,
            false,
        );
        assert!(!rotated.is_empty(), "angle={degrees}");
        assert!((base[0] - rotated[0]).abs() <= 8.0, "angle={degrees}");
    }
}
#[test]
fn oscillation_during_a_large_sweep_does_not_trigger() {
    for degrees in [0, 30, 90, 135] {
        let (sin, cos) = f64::from(degrees).to_radians().sin_cos();
        assert!(
            trace(
                |t| {
                    let x = 80.0 * (t / 45.0).sin();
                    let y = 4.0 * t;
                    (x * cos - y * sin, x * sin + y * cos)
                },
                126,
                8.0,
                3,
                false,
            )
            .is_empty(),
            "angle={degrees}"
        );
    }
}
#[test]
fn deliberate_shake_tolerates_small_drift_and_noise() {
    assert!(
        !trace(
            |t| (
                wave(t).0 + 2.0 * (t / 7.0).sin(),
                0.08 * t + 3.0 * (t / 11.0).sin()
            ),
            126,
            8.0,
            3,
            false,
        )
        .is_empty()
    );
}
#[test]
fn ordinary_movement() {
    assert!(trace(|t| (t * 0.18, 35.0 * (t / 400.0).sin()), 80, 16.0, 3, false).is_empty());
}
#[test]
fn one_way_movement() {
    assert!(trace(|t| (t * 5.0, t * 2.0), 80, 16.0, 3, false).is_empty());
}
#[test]
fn tiny_jitter() {
    assert!(
        trace(
            |t| (4.0 * (t / 12.0).sin(), 5.0 * (t / 9.0).cos()),
            80,
            16.0,
            3,
            false
        )
        .is_empty()
    );
}
#[test]
fn slow_movement() {
    assert!(trace(|t| (90.0 * (t / 420.0).sin(), 0.0), 200, 16.0, 3, false).is_empty());
}
#[test]
fn single_reversal() {
    assert!(
        trace(
            |t| (
                if t < 250.0 {
                    t * 2.0
                } else {
                    (500.0 - t) * 2.0
                },
                0.0
            ),
            32,
            16.0,
            3,
            false
        )
        .is_empty()
    );
}
#[test]
fn large_sweep() {
    assert!(trace(|t| (900.0 * (t / 70.0).sin(), 0.0), 80, 16.0, 3, false).is_empty());
}
#[test]
fn drag_suppression() {
    assert!(trace(wave, 80, 16.0, 3, true).is_empty());
}
#[test]
fn trigger_cooldown() {
    let result = trace(wave, 160, 16.0, 3, false);
    assert!(result.len() >= 3);
    assert!(result.windows(2).all(|pair| pair[1] - pair[0] >= 220.0));
}
#[test]
fn no_stale_retrigger() {
    let mut detector = Detector::default();
    let (mut count, mut x) = (0, 0.0);
    for i in 0..60 {
        let t = f64::from(i * 16);
        x = wave(t).0;
        count += i32::from(detector.add(t, x, 0.0, false));
    }
    assert!(count > 0);
    for i in 60..160 {
        assert!(!detector.add(f64::from(i * 16), x, 0.0, false));
    }
}
#[test]
fn fresh_after_gap() {
    let mut detector = Detector::default();
    let mut count = 0;
    for j in 0..2 {
        for i in 0..60 {
            count += i32::from(detector.add(
                f64::from(j * 2000 + i * 16),
                wave(f64::from(i * 16)).0,
                0.0,
                false,
            ));
        }
    }
    assert!(count >= 2);
}
#[test]
fn release_cooldown() {
    let mut detector = Detector::default();
    detector.add(0.0, 0.0, 0.0, true);
    for i in 0..11 {
        assert!(!detector.add(f64::from(i * 16 + 1), wave(f64::from(i * 16)).0, 0.0, false));
    }
    assert!((0..60).any(|i| detector.add(
        f64::from(200 + i * 16),
        wave(f64::from(i * 16)).0,
        0.0,
        false
    )));
}
#[test]
fn partial_gesture_expires() {
    let mut detector = Detector::default();
    for j in 0..2 {
        for i in 0..10 {
            assert!(!detector.add(
                f64::from(j * 1000 + i * 16),
                wave(f64::from(i * 16)).0,
                0.0,
                false
            ));
        }
    }
}
#[test]
fn sensitivity() {
    let small = |t: f64| (30.0 * (t / 40.0).sin(), 0.0);
    assert!(trace(small, 80, 16.0, 1, false).is_empty());
    assert!(!trace(small, 80, 16.0, 5, false).is_empty());
}
#[test]
fn invalid_samples() {
    let mut detector = Detector::default();
    assert!(!detector.add(f64::NAN, 0.0, 0.0, false));
    assert!(!detector.add(1.0, f64::INFINITY, 0.0, false));
    assert!(!detector.add(10.0, 0.0, 0.0, false));
    assert!(!detector.add(5.0, 0.0, 0.0, false));
}
#[test]
fn different_sample_rates() {
    for interval in [8, 16, 25, 33] {
        assert!(!trace(wave, 1000 / interval, interval as f64, 3, false).is_empty());
    }
}
#[test]
fn animation_phases() {
    let mut animation = Animation::default();
    animation.trigger(0.0, 4.0, 1100);
    assert!((1.0..4.0).contains(&animation.frame(75.0).scale));
    assert_eq!(animation.frame(400.0).scale, 4.0);
    assert!((1.0..4.0).contains(&animation.frame(980.0).scale));
    assert!(!animation.frame(1100.0).visible);
}
#[test]
fn shrink_retrigger_continuity() {
    let mut a = Animation::default();
    a.trigger(0.0, 4.0, 1000);
    let before = a.frame(850.0);
    a.trigger(850.0, 4.0, 1000);
    assert_eq!(a.frame(850.0).scale, before.scale);
    assert_eq!(a.frame(1000.0).scale, 4.0);
    assert!(a.frame(1600.0).visible);
    assert!(!a.frame(1850.0).visible);
}
#[test]
fn grow_retrigger_continuity() {
    let mut a = Animation::default();
    a.trigger(0.0, 6.0, 500);
    let before = a.frame(70.0);
    a.trigger(70.0, 6.0, 500);
    assert_eq!(a.frame(70.0).scale, before.scale);
    assert_eq!(a.frame(220.0).scale, 6.0);
}
#[test]
fn reset_animation() {
    let mut a = Animation::default();
    a.trigger(0.0, 4.0, 1100);
    a.reset();
    assert!(!a.frame(100.0).visible);
}
#[test]
fn settings_clamp() {
    let mut s = Settings {
        sensitivity: 99,
        maximum_scale: f64::NAN,
        duration_ms: -1,
        ..Default::default()
    };
    assert!(!s.startup);
    s.normalize();
    assert_eq!(
        (s.sensitivity, s.maximum_scale, s.duration_ms),
        (5, 4.0, 500)
    );
}
#[test]
fn right_edge_mirror() {
    assert!(flip_at_edge(1919, 0, 200, false, 24));
    assert!(!flip_at_edge(0, 1919, 200, true, 24));
}
#[test]
fn edge_hysteresis() {
    assert!(flip_at_edge(1700, 210, 200, true, 24));
    assert!(!flip_at_edge(1700, 210, 200, false, 24));
    assert!(!flip_at_edge(1600, 300, 200, true, 24));
}
#[test]
fn holding_reduces_wakeups_and_retrigger_keeps_slow_timer() {
    let mut a = Animation::default();
    a.trigger(0.0, 4.0, 1100);
    assert_eq!(a.timer_delay(75.0), 15);
    assert_eq!(a.timer_delay(400.0), 100);
    assert_eq!(a.frame(450.0).scale, 4.0);
    a.trigger(450.0, 4.0, 1100);
    assert_eq!(a.timer_delay(450.0), 100);
    assert_eq!(a.timer_delay(1300.0), 15);
}
#[test]
fn invalid_animation_cannot_produce_nan() {
    let mut a = Animation::default();
    a.trigger(0.0, f64::NAN, 1100);
    assert!(!a.frame(10.0).visible);
}
#[test]
fn strict_settings_parse_and_roundtrip() {
    let s = settings::parse(
        "sensitivity=4junk\nmaximumScale=NaN\ndurationMs=9999\nenabled=no\nstartup=1\n",
    )
    .unwrap();
    assert_eq!(s.sensitivity, 3);
    assert_eq!(s.maximum_scale, 4.0);
    assert_eq!(s.duration_ms, 3000);
    assert!(s.enabled && s.startup);
    assert_eq!(settings::parse(&settings::encode(&s)).unwrap(), s);
}
#[test]
fn oversized_settings_rejected() {
    let path = std::env::temp_dir().join(format!(
        "shakespot-settings-limit-{}.ini",
        std::process::id()
    ));
    std::fs::write(&path, vec![b'x'; settings::MAX_SETTINGS_BYTES as usize + 1]).unwrap();
    let result = settings::load(&path);
    std::fs::remove_file(path).unwrap();
    assert!(result.is_err());
}
#[test]
fn bounded_detector_survives_sustained_and_nonmonotonic_input() {
    let mut detector = Detector::default();
    for i in 0..100_000 {
        let t = f64::from(i) * 0.125;
        detector.add(t, wave(t).0, 0.0, false);
    }
    detector.add(-1.0, 0.0, 0.0, false);
    assert!((0..100).any(|i| {
        let t = f64::from(i) * 16.0;
        detector.add(t, wave(t).0, 0.0, false)
    }));
    assert!(std::mem::size_of::<Detector>() < 4096);
}
