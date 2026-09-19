using System;
using System.ComponentModel;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using ShakeSpot.Core;

namespace ShakeSpot;

internal sealed class OverlayWindow : NativeWindow, IDisposable
{
    private readonly GraphicsPath _outer = ArrowArtwork.Outer();
    private readonly GraphicsPath _inner = ArrowArtwork.Inner();
    private nint _dc, _dib, _oldBitmap;
    private Bitmap? _bitmap;
    private Graphics? _graphics;
    private int _width, _height;
    private double _paintedScale = -1;
    private byte _paintedAlpha;
    private bool _flipX, _flipY;
    private nint _lastMonitor;
    internal bool IsVisible { get; private set; }

    internal OverlayWindow()
    {
        CreateHandle(new CreateParams
        {
            Caption = "ShakeSpot.Overlay", Style = NativeMethods.WsPopup,
            ExStyle = NativeMethods.WsExLayered | NativeMethods.WsExTransparent |
                      NativeMethods.WsExNoActivate | NativeMethods.WsExToolWindow | NativeMethods.WsExTopmost,
            Width = 1, Height = 1
        });
    }

    internal void Render(NativeMethods.Point cursor, double dpiScale, double maximumScale, EffectFrame frame)
    {
        if (!frame.Visible || frame.Opacity <= 0) { Hide(); return; }
        var width = (int)Math.Ceiling(28 * dpiScale * maximumScale) + 4;
        var height = (int)Math.Ceiling(37 * dpiScale * maximumScale) + 4;
        if (width != _width || height != _height) CreateSurface(width, height);

        var scale = frame.Scale * dpiScale;
        var alpha = (byte)Math.Clamp((int)Math.Round(frame.Opacity * 255), 0, 255);
        var monitor = NativeMethods.MonitorFromPoint(cursor, 2);
        var monitorInfo = new NativeMethods.MonitorInfo { Size = Marshal.SizeOf<NativeMethods.MonitorInfo>() };
        if (!NativeMethods.GetMonitorInfo(monitor, ref monitorInfo))
            throw new Win32Exception(Marshal.GetLastWin32Error(), "Monitor bounds unavailable.");
        var bounds = monitorInfo.Monitor;
        var placement = ArrowLayout.Place(cursor.X, cursor.Y, _width, _height,
            new ScreenBounds(bounds.Left, bounds.Top, bounds.Right, bounds.Bottom),
            IsVisible && monitor == _lastMonitor && _flipX,
            IsVisible && monitor == _lastMonitor && _flipY, (int)(24 * dpiScale));
        if (placement.FlipX != _flipX || placement.FlipY != _flipY) _paintedScale = -1;
        _flipX = placement.FlipX;
        _flipY = placement.FlipY;
        _lastMonitor = monitor;
        var position = new NativeMethods.Point(placement.Left, placement.Top);
        if (Math.Abs(scale - _paintedScale) > 0.0001 || alpha != _paintedAlpha || !IsVisible)
        {
            if (Math.Abs(scale - _paintedScale) > 0.0001)
            {
                _graphics!.ResetTransform();
                _graphics.Clear(Color.Transparent);
                _graphics.TranslateTransform(placement.TipX, placement.TipY);
                _graphics.ScaleTransform((float)(_flipX ? -scale : scale), (float)(_flipY ? -scale : scale));
                _graphics.FillPath(Brushes.White, _outer);
                _graphics.FillPath(Brushes.Black, _inner);
                _graphics.Flush(FlushIntention.Sync);
                _paintedScale = scale;
            }
            var size = new NativeMethods.Size(_width, _height);
            var source = new NativeMethods.Point(0, 0);
            var blend = new NativeMethods.BlendFunction { Alpha = alpha, Format = 1 };
            if (!NativeMethods.UpdateLayeredWindow(Handle, 0, ref position, ref size, _dc, ref source, 0, ref blend, 2))
                throw new Win32Exception(Marshal.GetLastWin32Error(), "UpdateLayeredWindow failed.");
            _paintedAlpha = alpha;
        }
        // 每帧仅移动这个小窗口；停留阶段复用位图，不重新绘图。
        if (!NativeMethods.SetWindowPos(Handle, NativeMethods.HwndTopmost, position.X, position.Y, 0, 0,
                NativeMethods.SwpNoActivate | NativeMethods.SwpNoSize | NativeMethods.SwpShowWindow))
            throw new Win32Exception(Marshal.GetLastWin32Error(), "Overlay positioning failed.");
        IsVisible = true;
    }

    internal void Hide()
    {
        if (IsVisible) NativeMethods.ShowWindow(Handle, 0);
        IsVisible = false;
    }

    protected override void WndProc(ref Message message)
    {
        if (message.Msg == 0x84) { message.Result = new nint(-1); return; } // HTTRANSPARENT
        if (message.Msg == 0x21) { message.Result = new nint(3); return; } // MA_NOACTIVATE
        // WM_DPICHANGED 不接受推荐矩形，位置和尺寸始终由物理像素决定。
        if (message.Msg == 0x2E0) { message.Result = 0; return; }
        base.WndProc(ref message);
    }

    private void CreateSurface(int width, int height)
    {
        ReleaseSurface();
        _width = width;
        _height = height;
        var info = new NativeMethods.BitmapInfo
        {
            Size = 40, Width = width, Height = -height, Planes = 1, BitCount = 32
        };
        _dc = NativeMethods.CreateCompatibleDC(0);
        _dib = NativeMethods.CreateDIBSection(_dc, ref info, 0, out var bits, 0, 0);
        if (_dc == 0 || _dib == 0 || bits == 0)
            throw new Win32Exception(Marshal.GetLastWin32Error(), "Overlay bitmap allocation failed.");
        _oldBitmap = NativeMethods.SelectObject(_dc, _dib);
        _bitmap = new Bitmap(width, height, width * 4, PixelFormat.Format32bppPArgb, bits);
        _graphics = Graphics.FromImage(_bitmap);
        _graphics.SmoothingMode = SmoothingMode.AntiAlias;
        _graphics.CompositingMode = CompositingMode.SourceOver;
        _graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
        _paintedScale = -1;
    }

    private void ReleaseSurface()
    {
        _graphics?.Dispose();
        _bitmap?.Dispose();
        _graphics = null;
        _bitmap = null;
        if (_oldBitmap != 0 && _dc != 0) NativeMethods.SelectObject(_dc, _oldBitmap);
        if (_dib != 0) NativeMethods.DeleteObject(_dib);
        if (_dc != 0) NativeMethods.DeleteDC(_dc);
        _oldBitmap = _dib = _dc = 0;
    }

    public void Dispose()
    {
        Hide();
        DestroyHandle();
        ReleaseSurface();
        _outer.Dispose();
        _inner.Dispose();
    }
}
