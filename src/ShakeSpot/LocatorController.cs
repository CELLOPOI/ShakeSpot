using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows.Threading;
using ShakeSpot.Core;

namespace ShakeSpot;

internal sealed class LocatorController : IDisposable
{
    private readonly ShakeDetector _detector = new();
    private readonly EffectAnimation _animation = new();
    private readonly OverlayWindow _overlay = new();
    private readonly MonitorScale _monitorScale = new();
    private readonly Stopwatch _clock = Stopwatch.StartNew();
    private readonly DispatcherTimer _timer;
    private AppSettings _settings;
    private bool _havePosition;
    private bool _suspended;
    private NativeMethods.Point _previous;
    private double _logicalX, _logicalY;

    internal long Samples { get; private set; }
    internal long Triggers { get; private set; }
    internal long VisibleFrames { get; private set; }
    internal bool OverlayVisible => _overlay.IsVisible;
    internal event Action<Exception>? Failed;

    internal LocatorController(AppSettings settings)
    {
        _settings = settings;
        _detector.Sensitivity = settings.Sensitivity;
        _timer = new DispatcherTimer(DispatcherPriority.Input)
        {
            // 15ms 不越过 Windows 常见的 15.6ms 时钟粒度，避免 16ms 被量化为约 31ms。
            // 不调用 timeBeginPeriod，更不改变系统计时器精度。
            Interval = TimeSpan.FromMilliseconds(15)
        };
        _timer.Tick += Tick;
        if (settings.Enabled) _timer.Start();
    }

    internal void Apply(AppSettings settings)
    {
        _settings = settings.Normalize();
        _detector.Sensitivity = settings.Sensitivity;
        Clear();
        if (_settings.Enabled && !_suspended) _timer.Start();
        else _timer.Stop();
    }

    internal void Suspend(bool suspended)
    {
        _suspended = suspended;
        Clear();
        if (!_suspended && _settings.Enabled) _timer.Start();
        else _timer.Stop();
    }

    internal void Preview()
    {
        if (!_settings.Enabled || _suspended) return;
        _animation.Trigger(_clock.Elapsed.TotalMilliseconds, _settings.MaximumScale, _settings.DurationMs);
    }

    private void Tick(object? sender, EventArgs args)
    {
        try
        {
            Samples++;
            var cursor = new NativeMethods.CursorInfo { Size = Marshal.SizeOf<NativeMethods.CursorInfo>() };
            if (!NativeMethods.GetCursorInfo(ref cursor) || cursor.Flags != 1 || cursor.Cursor == 0 ||
                !NativeMethods.GetPhysicalCursorPos(out var point))
            {
                Clear();
                return;
            }
            var now = _clock.Elapsed.TotalMilliseconds;
            var dpiScale = _monitorScale.At(point);
            if (!_havePosition)
            {
                _logicalX = _logicalY = 0;
                _previous = point;
                _havePosition = true;
            }
            _logicalX += (point.X - _previous.X) / dpiScale;
            _logicalY += (point.Y - _previous.Y) / dpiScale;
            _previous = point;

            var buttonDown = IsDown(1) || IsDown(2) || IsDown(4) || IsDown(5) || IsDown(6);
            if (_detector.AddSample(now, _logicalX, _logicalY, buttonDown))
            {
                Triggers++;
                _animation.Trigger(now, _settings.MaximumScale, _settings.DurationMs);
            }
            // 开始点击/拖动即撤下效果，但保持识别器的按键释放冷却。
            if (buttonDown) _animation.Reset();
            var frame = _animation.GetFrame(now);
            _overlay.Render(point, dpiScale, _settings.MaximumScale, frame);
            if (_overlay.IsVisible) VisibleFrames++;
        }
        catch (Exception ex)
        {
            _timer.Stop();
            Clear();
            Failed?.Invoke(ex);
        }
    }

    private static bool IsDown(int key) => (NativeMethods.GetAsyncKeyState(key) & 0x8000) != 0;

    private void Clear()
    {
        _detector.Reset();
        _animation.Reset();
        _havePosition = false;
        _overlay.Hide();
    }

    public void Dispose()
    {
        _timer.Stop();
        _timer.Tick -= Tick;
        _overlay.Dispose();
        _monitorScale.Dispose();
    }
}
