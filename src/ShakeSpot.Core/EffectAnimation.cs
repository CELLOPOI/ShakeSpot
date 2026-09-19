namespace ShakeSpot.Core;

public readonly record struct EffectFrame(double Scale, double Opacity, bool Visible);

/// <summary>用绝对时间计算动画；再次触发时从当前大小平滑接续。</summary>
public sealed class EffectAnimation
{
    private const double GrowMs = 150;
    private const double ShrinkMs = 260;
    private double _growStart;
    private double _holdUntil;
    private double _fromScale = 1;
    private double _fromOpacity;
    private double _maximumScale = 4;
    private bool _active;

    public void Trigger(double now, double maximumScale, double durationMs)
    {
        var current = GetFrame(now);
        _fromScale = current.Visible ? current.Scale : 1;
        _fromOpacity = current.Visible ? current.Opacity : 0;
        _maximumScale = Math.Clamp(maximumScale, 2, 8);
        _growStart = now;
        _holdUntil = Math.Max(_active ? _holdUntil : now, now + Math.Clamp(durationMs, 500, 3000) - ShrinkMs);
        _active = true;
    }

    public EffectFrame GetFrame(double now)
    {
        if (!_active) return new(1, 0, false);
        if (now < _growStart + GrowMs)
        {
            var progress = Smooth((now - _growStart) / GrowMs);
            return new(Lerp(_fromScale, _maximumScale, progress), Lerp(_fromOpacity, 1, progress), true);
        }
        if (now <= _holdUntil) return new(_maximumScale, 1, true);
        var remaining = (now - _holdUntil) / ShrinkMs;
        if (remaining >= 1)
        {
            _active = false;
            return new(1, 0, false);
        }
        var eased = Smooth(remaining);
        return new(Lerp(_maximumScale, 1, eased), 1 - eased, true);
    }

    public void Reset() => _active = false;
    private static double Smooth(double value)
    {
        value = Math.Clamp(value, 0, 1);
        return value * value * (3 - 2 * value);
    }
    private static double Lerp(double from, double to, double progress) => from + (to - from) * progress;
}
