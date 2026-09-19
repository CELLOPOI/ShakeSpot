use super::{platform::*, ui};
use std::ptr::null_mut;
use windows_sys::Win32::{
    Foundation::*,
    System::Threading::*,
    UI::{Accessibility::*, WindowsAndMessaging::*},
};

pub struct Watcher(HWINEVENTHOOK);

unsafe extern "system" fn changed(
    _: HWINEVENTHOOK,
    _: u32,
    _: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    // 回调只入队；不持有 App 或跨 Win32 重入的 Rust 可变借用。
    ui::foreground_changed();
}

impl Watcher {
    pub fn new() -> Result<Self> {
        // SAFETY: 进程外通知由本线程消息循环接收，回调为静态函数；不注入其他进程。
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                null_mut(),
                Some(changed),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.is_null() {
            Err(win_error("Watch foreground window"))
        } else {
            Ok(Self(hook))
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        // SAFETY: hook 是本对象唯一持有的有效句柄，在注册线程释放。
        unsafe {
            UnhookWinEvent(self.0);
        }
    }
}

pub fn current() -> HWND {
    // SAFETY: 仅读取前台窗口句柄。
    unsafe { GetForegroundWindow() }
}

pub fn executable(window: HWND) -> Option<String> {
    if window.is_null() {
        return None;
    }
    let mut process_id = 0;
    // SAFETY: 输出 PID 有效；仅申请查询权限，Handle 负责释放。
    unsafe {
        if GetWindowThreadProcessId(window, &mut process_id) == 0 || process_id == 0 {
            return None;
        }
        let process = Handle::owned(OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            process_id,
        ))
        .ok()?;
        let mut buffer = [0u16; 32768];
        let mut length = buffer.len() as u32;
        if QueryFullProcessImageNameW(process.get(), 0, buffer.as_mut_ptr(), &mut length) == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buffer[..length as usize]))
    }
}
