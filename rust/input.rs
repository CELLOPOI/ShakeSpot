use crate::core::Detector;

pub const SAMPLE_INTERVAL_MS: f64 = 8.0;
// 相对移动以设备计数为单位：默认有效反向 96、范围 208、累计路程 840 个计数。
pub const RAW_COUNTS_PER_UNIT: f64 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinateMode {
    Relative,
    AbsolutePrimary,
    AbsoluteVirtual,
}

#[derive(Clone, Copy, Debug)]
pub struct RawMotion {
    pub device: usize,
    pub mode: CoordinateMode,
    pub started: f64,
    pub time: f64,
    pub x: i64,
    pub y: i64,
}

impl RawMotion {
    pub fn merge(&mut self, next: Self) -> bool {
        if self.mode != CoordinateMode::Relative
            || next.mode != self.mode
            || next.device != self.device
            || next.time < self.time
            || next.time - self.started > SAMPLE_INTERVAL_MS
            || self.x.signum() != next.x.signum()
            || self.y.signum() != next.y.signum()
        {
            return false;
        }
        self.x = self.x.saturating_add(next.x);
        self.y = self.y.saturating_add(next.y);
        self.time = next.time;
        true
    }
}

#[derive(Clone, Copy, Default)]
struct Point {
    time: f64,
    x: f64,
    y: f64,
}

pub struct MotionTracker {
    detector: Detector,
    source: Option<(usize, CoordinateMode)>,
    absolute: Option<(i64, i64)>,
    point: Point,
    anchor: Point,
    direction: (f64, f64),
    blocked_until: f64,
}

#[derive(Default)]
pub struct MotionResult {
    pub triggered: bool,
    pub samples: usize,
}

impl Default for MotionTracker {
    fn default() -> Self {
        Self {
            detector: Detector::default(),
            source: None,
            absolute: None,
            point: Point::default(),
            anchor: Point {
                time: -1e20,
                ..Point::default()
            },
            direction: (0.0, 0.0),
            blocked_until: -1e20,
        }
    }
}

impl MotionTracker {
    pub fn reset(&mut self) {
        self.detector.reset();
        self.source = None;
        self.absolute = None;
        self.point = Point::default();
        self.anchor = Point {
            time: -1e20,
            ..Point::default()
        };
        self.direction = (0.0, 0.0);
        self.blocked_until = -1e20;
    }

    pub fn set_sensitivity(&mut self, sensitivity: i32) {
        self.reset();
        self.detector.set_sensitivity(sensitivity);
    }

    pub fn block(&mut self, now: f64) {
        self.reset();
        self.blocked_until = now + 180.0;
    }

    fn sample(&mut self, point: Point, result: &mut MotionResult) {
        self.anchor = point;
        result.samples += 1;
        result.triggered |= self.detector.add(point.time, point.x, point.y, false);
    }

    pub fn push(&mut self, packet: RawMotion, absolute_scale: (f64, f64)) -> MotionResult {
        let mut result = MotionResult::default();
        if !packet.time.is_finite() || packet.time < self.blocked_until {
            return result;
        }
        if self.source != Some((packet.device, packet.mode))
            || packet.time <= self.point.time
            || packet.time - self.point.time > 130.0
        {
            self.reset();
            self.source = Some((packet.device, packet.mode));
        }
        let delta = match packet.mode {
            CoordinateMode::Relative => (
                packet.x as f64 / RAW_COUNTS_PER_UNIT,
                packet.y as f64 / RAW_COUNTS_PER_UNIT,
            ),
            CoordinateMode::AbsolutePrimary | CoordinateMode::AbsoluteVirtual => {
                let previous = self.absolute.replace((packet.x, packet.y));
                match previous {
                    Some((x, y)) => (
                        (packet.x - x) as f64 * absolute_scale.0,
                        (packet.y - y) as f64 * absolute_scale.1,
                    ),
                    None => (0.0, 0.0),
                }
            }
        };
        if !delta.0.is_finite() || !delta.1.is_finite() || delta.0.hypot(delta.1) > 900.0 {
            self.reset();
            return result;
        }
        let previous = self.point;
        self.point = Point {
            time: packet.time,
            x: previous.x + delta.0,
            y: previous.y + delta.1,
        };
        // 在采样间隔内遇到有幅度的反向，先提交转折前的点，不能把往返抵消成零。
        if delta.0 * self.direction.0 + delta.1 * self.direction.1 < 0.0
            && previous.time > self.anchor.time
            && (previous.x - self.anchor.x).hypot(previous.y - self.anchor.y) >= 4.0
        {
            self.sample(previous, &mut result);
        }
        if packet.time - self.anchor.time >= SAMPLE_INTERVAL_MS {
            self.sample(self.point, &mut result);
        }
        if delta != (0.0, 0.0) {
            self.direction = delta;
        }
        result
    }
}
