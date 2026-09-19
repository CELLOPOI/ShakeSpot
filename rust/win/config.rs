use super::platform::*;
use shakespot::{core::Settings, settings};
use std::{
    io::Write,
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::{
    Win32::{Foundation::*, Storage::FileSystem::*, System::Registry::*},
    core::w,
};

pub fn save(directory: &Path, settings: &Settings) -> Result<()> {
    std::fs::create_dir_all(directory)?;
    let file = directory.join(SETTINGS_FILE);
    let temporary = directory.join("native-settings.ini.tmp");
    let mut output = std::fs::File::create(&temporary)?;
    output.write_all(settings::encode(settings).as_bytes())?;
    output.sync_all()?;
    drop(output);
    // SAFETY: 零结尾的路径在调用期间有效，同目录替换保持原文件直到新文件完整写入。
    check(
        unsafe {
            MoveFileExW(
                wide(temporary).as_ptr(),
                wide(file).as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        },
        "Save settings",
    )
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: 仅包装成功打开的注册表句柄。
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
const RUN_KEY: *const u16 = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_NAME: *const u16 = w!("ShakeSpotNative");

fn quoted_exe() -> Result<Vec<u16>> {
    let mut value = std::ffi::OsString::from("\"");
    value.push(std::env::current_exe()?);
    value.push("\"");
    Ok(wide(value))
}

pub fn startup_enabled() -> bool {
    // SAFETY: 所有输出长度按字节传递，查询结果经类型、范围和字符串精确匹配检查。
    unsafe {
        let mut key = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, 0, KEY_QUERY_VALUE, &mut key) != ERROR_SUCCESS
        {
            return false;
        }
        let key = Key(key);
        let mut value = [0u16; 32768];
        let mut bytes = size_of_val(&value) as u32;
        let mut kind = 0;
        if RegQueryValueExW(
            key.0,
            RUN_NAME,
            null(),
            &mut kind,
            value.as_mut_ptr().cast(),
            &mut bytes,
        ) != ERROR_SUCCESS
            || kind != REG_SZ
        {
            return false;
        }
        let length = bytes as usize / 2;
        quoted_exe().is_ok_and(|expected| value.get(..length) == Some(expected.as_slice()))
    }
}

pub fn set_startup(enabled: bool) -> Result<()> {
    // SAFETY: 只修改当前用户的专属启动项，字符串包括末尾零。
    unsafe {
        let mut key = null_mut();
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            0,
            null(),
            0,
            KEY_SET_VALUE,
            null(),
            &mut key,
            null_mut(),
        );
        if status != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(status as i32).into());
        }
        let key = Key(key);
        let path = quoted_exe()?;
        let status = if enabled {
            RegSetValueExW(
                key.0,
                RUN_NAME,
                0,
                REG_SZ,
                path.as_ptr().cast(),
                (path.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(key.0, RUN_NAME)
        };
        if status == ERROR_SUCCESS || (!enabled && status == ERROR_FILE_NOT_FOUND) {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(status as i32).into())
        }
    }
}
