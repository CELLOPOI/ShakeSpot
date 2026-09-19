use super::platform::*;
use shakespot::{
    core::Settings,
    input::{CoordinateMode, RawMotion},
};
use std::{
    cell::{Cell, RefCell},
    ptr::{null, null_mut},
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::{
    Win32::{
        Foundation::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Controls::*, Input::*, WindowsAndMessaging::*},
    },
    core::w,
};

pub const TRAY_MESSAGE: u32 = WM_APP + 1;
pub const ANIMATION_TIMER: usize = 1;
pub const CACHE_TIMER: usize = 2;
pub const DURATIONS: [i32; 6] = [500, 800, 1100, 1500, 2000, 3000];
pub const OPEN_SETTINGS: u32 = 1;
pub const PAUSE: u32 = 2;
pub const RESUME: u32 = 3;
pub const PREVIEW: u32 = 4;
pub const QUIT: u32 = 5;
pub const RESTORE: u32 = 6;
pub const TEST_HANG: u32 = 7;
const SENSITIVITY: i32 = 1001;
const SCALE: i32 = 1002;
const DURATION: i32 = 1003;
const STARTUP: i32 = 1004;
const DEFAULTS: u32 = 1005;
const EXCLUDED_APPS: i32 = 1007;

#[derive(Clone)]
pub enum Event {
    None,
    Raw {
        motion: RawMotion,
        pressed: bool,
        count: u32,
    },
    Timer(usize),
    Command(u32),
    Menu,
    Save(Settings),
    DialogClosed,
    Suspend(bool),
    Reset,
    Taskbar,
    Fault,
    Foreground,
}

struct Inbox {
    entries: [Event; 64],
    start: usize,
    len: usize,
    overflow: bool,
}
impl Inbox {
    const fn new() -> Self {
        Self {
            entries: [const { Event::None }; 64],
            start: 0,
            len: 0,
            overflow: false,
        }
    }
    fn push(&mut self, event: Event) {
        if let Event::Raw {
            motion,
            pressed,
            count,
        } = &event
        {
            if self.len > 0 {
                let last = (self.start + self.len - 1) % self.entries.len();
                if let Event::Raw {
                    motion: previous_motion,
                    pressed: previous,
                    count: total,
                } = &mut self.entries[last]
                {
                    if !*pressed && !*previous && previous_motion.merge(*motion) {
                        *total = total.saturating_add(*count);
                        return;
                    }
                }
            }
        }
        if self.len == self.entries.len() {
            self.overflow = true;
            return;
        }
        self.entries[(self.start + self.len) % self.entries.len()] = event;
        self.len += 1;
    }
    fn pop(&mut self) -> Option<Event> {
        if self.overflow {
            self.overflow = false;
            self.entries.fill_with(|| Event::None);
            self.len = 0;
            self.start = 0;
            return Some(Event::Fault);
        }
        if self.len == 0 {
            return None;
        }
        let result = std::mem::replace(&mut self.entries[self.start], Event::None);
        self.start = (self.start + 1) % self.entries.len();
        self.len -= 1;
        Some(result)
    }
}

thread_local! {
    static INBOX: RefCell<Inbox> = const { RefCell::new(Inbox::new()) };
    static QUERY: Cell<[isize; 16]> = const { Cell::new([0; 16]) };
    static DIALOG_SETTINGS: RefCell<(Settings, bool)> = RefCell::new((Settings::default(), false));
    static ACCEPT_INPUT: Cell<bool> = const { Cell::new(false) };
    static TASKBAR: Cell<u32> = const { Cell::new(0) };
}
pub fn accept_input(value: bool) {
    ACCEPT_INPUT.set(value);
}
fn push(event: Event) {
    INBOX.with_borrow_mut(|queue| queue.push(event));
}
pub fn foreground_changed() {
    push(Event::Foreground);
}
pub fn pop() -> Option<Event> {
    INBOX.with_borrow_mut(Inbox::pop)
}
pub fn snapshot(value: [isize; 16]) {
    QUERY.set(value);
}
pub fn prepare_dialog(settings: Settings, test: bool) {
    DIALOG_SETTINGS.replace((settings, test));
}

// 窗口回调只写入固定队列或读取快照，不取得 App 指针，也不持有其可变借用。
// CreateWindow、DestroyWindow、SendMessage、系统设置广播导致的重入不会产生 &mut 别名。
unsafe extern "system" fn window_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: 本函数只接收 Windows 分发的消息；Raw Input 在消息返回前复制到固定栈结构。
    unsafe {
        if TASKBAR.get() != 0 && message == TASKBAR.get() {
            push(Event::Taskbar);
            return 0;
        }
        match message {
            WM_INPUT => {
                if !ACCEPT_INPUT.get() {
                    return DefWindowProcW(window, message, w, l);
                }
                let mut input = RAWINPUT::default();
                let mut bytes = size_of::<RAWINPUT>() as u32;
                let read = GetRawInputData(
                    l as HRAWINPUT,
                    RID_INPUT,
                    (&mut input as *mut RAWINPUT).cast(),
                    &mut bytes,
                    size_of::<RAWINPUTHEADER>() as u32,
                );
                if read != u32::MAX
                    && read >= (size_of::<RAWINPUTHEADER>() + size_of::<RAWMOUSE>()) as u32
                    && input.header.dwType == RIM_TYPEMOUSE
                {
                    let mouse = input.data.mouse;
                    let flags = mouse.Anonymous.Anonymous.usButtonFlags;
                    let time = clock_ms();
                    push(Event::Raw {
                        motion: RawMotion {
                            device: input.header.hDevice as usize,
                            mode: if mouse.usFlags & MOUSE_MOVE_ABSOLUTE == 0 {
                                CoordinateMode::Relative
                            } else if mouse.usFlags & MOUSE_VIRTUAL_DESKTOP == 0 {
                                CoordinateMode::AbsolutePrimary
                            } else {
                                CoordinateMode::AbsoluteVirtual
                            },
                            started: time,
                            time,
                            x: i64::from(mouse.lLastX),
                            y: i64::from(mouse.lLastY),
                        },
                        pressed: flags & 0x0155 != 0,
                        count: 1,
                    });
                }
                return DefWindowProcW(window, message, w, l);
            }
            WM_TIMER => push(Event::Timer(w)),
            COMMAND_MESSAGE => push(Event::Command(w as u32)),
            QUERY_MESSAGE => return QUERY.get().get(w).copied().unwrap_or(0),
            TRAY_MESSAGE => match l as u32 & 0xffff {
                WM_CONTEXTMENU => push(Event::Menu),
                WM_LBUTTONDBLCLK | 0x401 => push(Event::Command(OPEN_SETTINGS)),
                _ => {}
            },
            WM_WTSSESSION_CHANGE => match w as u32 {
                WTS_SESSION_LOCK | WTS_CONSOLE_DISCONNECT | WTS_REMOTE_DISCONNECT => {
                    push(Event::Suspend(true))
                }
                WTS_SESSION_UNLOCK | WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT => {
                    push(Event::Suspend(false))
                }
                _ => {}
            },
            WM_POWERBROADCAST => {
                if w as u32 == PBT_APMSUSPEND {
                    reload_cursors();
                    push(Event::Suspend(true));
                }
                if w as u32 == PBT_APMRESUMEAUTOMATIC {
                    push(Event::Suspend(false));
                }
                return 1;
            }
            WM_SETTINGCHANGE | WM_DISPLAYCHANGE => push(Event::Reset),
            WM_QUERYENDSESSION => {
                reload_cursors();
                push(Event::Suspend(true));
                return 1;
            }
            WM_ENDSESSION => {
                if w != 0 {
                    push(Event::Command(QUIT));
                }
            }
            WM_CLOSE | WM_DESTROY => push(Event::Command(QUIT)),
            _ => return DefWindowProcW(window, message, w, l),
        }
        0
    }
}

fn populate(dialog: HWND, settings: &Settings) {
    // SAFETY: dialog 属于本线程；控件 ID 来自嵌入资源，消息只借用同步有效的参数。
    unsafe {
        SendDlgItemMessageW(
            dialog,
            SENSITIVITY,
            CB_SETCURSEL,
            (settings.sensitivity - 1) as usize,
            0,
        );
        SendDlgItemMessageW(
            dialog,
            SCALE,
            CB_SETCURSEL,
            ((settings.maximum_scale - 2.0) * 2.0).round() as usize,
            0,
        );
        let nearest = DURATIONS
            .iter()
            .enumerate()
            .min_by_key(|(_, n)| (**n - settings.duration_ms).abs())
            .map_or(2, |(i, _)| i);
        SendDlgItemMessageW(dialog, DURATION, CB_SETCURSEL, nearest, 0);
        SetDlgItemTextW(
            dialog,
            EXCLUDED_APPS,
            wide(settings.excluded_apps.text()).as_ptr(),
        );
        SetDlgItemTextW(dialog, 1006, wide("").as_ptr());
        CheckDlgButton(
            dialog,
            STARTUP,
            if settings.startup {
                BST_CHECKED
            } else {
                BST_UNCHECKED
            },
        );
    }
}
unsafe extern "system" fn dialog_proc(dialog: HWND, message: u32, w: WPARAM, _: LPARAM) -> isize {
    // SAFETY: 所有控件属于当前对话框；不会访问 App 或跨回调保存引用。
    unsafe {
        match message {
            WM_INITDIALOG => {
                for text in ["1 · 不易触发", "2", "3 · 默认", "4", "5 · 容易触发"] {
                    SendDlgItemMessageW(
                        dialog,
                        SENSITIVITY,
                        CB_ADDSTRING,
                        0,
                        wide(text).as_ptr() as isize,
                    );
                }
                for n in 4..=16 {
                    SendDlgItemMessageW(
                        dialog,
                        SCALE,
                        CB_ADDSTRING,
                        0,
                        wide(format!("{:.1} 倍", f64::from(n) / 2.0)).as_ptr() as isize,
                    );
                }
                for n in DURATIONS {
                    SendDlgItemMessageW(
                        dialog,
                        DURATION,
                        CB_ADDSTRING,
                        0,
                        wide(format!("{:.1} 秒", f64::from(n) / 1000.0)).as_ptr() as isize,
                    );
                }
                let (settings, test) = DIALOG_SETTINGS.with_borrow(|value| value.clone());
                SendDlgItemMessageW(dialog, EXCLUDED_APPS, EM_SETLIMITTEXT, 4096, 0);
                populate(dialog, &settings);
                if test {
                    EnableWindow(GetDlgItem(dialog, STARTUP), 0);
                }
                1
            }
            WM_COMMAND => match w as u32 & 0xffff {
                DEFAULTS => {
                    populate(dialog, &Settings::default());
                    1
                }
                1 => {
                    let mut settings = DIALOG_SETTINGS.with_borrow(|value| value.0.clone());
                    settings.sensitivity =
                        SendDlgItemMessageW(dialog, SENSITIVITY, CB_GETCURSEL, 0, 0) as i32 + 1;
                    settings.maximum_scale =
                        2.0 + SendDlgItemMessageW(dialog, SCALE, CB_GETCURSEL, 0, 0) as f64 / 2.0;
                    let index = SendDlgItemMessageW(dialog, DURATION, CB_GETCURSEL, 0, 0)
                        .clamp(0, 5) as usize;
                    settings.duration_ms = DURATIONS[index];
                    settings.startup = IsDlgButtonChecked(dialog, STARTUP) == BST_CHECKED;
                    let mut buffer = [0u16; 4097];
                    let length = GetDlgItemTextW(
                        dialog,
                        EXCLUDED_APPS,
                        buffer.as_mut_ptr(),
                        buffer.len() as i32,
                    );
                    let excluded = String::from_utf16_lossy(&buffer[..length as usize]);
                    match shakespot::exclusions::Exclusions::parse(&excluded) {
                        Ok(rules) => settings.excluded_apps = rules,
                        Err(_) => {
                            SetDlgItemTextW(dialog, 1006, wide("排除项须为 .exe 程序名或完整路径，最多 32 项、总计 4096 字节。").as_ptr());
                            return 1;
                        }
                    }
                    settings.normalize();
                    push(Event::Save(settings));
                    1
                }
                2 => {
                    DestroyWindow(dialog);
                    1
                }
                _ => 0,
            },
            WM_CLOSE => {
                DestroyWindow(dialog);
                1
            }
            WM_DESTROY => {
                push(Event::DialogClosed);
                1
            }
            _ => 0,
        }
    }
}

pub fn create_windows() -> Result<(HWND, HWND)> {
    // SAFETY: 类名和回调静态有效；窗口状态不通过裸指针传递。
    unsafe {
        let controls = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES,
        };
        check(InitCommonControlsEx(&controls), "Initialize controls")?;
        TASKBAR.set(RegisterWindowMessageW(w!("TaskbarCreated")));
        let class = WNDCLASSW {
            hInstance: GetModuleHandleW(null()),
            lpszClassName: WINDOW_CLASS,
            lpfnWndProc: Some(window_proc),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(win_error("Register window class"));
        }
        let dpi = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("STATIC"),
            w!("ShakeSpot DPI"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            null_mut(),
            null_mut(),
            class.hInstance,
            null(),
        );
        if dpi.is_null() {
            return Err(win_error("Create DPI window"));
        }
        let control = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            WINDOW_CLASS,
            w!("ShakeSpot Native"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            class.hInstance,
            null(),
        );
        if control.is_null() {
            DestroyWindow(dpi);
            return Err(win_error("Create control window"));
        }
        Ok((control, dpi))
    }
}
pub fn open_dialog() -> Result<HWND> {
    // SAFETY: 对话框模板嵌入在同一模块，回调静态有效。
    let dialog = unsafe {
        CreateDialogParamW(
            GetModuleHandleW(null()),
            resource(101),
            null_mut(),
            Some(dialog_proc),
            0,
        )
    };
    if dialog.is_null() {
        Err(win_error("Create settings dialog"))
    } else {
        Ok(dialog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn motion(x: i64) -> RawMotion {
        RawMotion {
            device: 1,
            mode: CoordinateMode::Relative,
            started: 0.0,
            time: 0.0,
            x,
            y: 0,
        }
    }
    #[test]
    fn burst_coalesces_straight_motion_but_preserves_turns_and_buttons() {
        let mut inbox = Inbox::new();
        for _ in 0..100_000 {
            inbox.push(Event::Raw {
                motion: motion(1),
                pressed: false,
                count: 1,
            });
        }
        inbox.push(Event::Raw {
            motion: motion(-20),
            pressed: false,
            count: 1,
        });
        inbox.push(Event::Raw {
            motion: motion(0),
            pressed: true,
            count: 1,
        });
        assert_eq!(inbox.len, 3);
        assert!(!inbox.overflow);
        assert!(matches!(
            inbox.pop(),
            Some(Event::Raw {
                motion: RawMotion { x: 100_000, .. },
                pressed: false,
                count: 100_000
            })
        ));
        assert!(matches!(
            inbox.pop(),
            Some(Event::Raw {
                motion: RawMotion { x: -20, .. },
                ..
            })
        ));
        assert!(matches!(
            inbox.pop(),
            Some(Event::Raw { pressed: true, .. })
        ));
        assert!(inbox.pop().is_none());
    }
    #[test]
    fn command_flood_fails_closed_with_bounded_storage() {
        let mut inbox = Inbox::new();
        for _ in 0..1000 {
            inbox.push(Event::Command(PREVIEW));
        }
        assert!(matches!(inbox.pop(), Some(Event::Fault)));
        assert!(inbox.pop().is_none());
    }
}
