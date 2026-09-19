#include "guardian.h"
#include "cursor.h"
#include "settings.h"
#include "resource.h"

using namespace shakespot;
namespace {
constexpr UINT TrayMessage = WM_APP + 1;
constexpr UINT_PTR AnimationTimer = 1, CacheTimer = 2;
constexpr int durations[]{500, 800, 1100, 1500, 2000, 3000};

class App {
    std::wstring directory_;
    Settings settings_;
    Guardian guardian_;
    Detector detector_;
    Animation animation_;
    CursorFrames frames_;
    CursorRoles roles_;
    HWND window_ = nullptr, dpiWindow_ = nullptr, dialog_ = nullptr;
    HMONITOR monitor_ = nullptr;
    UINT dpi_ = 96, taskbarMessage_ = RegisterWindowMessageW(L"TaskbarCreated");
    HICON trayIcon_ = nullptr;
    bool registered_ = false, suspended_ = false, fault_ = false, active_ = false, testMode_ = false;
    bool havePoint_ = false, flipX_ = false, flipY_ = false;
    POINT previous_{};
    double x_ = 0, y_ = 0, lastSample_ = -1e20;
    int lastHeight_ = -1, lastRole_ = -1, lastOrientation_ = -1;
    int lastStopReason_ = 0;
    unsigned long long events_ = 0, samples_ = 0, triggers_ = 0, updates_ = 0, skipped_ = 0, ticks_ = 0;

    void notify(const wchar_t* message) {
        NOTIFYICONDATAW data{sizeof(data)}; data.hWnd = window_; data.uID = 1;
        data.uFlags = NIF_INFO; data.dwInfoFlags = NIIF_INFO;
        wcscpy_s(data.szInfoTitle, L"ShakeSpot 原生版"); wcsncpy_s(data.szInfo, message, _TRUNCATE);
        Shell_NotifyIconW(NIM_MODIFY, &data);
    }
    void tray(bool add = false) {
        NOTIFYICONDATAW data{sizeof(data)}; data.hWnd = window_; data.uID = 1;
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
        data.hIcon = trayIcon_; data.uCallbackMessage = TrayMessage;
        wcscpy_s(data.szTip, settings_.enabled && !fault_ ? L"ShakeSpot 原生版 · 已启用" : L"ShakeSpot 原生版 · 已暂停");
        if (Shell_NotifyIconW(add ? NIM_ADD : NIM_MODIFY, &data) && add) {
            data.uVersion = NOTIFYICON_VERSION_4; Shell_NotifyIconW(NIM_SETVERSION, &data);
        }
    }
    void inputRegistration() {
        bool enable = settings_.enabled && !suspended_ && !fault_ && !dialog_;
        if (registered_ == enable) return;
        RAWINPUTDEVICE device{1, 2, static_cast<DWORD>(enable ? RIDEV_INPUTSINK : RIDEV_REMOVE), enable ? window_ : nullptr};
        if (!RegisterRawInputDevices(&device, 1, sizeof(device))) throw std::runtime_error("Raw Input registration failed.");
        registered_ = enable;
        detector_.reset(); havePoint_ = false;
    }
    void cancel() {
        animation_.reset(); active_ = false;
        if (window_) KillTimer(window_, AnimationTimer);
        if (!guardian_.restore()) throw std::runtime_error("System cursor restoration failed; recovery process remains active.");
        roles_.refresh(); lastHeight_ = lastRole_ = lastOrientation_ = -1;
        if (window_) SetTimer(window_, CacheTimer, 3000, nullptr);
    }
    void fail(const std::string& reason) noexcept {
        logError(directory_, reason); fault_ = true;
        try { cancel(); inputRegistration(); tray(); notify(L"效果已暂停。请退出并重新启动；详情见配置目录中的 native-error.log。"); }
        catch (...) { PostQuitMessage(3); }
    }
    UINT pointDpi(POINT point) {
        HMONITOR monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        if (monitor != monitor_) {
            monitor_ = monitor;
            SetWindowPos(dpiWindow_, nullptr, point.x, point.y, 1, 1, SWP_NOACTIVATE | SWP_NOZORDER);
            dpi_ = GetDpiForWindow(dpiWindow_); if (!dpi_) dpi_ = 96;
        }
        return dpi_;
    }
    bool visible(CURSORINFO& info) { return cursorInfo(info) && (info.flags & CURSOR_SHOWING) && !(info.flags & CURSOR_SUPPRESSED); }
    void trigger() {
        if (!settings_.enabled || suspended_ || fault_ || dialog_ || buttonsDown()) return;
        CURSORINFO info{};
        if (!visible(info) || roles_.identify(info.hCursor) < 0) { ++skipped_; return; }
        ++triggers_; animation_.trigger(clockMs(), settings_.maximumScale, settings_.durationMs);
        if (!active_) {
            active_ = true; KillTimer(window_, CacheTimer);
            if (!SetTimer(window_, AnimationTimer, 15, nullptr)) throw std::runtime_error("Animation timer creation failed.");
        }
        tick();
    }
    void tick() {
        ++ticks_; guardian_.heartbeat();
        CURSORINFO info{};
        if (!visible(info) || buttonsDown()) { lastStopReason_ = 1; cancel(); detector_.reset(); return; }
        const auto frame = animation_.frame(clockMs());
        if (!frame.visible) { lastStopReason_ = 2; cancel(); return; }
        const int role = roles_.identify(info.hCursor);
        if (role < 0) { lastStopReason_ = 3; ++skipped_; cancel(); detector_.reset(); return; }
        POINT point{};
        if (!GetPhysicalCursorPos(&point)) { cancel(); return; }
        double dpiScale = pointDpi(point) / 96.0;
        int height = std::clamp(static_cast<int>(std::lround(35 * dpiScale * frame.scale)), 24, 256);
        height = (height + 1) / 2 * 2;
        MONITORINFO monitor{sizeof(monitor)};
        if (!GetMonitorInfoW(monitor_, &monitor)) { cancel(); return; }
        // 使用最大尺寸决定朝向，避免缩放过程中在同一个边缘反复翻转。
        int maximumHeight = std::min(256, static_cast<int>(std::lround(35 * dpiScale * settings_.maximumScale)));
        int maximumWidth = static_cast<int>(std::ceil(26 * (maximumHeight - 4) / 35.0)) + 4;
        int hysteresis = static_cast<int>(24 * dpiScale);
        flipX_ = flipAtEdge(point.x - monitor.rcMonitor.left, monitor.rcMonitor.right - 1 - point.x, maximumWidth, flipX_, hysteresis);
        flipY_ = flipAtEdge(point.y - monitor.rcMonitor.top, monitor.rcMonitor.bottom - 1 - point.y, maximumHeight, flipY_, hysteresis);
        int orientation = (flipX_ ? 1 : 0) | (flipY_ ? 2 : 0);
        if (height == lastHeight_ && role == lastRole_ && orientation == lastOrientation_) return;
        HCURSOR source = frames_.get(height, flipX_, flipY_);
        if (!source || !guardian_.begin()) throw std::runtime_error("Cursor frame or recovery process unavailable.");
        // SetSystemCursor 消耗传入句柄；缓存始终保留独立副本。
        HCURSOR copy = static_cast<HCURSOR>(CopyImage(source, IMAGE_CURSOR, 0, 0, 0));
        if (!copy) throw std::runtime_error("Cursor frame copy failed.");
        if (!SetSystemCursor(copy, CursorIds[role])) throw std::runtime_error("SetSystemCursor failed.");
        ++updates_; lastHeight_ = height; lastRole_ = role; lastOrientation_ = orientation;
    }
    void rawInput(HRAWINPUT handle) {
        RAWINPUT input{}; UINT bytes = sizeof(input);
        if (GetRawInputData(handle, RID_INPUT, &input, &bytes, sizeof(RAWINPUTHEADER)) == UINT(-1) || input.header.dwType != RIM_TYPEMOUSE) return;
        if (!registered_) return;
        ++events_;
        double now = clockMs();
        constexpr USHORT presses = RI_MOUSE_LEFT_BUTTON_DOWN | RI_MOUSE_RIGHT_BUTTON_DOWN | RI_MOUSE_MIDDLE_BUTTON_DOWN | RI_MOUSE_BUTTON_4_DOWN | RI_MOUSE_BUTTON_5_DOWN;
        if ((input.data.mouse.usButtonFlags & presses) || buttonsDown()) {
            if (active_) cancel();
            detector_.add(now, x_, y_, true); havePoint_ = false; return;
        }
        if (now - lastSample_ < 8) return;
        lastSample_ = now;
        CURSORINFO info{}; POINT point{};
        if (!visible(info) || !GetPhysicalCursorPos(&point)) {
            if (active_) cancel(); detector_.reset(); havePoint_ = false; return;
        }
        double dpiScale = pointDpi(point) / 96.0;
        if (havePoint_) { x_ += (point.x - previous_.x) / dpiScale; y_ += (point.y - previous_.y) / dpiScale; }
        previous_ = point; havePoint_ = true; ++samples_;
        if (detector_.add(now, x_, y_)) trigger();
    }
    void setEnabled(bool value) {
        if (value && fault_) { notify(L"恢复进程不可用，请退出后重新启动。"); return; }
        cancel(); settings_.enabled = value; inputRegistration(); tray();
        if (!saveSettings(directory_, settings_)) notify(L"无法保存配置，请检查配置目录权限。");
    }
    void populateDialog(HWND dialog, const Settings& settings) {
        SendDlgItemMessageW(dialog, IDC_SENSITIVITY, CB_SETCURSEL, settings.sensitivity - 1, 0);
        SendDlgItemMessageW(dialog, IDC_SCALE, CB_SETCURSEL, static_cast<WPARAM>(std::lround((settings.maximumScale - 2) * 2)), 0);
        auto closest = std::min_element(std::begin(durations), std::end(durations), [&](int a, int b) { return std::abs(a - settings.durationMs) < std::abs(b - settings.durationMs); });
        SendDlgItemMessageW(dialog, IDC_DURATION, CB_SETCURSEL, closest - std::begin(durations), 0);
        CheckDlgButton(dialog, IDC_STARTUP, settings.startup ? BST_CHECKED : BST_UNCHECKED);
    }
    static INT_PTR CALLBACK dialogProc(HWND dialog, UINT message, WPARAM w, LPARAM l) {
        auto* app = reinterpret_cast<App*>(GetWindowLongPtrW(dialog, DWLP_USER));
        if (message == WM_INITDIALOG) {
            app = reinterpret_cast<App*>(l); SetWindowLongPtrW(dialog, DWLP_USER, l);
            for (const wchar_t* text : {L"1 · 不易触发", L"2", L"3 · 默认", L"4", L"5 · 容易触发"})
                SendDlgItemMessageW(dialog, IDC_SENSITIVITY, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text));
            for (int i = 4; i <= 16; ++i) {
                wchar_t text[24]{}; swprintf_s(text, L"%.1f 倍", i / 2.0);
                SendDlgItemMessageW(dialog, IDC_SCALE, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text));
            }
            for (int duration : durations) {
                wchar_t text[24]{}; swprintf_s(text, L"%.1f 秒", duration / 1000.0);
                SendDlgItemMessageW(dialog, IDC_DURATION, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text));
            }
            app->settings_.startup = startupEnabled(); app->populateDialog(dialog, app->settings_);
            if (app->testMode_) EnableWindow(GetDlgItem(dialog, IDC_STARTUP), FALSE);
            return TRUE;
        }
        if (!app) return FALSE;
        try {
            if (message == WM_COMMAND) {
                switch (LOWORD(w)) {
                case IDC_DEFAULTS: app->populateDialog(dialog, Settings{}); return TRUE;
                case IDOK: {
                    Settings updated = app->settings_;
                    updated.sensitivity = static_cast<int>(SendDlgItemMessageW(dialog, IDC_SENSITIVITY, CB_GETCURSEL, 0, 0)) + 1;
                    updated.maximumScale = 2 + SendDlgItemMessageW(dialog, IDC_SCALE, CB_GETCURSEL, 0, 0) / 2.0;
                    int index = static_cast<int>(SendDlgItemMessageW(dialog, IDC_DURATION, CB_GETCURSEL, 0, 0));
                    updated.durationMs = durations[std::clamp(index, 0, 5)];
                    updated.startup = IsDlgButtonChecked(dialog, IDC_STARTUP) == BST_CHECKED;
                    updated.normalize();
                    if (app->testMode_) updated.startup = startupEnabled();
                    if (updated.startup != startupEnabled() && !setStartup(updated.startup)) {
                        SetDlgItemTextW(dialog, IDC_STATUS, L"开机启动设置失败，请重试。"); return TRUE;
                    }
                    if (!saveSettings(app->directory_, updated)) {
                        SetDlgItemTextW(dialog, IDC_STATUS, L"保存失败，请检查配置目录权限。"); return TRUE;
                    }
                    app->settings_ = updated; app->detector_.sensitivity(updated.sensitivity);
                    DestroyWindow(dialog); return TRUE;
                }
                case IDCANCEL: DestroyWindow(dialog); return TRUE;
                }
            }
            if (message == WM_CLOSE) { DestroyWindow(dialog); return TRUE; }
            if (message == WM_DESTROY) { app->dialog_ = nullptr; app->inputRegistration(); return TRUE; }
        } catch (const std::exception& error) { app->fail(error.what()); }
        return FALSE;
    }
    void openSettings() {
        if (!dialog_) {
            cancel();
            dialog_ = CreateDialogParamW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(IDD_SETTINGS), nullptr, dialogProc, reinterpret_cast<LPARAM>(this));
            if (!dialog_) throw std::runtime_error("Settings dialog creation failed.");
            inputRegistration();
        }
        ShowWindow(dialog_, SW_SHOWNORMAL);
        // 托盘进程可能以 SW_HIDE 启动，第一次 ShowWindow 会使用启动参数。
        if (!IsWindowVisible(dialog_)) ShowWindow(dialog_, SW_SHOWNORMAL);
        SetForegroundWindow(dialog_);
    }
    void menu() {
        cancel();
        HMENU menu = CreatePopupMenu();
        AppendMenuW(menu, MF_STRING, settings_.enabled ? Pause : Resume, settings_.enabled ? L"暂停效果" : L"启用效果");
        AppendMenuW(menu, MF_STRING, OpenSettings, L"设置…");
        AppendMenuW(menu, MF_STRING, Preview, L"预览放大效果");
        AppendMenuW(menu, MF_STRING, Restore, L"恢复原系统指针并暂停");
        AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
        AppendMenuW(menu, MF_STRING, Quit, L"退出");
        POINT point{}; GetCursorPos(&point); SetForegroundWindow(window_);
        UINT result = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, point.x, point.y, 0, window_, nullptr);
        DestroyMenu(menu); PostMessageW(window_, WM_NULL, 0, 0);
        if (result) command(result);
    }
    void command(UINT value) {
        switch (value) {
        case OpenSettings: openSettings(); break;
        case Pause: setEnabled(false); break;
        case Resume: setEnabled(true); break;
        case Preview: trigger(); break;
        case Quit: DestroyWindow(window_); break;
        case Restore: setEnabled(false); if (!reloadCursors()) fail("Explicit cursor restoration failed."); roles_.refresh(); break;
        case TestHang: if (testMode_ && guardian_.dirty()) Sleep(6000); break;
        }
    }
    LRESULT message(UINT message, WPARAM w, LPARAM l) {
        if (message == taskbarMessage_) { tray(true); return 0; }
        switch (message) {
        case WM_INPUT: rawInput(reinterpret_cast<HRAWINPUT>(l)); return DefWindowProcW(window_, message, w, l);
        case WM_TIMER:
            if (w == AnimationTimer) tick();
            if (w == CacheTimer) { KillTimer(window_, CacheTimer); frames_.clear(); }
            return 0;
        case CommandMessage: command(static_cast<UINT>(w)); return 0;
        case QueryMessage:
            switch (w) {
            case 1: return guardian_.dirty(); case 2: return static_cast<LRESULT>(updates_);
            case 3: return static_cast<LRESULT>(triggers_); case 4: return guardian_.processId();
            case 5: return static_cast<LRESULT>(samples_); case 6: return settings_.enabled && !fault_;
            case 7: return static_cast<LRESULT>(skipped_); case 8: return static_cast<LRESULT>(ticks_);
            case 9: return static_cast<LRESULT>(events_); case 10: return reinterpret_cast<LRESULT>(dialog_);
            case 11: return lastStopReason_;
            case 12: return lastRole_;
            } return 0;
        case TrayMessage:
            if (LOWORD(l) == WM_CONTEXTMENU) menu();
            else if (LOWORD(l) == WM_LBUTTONDBLCLK || LOWORD(l) == NIN_KEYSELECT) openSettings();
            return 0;
        case WM_WTSSESSION_CHANGE:
            if (w == WTS_SESSION_LOCK || w == WTS_CONSOLE_DISCONNECT || w == WTS_REMOTE_DISCONNECT) suspended_ = true;
            if (w == WTS_SESSION_UNLOCK || w == WTS_CONSOLE_CONNECT || w == WTS_REMOTE_CONNECT) suspended_ = false;
            cancel(); inputRegistration(); return 0;
        case WM_POWERBROADCAST:
            if (w == PBT_APMSUSPEND) { suspended_ = true; cancel(); inputRegistration(); }
            if (w == PBT_APMRESUMEAUTOMATIC) { suspended_ = false; inputRegistration(); }
            return TRUE;
        case WM_SETTINGCHANGE: case WM_DISPLAYCHANGE: lastStopReason_ = 4; monitor_ = nullptr; cancel(); detector_.reset(); return 0;
        case WM_QUERYENDSESSION: cancel(); return TRUE;
        case WM_CLOSE: DestroyWindow(window_); return 0;
        case WM_DESTROY:
            cancel(); suspended_ = true; inputRegistration();
            if (dialog_) DestroyWindow(dialog_);
            { NOTIFYICONDATAW data{sizeof(data)}; data.hWnd = window_; data.uID = 1; Shell_NotifyIconW(NIM_DELETE, &data); }
            WTSUnRegisterSessionNotification(window_); PostQuitMessage(0); return 0;
        }
        return DefWindowProcW(window_, message, w, l);
    }
    static LRESULT CALLBACK windowProc(HWND window, UINT message, WPARAM w, LPARAM l) {
        auto* app = reinterpret_cast<App*>(GetWindowLongPtrW(window, GWLP_USERDATA));
        if (message == WM_NCCREATE) {
            app = static_cast<App*>(reinterpret_cast<CREATESTRUCTW*>(l)->lpCreateParams);
            app->window_ = window; SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(app));
        }
        if (app) { try { return app->message(message, w, l); } catch (const std::exception& error) { app->fail(error.what()); return 0; } }
        return DefWindowProcW(window, message, w, l);
    }
public:
    App(std::wstring directory, bool testMode) : directory_(std::move(directory)), settings_(loadSettings(directory_)), guardian_(directory_), testMode_(testMode) {
        detector_.sensitivity(settings_.sensitivity);
    }
    ~App() {
        if (window_ && IsWindow(window_)) DestroyWindow(window_);
        if (dpiWindow_) DestroyWindow(dpiWindow_);
        if (trayIcon_) DestroyIcon(trayIcon_);
    }
    int run(bool settingsOnStart) {
        INITCOMMONCONTROLSEX controls{sizeof(controls), ICC_STANDARD_CLASSES}; InitCommonControlsEx(&controls);
        WNDCLASSW type{}; type.hInstance = GetModuleHandleW(nullptr); type.lpszClassName = WindowClass;
        type.lpfnWndProc = windowProc; type.hCursor = LoadCursorW(nullptr, IDC_ARROW);
        if (!RegisterClassW(&type)) throw std::runtime_error("Window class registration failed.");
        dpiWindow_ = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, L"STATIC", L"ShakeSpot DPI", WS_POPUP, 0, 0, 1, 1, nullptr, nullptr, type.hInstance, nullptr);
        if (!dpiWindow_ || !CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, WindowClass, L"ShakeSpot Native", WS_POPUP, 0, 0, 0, 0, nullptr, nullptr, type.hInstance, this)) throw std::runtime_error("Control window creation failed.");
        trayIcon_ = reinterpret_cast<HICON>(makeArrow(32, false, false, true));
        if (!trayIcon_) throw std::runtime_error("Tray icon creation failed.");
        tray(true); inputRegistration(); WTSRegisterSessionNotification(window_, NOTIFY_FOR_THIS_SESSION);
        if (settingsOnStart) PostMessageW(window_, CommandMessage, OpenSettings, 0);
        bool watchGuardian = true;
        for (;;) {
            HANDLE guard = guardian_.process();
            DWORD wait = MsgWaitForMultipleObjectsEx(watchGuardian ? 1 : 0, &guard, INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            if (watchGuardian && wait == WAIT_OBJECT_0) { watchGuardian = false; fail("Recovery process exited; effects disabled."); }
            if (wait == WAIT_FAILED) throw std::runtime_error("Message wait failed.");
            MSG message{};
            while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
                if (message.message == WM_QUIT) return static_cast<int>(message.wParam);
                if (!dialog_ || !IsDialogMessageW(dialog_, &message)) { TranslateMessage(&message); DispatchMessageW(&message); }
            }
        }
    }
};
}

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR, int) {
    int count = 0; LPWSTR* argv = CommandLineToArgvW(GetCommandLineW(), &count);
    std::vector<std::wstring> arguments; for (int i = 1; argv && i < count; ++i) arguments.emplace_back(argv[i]);
    if (argv) LocalFree(argv);
    std::wstring directory;
    try {
        if (arguments.size() == 6 && arguments[0] == L"--guardian") {
            auto handle = [&](int i) { return reinterpret_cast<HANDLE>(static_cast<uintptr_t>(std::stoull(arguments[i]))); };
            return runGuardian(handle(1), handle(2), handle(3), handle(4), arguments[5]);
        }
        directory = profilePath(); bool testMode = false; UINT command = 0;
        for (size_t i = 0; i < arguments.size(); ++i) {
            const auto& argument = arguments[i];
            if (argument == L"--data-dir" && i + 1 < arguments.size()) directory = arguments[++i];
            else if (argument == L"--test-mode") testMode = true;
            else if (argument == L"--settings") command = OpenSettings;
            else if (argument == L"--pause") command = Pause;
            else if (argument == L"--resume") command = Resume;
            else if (argument == L"--preview") command = Preview;
            else if (argument == L"--exit") command = Quit;
            else if (argument == L"--restore") command = Restore;
            else throw std::runtime_error("Unknown or incomplete argument.");
        }
        if (HWND existing = FindWindowW(WindowClass, nullptr)) {
            PostMessageW(existing, CommandMessage, command ? command : OpenSettings, 0); return 0;
        }
        Handle mutex(CreateMutexW(nullptr, FALSE, MutexName));
        if (!mutex.get() || GetLastError() == ERROR_ALREADY_EXISTS) return 2;
        if (command == Restore) {
            if (!reloadCursors()) return 3;
            DeleteFileW((directory + L"\\native-recovery.flag").c_str()); return 0;
        }
        if (command && command != OpenSettings) return 2;
        App app(directory, testMode);
        return app.run(command == OpenSettings);
    } catch (const std::exception& error) {
        logError(directory, error.what());
        MessageBoxW(nullptr, L"ShakeSpot 无法启动。请查看配置目录中的 native-error.log。", L"ShakeSpot 原生版", MB_OK | MB_ICONERROR);
        return 3;
    }
}
