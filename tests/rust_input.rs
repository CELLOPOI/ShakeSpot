use shakespot::input::{CoordinateMode, MotionTracker, RawMotion};

fn packet(time: f64, x: i64, y: i64) -> RawMotion {
    RawMotion {
        device: 1,
        mode: CoordinateMode::Relative,
        started: time,
        time,
        x,
        y,
    }
}

fn shake(rate: usize, tracker: &mut MotionTracker, offset: f64) -> (Vec<f64>, usize) {
    let mut previous = 0;
    let mut triggers = Vec::new();
    let mut samples = 0;
    for i in 0..=rate {
        let t = i as f64 * 1000.0 / rate as f64;
        let x = (340.0 * (t / 48.0).sin()).round() as i64;
        let result = tracker.push(packet(offset + t, x - previous, 0), (1.0, 1.0));
        previous = x;
        samples += result.samples;
        if result.triggered {
            triggers.push(offset + t);
        }
    }
    (triggers, samples)
}

#[test]
fn relative_gestures_survive_high_polling_rates_with_bounded_sampling() {
    let mut baseline = None;
    for rate in [125, 250, 1000, 8000] {
        let (triggers, samples) = shake(rate, &mut MotionTracker::default(), 0.0);
        assert!(!triggers.is_empty(), "rate={rate}");
        let first = *baseline.get_or_insert(triggers[0]);
        assert!(
            (first - triggers[0]).abs() <= 24.0,
            "rate={rate} first={}",
            triggers[0]
        );
        assert!(samples <= 140, "rate={rate} samples={samples}");
    }
}

#[test]
fn movement_keeps_a_turn_inside_the_sampling_interval() {
    let mut tracker = MotionTracker::default();
    tracker.push(packet(0.0, 0, 0), (1.0, 1.0));
    assert_eq!(tracker.push(packet(2.0, 80, 0), (1.0, 1.0)).samples, 0);
    assert_eq!(tracker.push(packet(4.0, -80, 0), (1.0, 1.0)).samples, 1);
}

#[test]
fn opposite_movements_and_devices_cannot_be_merged() {
    let mut first = packet(0.0, 40, 0);
    assert!(!first.merge(packet(1.0, -40, 0)));
    let mut other = packet(1.0, 40, 0);
    other.device = 2;
    assert!(!first.merge(other));
    assert!(first.merge(packet(2.0, 10, 0)));
    assert_eq!((first.x, first.y), (50, 0));
    assert!(!first.merge(packet(12.0, 10, 0)));
}

#[test]
fn blocked_gestures_need_fresh_evidence_after_release() {
    let mut tracker = MotionTracker::default();
    tracker.block(0.0);
    for i in 1..180 {
        assert!(
            !tracker
                .push(
                    packet(i as f64, if i % 2 == 0 { 500 } else { -500 }, 0),
                    (1.0, 1.0)
                )
                .triggered
        );
    }
    assert!(!shake(1000, &mut tracker, 200.0).0.is_empty());
}

#[test]
fn switching_devices_does_not_combine_partial_gestures() {
    let mut tracker = MotionTracker::default();
    let mut previous = 0;
    for i in 0..125 {
        let t = f64::from(i * 8);
        let x = (340.0 * (t / 48.0).sin()).round() as i64;
        let mut p = packet(t, x - previous, 0);
        previous = x;
        p.device = (i / 10) as usize;
        assert!(!tracker.push(p, (1.0, 1.0)).triggered);
    }
}

#[test]
fn absolute_input_seeds_position_and_accepts_zero_coordinates() {
    let mut tracker = MotionTracker::default();
    let mut first = packet(0.0, 50000, 40000);
    first.mode = CoordinateMode::AbsoluteVirtual;
    assert!(!tracker.push(first, (0.03, 0.03)).triggered);
    let mut count = 0;
    for i in 1..126 {
        let t = f64::from(i * 8);
        let mut p = packet(t, (3000.0 + 3000.0 * (t / 48.0).sin()).round() as i64, 0);
        p.mode = CoordinateMode::AbsoluteVirtual;
        count += usize::from(tracker.push(p, (0.03, 0.03)).triggered);
    }
    assert!(count > 0);
}

#[test]
fn coordinate_mode_changes_and_long_gaps_discard_old_evidence() {
    let mut tracker = MotionTracker::default();
    assert!(!shake(125, &mut tracker, 0.0).0.is_empty());
    let mut absolute = packet(2000.0, 65000, 65000);
    absolute.mode = CoordinateMode::AbsolutePrimary;
    assert!(!tracker.push(absolute, (0.03, 0.03)).triggered);
    assert!(!tracker.push(packet(2008.0, 1, 0), (1.0, 1.0)).triggered);
    assert!(!shake(125, &mut tracker, 3000.0).0.is_empty());
}
