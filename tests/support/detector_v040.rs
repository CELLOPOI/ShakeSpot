// 冻结 0.4.0 的判定作为差分回归基准；不得随优化实现一起改写。
#[derive(Clone, Copy, Default)]
struct Sample {
    time: f64,
    x: f64,
    y: f64,
}

pub struct Detector {
    samples: [Sample; 96],
    start: usize,
    count: usize,
    sensitivity: i32,
    last_trigger: f64,
    blocked_until: f64,
}

impl Default for Detector {
    fn default() -> Self {
        Self {
            samples: [Sample::default(); 96],
            start: 0,
            count: 0,
            sensitivity: 3,
            last_trigger: -1e20,
            blocked_until: -1e20,
        }
    }
}

impl Detector {
    pub fn reset(&mut self) {
        self.start = 0;
        self.count = 0;
        self.last_trigger = -1e20;
        self.blocked_until = -1e20;
    }

    pub fn set_sensitivity(&mut self, value: i32) {
        self.sensitivity = value.clamp(1, 5);
        self.reset();
    }

    fn at(&self, i: usize) -> Sample {
        self.samples[(self.start + i) % self.samples.len()]
    }

    fn shake(&self, elapsed: f64) -> bool {
        let origin = self.at(0);
        let (mut sx, mut sy, mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
        let mut distance = 0.0;
        let mut previous = origin;
        for i in 0..self.count {
            let p = self.at(i);
            let (x, y) = (p.x - origin.x, p.y - origin.y);
            sx += x;
            sy += y;
            xx += x * x;
            xy += x * y;
            yy += y * y;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            distance += (p.x - previous.x).hypot(p.y - previous.y);
            previous = p;
        }
        let diagonal = (max_x - min_x).hypot(max_y - min_y);
        // 单轴往返不能掩盖另一轴的大幅平移；先用整段二维轨迹排除这类动作。
        if diagonal <= 0.0
            || distance / diagonal < 2.4
            || (previous.x - origin.x).hypot(previous.y - origin.y) > distance * 0.42
        {
            return false;
        }
        let count = self.count as f64;
        xx -= sx * sx / count;
        xy -= sx * sy / count;
        yy -= sy * sy / count;
        // 二维协方差的主轴随手势旋转，避免斜向动作被分摊到 X/Y 后达不到幅度门槛。
        let (axis_y, axis_x) = (0.5 * (2.0 * xy).atan2(xx - yy)).sin_cos();
        let leg = f64::from(24 - (self.sensitivity - 3) * 4);
        let min_range = f64::from(52 - (self.sensitivity - 3) * 8);
        let min_travel = f64::from(210 - (self.sensitivity - 3) * 30);
        let coordinate = |s: Sample| (s.x - origin.x) * axis_x + (s.y - origin.y) * axis_y;
        let first = coordinate(self.at(0));
        let (mut previous, mut extreme, mut low, mut high) = (first, first, first, first);
        let (mut travel, mut direction, mut reversals) = (0.0, 0.0, 0);
        for i in 1..self.count {
            let value = coordinate(self.at(i));
            low = low.min(value);
            high = high.max(value);
            travel += (value - previous).abs();
            previous = value;
            if direction == 0.0 {
                if (value - first).abs() >= leg {
                    direction = if value > first { 1.0 } else { -1.0 };
                    extreme = value;
                }
            } else if (value - extreme) * direction >= 0.0 {
                extreme = value;
            } else if (value - extreme).abs() >= leg {
                direction = -direction;
                extreme = value;
                reversals += 1;
            }
        }
        let range = high - low;
        reversals >= 3
            && range >= min_range
            && range <= 600.0
            && travel >= min_travel
            && travel / range >= 2.8
            && (previous - first).abs() / travel <= 0.42
            && travel / elapsed >= 0.38
    }

    pub fn add(&mut self, now: f64, x: f64, y: f64, down: bool) -> bool {
        if !now.is_finite() || !x.is_finite() || !y.is_finite() {
            self.reset();
            return false;
        }
        if self.count > 0 && now <= self.at(self.count - 1).time {
            self.reset();
        }
        if down {
            self.start = 0;
            self.count = 0;
            self.blocked_until = now + 180.0;
            return false;
        }
        if now < self.blocked_until {
            return false;
        }
        if self.count > 0 {
            let p = self.at(self.count - 1);
            if now - p.time > 130.0 || (x - p.x).hypot(y - p.y) > 900.0 {
                self.start = 0;
                self.count = 0;
            }
        }
        while self.count > 0 && now - self.at(0).time > f64::from(560 + (self.sensitivity - 3) * 35)
        {
            self.start = (self.start + 1) % self.samples.len();
            self.count -= 1;
        }
        if self.count == self.samples.len() {
            self.start = (self.start + 1) % self.samples.len();
            self.count -= 1;
        }
        self.samples[(self.start + self.count) % self.samples.len()] = Sample { time: now, x, y };
        self.count += 1;
        if self.count < 7 || now - self.last_trigger < 220.0 {
            return false;
        }
        let elapsed = now - self.at(0).time;
        if elapsed < 90.0 || !self.shake(elapsed) {
            return false;
        }
        self.last_trigger = now;
        self.samples[0] = Sample { time: now, x, y };
        self.start = 0;
        self.count = 1;
        true
    }
}
