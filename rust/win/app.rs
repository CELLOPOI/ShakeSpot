use super::{
    config,
    cursor::*,
    foreground,
    guardian::Guardian,
    platform::*,
    ui::{self, *},
};
use shakespot::{
    core::{Animation, Settings, flip_at_edge},
    input::{CoordinateMode, MotionTracker, RawMotion, SAMPLE_INTERVAL_MS},
    settings,
};
use std::{
    path::PathBuf,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{RemoteDesktop::*, Threading::*},
    UI::{HiDpi::GetDpiForWindow, Input::*, Shell::*, WindowsAndMessaging::*},
};

pub struct App {
    directory: PathBuf,
    settings: Settings,
    guardian: Guardian,
    detector: MotionTracker,
    animation: Animation,
    frames: CursorFrames,
    roles: CursorRoles,
    window: HWND,
    dpi_window: HWND,
    dialog: HWND,
    monitor: HMONITOR,
    dpi: u32,
    bounds: RECT,
    tray_icon: Option<Cursor>,
    registered: bool,
    foreground_watcher: Option<foreground::Watcher>,
    foreground_window: HWND,
    context_blocked: bool,
    input_since: f64,
    suspended: bool,
    fault: bool,
    active: bool,
    test_mode: bool,
    running: bool,
    last_sample: f64,
    input_visible: bool,
    buttons_held: bool,
    flip_x: bool,
    flip_y: bool,
    last_frame: Option<(i32, usize, u32)>,
    timer_delay: u32,
    stop_reason: isize,
    events: isize,
    samples: isize,
    triggers: isize,
    updates: isize,
    skipped: isize,
    ticks: isize,
}

impl App {
    pub fn new(directory: PathBuf, test_mode: bool) -> Result<Self> {
        let settings = settings::load(&directory.join(SETTINGS_FILE))?;
        let guardian = Guardian::new(&directory)?;
        let mut detector = MotionTracker::default();
        detector.set_sensitivity(settings.sensitivity);
        Ok(Self {
            directory,
            settings,
            guardian,
            detector,
            animation: Animation::default(),
            frames: CursorFrames::default(),
            roles: CursorRoles::default(),
            window: null_mut(),
            dpi_window: null_mut(),
            dialog: null_mut(),
            monitor: null_mut(),
            dpi: 96,
            bounds: RECT::default(),
            tray_icon: None,
            registered: false,
            foreground_watcher: None,
            foreground_window: null_mut(),
            context_blocked: false,
            input_since: -1e20,
            suspended: false,
            fault: false,
            active: false,
            test_mode,
            running: true,
            input_visible: false,
            buttons_held: false,
            last_sample: -1e20,
            flip_x: false,
            flip_y: false,
            last_frame: None,
            timer_delay: 0,
            stop_reason: 0,
            events: 0,
            samples: 0,
            triggers: 0,
            updates: 0,
            skipped: 0,
            ticks: 0,
        })
    }
    fn snapshot(&self) {
        let mut value = [0; 16];
        value[0] = isize::from(self.context_blocked);
        value[1] = isize::from(self.guardian.dirty());
        value[2] = self.updates;
        value[3] = self.triggers;
        value[4] = self.guardian.id() as isize;
        value[5] = self.samples;
        value[6] = isize::from(self.settings.enabled && !self.fault);
        value[7] = self.skipped;
        value[8] = self.ticks;
        value[9] = self.events;
        value[10] = self.dialog as isize;
        value[11] = self.stop_reason;
        value[12] = self.last_frame.map_or(-1, |(_, role, _)| role as isize);
        value[13] = self.frames.len() as isize;
        value[14] = self.frames.bytes() as isize;
        value[15] = self.timer_delay as isize;
        ui::snapshot(value);
    }
    fn tray(&self, add: bool) {
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.window,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: TRAY_MESSAGE,
            hIcon: self.tray_icon.as_ref().map_or(null_mut(), Cursor::get),
            ..Default::default()
        };
        copy_text(
            &mut data.szTip,
            if self.settings.enabled && !self.fault {
                if self.context_blocked {
                    "ShakeSpot · 当前应用已避让"
                } else {
                    "ShakeSpot · 已启用"
                }
            } else {
                "ShakeSpot · 已暂停"
            },
        );
        // SAFETY: Shell 同步复制此结构的内容，图标资源由 App 持有。
        unsafe {
            if Shell_NotifyIconW(if add { NIM_ADD } else { NIM_MODIFY }, &data) != 0 && add {
                data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
                Shell_NotifyIconW(NIM_SETVERSION, &data);
            }
        }
    }
    fn notify(&self, text: &str) {
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.window,
            uID: 1,
            uFlags: NIF_INFO,
            dwInfoFlags: NIIF_INFO,
            ..Default::default()
        };
        copy_text(&mut data.szInfoTitle, "ShakeSpot");
        copy_text(&mut data.szInfo, text);
        // SAFETY: Shell 同步复制栈上结构，不保存其地址。
        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &data);
        }
    }
    fn input_registration(&mut self) -> Result<()> {
        let enable = self.settings.enabled
            && !self.suspended
            && !self.fault
            && self.dialog.is_null()
            && !self.context_blocked;
        if enable == self.registered {
            return Ok(());
        }
        let device = RAWINPUTDEVICE {
            usUsagePage: 1,
            usUsage: 2,
            dwFlags: if enable {
                RIDEV_INPUTSINK
            } else {
                RIDEV_REMOVE
            },
            hwndTarget: if enable { self.window } else { null_mut() },
        };
        check(
            // SAFETY: 只接收 Raw Input 副本，移除时目标按 API 要求为 null。
            unsafe { RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as u32) },
            "Register Raw Input",
        )?;
        self.registered = enable;
        self.input_since = clock_ms();
        ui::accept_input(enable);
        self.detector.reset();
        self.input_visible = false;
        self.buttons_held = false;
        self.last_sample = -1e20;
        Ok(())
    }
    fn refresh_foreground(&mut self, force: bool) -> Result<bool> {
        if self.settings.excluded_apps.is_empty() && !self.context_blocked {
            self.foreground_window = null_mut();
            return Ok(false);
        }
        let window = foreground::current();
        if !force && window == self.foreground_window {
            return Ok(false);
        }
        let blocked = !self.settings.excluded_apps.is_empty()
            && foreground::executable(window)
                .is_none_or(|path| self.settings.excluded_apps.matches(&path));
        let changed = window != self.foreground_window || blocked != self.context_blocked;
        self.foreground_window = window;
        self.context_blocked = blocked;
        if changed {
            self.detector.reset();
            self.input_since = clock_ms();
            if blocked && self.active {
                self.stop_reason = 5;
                self.cancel()?;
            }
            self.input_registration()?;
            self.tray(false);
        }
        Ok(changed)
    }

    fn cancel(&mut self) -> Result<()> {
        self.animation.reset();
        self.active = false;
        self.timer_delay = 0;
        // SAFETY: KillTimer 接受有效窗口；重复删除同一 timer 没有资源所有权转移。
        unsafe {
            if !self.window.is_null() {
                KillTimer(self.window, ANIMATION_TIMER);
            }
        }
        self.guardian.restore()?;
        self.roles.refresh();
        self.last_frame = None;
        if self.frames.len() > 0 && !self.window.is_null() {
            // SAFETY: 无回调的窗口计时器；创建失败时同步释放缓存。
            if unsafe { SetTimer(self.window, CACHE_TIMER, 3000, None) } == 0 {
                self.frames.clear();
            }
        }
        Ok(())
    }
    fn fail(&mut self, error: &str) {
        log_error(&self.directory, error);
        self.fault = true;
        if self.cancel().is_err() || self.input_registration().is_err() {
            self.running = false;
        }
        self.tray(false);
        self.notify("效果已暂停。请退出后重新启动；详情见配置目录中的 rust-error.log。");
    }
    fn point_dpi(&mut self, point: POINT) -> Result<u32> {
        // SAFETY: 隐藏窗口由 App 持有；显示器边界只在切屏或系统变化时刷新。
        unsafe {
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
            if monitor != self.monitor {
                let mut info = MONITORINFO {
                    cbSize: size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                check(GetMonitorInfoW(monitor, &mut info), "Query monitor bounds")?;
                check(
                    SetWindowPos(
                        self.dpi_window,
                        null_mut(),
                        point.x,
                        point.y,
                        1,
                        1,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    ),
                    "Move DPI window",
                )?;
                self.monitor = monitor;
                self.bounds = info.rcMonitor;
                self.dpi = GetDpiForWindow(self.dpi_window).max(96);
            }
            Ok(self.dpi)
        }
    }
    fn geometry(&self, point: POINT, scale: f64) -> (i32, u32) {
        let dpi_scale = f64::from(self.dpi) / 96.0;
        let height = ((35.0 * dpi_scale * scale).round() as i32).clamp(24, 256);
        let maximum_height = (35.0 * dpi_scale * self.settings.maximum_scale)
            .round()
            .min(256.0) as i32;
        let maximum_width = (26.0 * f64::from(maximum_height - 4) / 35.0).ceil() as i32 + 4;
        let hysteresis = (24.0 * dpi_scale) as i32;
        let flip_x = flip_at_edge(
            point.x - self.bounds.left,
            self.bounds.right - 1 - point.x,
            maximum_width,
            self.flip_x,
            hysteresis,
        );
        let flip_y = flip_at_edge(
            point.y - self.bounds.top,
            self.bounds.bottom - 1 - point.y,
            maximum_height,
            self.flip_y,
            hysteresis,
        );
        (
            (height + 1) / 2 * 2,
            u32::from(flip_x) | (u32::from(flip_y) << 1),
        )
    }
    fn schedule(&mut self, now: f64) -> Result<()> {
        let delay = self.animation.timer_delay(now);
        if delay != self.timer_delay {
            // SAFETY: timer 与 App 窗口同寿命，消息中没有 Rust 指针。
            if unsafe { SetTimer(self.window, ANIMATION_TIMER, delay, None) } == 0 {
                return Err(win_error("Start animation timer"));
            }
            self.timer_delay = delay;
        }
        Ok(())
    }
    fn trigger(&mut self, from_gesture: bool) -> Result<()> {
        let context_changed = self.refresh_foreground(true)?;
        if (from_gesture && context_changed)
            || self.context_blocked
            || !self.settings.enabled
            || self.suspended
            || self.fault
            || !self.dialog.is_null()
            || buttons_down()
        {
            return Ok(());
        }
        let Some(info) = visible_cursor() else {
            self.skipped += 1;
            return Ok(());
        };
        if self.roles.identify(info.hCursor).is_none() {
            self.skipped += 1;
            return Ok(());
        }
        self.triggers += 1;
        self.animation.trigger(
            clock_ms(),
            self.settings.maximum_scale,
            self.settings.duration_ms,
        );
        self.active = true;
        // SAFETY: 取消缓存释放 timer 防止动画中释放当前帧。
        unsafe {
            KillTimer(self.window, CACHE_TIMER);
        }
        self.tick()
    }
    fn tick(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        self.refresh_foreground(false)?;
        if !self.active {
            return Ok(());
        }
        self.ticks += 1;
        self.guardian.heartbeat();
        let info = visible_cursor();
        if info.is_none() || buttons_down() {
            self.stop_reason = 1;
            self.detector.reset();
            return self.cancel();
        }
        let now = clock_ms();
        let frame = self.animation.frame(now);
        if !frame.visible {
            self.stop_reason = 2;
            return self.cancel();
        }
        let Some(role) = self
            .roles
            .identify(info.ok_or("Cursor unavailable.")?.hCursor)
        else {
            self.stop_reason = 3;
            self.skipped += 1;
            self.detector.reset();
            return self.cancel();
        };
        let mut point = POINT::default();
        // SAFETY: 输出结构体有效，读屏幕物理坐标不修改用户输入。
        if unsafe { GetPhysicalCursorPos(&mut point) } == 0 {
            return self.cancel();
        }
        self.point_dpi(point)?;
        let (height, orientation) = self.geometry(point, frame.scale);
        self.flip_x = orientation & 1 != 0;
        self.flip_y = orientation & 2 != 0;
        if self.last_frame != Some((height, role, orientation)) {
            let source = self.frames.get(height, self.flip_x, self.flip_y)?;
            self.guardian.begin()?;
            source.replace_system(role)?;
            self.updates += 1;
            self.last_frame = Some((height, role, orientation));
        }
        self.schedule(now)
    }
    fn raw_input(&mut self, motion: RawMotion, pressed: bool, count: u32) -> Result<()> {
        if !self.registered || motion.started < self.input_since {
            return Ok(());
        }
        self.events = self.events.saturating_add(count as isize);
        let now = motion.time;
        let check_state = now - self.last_sample >= SAMPLE_INTERVAL_MS;
        if check_state || pressed {
            self.buttons_held = pressed || buttons_down();
        }
        if self.buttons_held {
            if self.active {
                self.cancel()?;
            }
            self.detector.block(now);
            self.last_sample = -1e20;
            return Ok(());
        }
        if check_state {
            let context_changed = self.refresh_foreground(false)?;
            if context_changed || !self.registered {
                return Ok(());
            }
            self.last_sample = now;
            let mut point = POINT::default();
            let info = visible_cursor();
            // SAFETY: point 为有效输出；屏幕坐标只用于可见性、显示器和绘制定位。
            self.input_visible = info.is_some() && unsafe { GetPhysicalCursorPos(&mut point) } != 0;
            if self.input_visible {
                self.point_dpi(point)?;
                if self.active && self.timer_delay > 15 {
                    let role = info.and_then(|info| self.roles.identify(info.hCursor));
                    let (height, orientation) = self.geometry(point, self.settings.maximum_scale);
                    if role.map(|role| (height, role, orientation)) != self.last_frame {
                        self.tick()?;
                    }
                }
            }
        }
        if !self.input_visible {
            if self.active {
                self.cancel()?;
            }
            self.detector.reset();
            return Ok(());
        }
        let absolute_scale = if motion.mode == CoordinateMode::Relative {
            (1.0, 1.0)
        } else {
            let (width, height) = if motion.mode == CoordinateMode::AbsoluteVirtual {
                (SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN)
            } else {
                (SM_CXSCREEN, SM_CYSCREEN)
            };
            // SAFETY: 查询桌面尺寸，不修改系统状态；绝对输入按当前显示缩放转换。
            unsafe {
                let divisor = 65535.0 * f64::from(self.dpi) / 96.0;
                (
                    f64::from((GetSystemMetrics(width) - 1).max(1)) / divisor,
                    f64::from((GetSystemMetrics(height) - 1).max(1)) / divisor,
                )
            }
        };
        let result = self.detector.push(motion, absolute_scale);
        self.samples += result.samples as isize;
        if result.triggered {
            self.trigger(true)?;
        }
        Ok(())
    }
    fn set_enabled(&mut self, enabled: bool) -> Result<()> {
        if enabled && self.fault {
            self.notify("恢复进程不可用，请退出后重新启动。");
            return Ok(());
        }
        self.cancel()?;
        self.settings.enabled = enabled;
        self.input_registration()?;
        self.tray(false);
        if config::save(&self.directory, &self.settings).is_err() {
            self.notify("无法保存配置，请检查配置目录权限。");
        }
        Ok(())
    }
    fn open_settings(&mut self) -> Result<()> {
        if self.dialog.is_null() {
            self.cancel()?;
            self.settings.startup = config::startup_enabled();
            ui::prepare_dialog(self.settings.clone(), self.test_mode);
            self.dialog = ui::open_dialog()?;
            self.input_registration()?;
        }
        // SAFETY: 对话框属于本线程。第一次 ShowWindow 可能采用进程的 SW_HIDE 启动参数。
        unsafe {
            ShowWindow(self.dialog, SW_SHOWNORMAL);
            if IsWindowVisible(self.dialog) == 0 {
                ShowWindow(self.dialog, SW_SHOWNORMAL);
            }
            SetForegroundWindow(self.dialog);
        }
        Ok(())
    }
    fn save_dialog(&mut self, mut settings: Settings) -> Result<()> {
        if self.dialog.is_null() {
            return Ok(());
        }
        let before = config::startup_enabled();
        if self.test_mode {
            settings.startup = before;
        }
        if settings.startup != before && config::set_startup(settings.startup).is_err() {
            self.dialog_error("开机启动设置失败，请重试。");
            return Ok(());
        }
        if config::save(&self.directory, &settings).is_err() {
            if settings.startup != before {
                let _ = config::set_startup(before);
            }
            self.dialog_error("保存失败，请检查配置目录权限。");
            return Ok(());
        }
        self.settings = settings;
        self.detector.set_sensitivity(self.settings.sensitivity);
        // SAFETY: 回调只入队 DialogClosed；不会重借用 self。
        unsafe {
            DestroyWindow(self.dialog);
        }
        Ok(())
    }
    fn dialog_error(&self, text: &str) {
        // SAFETY: 控件同步复制以零结尾的文本。
        unsafe {
            SetDlgItemTextW(self.dialog, 1006, wide(text).as_ptr());
        }
    }
    fn menu(&mut self) -> Result<()> {
        self.cancel()?;
        let previous_suspended = self.suspended;
        self.suspended = true;
        self.input_registration()?;
        // SAFETY: 菜单仅在此函数中使用并销毁；嵌套消息循环只写入 Inbox。
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() {
                self.suspended = previous_suspended;
                self.input_registration()?;
                return Err(win_error("Create tray menu"));
            }
            for (id, text) in [
                (
                    if self.settings.enabled { PAUSE } else { RESUME },
                    if self.settings.enabled {
                        "暂停效果"
                    } else {
                        "启用效果"
                    },
                ),
                (OPEN_SETTINGS, "设置…"),
                (PREVIEW, "预览放大效果"),
                (RESTORE, "恢复原系统指针并暂停"),
                (QUIT, "退出"),
            ] {
                AppendMenuW(menu, MF_STRING, id as usize, wide(text).as_ptr());
            }
            let mut point = POINT::default();
            GetCursorPos(&mut point);
            SetForegroundWindow(self.window);
            let result = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                0,
                self.window,
                null(),
            );
            DestroyMenu(menu);
            PostMessageW(self.window, WM_NULL, 0, 0);
            self.suspended = previous_suspended;
            self.input_registration()?;
            if result != 0 {
                self.command(result as u32)?;
            }
        }
        Ok(())
    }
    fn command(&mut self, value: u32) -> Result<()> {
        match value {
            OPEN_SETTINGS => self.open_settings()?,
            PAUSE => self.set_enabled(false)?,
            RESUME => self.set_enabled(true)?,
            PREVIEW => self.trigger(false)?,
            QUIT => self.running = false,
            RESTORE => {
                self.set_enabled(false)?;
                if !reload_cursors() {
                    return Err("Explicit cursor restoration failed.".into());
                }
                self.roles.refresh();
            }
            TEST_HANG if self.test_mode && self.guardian.dirty() => {
                std::thread::sleep(std::time::Duration::from_secs(6))
            }
            _ => {}
        }
        Ok(())
    }
    fn event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Raw {
                motion,
                pressed,
                count,
            } => self.raw_input(motion, pressed, count)?,
            Event::Timer(ANIMATION_TIMER) => self.tick()?,
            Event::Timer(CACHE_TIMER) => {
                // SAFETY: 一次性缓存释放 timer 不在动画活跃时使用。
                unsafe {
                    KillTimer(self.window, CACHE_TIMER);
                }
                if !self.active {
                    self.frames.clear();
                }
            }
            Event::Command(value) => self.command(value)?,
            Event::Menu => self.menu()?,
            Event::Save(settings) => self.save_dialog(settings)?,
            Event::DialogClosed => {
                self.dialog = null_mut();
                self.refresh_foreground(true)?;
                self.input_registration()?;
            }
            Event::Suspend(value) => {
                self.suspended = value;
                self.cancel()?;
                self.input_registration()?;
            }
            Event::Foreground => {
                self.refresh_foreground(false)?;
            }
            Event::Reset => {
                self.monitor = null_mut();
                self.stop_reason = 4;
                self.cancel()?;
                self.detector.reset();
            }
            Event::Taskbar => self.tray(true),
            Event::Fault => return Err("Window event queue overflow; effects disabled.".into()),
            _ => {}
        }
        Ok(())
    }
    fn drain(&mut self) {
        while let Some(event) = ui::pop() {
            if let Err(error) = self.event(event) {
                self.fail(&error.to_string());
            }
            self.snapshot();
            if !self.running {
                break;
            }
        }
    }
    pub fn run(&mut self, settings_on_start: bool) -> Result<i32> {
        (self.window, self.dpi_window) = ui::create_windows()?;
        self.tray_icon = Some(make_arrow(32, false, false, true)?);
        self.tray(true);
        self.foreground_watcher = Some(foreground::Watcher::new()?);
        self.refresh_foreground(true)?;
        self.input_registration()?;
        // SAFETY: 注册窗口接收当前会话通知，退出时注销。
        unsafe {
            WTSRegisterSessionNotification(self.window, NOTIFY_FOR_THIS_SESSION);
        }
        if settings_on_start {
            self.open_settings()?;
        }
        self.snapshot();
        let mut watch_guardian = true;
        while self.running {
            self.drain();
            if !self.running {
                break;
            }
            let guard = self.guardian.process();
            // SAFETY: 单线程消息循环；Dispatch 期间没有回调能访问 App，进程句柄保持有效。
            unsafe {
                let wait = MsgWaitForMultipleObjectsEx(
                    u32::from(watch_guardian),
                    &guard,
                    INFINITE,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
                if watch_guardian && wait == WAIT_OBJECT_0 {
                    watch_guardian = false;
                    self.fail("Recovery process exited; effects disabled.");
                    self.snapshot();
                }
                if wait == WAIT_FAILED {
                    return Err(win_error("Wait for messages"));
                }
                let mut message = MSG::default();
                while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if message.message == WM_QUIT {
                        self.running = false;
                        break;
                    }
                    if self.dialog.is_null() || IsDialogMessageW(self.dialog, &message) == 0 {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                    self.drain();
                    if !self.running {
                        break;
                    }
                }
                // PeekMessage 也会分发跨线程的 SendMessage，必须处理其入队事件。
                self.drain();
            }
        }
        self.cancel()?;
        Ok(if self.fault { 3 } else { 0 })
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = self.cancel();
        self.suspended = true;
        self.foreground_watcher = None;
        let _ = self.input_registration();
        // SAFETY: 清理顺序先移除 tray/timers 再销毁窗口，图标与 Guardian 在字段析构时释放。
        unsafe {
            if !self.dialog.is_null() {
                DestroyWindow(self.dialog);
            }
            if !self.window.is_null() {
                KillTimer(self.window, ANIMATION_TIMER);
                KillTimer(self.window, CACHE_TIMER);
                let data = NOTIFYICONDATAW {
                    cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: self.window,
                    uID: 1,
                    ..Default::default()
                };
                Shell_NotifyIconW(NIM_DELETE, &data);
                WTSUnRegisterSessionNotification(self.window);
                DestroyWindow(self.window);
            }
            if !self.dpi_window.is_null() {
                DestroyWindow(self.dpi_window);
            }
        }
    }
}
