mod app;
mod config;
mod cursor;
mod foreground;
mod guardian;
mod platform;
mod ui;

use platform::*;
use std::{
    ffi::OsString,
    path::PathBuf,
    ptr::{null, null_mut},
};
use windows_sys::{
    Win32::{Foundation::*, System::Threading::*, UI::WindowsAndMessaging::*},
    core::w,
};

fn entry(arguments: Vec<OsString>, directory: &mut PathBuf) -> Result<i32> {
    if arguments.first().is_some_and(|arg| arg == "--guardian") {
        if arguments.len() != 6 {
            return Err("Invalid recovery arguments.".into());
        }
        let handle = |index: usize| -> Result<Handle> {
            let value = arguments[index]
                .to_str()
                .ok_or("Invalid recovery handle.")?
                .parse::<usize>()?;
            // SAFETY: 私有启动入口接收父进程创建的继承句柄；拒绝无效数值。
            unsafe { Handle::owned(value as HANDLE) }
        };
        return guardian::run(
            handle(1)?,
            handle(2)?,
            handle(3)?,
            handle(4)?,
            &PathBuf::from(&arguments[5]),
        );
    }
    *directory = profile_path()?;
    let (mut command, mut test_mode) = (0, false);
    let mut args = arguments.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--data-dir") => *directory = args.next().ok_or("Missing data directory.")?.into(),
            Some("--test-mode") => test_mode = true,
            Some("--settings") => command = ui::OPEN_SETTINGS,
            Some("--pause") => command = ui::PAUSE,
            Some("--resume") => command = ui::RESUME,
            Some("--preview") => command = ui::PREVIEW,
            Some("--exit") => command = ui::QUIT,
            Some("--restore") => command = ui::RESTORE,
            _ => return Err("Unknown or incomplete argument.".into()),
        }
    }
    *directory = std::path::absolute(&*directory)?;
    let existing = existing_window();
    if !existing.is_null() {
        check(
            // SAFETY: 仅向本产品控制窗口发送不含指针的命令。
            unsafe {
                PostMessageW(
                    existing,
                    COMMAND_MESSAGE,
                    if command == 0 {
                        ui::OPEN_SETTINGS
                    } else {
                        command
                    } as usize,
                    0,
                )
            },
            "Send command",
        )?;
        return Ok(0);
    }
    // SAFETY: 命名 mutex 只用于当前会话单实例检测，与 C++ 版本共享以避免同时替换指针。
    let (mutex, error) = unsafe {
        let handle = CreateMutexW(null(), 0, MUTEX_NAME);
        (handle, GetLastError())
    };
    // SAFETY: mutex 为新获取的独占句柄。
    let _mutex = unsafe { Handle::owned(mutex)? };
    if error == ERROR_ALREADY_EXISTS {
        return Ok(2);
    }
    if command == ui::RESTORE {
        if !reload_cursors() {
            return Err("Cursor restoration failed.".into());
        }
        let _ = std::fs::remove_file(directory.join(MARKER_FILE));
        return Ok(0);
    }
    if command != 0 && command != ui::OPEN_SETTINGS {
        return Ok(2);
    }
    app::App::new(directory.clone(), test_mode)?.run(command == ui::OPEN_SETTINGS)
}

pub fn main() -> i32 {
    let mut directory = PathBuf::new();
    let arguments = std::env::args_os().skip(1).collect();
    match entry(arguments, &mut directory) {
        Ok(code) => code,
        Err(error) => {
            if !directory.as_os_str().is_empty() {
                log_error(&directory, &error.to_string());
            }
            // SAFETY: 只显示静态错误信息；不包含私密路径或输入轨迹。
            unsafe {
                MessageBoxW(
                    null_mut(),
                    w!("ShakeSpot 无法启动。请查看配置目录中的 rust-error.log。"),
                    w!("ShakeSpot"),
                    MB_OK | MB_ICONERROR,
                );
            }
            3
        }
    }
}
