using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace ShakeSpot;

internal sealed class ControlWindow : NativeWindow, IDisposable
{
    internal const int Settings = 0x8001, Pause = 0x8002, Resume = 0x8003, Exit = 0x8004, Preview = 0x8005;
    private readonly Action<int> _command;
    internal ControlWindow(string title, Action<int> command)
    {
        _command = command;
        CreateHandle(new CreateParams
        {
            Caption = title, Style = NativeMethods.WsPopup,
            ExStyle = NativeMethods.WsExNoActivate | NativeMethods.WsExToolWindow
        });
    }
    protected override void WndProc(ref Message message)
    {
        if (message.Msg >= Settings && message.Msg <= Preview)
        {
            _command(message.Msg);
            return;
        }
        base.WndProc(ref message);
    }
    internal static bool Send(string title, int command)
    {
        var window = FindWindow(null, title);
        return window != 0 && PostMessage(window, command, 0, 0);
    }
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern nint FindWindow(string? className, string title);
    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool PostMessage(nint window, int message, nint wParam, nint lParam);
    public void Dispose() => DestroyHandle();
}
