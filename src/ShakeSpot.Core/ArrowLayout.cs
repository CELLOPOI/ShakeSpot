namespace ShakeSpot.Core;

public readonly record struct ScreenBounds(int Left, int Top, int Right, int Bottom);
public readonly record struct ArrowPlacement(int Left, int Top, int TipX, int TipY, bool FlipX, bool FlipY);

public static class ArrowLayout
{
    /// <summary>在右/下边缘将箭身朝屏内翻转；绝不为容纳窗口而挪动热点。</summary>
    public static ArrowPlacement Place(int x, int y, int width, int height, ScreenBounds screen,
        bool wasFlippedX, bool wasFlippedY, int hysteresis)
    {
        var flipX = Flip(x - screen.Left, screen.Right - 1 - x, width - 4, wasFlippedX, hysteresis);
        var flipY = Flip(y - screen.Top, screen.Bottom - 1 - y, height - 4, wasFlippedY, hysteresis);
        var tipX = flipX ? width - 2 : 2;
        var tipY = flipY ? height - 2 : 2;
        return new(x - tipX, y - tipY, tipX, tipY, flipX, flipY);
    }

    private static bool Flip(int before, int after, int extent, bool flipped, int hysteresis)
    {
        if (before < after && before < extent) return false;
        if (flipped) return after < extent + hysteresis;
        return after < extent && before > after;
    }
}
