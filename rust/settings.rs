use crate::core::Settings;
use std::io::{self, Read};
use std::path::Path;

pub const MAX_SETTINGS_BYTES: u64 = 16 * 1024;

pub fn parse(text: &str) -> io::Result<Settings> {
    let mut result = Settings::default();
    let mut excluded = String::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "excludedApp" => {
                if !excluded.is_empty() {
                    excluded.push('\n');
                }
                excluded.push_str(value);
            }
            "sensitivity" => {
                if let Ok(n) = value.parse() {
                    result.sensitivity = n;
                }
            }
            "maximumScale" => {
                if let Ok(n) = value.parse() {
                    result.maximum_scale = n;
                }
            }
            "durationMs" => {
                if let Ok(n) = value.parse() {
                    result.duration_ms = n;
                }
            }
            "enabled" => {
                if let Ok(n) = value.parse::<i32>() {
                    result.enabled = n != 0;
                }
            }
            "startup" => {
                if let Ok(n) = value.parse::<i32>() {
                    result.startup = n != 0;
                }
            }
            _ => {}
        }
    }
    result.normalize();
    result.excluded_apps = crate::exclusions::Exclusions::parse(&excluded)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(result)
}

pub fn load(path: &Path) -> io::Result<Settings> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(e) => return Err(e),
    };
    // 不信任文件元数据；读取本身也有硬上限，避免异常大配置耗尽内存。
    let mut text = String::new();
    file.take(MAX_SETTINGS_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Settings file exceeds 16 KiB.",
        ));
    }
    parse(&text)
}

pub fn encode(settings: &Settings) -> String {
    let mut text = format!(
        "sensitivity={}\nmaximumScale={}\ndurationMs={}\nenabled={}\nstartup={}\n",
        settings.sensitivity,
        settings.maximum_scale,
        settings.duration_ms,
        i32::from(settings.enabled),
        i32::from(settings.startup)
    );
    for entry in settings.excluded_apps.entries() {
        text.push_str("excludedApp=");
        text.push_str(entry);
        text.push('\n');
    }
    text
}
