using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Windows;
using System.Windows.Threading;
using Microsoft.Win32;
using ShakeSpot.Core;
using Forms = System.Windows.Forms;

namespace ShakeSpot;

public partial class App : Application
{
    private Mutex? _mutex;
    private bool _ownsMutex;
    private SettingsStore? _store;
    private AppSettings _settings = new();
    private LocatorController? _controller;
    private ControlWindow? _controlWindow;
    private Forms.NotifyIcon? _tray;
    private Forms.ContextMenuStrip? _menu;
    private Forms.ToolStripMenuItem? _toggle;
    private Icon? _enabledIcon, _pausedIcon;
    private SettingsWindow? _settingsWindow;
    private bool _isolated;
    private string _directory = "";
    private string? _diagnostics;
    private readonly Stopwatch _uptime = Stopwatch.StartNew();
    private DispatcherTimer? _exitTimer;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        DispatcherUnhandledException += OnUnhandledException;
        try
        {
            _isolated = Argument(e.Args, "--data-dir") is not null;
            _directory = Path.GetFullPath(Argument(e.Args, "--data-dir") ??
                Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "ShakeSpot"));
            _diagnostics = Argument(e.Args, "--diagnostics");
            var instance = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(_directory.ToUpperInvariant())))[..16];
            var controlTitle = "ShakeSpot.Control." + instance;
            var command = e.Args.Contains("--exit") ? ControlWindow.Exit :
                e.Args.Contains("--pause") ? ControlWindow.Pause :
                e.Args.Contains("--resume") ? ControlWindow.Resume :
                e.Args.Contains("--preview") ? ControlWindow.Preview : ControlWindow.Settings;
            if (command != ControlWindow.Settings)
            {
                Shutdown(ControlWindow.Send(controlTitle, command) ? 0 : 2);
                return;
            }
            _mutex = new Mutex(true, "Local\\ShakeSpot." + instance, out _ownsMutex);
            if (!_ownsMutex)
            {
                ControlWindow.Send(controlTitle, ControlWindow.Settings);
                Shutdown();
                return;
            }
            _store = new SettingsStore(_directory);
            _settings = _store.Load(out var warning);
            if (!_isolated)
            {
                try { _settings = _settings with { StartWithWindows = StartupRegistration.IsEnabled() }; }
                catch (Exception ex) { warning = "Cannot read startup registration: " + ex.Message; }
            }
            else _settings = _settings with { StartWithWindows = false };
            _controller = new LocatorController(_settings);
            _controller.Failed += ControllerFailed;
            _controlWindow = new ControlWindow(controlTitle, HandleCommand);
            CreateTray();
            SystemEvents.SessionSwitch += SessionSwitch;
            SystemEvents.PowerModeChanged += PowerModeChanged;
            if (warning is not null)
            {
                Log(new InvalidOperationException(warning));
                _tray!.ShowBalloonTip(5000, "ShakeSpot", "配置读取异常，已使用可用的默认值。可打开设置重新保存。", Forms.ToolTipIcon.Warning);
            }
            if (e.Args.Contains("--settings")) OpenSettings();
            if (double.TryParse(Argument(e.Args, "--exit-after"), out var seconds) && seconds > 0)
            {
                _exitTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(seconds) };
                _exitTimer.Tick += (_, _) => Shutdown();
                _exitTimer.Start();
            }
        }
        catch (Exception ex)
        {
            Log(ex);
            MessageBox.Show("ShakeSpot 启动失败：" + ex.Message, "ShakeSpot", MessageBoxButton.OK, MessageBoxImage.Error);
            Shutdown(1);
        }
    }

    private static string? Argument(string[] args, string name)
    {
        var index = Array.IndexOf(args, name);
        return index >= 0 && index + 1 < args.Length ? args[index + 1] : null;
    }

    private void CreateTray()
    {
        _enabledIcon = ArrowArtwork.TrayIcon(true);
        _pausedIcon = ArrowArtwork.TrayIcon(false);
        _menu = new Forms.ContextMenuStrip();
        _toggle = new Forms.ToolStripMenuItem();
        _toggle.Click += (_, _) => SetEnabled(!_settings.Enabled);
        _menu.Items.Add(_toggle);
        _menu.Items.Add("设置…", null, (_, _) => OpenSettings());
        _menu.Items.Add("预览效果", null, (_, _) => _controller?.Preview());
        _menu.Items.Add(new Forms.ToolStripSeparator());
        _menu.Items.Add("退出", null, (_, _) => Shutdown());
        _tray = new Forms.NotifyIcon { ContextMenuStrip = _menu, Visible = true };
        _tray.DoubleClick += (_, _) => OpenSettings();
        RefreshTray();
    }

    private void RefreshTray()
    {
        if (_tray is null || _toggle is null) return;
        _tray.Icon = _settings.Enabled ? _enabledIcon : _pausedIcon;
        _tray.Text = _settings.Enabled ? "ShakeSpot · 晃动鼠标以定位" : "ShakeSpot · 已暂停";
        _toggle.Text = _settings.Enabled ? "暂停效果" : "启用效果";
        _toggle.Checked = _settings.Enabled;
    }

    private void HandleCommand(int command)
    {
        switch (command)
        {
            case ControlWindow.Settings: OpenSettings(); break;
            case ControlWindow.Pause: SetEnabled(false); break;
            case ControlWindow.Resume: SetEnabled(true); break;
            case ControlWindow.Exit: Shutdown(); break;
            case ControlWindow.Preview: _controller?.Preview(); break;
        }
    }

    private void SetEnabled(bool enabled)
    {
        _settings = _settings with { Enabled = enabled };
        _controller?.Apply(_settings);
        RefreshTray();
        try { _store?.Save(_settings); }
        catch (Exception ex)
        {
            Log(ex);
            _tray?.ShowBalloonTip(4000, "ShakeSpot", "状态已切换，但无法保存配置：" + ex.Message, Forms.ToolTipIcon.Warning);
        }
    }

    private void OpenSettings()
    {
        if (_settingsWindow is not null)
        {
            _settingsWindow.WindowState = WindowState.Normal;
            _settingsWindow.Activate();
            return;
        }
        _settingsWindow = new SettingsWindow(_settings, SaveSettings, _isolated);
        _settingsWindow.Closed += (_, _) => _settingsWindow = null;
        _settingsWindow.Show();
        _settingsWindow.Activate();
    }

    private string? SaveSettings(AppSettings settings)
    {
        settings = settings.Normalize() with { Enabled = _settings.Enabled };
        if (_isolated) settings = settings with { StartWithWindows = false };
        try
        {
            _store!.Save(settings);
            try
            {
                if (!_isolated && settings.StartWithWindows != _settings.StartWithWindows)
                    StartupRegistration.SetEnabled(settings.StartWithWindows);
            }
            catch
            {
                _store.Save(_settings);
                throw;
            }
            _settings = settings;
            _controller?.Apply(settings);
            RefreshTray();
            return null;
        }
        catch (Exception ex) { Log(ex); return ex.Message; }
    }

    private void ControllerFailed(Exception exception)
    {
        Log(exception);
        SetEnabled(false);
        _tray?.ShowBalloonTip(5000, "ShakeSpot 已暂停", "绘制出现异常。可在托盘重新启用；详细信息已保存在配置目录。", Forms.ToolTipIcon.Warning);
    }

    private void SessionSwitch(object sender, SessionSwitchEventArgs e)
    {
        if (e.Reason is SessionSwitchReason.SessionLock or SessionSwitchReason.SessionLogoff or SessionSwitchReason.RemoteDisconnect)
            Dispatcher.BeginInvoke(() => _controller?.Suspend(true));
        else if (e.Reason is SessionSwitchReason.SessionUnlock or SessionSwitchReason.SessionLogon or SessionSwitchReason.RemoteConnect)
            Dispatcher.BeginInvoke(() => _controller?.Suspend(false));
    }

    private void PowerModeChanged(object sender, PowerModeChangedEventArgs e)
    {
        if (e.Mode == PowerModes.Suspend) Dispatcher.BeginInvoke(() => _controller?.Suspend(true));
        else if (e.Mode == PowerModes.Resume) Dispatcher.BeginInvoke(() => _controller?.Suspend(false));
    }

    private void OnUnhandledException(object sender, DispatcherUnhandledExceptionEventArgs e)
    {
        Log(e.Exception);
        e.Handled = true;
        Shutdown(1);
    }

    private void Log(Exception exception)
    {
        try
        {
            if (string.IsNullOrEmpty(_directory)) return;
            Directory.CreateDirectory(_directory);
            File.WriteAllText(Path.Combine(_directory, "last-error.log"), DateTimeOffset.Now + Environment.NewLine + exception);
        }
        catch (Exception) { /* 错误日志不可写时不能阻碍退出。 */ }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _exitTimer?.Stop();
        SystemEvents.SessionSwitch -= SessionSwitch;
        SystemEvents.PowerModeChanged -= PowerModeChanged;
        if (_controller is not null && _diagnostics is not null)
        {
            try
            {
                using var process = Process.GetCurrentProcess();
                var fullPath = Path.GetFullPath(_diagnostics);
                Directory.CreateDirectory(Path.GetDirectoryName(fullPath)!);
                File.WriteAllText(fullPath, JsonSerializer.Serialize(new
                {
                    UptimeSeconds = _uptime.Elapsed.TotalSeconds,
                    _controller.Samples, _controller.Triggers, _controller.VisibleFrames,
                    CpuSeconds = process.TotalProcessorTime.TotalSeconds,
                    process.WorkingSet64, process.PrivateMemorySize64,
                    AllocatedBytes = GC.GetTotalAllocatedBytes(), ExitCode = e.ApplicationExitCode
                }, new JsonSerializerOptions { WriteIndented = true }));
            }
            catch (Exception ex) { Log(ex); }
        }
        _controller?.Dispose();
        _controlWindow?.Dispose();
        if (_tray is not null) { _tray.Visible = false; _tray.Dispose(); }
        _menu?.Dispose();
        _enabledIcon?.Dispose();
        _pausedIcon?.Dispose();
        if (_ownsMutex) _mutex?.ReleaseMutex();
        _mutex?.Dispose();
        base.OnExit(e);
    }
}
