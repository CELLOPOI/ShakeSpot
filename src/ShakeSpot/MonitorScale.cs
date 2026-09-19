using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace ShakeSpot;

/// <summary>在目标屏幕内放置一个不显示的 1px HWND，用 GetDpiForWindow 获取实际 DPI。</summary>
internal sealed class MonitorScale : NativeWindow, IDisposable
{
    private nint _monitor;
    private long _nextRefresh;
    internal double Scale { get; private set; } = 1;

    internal MonitorScale()
    {
        CreateHandle(new CreateParams
        {
            Caption = "ShakeSpot.DpiProbe", Style = NativeMethods.WsPopup,
            ExStyle = NativeMethods.WsExToolWindow | NativeMethods.WsExNoActivate,
            Width = 1, Height = 1
        });
    }

    internal double At(NativeMethods.Point point)
    {
        var monitor = NativeMethods.MonitorFromPoint(point, 2);
        var now = Environment.TickCount64;
        if (monitor == _monitor && now < _nextRefresh) return Scale;
        var info = new NativeMethods.MonitorInfo { Size = Marshal.SizeOf<NativeMethods.MonitorInfo>() };
        if (NativeMethods.GetMonitorInfo(monitor, ref info))
        {
            NativeMethods.SetWindowPos(Handle, 0, info.Monitor.Left + 1, info.Monitor.Top + 1,
                1, 1, NativeMethods.SwpNoActivate | NativeMethods.SwpNoZOrder);
            var dpi = NativeMethods.GetDpiForWindow(Handle);
            Scale = Math.Clamp(dpi == 0 ? 1 : dpi / 96.0, 0.75, 5);
            _monitor = monitor;
            _nextRefresh = now + 1000;
        }
        return Scale;
    }

    public void Dispose() => DestroyHandle();
}
