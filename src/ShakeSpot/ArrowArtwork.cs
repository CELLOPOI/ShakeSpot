using System.Drawing;
using System.Drawing.Drawing2D;

namespace ShakeSpot;

internal static class ArrowArtwork
{
    // 原创矢量轮廓；外轮廓尖端就是 (0, 0)，缩放不会移动热点。
    internal static GraphicsPath Outer()
    {
        var path = new GraphicsPath();
        path.AddPolygon(new PointF[] { new(0, 0), new(0, 29), new(8, 22), new(14, 35), new(21, 31), new(14.5f, 19), new(26, 19) });
        return path;
    }

    internal static GraphicsPath Inner()
    {
        var path = new GraphicsPath();
        path.AddPolygon(new PointF[] { new(1.6f, 3.4f), new(1.6f, 25.5f), new(8.5f, 19.5f), new(14.8f, 32.7f), new(18.7f, 30.3f), new(11.9f, 17.4f), new(21.3f, 17.4f) });
        return path;
    }

    internal static Icon TrayIcon(bool enabled)
    {
        using var bitmap = new Bitmap(32, 32);
        using var graphics = Graphics.FromImage(bitmap);
        graphics.SmoothingMode = SmoothingMode.AntiAlias;
        graphics.Clear(Color.Transparent);
        graphics.TranslateTransform(5, 2);
        graphics.ScaleTransform(0.78f, 0.78f);
        using var outer = Outer();
        using var inner = Inner();
        graphics.FillPath(Brushes.White, outer);
        graphics.FillPath(enabled ? Brushes.Black : Brushes.Gray, inner);
        var handle = bitmap.GetHicon();
        try
        {
            using var borrowed = Icon.FromHandle(handle);
            return (Icon)borrowed.Clone();
        }
        finally { NativeMethods.DestroyIcon(handle); }
    }
}
