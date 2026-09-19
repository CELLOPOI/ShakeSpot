using ShakeSpot.Core;

var cases = new (string Name, Action Body)[]
{
    ("Horizontal shake", () => Require(Count(Wave()) >= 1)),
    ("Vertical shake", () => Require(Count(Wave().Select(p => (p.T, p.Y, p.X))) >= 1)),
    ("Diagonal shake", () => Require(Count(Wave().Select(p => (p.T, p.X, p.X * 0.7))) >= 1)),
    ("Ordinary curved movement", () => Require(Count(Trace(100, t => (t * 0.18, 35 * Math.Sin(t / 400)))) == 0)),
    ("Fast one-way movement", () => Require(Count(Trace(50, t => (t * 5, t * 2))) == 0)),
    ("Tiny high-frequency jitter", () => Require(Count(Trace(160, t => (4 * Math.Sin(t / 12), 5 * Math.Cos(t / 9)))) == 0)),
    ("Slow back and forth", () => Require(Count(Trace(200, t => (90 * Math.Sin(t / 420), 0))) == 0)),
    ("One reversal is insufficient", () => Require(Count(Trace(32, t => (t < 250 ? t * 2 : (500 - t) * 2, 0))) == 0)),
    ("Large sweeping movement", () => Require(Count(Trace(100, t => (900 * Math.Sin(t / 70), 0))) == 0)),
    ("Held mouse button suppresses shake", () => Require(Count(Wave(), buttonDown: true) == 0)),
    ("Continuous shake refreshes at bounded rate", () =>
    {
        var triggerTimes = TriggerTimes(Wave(160));
        Require(triggerTimes.Count >= 3);
        Require(triggerTimes.Zip(triggerTimes.Skip(1), (a, b) => b - a).All(delta => delta >= 220));
    }),
    ("Standing still does not reuse old evidence", () =>
    {
        var detector = new ShakeDetector();
        var points = Wave().ToArray();
        Require(points.Count(p => detector.AddSample(p.T, p.X, p.Y)) > 0);
        var last = points[^1];
        Require(Enumerable.Range(1, 100).Count(i => detector.AddSample(last.T + i * 16, last.X, last.Y)) == 0);
    }),
    ("Fresh second shake can trigger", () => Require(Count(Wave().Concat(Wave().Select(p => (p.T + 2000, p.X, p.Y)))) >= 2)),
    ("Reset discards partial evidence", () =>
    {
        var detector = new ShakeDetector();
        foreach (var p in Wave(12)) detector.AddSample(p.T, p.X, p.Y);
        detector.Reset();
        Require(!detector.AddSample(208, 0, 0));
    }),
    ("Button release has cooldown", () =>
    {
        var detector = new ShakeDetector();
        detector.AddSample(0, 0, 0, true);
        Require(!Wave(11).Any(p => detector.AddSample(p.T + 1, p.X, p.Y)));
        Require(Wave(60).Any(p => detector.AddSample(p.T + 200, p.X, p.Y)));
    }),
    ("Sample gaps invalidate partial gesture", () =>
    {
        var detector = new ShakeDetector();
        Require(!Wave(15).Any(p => detector.AddSample(p.T, p.X, p.Y)));
        Require(!Wave(10).Any(p => detector.AddSample(p.T + 1000, p.X, p.Y)));
    }),
    ("Sensitivity changes small gesture acceptance", () =>
    {
        var small = Trace(80, t => (30 * Math.Sin(t / 40), 0)).ToArray();
        Require(Count(small, 1) == 0);
        Require(Count(small, 5) > 0);
    }),
    ("Invalid data cannot trigger", () =>
    {
        var detector = new ShakeDetector();
        Require(!detector.AddSample(double.NaN, 0, 0));
        Require(!detector.AddSample(0, double.PositiveInfinity, 0));
        Require(!detector.AddSample(10, 0, 0));
        Require(!detector.AddSample(5, 0, 0));
    }),
    ("Detection at different sampling rates", () =>
    {
        foreach (var interval in new[] { 8, 16, 25, 33 })
            Require(Count(Trace(1000 / interval, t => (85 * Math.Sin(t / 48), 0), interval)) > 0);
    }),
    ("Animation grows, holds and disappears", () =>
    {
        var animation = new EffectAnimation();
        animation.Trigger(0, 4, 1100);
        Require(animation.GetFrame(75).Scale is > 1 and < 4);
        Require(animation.GetFrame(400).Scale == 4);
        Require(animation.GetFrame(980).Scale is > 1 and < 4);
        Require(!animation.GetFrame(1100).Visible);
    }),
    ("Retrigger during shrink has no size or opacity jump", () =>
    {
        var animation = new EffectAnimation();
        animation.Trigger(0, 4, 1000);
        var before = animation.GetFrame(850);
        animation.Trigger(850, 4, 1000);
        Require(animation.GetFrame(850) == before);
        Require(animation.GetFrame(1000).Scale == 4);
        Require(animation.GetFrame(1600).Visible);
        Require(!animation.GetFrame(1850).Visible);
    }),
    ("Retrigger during growth is continuous", () =>
    {
        var animation = new EffectAnimation();
        animation.Trigger(0, 6, 500);
        var before = animation.GetFrame(70);
        animation.Trigger(70, 6, 500);
        Require(animation.GetFrame(70) == before);
        Require(animation.GetFrame(220).Scale == 6);
    }),
    ("Pause immediately removes animation", () =>
    {
        var animation = new EffectAnimation();
        animation.Trigger(0, 4, 1000);
        animation.Reset();
        Require(!animation.GetFrame(100).Visible);
    }),
    ("Settings ranges are normalized", () =>
    {
        var settings = new AppSettings { Sensitivity = 99, MaximumScale = double.NaN, DurationMs = -50 }.Normalize();
        Require(settings.Sensitivity == 5 && settings.MaximumScale == 4 && settings.DurationMs == 500);
        Require(!new AppSettings().StartWithWindows);
    }),
    ("Center arrow preserves normal orientation and hotspot", () =>
    {
        var placement = ArrowLayout.Place(800, 500, 220, 290, new(0, 0, 1920, 1080), false, false, 24);
        Require(!placement.FlipX && !placement.FlipY);
        Require(placement.Left + placement.TipX == 800 && placement.Top + placement.TipY == 500);
    }),
    ("Bottom-right corner keeps arrow inside screen", () =>
    {
        var placement = ArrowLayout.Place(1919, 1079, 220, 290, new(0, 0, 1920, 1080), false, false, 24);
        Require(placement.FlipX && placement.FlipY);
        Require(placement.Left + placement.TipX == 1919 && placement.Top + placement.TipY == 1079);
        Require(placement.Left >= 0 && placement.Top >= 0);
    }),
    ("Negative-coordinate monitor preserves hotspot", () =>
    {
        var placement = ArrowLayout.Place(-2, -2, 330, 435, new(-2560, -1440, 0, 0), false, false, 36);
        Require(placement.FlipX && placement.FlipY);
        Require(placement.Left + placement.TipX == -2 && placement.Top + placement.TipY == -2);
    }),
    ("Edge hysteresis avoids rapid orientation toggles", () =>
    {
        var bounds = new ScreenBounds(0, 0, 1920, 1080);
        var a = ArrowLayout.Place(1710, 400, 220, 290, bounds, false, false, 24);
        var b = ArrowLayout.Place(1695, 400, 220, 290, bounds, a.FlipX, false, 24);
        var c = ArrowLayout.Place(1660, 400, 220, 290, bounds, b.FlipX, false, 24);
        Require(a.FlipX && b.FlipX && !c.FlipX);
    }),
    ("Settings roundtrip and damaged-file recovery", () =>
    {
        var directory = Path.Combine(Path.GetTempPath(), "ShakeSpotTests-" + Guid.NewGuid().ToString("N"));
        try
        {
            var store = new SettingsStore(directory);
            var expected = new AppSettings { Sensitivity = 4, MaximumScale = 5.5, DurationMs = 1600, Enabled = false };
            store.Save(expected);
            Require(store.Load(out var warning) == expected && warning is null);
            File.WriteAllText(store.FilePath, "{damaged json");
            Require(store.Load(out warning) == new AppSettings() && warning is not null);
        }
        finally { Directory.Delete(directory, recursive: true); }
    })
};

var failures = 0;
foreach (var (name, body) in cases)
{
    try { body(); Console.WriteLine($"PASS {name}"); }
    catch (Exception ex) { failures++; Console.WriteLine($"FAIL {name}: {ex.Message}"); }
}
Console.WriteLine($"{cases.Length - failures}/{cases.Length} passed.");
return failures == 0 ? 0 : 1;

static void Require(bool condition)
{
    if (!condition) throw new InvalidOperationException("Behavioral assertion failed.");
}
static IEnumerable<(double T, double X, double Y)> Trace(int count, Func<double, (double X, double Y)> position, int interval = 16)
{
    for (var i = 0; i < count; i++)
    {
        var t = (double)i * interval;
        var p = position(t);
        yield return (t, p.X, p.Y);
    }
}
static IEnumerable<(double T, double X, double Y)> Wave(int count = 45) => Trace(count, t => (85 * Math.Sin(t / 48), 0));
static int Count(IEnumerable<(double T, double X, double Y)> points, int sensitivity = 3, bool buttonDown = false)
{
    var detector = new ShakeDetector { Sensitivity = sensitivity };
    return points.Count(p => detector.AddSample(p.T, p.X, p.Y, buttonDown));
}
static List<double> TriggerTimes(IEnumerable<(double T, double X, double Y)> points)
{
    var detector = new ShakeDetector();
    return points.Where(p => detector.AddSample(p.T, p.X, p.Y)).Select(p => p.T).ToList();
}
