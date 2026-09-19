namespace ShakeSpot.Core;

/// <summary>只接收单调时间和连续的 DIP 坐标；不依赖 UI、Win32 或系统时钟。</summary>
public sealed class ShakeDetector
{
    private const int Capacity = 96;
    private readonly Sample[] _samples = new Sample[Capacity];
    private int _start;
    private int _count;
    private double _lastTrigger = double.NegativeInfinity;
    private double _blockedUntil = double.NegativeInfinity;
    private int _sensitivity = 3;

    public int Sensitivity
    {
        get => _sensitivity;
        set { _sensitivity = Math.Clamp(value, 1, 5); Reset(); }
    }

    public void Reset()
    {
        _start = _count = 0;
        _lastTrigger = _blockedUntil = double.NegativeInfinity;
    }

    public bool AddSample(double milliseconds, double x, double y, bool buttonDown = false)
    {
        if (!double.IsFinite(milliseconds) || !double.IsFinite(x) || !double.IsFinite(y))
        {
            Reset();
            return false;
        }

        if (_count > 0 && milliseconds <= At(_count - 1).Time)
            Reset();

        if (buttonDown)
        {
            _start = _count = 0;
            _blockedUntil = milliseconds + 180;
            return false;
        }
        if (milliseconds < _blockedUntil) return false;

        if (_count > 0)
        {
            var previous = At(_count - 1);
            // 睡眠唤醒、远程光标跳转、采样停顿后不拼接旧轨迹。
            if (milliseconds - previous.Time > 130 ||
                Math.Abs(x - previous.X) > 900 || Math.Abs(y - previous.Y) > 900)
                _start = _count = 0;
        }

        var windowMs = 560 + (_sensitivity - 3) * 35;
        while (_count > 0 && milliseconds - At(0).Time > windowMs)
        {
            _start = (_start + 1) % Capacity;
            _count--;
        }
        if (_count == Capacity)
        {
            _start = (_start + 1) % Capacity;
            _count--;
        }
        _samples[(_start + _count++) % Capacity] = new(milliseconds, x, y);

        if (_count < 7 || milliseconds - _lastTrigger < 220) return false;
        var elapsed = milliseconds - At(0).Time;
        if (elapsed < 90) return false;
        if (!IsShake(true, elapsed) && !IsShake(false, elapsed)) return false;

        _lastTrigger = milliseconds;
        // 触发后消费已有证据；延长效果必须来自新的往返，静止不能反复触发。
        _samples[0] = new(milliseconds, x, y);
        _start = 0;
        _count = 1;
        return true;
    }

    private bool IsShake(bool horizontal, double elapsed)
    {
        var minimumLeg = 24 - (_sensitivity - 3) * 4;
        var minimumRange = 52 - (_sensitivity - 3) * 8;
        var minimumTravel = 210 - (_sensitivity - 3) * 30;
        double first = Value(At(0), horizontal), previous = first, extreme = first;
        double minimum = first, maximum = first, travel = 0;
        int direction = 0, reversals = 0;
        for (var i = 1; i < _count; i++)
        {
            var value = Value(At(i), horizontal);
            minimum = Math.Min(minimum, value);
            maximum = Math.Max(maximum, value);
            travel += Math.Abs(value - previous);
            previous = value;
            if (direction == 0)
            {
                if (Math.Abs(value - first) >= minimumLeg)
                {
                    direction = Math.Sign(value - first);
                    extreme = value;
                }
            }
            else if ((value - extreme) * direction >= 0)
                extreme = value;
            else if (Math.Abs(value - extreme) >= minimumLeg)
            {
                direction = -direction;
                extreme = value;
                reversals++;
            }
        }
        var range = maximum - minimum;
        return reversals >= 3 && range >= minimumRange && range <= 600 &&
               travel >= minimumTravel && travel / range >= 2.8 &&
               Math.Abs(previous - first) / travel <= 0.42 &&
               travel / elapsed >= 0.38;
    }

    private Sample At(int index) => _samples[(_start + index) % Capacity];
    private static double Value(Sample sample, bool horizontal) => horizontal ? sample.X : sample.Y;
    private readonly record struct Sample(double Time, double X, double Y);
}
