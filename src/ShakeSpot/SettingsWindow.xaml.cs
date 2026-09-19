using System;
using System.Windows;
using ShakeSpot.Core;

namespace ShakeSpot;

public partial class SettingsWindow : Window
{
    private readonly Func<AppSettings, string?> _save;
    private readonly AppSettings _original;
    internal SettingsWindow(AppSettings settings, Func<AppSettings, string?> save, bool isolated)
    {
        _original = settings;
        _save = save;
        InitializeComponent();
        Populate(settings);
        StartupCheckBox.IsEnabled = !isolated;
        if (isolated) StatusText.Text = "当前使用独立测试配置，开机启动不可用。";
    }
    private void Populate(AppSettings settings)
    {
        SensitivitySlider.Value = settings.Sensitivity;
        ScaleSlider.Value = settings.MaximumScale;
        DurationSlider.Value = settings.DurationMs;
        StartupCheckBox.IsChecked = settings.StartWithWindows;
        UpdateLabels();
    }
    private void ValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e) => UpdateLabels();
    private void UpdateLabels()
    {
        if (DurationValue is null || DurationSlider is null || ScaleSlider is null) return;
        SensitivityValue.Text = $"{SensitivitySlider.Value:0} / 5";
        ScaleValue.Text = $"{ScaleSlider.Value:0.0} 倍";
        DurationValue.Text = $"{DurationSlider.Value / 1000:0.0} 秒";
    }
    private void DefaultsClick(object sender, RoutedEventArgs e) => Populate(new AppSettings());
    private void CancelClick(object sender, RoutedEventArgs e) => Close();
    private void SaveClick(object sender, RoutedEventArgs e)
    {
        var error = _save(_original with
        {
            Sensitivity = (int)SensitivitySlider.Value,
            MaximumScale = ScaleSlider.Value,
            DurationMs = (int)DurationSlider.Value,
            StartWithWindows = StartupCheckBox.IsChecked == true
        });
        if (error is null) Close();
        else StatusText.Text = "保存失败：" + error;
    }
}
