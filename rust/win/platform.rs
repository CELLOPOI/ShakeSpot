use std::{
    ffi::OsStr,
    io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    sync::OnceLock,
};
use windows_sys::{
    Win32::{
        Foundation::*,
        System::{Com::CoTaskMemFree, Performance::*},
        UI::{Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
    },
    core::w,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub const WINDOW_CLASS: *const u16 = w!("ShakeSpot.Native.Control.v2");
pub const MUTEX_NAME: *const u16 = w!("Local\\ShakeSpot.Native.SystemCursor.v2");
pub const COMMAND_MESSAGE: u32 = WM_APP + 30;
pub const QUERY_MESSAGE: u32 = WM_APP + 31;
pub const SETTINGS_FILE: &str = "native-settings.ini";
pub const MARKER_FILE: &str = "native-recovery.flag";

pub fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
pub fn win_error(context: &str) -> Box<dyn std::error::Error> {
    format!("{context}: {}", io::Error::last_os_error()).into()
}
pub fn check(value: i32, context: &str) -> Result<()> {
    if value == 0 {
        Err(win_error(context))
    } else {
        Ok(())
    }
}

pub struct Handle(HANDLE);
impl Handle {
    // SAFETY: 调用方转交独占句柄；共享、伪句柄不能传入。
    pub unsafe fn owned(value: HANDLE) -> Result<Self> {
        if value.is_null() || value == INVALID_HANDLE_VALUE {
            Err(win_error("Handle creation failed"))
        } else {
            Ok(Self(value))
        }
    }
    pub fn get(&self) -> HANDLE {
        self.0
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: 构造时排除无效和伪句柄，类型不可复制，恰好关闭一次。
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub fn clock_ms() -> f64 {
    static FREQUENCY: OnceLock<f64> = OnceLock::new();
    // SAFETY: Windows 将计数写入有效的本地 i64；频率只初始化一次。
    unsafe {
        let frequency = FREQUENCY.get_or_init(|| {
            let mut value = 0;
            QueryPerformanceFrequency(&mut value);
            value as f64 / 1000.0
        });
        let mut value = 0;
        QueryPerformanceCounter(&mut value);
        value as f64 / frequency
    }
}
pub fn reload_cursors() -> bool {
    // SAFETY: SPI_SETCURSORS 不使用 pvParam，不持有 Rust 借用跨越窗口回调。
    unsafe { SystemParametersInfoW(SPI_SETCURSORS, 0, null_mut(), 0) != 0 }
}
pub fn visible_cursor() -> Option<CURSORINFO> {
    let mut info = CURSORINFO {
        cbSize: size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: cbSize 与可写结构体匹配。
    if unsafe { GetCursorInfo(&mut info) } != 0
        && info.flags & CURSOR_SHOWING != 0
        && info.flags & CURSOR_SUPPRESSED == 0
    {
        Some(info)
    } else {
        None
    }
}
pub fn buttons_down() -> bool {
    // SAFETY: 仅读取有效虚拟按键状态。
    unsafe {
        [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
            .iter()
            .any(|&key| GetAsyncKeyState(i32::from(key)) < 0)
    }
}
pub fn profile_path() -> Result<PathBuf> {
    let mut buffer = null_mut();
    // SAFETY: 系统分配以零结尾的路径；复制完后使用对应分配器释放。
    unsafe {
        if SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, null_mut(), &mut buffer) < 0 {
            return Err("LocalAppData unavailable.".into());
        }
        let mut length = 0;
        while *buffer.add(length) != 0 {
            length += 1;
        }
        use std::os::windows::ffi::OsStringExt;
        let value = std::ffi::OsString::from_wide(std::slice::from_raw_parts(buffer, length));
        CoTaskMemFree(buffer.cast());
        Ok(PathBuf::from(value).join("ShakeSpot"))
    }
}
pub fn log_error(directory: &Path, error: &str) {
    if std::fs::create_dir_all(directory).is_ok() {
        let _ = std::fs::write(directory.join("rust-error.log"), error);
    }
}
pub fn copy_text<const N: usize>(destination: &mut [u16; N], value: &str) {
    for (target, source) in destination
        .iter_mut()
        .take(N.saturating_sub(1))
        .zip(value.encode_utf16())
    {
        *target = source;
    }
}
pub fn resource(id: u32) -> *const u16 {
    id as usize as *const u16
}
pub fn existing_window() -> HWND {
    // SAFETY: 静态、零结尾的类名。
    unsafe { FindWindowW(WINDOW_CLASS, null()) }
}
