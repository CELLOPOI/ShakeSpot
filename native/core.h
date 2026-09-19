#pragma once
#include <algorithm>
#include <array>
#include <cmath>
#include <limits>

namespace shakespot {
struct Settings {
    int sensitivity = 3;
    double maximumScale = 4;
    int durationMs = 1100;
    bool enabled = true;
    bool startup = false;
    void normalize() {
        sensitivity = std::clamp(sensitivity, 1, 5);
        maximumScale = std::isfinite(maximumScale) ? std::clamp(maximumScale, 2.0, 8.0) : 4;
        durationMs = std::clamp(durationMs, 500, 3000);
    }
};

class Detector {
    struct Sample { double time, x, y; };
    std::array<Sample, 96> samples_{};
    int start_ = 0, count_ = 0, sensitivity_ = 3;
    double lastTrigger_ = -1e20, blockedUntil_ = -1e20;
    const Sample& at(int i) const { return samples_[(start_ + i) % samples_.size()]; }
    bool shake(bool horizontal, double elapsed) const {
        const double leg = 24 - (sensitivity_ - 3) * 4;
        const double minRange = 52 - (sensitivity_ - 3) * 8;
        const double minTravel = 210 - (sensitivity_ - 3) * 30;
        const double first = horizontal ? at(0).x : at(0).y;
        double previous = first, extreme = first, low = first, high = first, travel = 0;
        int direction = 0, reversals = 0;
        for (int i = 1; i < count_; ++i) {
            double value = horizontal ? at(i).x : at(i).y;
            low = std::min(low, value); high = std::max(high, value);
            travel += std::abs(value - previous); previous = value;
            if (!direction) {
                if (std::abs(value - first) >= leg) { direction = value > first ? 1 : -1; extreme = value; }
            } else if ((value - extreme) * direction >= 0) extreme = value;
            else if (std::abs(value - extreme) >= leg) { direction = -direction; extreme = value; ++reversals; }
        }
        double range = high - low;
        return reversals >= 3 && range >= minRange && range <= 600 && travel >= minTravel &&
            travel / range >= 2.8 && std::abs(previous - first) / travel <= .42 && travel / elapsed >= .38;
    }
public:
    void reset() { start_ = count_ = 0; lastTrigger_ = blockedUntil_ = -1e20; }
    void sensitivity(int value) { sensitivity_ = std::clamp(value, 1, 5); reset(); }
    bool add(double now, double x, double y, bool down = false) {
        if (!std::isfinite(now) || !std::isfinite(x) || !std::isfinite(y)) { reset(); return false; }
        if (count_ && now <= at(count_ - 1).time) reset();
        if (down) { start_ = count_ = 0; blockedUntil_ = now + 180; return false; }
        if (now < blockedUntil_) return false;
        if (count_) {
            const auto& p = at(count_ - 1);
            if (now - p.time > 130 || std::abs(x - p.x) > 900 || std::abs(y - p.y) > 900) start_ = count_ = 0;
        }
        while (count_ && now - at(0).time > 560 + (sensitivity_ - 3) * 35) { start_ = (start_ + 1) % 96; --count_; }
        if (count_ == 96) { start_ = (start_ + 1) % 96; --count_; }
        samples_[(start_ + count_++) % 96] = {now, x, y};
        if (count_ < 7 || now - lastTrigger_ < 220) return false;
        double elapsed = now - at(0).time;
        if (elapsed < 90 || (!shake(true, elapsed) && !shake(false, elapsed))) return false;
        lastTrigger_ = now;
        samples_[0] = {now, x, y}; start_ = 0; count_ = 1;
        return true;
    }
};

struct Frame { double scale; bool visible; };
class Animation {
    double growStart_ = 0, holdUntil_ = 0, from_ = 1, maximum_ = 4;
    bool active_ = false;
    static double smooth(double t) { t = std::clamp(t, 0.0, 1.0); return t * t * (3 - 2 * t); }
public:
    void reset() { active_ = false; }
    Frame frame(double now) {
        if (!active_) return {1, false};
        if (now < growStart_ + 150) return {from_ + (maximum_ - from_) * smooth((now - growStart_) / 150), true};
        if (now <= holdUntil_) return {maximum_, true};
        double t = (now - holdUntil_) / 260;
        if (t >= 1) { active_ = false; return {1, false}; }
        return {maximum_ + (1 - maximum_) * smooth(t), true};
    }
    void trigger(double now, double maximum, int duration) {
        auto before = frame(now);
        from_ = before.visible ? before.scale : 1;
        maximum_ = std::clamp(maximum, 2.0, 8.0);
        growStart_ = now;
        holdUntil_ = std::max(active_ ? holdUntil_ : now, now + std::clamp(duration, 500, 3000) - 260);
        active_ = true;
    }
};

inline bool flipAtEdge(int before, int after, int extent, bool flipped, int hysteresis) {
    if (before < after && before < extent) return false;
    return flipped ? after < extent + hysteresis : after < extent && before > after;
}
}
