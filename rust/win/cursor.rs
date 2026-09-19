use super::platform::*;
use std::ptr::null_mut;
use windows_sys::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};

pub const CURSOR_IDS: [u32; 13] = [
    OCR_NORMAL,
    OCR_IBEAM,
    OCR_WAIT,
    OCR_CROSS,
    OCR_UP,
    OCR_SIZENWSE,
    OCR_SIZENESW,
    OCR_SIZEWE,
    OCR_SIZENS,
    OCR_SIZEALL,
    OCR_NO,
    OCR_HAND,
    OCR_APPSTARTING,
];
pub const MAX_CACHE_ENTRIES: usize = 16;
pub const MAX_CACHE_BYTES: usize = 2 * 1024 * 1024;

pub struct Cursor {
    handle: HCURSOR,
    icon: bool,
}
impl Cursor {
    pub fn get(&self) -> HCURSOR {
        self.handle
    }
    pub fn replace_system(&self, role: usize) -> Result<()> {
        let id = *CURSOR_IDS.get(role).ok_or("Invalid cursor role.")?;
        // SAFETY: CopyImage 返回独立资源；SetSystemCursor 消耗副本，缓存原件继续归本对象。
        unsafe {
            let copy = CopyImage(self.handle, IMAGE_CURSOR, 0, 0, 0);
            if copy.is_null() {
                return Err(win_error("Copy cursor frame"));
            }
            check(SetSystemCursor(copy, id), "Replace system cursor")
        }
    }
}
impl Drop for Cursor {
    fn drop(&mut self) {
        // SAFETY: 只封装 CreateIconIndirect 的非共享返回值；使用对应析构函数。
        unsafe {
            if self.icon {
                DestroyIcon(self.handle);
            } else {
                DestroyCursor(self.handle);
            }
        }
    }
}
struct Bitmap(HBITMAP);
impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: 位图从未选择进 DC，只在本对象拥有的范围内借给 CreateIconIndirect。
        unsafe {
            DeleteObject(self.0);
        }
    }
}
pub fn make_arrow(height: i32, flip_x: bool, flip_y: bool, icon: bool) -> Result<Cursor> {
    let height = height.clamp(24, 256) as usize;
    let (width, _) = shakespot::raster::dimensions(height).ok_or("Invalid cursor dimensions.")?;
    let mut info = BITMAPINFO::default();
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width as i32;
    info.bmiHeader.biHeight = -(height as i32);
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    // SAFETY: 32 位 DIB 的容量与切片严格匹配；CPU 写入结束后系统才读取位图。
    unsafe {
        let mut address = null_mut();
        let color = CreateDIBSection(
            null_mut(),
            &info,
            DIB_RGB_COLORS,
            &mut address,
            null_mut(),
            0,
        );
        if color.is_null() {
            return Err(win_error("Create cursor color bitmap"));
        }
        let _color = Bitmap(color);
        let pixels = std::slice::from_raw_parts_mut(address.cast::<u32>(), width * height);
        shakespot::raster::render(pixels, height, flip_x, flip_y)?;
        let mask_bits = [0u8; 8192];
        let mask = CreateBitmap(width as i32, height as i32, 1, 1, mask_bits.as_ptr().cast());
        if mask.is_null() {
            return Err(win_error("Create cursor mask"));
        }
        let _mask = Bitmap(mask);
        let cursor = ICONINFO {
            fIcon: i32::from(icon),
            xHotspot: if flip_x { width - 3 } else { 2 } as u32,
            yHotspot: if flip_y { height - 3 } else { 2 } as u32,
            hbmMask: mask,
            hbmColor: color,
        };
        let handle = CreateIconIndirect(&cursor);
        if handle.is_null() {
            return Err(win_error("Create cursor frame"));
        }
        Ok(Cursor { handle, icon })
    }
}

struct Entry {
    cursor: Cursor,
    key: i32,
    used: u64,
    bytes: usize,
}
pub struct CursorFrames {
    entries: [Option<Entry>; MAX_CACHE_ENTRIES],
    clock: u64,
    bytes: usize,
}
impl Default for CursorFrames {
    fn default() -> Self {
        Self {
            entries: std::array::from_fn(|_| None),
            clock: 0,
            bytes: 0,
        }
    }
}
impl CursorFrames {
    pub fn clear(&mut self) {
        self.entries.fill_with(|| None);
        self.clock = 0;
        self.bytes = 0;
    }
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|entry| entry.is_some()).count()
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    fn evict(&mut self, index: usize) {
        if let Some(entry) = self.entries[index].take() {
            self.bytes -= entry.bytes;
        }
    }
    fn oldest(&self) -> usize {
        self.entries
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| e.as_ref().map_or(0, |e| e.used))
            .map_or(0, |(i, _)| i)
    }
    pub fn get(&mut self, height: i32, flip_x: bool, flip_y: bool) -> Result<&Cursor> {
        let height = (height.clamp(24, 256) + 1) / 2 * 2;
        let key = (i32::from(flip_x) | (i32::from(flip_y) << 1)) * 129 + height / 2;
        self.clock = self.clock.saturating_add(1);
        if let Some(index) = self
            .entries
            .iter()
            .position(|e| e.as_ref().is_some_and(|e| e.key == key))
        {
            let entry = self.entries[index].as_mut().ok_or("Cache entry missing.")?;
            entry.used = self.clock;
            return Ok(&entry.cursor);
        }
        let width = (26.0 * f64::from(height - 4) / 35.0).ceil() as usize + 4;
        let bytes = (width * 4 + width.div_ceil(16) * 2) * height as usize;
        // 先淘汰再创建，避免缓存满时短暂持有第 17 帧。
        let index = self.oldest();
        self.evict(index);
        while self.bytes + bytes > MAX_CACHE_BYTES {
            let oldest = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| e.as_ref().map(|e| (i, e.used)))
                .min_by_key(|e| e.1)
                .ok_or("Invalid cursor budget.")?
                .0;
            self.evict(oldest);
        }
        let cursor = make_arrow(height, flip_x, flip_y, false)?;
        self.entries[index] = Some(Entry {
            cursor,
            key,
            used: self.clock,
            bytes,
        });
        self.bytes += bytes;
        Ok(&self.entries[index]
            .as_ref()
            .ok_or("Cache insert failed.")?
            .cursor)
    }
}

pub struct CursorRoles([HCURSOR; CURSOR_IDS.len()]);
impl Default for CursorRoles {
    fn default() -> Self {
        let mut result = Self([null_mut(); CURSOR_IDS.len()]);
        result.refresh();
        result
    }
}
impl CursorRoles {
    pub fn refresh(&mut self) {
        for (slot, id) in self.0.iter_mut().zip(CURSOR_IDS) {
            // SAFETY: 标准共享光标不归本对象所有，也不会 DestroyCursor。
            *slot = unsafe { LoadCursorW(null_mut(), resource(id)) };
        }
    }
    pub fn identify(&self, cursor: HCURSOR) -> Option<usize> {
        self.0
            .iter()
            .position(|&known| !known.is_null() && known == cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Threading::*;
    #[test]
    fn cache_budget_and_gdi_resources_return_to_baseline() {
        // SAFETY: 只查询当前测试进程，没有替换任何系统指针。
        let usage = || unsafe {
            (
                GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS),
                GetGuiResources(GetCurrentProcess(), GR_USEROBJECTS),
            )
        };
        drop(make_arrow(32, false, false, false).unwrap());
        let baseline = usage();
        let mut frames = CursorFrames::default();
        for round in 0..2 {
            for orientation in 0..4 {
                for height in (24..=256).step_by(2) {
                    frames
                        .get(height, orientation & 1 != 0, orientation & 2 != 0)
                        .unwrap();
                    assert!(frames.len() <= MAX_CACHE_ENTRIES);
                    assert!(frames.bytes() <= MAX_CACHE_BYTES);
                }
            }
            frames.clear();
            assert_eq!(frames.len(), 0);
            assert_eq!(frames.bytes(), 0);
            assert_eq!(usage(), baseline, "GUI resource leak after round {round}");
        }
    }
}
