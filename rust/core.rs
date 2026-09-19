#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub sensitivity: i32,
    pub maximum_scale: f64,
    pub duration_ms: i32,
    pub enabled: bool,
    pub startup: bool,
    pub excluded_apps: crate::exclusions::Exclusions,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sensitivity: 3,
            maximum_scale: 4.0,
            duration_ms: 1100,
            enabled: true,
            startup: false,
            excluded_apps: crate::exclusions::Exclusions::default(),
        }
    }
}

impl Settings {
    pub fn normalize(&mut self) {
        self.sensitivity = self.sensitivity.clamp(1, 5);
        self.maximum_scale = if self.maximum_scale.is_finite() {
            self.maximum_scale.clamp(2.0, 8.0)
        } else {
            4.0
        };
        self.duration_ms = self.duration_ms.clamp(500, 3000);
    }
}

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
    travel_bound: f64,
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
            travel_bound: 0.0,
        }
    }
}

impl Detector {
    pub fn reset(&mut self) {
        self.clear_history();
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

    fn clear_history(&mut self) {
        self.start = 0;
        self.count = 0;
        self.travel_bound = 0.0;
    }

    fn manhattan(a: Sample, b: Sample) -> f64 {
        (a.x - b.x).abs() + (a.y - b.y).abs()
    }

    fn discard_oldest(&mut self) {
        if self.count > 1 {
            self.travel_bound =
                (self.travel_bound - Self::manhattan(self.at(0), self.at(1))).max(0.0);
        } else {
            self.travel_bound = 0.0;
        }
        self.start = (self.start + 1) % self.samples.len();
        self.count -= 1;
        // 每绕回一轮重新求和，限制长时间增减累计的舍入误差。
        if self.start == 0 {
            self.travel_bound = (1..self.count)
                .map(|i| Self::manhattan(self.at(i - 1), self.at(i)))
                .sum();
        }
    }

    fn could_shake(&self, elapsed: f64) -> bool {
        // L1 路程是二维路程和任意主轴投影路程的上界；这里只剔除不可能成立的轨迹。
        // 留出舍入余量，最终仍由原判定按全部样本精算，不改变反向幅度确认时机。
        let upper = self.travel_bound + 1e-6;
        let first = self.at(0);
        let last = self.at(self.count - 1);
        let net_lower = (last.x - first.x).abs().max((last.y - first.y).abs());
        upper >= f64::from(210 - (self.sensitivity - 3) * 30)
            && upper >= elapsed * 0.38
            && net_lower <= upper * 0.42
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
            self.clear_history();
            self.blocked_until = now + 180.0;
            return false;
        }
        if now < self.blocked_until {
            return false;
        }
        if self.count > 0 {
            let p = self.at(self.count - 1);
            if now - p.time > 130.0 || (x - p.x).hypot(y - p.y) > 900.0 {
                self.clear_history();
            }
        }
        while self.count > 0 && now - self.at(0).time > f64::from(560 + (self.sensitivity - 3) * 35)
        {
            self.discard_oldest();
        }
        if self.count == self.samples.len() {
            self.discard_oldest();
        }
        let sample = Sample { time: now, x, y };
        if self.count > 0 {
            self.travel_bound += Self::manhattan(self.at(self.count - 1), sample);
        }
        self.samples[(self.start + self.count) % self.samples.len()] = sample;
        self.count += 1;
        if self.count < 7 || now - self.last_trigger < 220.0 {
            return false;
        }
        let elapsed = now - self.at(0).time;
        if elapsed < 90.0 || !self.could_shake(elapsed) || !self.shake(elapsed) {
            return false;
        }
        self.last_trigger = now;
        self.samples[0] = Sample { time: now, x, y };
        self.start = 0;
        self.count = 1;
        self.travel_bound = 0.0;
        true
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub scale: f64,
    pub visible: bool,
}

#[derive(Default)]
pub struct Animation {
    grow_start: f64,
    hold_until: f64,
    from: f64,
    maximum: f64,
    active: bool,
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Animation {
    pub fn reset(&mut self) {
        self.active = false;
    }
    pub fn frame(&mut self, now: f64) -> Frame {
        if !self.active {
            return Frame {
                scale: 1.0,
                visible: false,
            };
        }
        let scale = if now < self.grow_start + 150.0 {
            self.from + (self.maximum - self.from) * smooth((now - self.grow_start) / 150.0)
        } else if now <= self.hold_until {
            self.maximum
        } else {
            let t = (now - self.hold_until) / 260.0;
            if t >= 1.0 {
                self.active = false;
                return Frame {
                    scale: 1.0,
                    visible: false,
                };
            }
            self.maximum + (1.0 - self.maximum) * smooth(t)
        };
        Frame {
            scale,
            visible: true,
        }
    }
    pub fn trigger(&mut self, now: f64, maximum: f64, duration: i32) {
        if !now.is_finite() || !maximum.is_finite() {
            self.reset();
            return;
        }
        let before = self.frame(now);
        self.from = if before.visible { before.scale } else { 1.0 };
        self.maximum = maximum.clamp(2.0, 8.0);
        // 已到最大尺寸时延长停留，不重新进入高频增长阶段。
        self.grow_start = if before.visible && (self.from - self.maximum).abs() < 0.001 {
            now - 150.0
        } else {
            now
        };
        self.hold_until = (if self.active { self.hold_until } else { now })
            .max(now + f64::from(duration.clamp(500, 3000)) - 260.0);
        self.active = true;
    }
    pub fn timer_delay(&self, now: f64) -> u32 {
        if now >= self.grow_start + 150.0 && now < self.hold_until {
            (self.hold_until - now).ceil().clamp(15.0, 100.0) as u32
        } else {
            15
        }
    }
}

pub fn flip_at_edge(before: i32, after: i32, extent: i32, flipped: bool, hysteresis: i32) -> bool {
    if before < after && before < extent {
        return false;
    }
    if flipped {
        after < extent + hysteresis
    } else {
        after < extent && before > after
    }
}
