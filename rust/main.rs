#![cfg_attr(not(test), windows_subsystem = "windows")]

#[cfg(windows)]
mod win;

fn main() {
    #[cfg(windows)]
    std::process::exit(win::main());
    #[cfg(not(windows))]
    eprintln!("ShakeSpot requires Windows 11 x64.");
}
