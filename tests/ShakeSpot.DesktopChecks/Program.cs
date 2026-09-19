using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows.Automation;
using System.Windows.Forms;
using ShakeSpot;
using ShakeSpot.Core;

internal static class Program
{
    private static readonly List<object> Results = new();
    private static string _artifacts = "";
    private static int _exitCode;

    [STAThread]
    private static int Main(string[] args)
    {
        Application.EnableVisualStyles();
        if (args.Length > 0 && args[0] == "--target")
        {
            Application.Run(new InputTarget(args[1]));
            return 0;
        }
        _artifacts = Path.GetFullPath(args.Length > 0 ? args[0] : "artifacts/desktop-checks");
        Directory.CreateDirectory(_artifacts);
        using var runner = new Form { Opacity = 0, ShowInTaskbar = false, Width = 1, Height = 1 };
        runner.Shown += async (_, _) =>
        {
            try { await RunChecks(); }
            catch (Exception ex)
            {
                _exitCode = 1;
                Results.Add(new { Test = "Desktop checks", Passed = false, Error = ex.ToString() });
                Console.WriteLine(ex);
            }
            finally
            {
                File.WriteAllText(Path.Combine(_artifacts, "report.json"), JsonSerializer.Serialize(new
                {
                    Date = DateTimeOffset.Now, OS = RuntimeInformation.OSDescription,
                    Monitors = Screen.AllScreens.Select(s => new { s.DeviceName, s.Bounds, s.Primary }),
                    Results,
                    Pending = new[] { "Multiple physical monitors", "Mixed monitor DPI", "Exclusive fullscreen games", "Physical mouse feel", "Logon startup after Windows restart" }
                }, new JsonSerializerOptions { WriteIndented = true }));
                runner.Close();
            }
        };
        Application.Run(runner);
        return _exitCode;
    }

    private static async Task RunChecks()
    {
        NativeMethods.GetPhysicalCursorPos(out var originalPosition);
        var originalForeground = Win.GetForegroundWindow();
        var targetTitle = "ShakeSpot desktop test · " + Guid.NewGuid().ToString("N")[..8];
        using var target = Start(Environment.ProcessPath!, "--target", targetTitle);
        Process? app = null;
        nint targetWindow = 0;
        string? controlTitle = null;
        try
        {
            await WaitFor(() => (targetWindow = Win.FindWindow(null, targetTitle)) != 0);
            await Task.Delay(400);
            NativeMethods.ShowWindow(targetWindow, 5);
            Win.SetForegroundWindow(targetWindow);
            await Task.Delay(300);
            Check("Target runs in a separate process", Win.GetWindowThreadProcessId(targetWindow, out var pid) != 0 && pid != Environment.ProcessId);
            Win.GetWindowRect(targetWindow, out var targetRect);
            var center = new NativeMethods.Point((targetRect.Left + targetRect.Right) / 2, (targetRect.Top + targetRect.Bottom) / 2);
            using var scaleProbe = new MonitorScale();
            var dpiScale = scaleProbe.At(center);
            Move(center);
            await Task.Delay(100);
            var hitWindow = Win.WindowFromPoint(center);
            Win.GetWindowThreadProcessId(hitWindow, out var hitPid);
            Console.WriteLine(JsonSerializer.Serialize(new
            {
                Target = targetWindow.ToInt64(), TargetVisible = Win.IsWindowVisible(targetWindow),
                Left = targetRect.Left, Top = targetRect.Top, Right = targetRect.Right, Bottom = targetRect.Bottom,
                CenterX = center.X, CenterY = center.Y, DpiScale = dpiScale,
                Foreground = Win.GetForegroundWindow().ToInt64(), HitWindow = hitWindow.ToInt64(), HitPid = hitPid
            }));
            Check("Input is confined to the owned test surface", Win.WindowFromPoint(center) == targetWindow);
            if (Win.GetForegroundWindow() != targetWindow)
            {
                Input(0x0002); Input(0x0004);
                await Task.Delay(100);
            }
            Check("Test target owns foreground before input", Win.GetForegroundWindow() == targetWindow);

            using (var overlay = new OverlayWindow())
            {
                var anchor = new NativeMethods.Point(center.X - 15, center.Y - 30);
                overlay.Render(anchor, dpiScale, 4, new EffectFrame(4, 1, true));
                await Task.Delay(180);
                Win.GetWindowRect(overlay.Handle, out var overlayRect);
                Check("Overlay physical hotspot alignment", overlayRect.Left + 2 == anchor.X && overlayRect.Top + 2 == anchor.Y);
                Check("Overlay does not activate", Win.GetForegroundWindow() == targetWindow);
                Check("Opaque arrow is hit-test transparent across processes", Win.WindowFromPoint(center) == targetWindow);
                var beforeClicks = Query(targetWindow, 0);
                Input(0x0002);
                await Task.Delay(50);
                Input(0x0004);
                await Task.Delay(80);
                Check("Click passes through opaque arrow", Query(targetWindow, 0) == beforeClicks + 1);
                var beforeWheel = Query(targetWindow, 1);
                Input(0x0800, 120);
                await Task.Delay(80);
                Check("Wheel passes through overlay", Query(targetWindow, 1) == beforeWheel + 1);
                var beforeDrag = Query(targetWindow, 2);
                Input(0x0002);
                for (var i = 0; i < 8; i++)
                {
                    Move(new(center.X + i * 3, center.Y + i * 2));
                    await Task.Delay(20);
                }
                Input(0x0004);
                await Task.Delay(80);
                Check("Drag messages pass through overlay", Query(targetWindow, 2) > beforeDrag);
                Check("Focus preserved after click wheel and drag", Win.GetForegroundWindow() == targetWindow);
                Move(anchor);
                Capture(targetRect, "arrow-preview.png");
                overlay.Hide();
                Check("Overlay hides on cleanup", !Win.IsWindowVisible(overlay.Handle));
                overlay.Render(new NativeMethods.Point(-140, -80), 1.5, 4, new EffectFrame(4, 1, true));
                Win.GetWindowRect(overlay.Handle, out var negativeRect);
                Check("Negative physical coordinates remain exact", negativeRect.Left == -142 && negativeRect.Top == -82);
            }

            var profile = Path.Combine(_artifacts, "profile");
            new SettingsStore(profile).Save(new AppSettings());
            var instance = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(profile.ToUpperInvariant())))[..16];
            controlTitle = "ShakeSpot.Control." + instance;
            var projectRoot = FindProjectRoot();
            var appPath = Path.Combine(projectRoot, "src/ShakeSpot/bin/Release/net10.0-windows/win-x64/ShakeSpot.exe");
            var diagnostics = Path.Combine(_artifacts, "app-diagnostics.json");
            app = Start(appPath, "--data-dir", profile, "--diagnostics", diagnostics);
            await WaitFor(() => Win.FindWindow(null, controlTitle) != 0 || app.HasExited);
            Check("Full application starts", !app.HasExited);
            await Task.Delay(500);
            var appOverlay = FindWindow(app.Id, "ShakeSpot.Overlay");
            Check("Application creates one inactive overlay", appOverlay != 0 && !Win.IsWindowVisible(appOverlay));
            Move(center);
            await Shake(center, dpiScale, 64);
            Check("Real sampled motion triggers overlay", Win.IsWindowVisible(appOverlay));
            Check("Shake preserves foreground", Win.GetForegroundWindow() == targetWindow);
            await Task.Delay(50);
            Win.GetWindowRect(appOverlay, out var activeRect);
            NativeMethods.GetPhysicalCursorPos(out var actualCursor);
            Check("Running effect follows physical cursor", Math.Abs(activeRect.Left + 2 - actualCursor.X) <= 2 && Math.Abs(activeRect.Top + 2 - actualCursor.Y) <= 2);
            await Task.Delay(1400);
            Check("Effect recovers automatically after motion stops", !Win.IsWindowVisible(appOverlay));

            Input(0x0002);
            await Shake(center, dpiScale, 45);
            Check("Shaking while dragging does not trigger", !Win.IsWindowVisible(appOverlay));
            Input(0x0004);
            await Task.Delay(250);

            ControlWindow.Send(controlTitle, ControlWindow.Preview);
            await Task.Delay(200);
            Check("Preview is visible", Win.IsWindowVisible(appOverlay));
            ControlWindow.Send(controlTitle, ControlWindow.Pause);
            await Task.Delay(120);
            Check("Pause removes current effect", !Win.IsWindowVisible(appOverlay));
            await Shake(center, dpiScale, 45);
            Check("Paused app ignores shake", !Win.IsWindowVisible(appOverlay));
            ControlWindow.Send(controlTitle, ControlWindow.Resume);
            await Task.Delay(150);
            await Shake(center, dpiScale, 45);
            Check("Resume accepts a fresh shake", Win.IsWindowVisible(appOverlay));

            Move(center);
            await Task.Delay(1400);
            await Measure(app, "Idle before opening settings", 10, null);
            var hideCount = Win.SendMessage(targetWindow, 0x8103, 0, 0);
            await Task.Delay(150);
            var hidden = ReadCursor();
            Check("Test target hides its own cursor", hidden.Flags == 0);
            ControlWindow.Send(controlTitle, ControlWindow.Preview);
            await Task.Delay(150);
            Check("Hidden system cursor suppresses effect", !Win.IsWindowVisible(appOverlay));
            var showCount = Win.SendMessage(targetWindow, 0x8104, 0, 0);
            Move(new NativeMethods.Point(center.X + 1, center.Y));
            Input(0x0001);
            await Task.Delay(100);
            hidden = ReadCursor();
            Console.WriteLine($"Cursor balance: hide={hideCount}, show={showCount}, flags={hidden.Flags}, foreground={Win.GetForegroundWindow()}, target={targetWindow}");
            Check("Test target balances its cursor display counter", showCount.ToInt64() == hideCount.ToInt64() + 1 && showCount.ToInt64() >= 0);

            ControlWindow.Send(controlTitle, ControlWindow.Settings);
            nint settingsWindow = 0;
            await WaitFor(() => (settingsWindow = FindWindow(app.Id, "ShakeSpot 设置")) != 0);
            NativeMethods.ShowWindow(targetWindow, 0);
            await Task.Delay(350);
            Win.GetWindowRect(settingsWindow, out var settingsRect);
            Capture(settingsRect, "settings-preview.png");
            Check("Settings window opens", Win.IsWindowVisible(settingsWindow));
            Check("Duration label displays decimal seconds", Control(settingsWindow, "1.1 秒", ControlType.Text) is not null);
            SetSlider(settingsWindow, "灵敏度", 4);
            SetSlider(settingsWindow, "最大放大倍数", 5.5);
            SetSlider(settingsWindow, "效果持续时间", 1500);
            Press(settingsWindow, "保存");
            await WaitFor(() => !Win.IsWindow(settingsWindow));
            var saved = new SettingsStore(profile).Load(out _);
            Check("Settings UI saves selected values", saved.Sensitivity == 4 && saved.MaximumScale == 5.5 && saved.DurationMs == 1500);
            ControlWindow.Send(controlTitle, ControlWindow.Settings);
            await WaitFor(() => (settingsWindow = FindWindow(app.Id, "ShakeSpot 设置")) != 0);
            Press(settingsWindow, "恢复默认");
            Press(settingsWindow, "保存");
            await WaitFor(() => !Win.IsWindow(settingsWindow));
            Check("Settings UI restores defaults", new SettingsStore(profile).Load(out _) == new AppSettings());
            Check("Closing settings keeps tray app running", !app.HasExited);
            NativeMethods.ShowWindow(targetWindow, 5);
            Win.SetForegroundWindow(targetWindow);
            Move(center);
            Input(0x0001);
            await Task.Delay(1600);
            hidden = ReadCursor();
            Check("Cursor visible before performance measurements", hidden.Flags == 1);

            await Measure(app, "Idle with visible stationary cursor", 10, null);
            await Measure(app, "Continuously refreshed effect", 10, () => ControlWindow.Send(controlTitle, ControlWindow.Preview));
            ControlWindow.Send(controlTitle, ControlWindow.Pause);
            await Task.Delay(200);
            await Measure(app, "Paused", 10, null);
            ControlWindow.Send(controlTitle, ControlWindow.Exit);
            await WaitFor(() => app.HasExited);
            Check("Normal exit succeeds", app.ExitCode == 0);
            Check("Normal exit destroys overlay", !Win.IsWindow(appOverlay));
            var afterExit = ReadCursor();
            Check("Cursor remains visible after normal exit", afterExit.Flags == 1);

            app.Dispose();
            app = Start(appPath, "--data-dir", profile);
            await WaitFor(() => Win.FindWindow(null, controlTitle) != 0);
            ControlWindow.Send(controlTitle, ControlWindow.Resume);
            ControlWindow.Send(controlTitle, ControlWindow.Preview);
            await Task.Delay(250);
            var crashOverlay = FindWindow(app.Id, "ShakeSpot.Overlay");
            Check("Effect active before forced termination", Win.IsWindowVisible(crashOverlay));
            app.Kill();
            await WaitFor(() => app.HasExited);
            await Task.Delay(100);
            afterExit = ReadCursor();
            Check("Forced termination leaves cursor visible and destroys overlay", afterExit.Flags == 1 && !Win.IsWindow(crashOverlay));
            var clicksAfterCrash = Query(targetWindow, 0);
            Input(0x0002); Input(0x0004);
            await Task.Delay(100);
            Check("Input still works after forced termination", Query(targetWindow, 0) == clicksAfterCrash + 1);
        }
        finally
        {
            // 只结束本次创建的进程；按键、测试窗口的光标与鼠标位置都成对恢复。
            try { Input(0x0004); } catch (InvalidOperationException) { }
            if (targetWindow != 0) Win.SendMessage(targetWindow, 0x8104, 0, 0);
            if (app is not null)
            {
                if (!app.HasExited)
                {
                    if (controlTitle is not null) ControlWindow.Send(controlTitle, ControlWindow.Exit);
                    await Task.Delay(200);
                    if (!app.HasExited) app.Kill();
                }
                app.Dispose();
            }
            if (!target.HasExited)
            {
                if (targetWindow != 0) Win.PostMessage(targetWindow, 0x10, 0, 0);
                await Task.Delay(150);
                if (!target.HasExited) target.Kill();
            }
            Win.SetPhysicalCursorPos(originalPosition.X, originalPosition.Y);
            Win.SetForegroundWindow(originalForeground);
        }
    }

    private static async Task Measure(Process process, string name, int seconds, Action? action)
    {
        process.Refresh();
        var cpu = CpuSeconds(process);
        var clock = Stopwatch.StartNew();
        var peak = process.WorkingSet64;
        while (clock.Elapsed.TotalSeconds < seconds)
        {
            action?.Invoke();
            await Task.Delay(500);
            process.Refresh();
            peak = Math.Max(peak, process.WorkingSet64);
        }
        var cpuEnd = CpuSeconds(process);
        var cpuSeconds = cpuEnd - cpu;
        var row = new
        {
            Measurement = name, Seconds = clock.Elapsed.TotalSeconds,
            CpuStartSeconds = cpu, CpuEndSeconds = cpuEnd,
            CpuSeconds = cpuSeconds, OneCorePercent = cpuSeconds / clock.Elapsed.TotalSeconds * 100,
            WholeMachinePercent = cpuSeconds / clock.Elapsed.TotalSeconds / Environment.ProcessorCount * 100,
            LogicalProcessors = Environment.ProcessorCount, PeakWorkingSetMiB = peak / 1048576.0,
            PrivateMiB = process.PrivateMemorySize64 / 1048576.0
        };
        Results.Add(row);
        Console.WriteLine(JsonSerializer.Serialize(row));
    }

    private static double CpuSeconds(Process process)
    {
        if (!Win.GetProcessTimes(process.Handle, out _, out _, out var kernel, out var user))
            throw new InvalidOperationException("GetProcessTimes failed.");
        return (kernel + user) / 10000000.0;
    }
    private static NativeMethods.CursorInfo ReadCursor()
    {
        var cursor = new NativeMethods.CursorInfo { Size = Marshal.SizeOf<NativeMethods.CursorInfo>() };
        if (!NativeMethods.GetCursorInfo(ref cursor)) throw new InvalidOperationException("GetCursorInfo failed: " + Marshal.GetLastWin32Error());
        return cursor;
    }
    private static AutomationElement Control(nint window, string name, ControlType type) =>
        AutomationElement.FromHandle(window).FindFirst(TreeScope.Descendants,
            new AndCondition(new PropertyCondition(AutomationElement.NameProperty, name),
                new PropertyCondition(AutomationElement.ControlTypeProperty, type))) ?? throw new InvalidOperationException("UI control not found: " + name);
    private static void SetSlider(nint window, string name, double value) =>
        ((RangeValuePattern)Control(window, name, ControlType.Slider).GetCurrentPattern(RangeValuePattern.Pattern)).SetValue(value);
    private static void Press(nint window, string name) =>
        ((InvokePattern)Control(window, name, ControlType.Button).GetCurrentPattern(InvokePattern.Pattern)).Invoke();

    private static async Task Shake(NativeMethods.Point center, double dpi, int count)
    {
        var clock = Stopwatch.StartNew();
        while (clock.Elapsed.TotalMilliseconds < count * 16)
        {
            Move(new(center.X + (int)(85 * dpi * Math.Sin(clock.Elapsed.TotalMilliseconds / 48)), center.Y));
            await Task.Delay(8);
        }
    }
    private static Process Start(string path, params string[] args)
    {
        var start = new ProcessStartInfo(path) { UseShellExecute = false, WindowStyle = ProcessWindowStyle.Hidden };
        foreach (var arg in args) start.ArgumentList.Add(arg);
        return Process.Start(start) ?? throw new InvalidOperationException("Process startup failed.");
    }
    private static void Check(string name, bool passed)
    {
        Results.Add(new { Test = name, Passed = passed });
        Console.WriteLine($"{(passed ? "PASS" : "FAIL")} {name}");
        if (!passed) throw new InvalidOperationException(name);
    }
    private static long Query(nint target, int counter) => Win.SendMessage(target, 0x8100 + counter, 0, 0).ToInt64();
    private static void Move(NativeMethods.Point point)
    {
        if (!Win.SetPhysicalCursorPos(point.X, point.Y)) throw new InvalidOperationException("Cannot move pointer in current desktop.");
    }
    private static void Input(uint flags, uint data = 0)
    {
        var input = new Win.Input { Mouse = new Win.MouseInput { Flags = flags, Data = data } };
        if (Win.SendInput(1, new[] { input }, Marshal.SizeOf<Win.Input>()) != 1)
            throw new InvalidOperationException("SendInput blocked in current desktop.");
    }
    private static async Task WaitFor(Func<bool> predicate)
    {
        var clock = Stopwatch.StartNew();
        while (!predicate())
        {
            if (clock.Elapsed.TotalSeconds > 8) throw new TimeoutException("Desktop condition did not become true.");
            await Task.Delay(50);
        }
    }
    private static nint FindWindow(int processId, string title)
    {
        nint found = 0;
        Win.EnumWindows((window, _) =>
        {
            Win.GetWindowThreadProcessId(window, out var pid);
            if (pid != processId) return true;
            var text = new StringBuilder(256);
            Win.GetWindowText(window, text, text.Capacity);
            if (text.ToString() != title) return true;
            found = window;
            return false;
        }, 0);
        return found;
    }
    private static string FindProjectRoot()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory is not null && !File.Exists(Path.Combine(directory.FullName, "ShakeSpot.slnx"))) directory = directory.Parent;
        return directory?.FullName ?? throw new DirectoryNotFoundException("ShakeSpot project root not found.");
    }
    private static void Capture(NativeMethods.Rect rect, string name)
    {
        using var bitmap = new Bitmap(rect.Right - rect.Left, rect.Bottom - rect.Top);
        using var graphics = Graphics.FromImage(bitmap);
        graphics.CopyFromScreen(rect.Left, rect.Top, 0, 0, bitmap.Size);
        bitmap.Save(Path.Combine(_artifacts, name), ImageFormat.Png);
    }
}

internal sealed class InputTarget : Form
{
    private int _clicks, _wheels, _drags;
    private bool _cursorHidden;
    private int _cursorCount;
    internal InputTarget(string title)
    {
        Text = title;
        ClientSize = new Size(680, 420);
        StartPosition = FormStartPosition.CenterScreen;
        TopMost = true;
        BackColor = Color.FromArgb(239, 244, 250);
        Font = new Font("Segoe UI", 13);
        SetStyle(ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer, true);
    }
    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        e.Graphics.DrawString("ShakeSpot · Desktop verification", Font, Brushes.MidnightBlue, 26, 24);
        e.Graphics.DrawString("Click / scroll / drag target (separate process)", Font, Brushes.DimGray, 26, 60);
        using var pen = new Pen(Color.FromArgb(205, 215, 230));
        for (var x = 25; x < Width; x += 30) e.Graphics.DrawLine(pen, x, 120, x, Height);
        for (var y = 120; y < Height; y += 30) e.Graphics.DrawLine(pen, 0, y, Width, y);
    }
    protected override void WndProc(ref Message message)
    {
        if (message.Msg == 0x201) _clicks++;
        if (message.Msg == 0x20A) _wheels++;
        if (message.Msg == 0x200 && (message.WParam.ToInt64() & 1) != 0) _drags++;
        if (message.Msg is >= 0x8100 and <= 0x8104)
        {
            if (message.Msg == 0x8103 && !_cursorHidden) { _cursorCount = Win.ShowCursor(false); _cursorHidden = true; }
            if (message.Msg == 0x8104 && _cursorHidden)
            {
                _cursorCount = Win.ShowCursor(true);
                Cursor.Current = Cursors.Default;
                _cursorHidden = false;
            }
            message.Result = new nint(message.Msg switch { 0x8100 => _clicks, 0x8101 => _wheels, 0x8102 => _drags, _ => _cursorCount });
            return;
        }
        base.WndProc(ref message);
    }
    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        if (_cursorHidden) Cursor.Show();
        base.OnFormClosed(e);
    }
}

internal static class Win
{
    [DllImport("user32.dll")] internal static extern int ShowCursor([MarshalAs(UnmanagedType.Bool)] bool show);
    [DllImport("kernel32.dll", SetLastError = true)] [return: MarshalAs(UnmanagedType.Bool)]
    internal static extern bool GetProcessTimes(nint process, out long created, out long exited, out long kernel, out long user);
    [StructLayout(LayoutKind.Sequential)] internal struct MouseInput { public int X, Y; public uint Data, Flags, Time; public nuint Extra; }
    [StructLayout(LayoutKind.Explicit, Size = 40)] internal struct Input { [FieldOffset(0)] public uint Type; [FieldOffset(8)] public MouseInput Mouse; }
    internal delegate bool EnumCallback(nint window, nint parameter);
    [DllImport("user32.dll")] internal static extern nint GetForegroundWindow();
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] internal static extern bool SetForegroundWindow(nint window);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] internal static extern nint FindWindow(string? className, string title);
    [DllImport("user32.dll")] internal static extern uint GetWindowThreadProcessId(nint window, out uint processId);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] internal static extern bool GetWindowRect(nint window, out NativeMethods.Rect rect);
    [DllImport("user32.dll")] internal static extern nint WindowFromPoint(NativeMethods.Point point);
    [DllImport("user32.dll")] internal static extern nint SendMessage(nint window, int message, nint wParam, nint lParam);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] internal static extern bool PostMessage(nint window, int message, nint wParam, nint lParam);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] internal static extern bool IsWindow(nint window);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] internal static extern bool IsWindowVisible(nint window);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] internal static extern bool SetPhysicalCursorPos(int x, int y);
    [DllImport("user32.dll", SetLastError = true)] internal static extern uint SendInput(uint count, Input[] inputs, int size);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] internal static extern bool EnumWindows(EnumCallback callback, nint parameter);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] internal static extern int GetWindowText(nint window, StringBuilder text, int length);
}
