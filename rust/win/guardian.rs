use super::platform::*;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    ptr::{NonNull, null, null_mut},
    sync::atomic::{AtomicI32, AtomicU64, Ordering::SeqCst},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::FILE_FLAG_WRITE_THROUGH,
    System::{Memory::*, SystemInformation::GetTickCount64, Threading::*},
    UI::WindowsAndMessaging::SW_HIDE,
};

#[repr(C, align(8))]
#[derive(Default)]
struct RecoveryState {
    dirty: AtomicI32,
    stopping: AtomicI32,
    heartbeat: AtomicU64,
}

struct View {
    address: NonNull<RecoveryState>,
}
impl View {
    fn map(mapping: &Handle) -> Result<Self> {
        // SAFETY: 映射大小固定；Windows 的页对齐满足 RecoveryState 的对齐要求。
        let address = unsafe {
            MapViewOfFile(
                mapping.get(),
                FILE_MAP_ALL_ACCESS,
                0,
                0,
                size_of::<RecoveryState>(),
            )
        };
        Ok(Self {
            address: NonNull::new(address.Value.cast())
                .ok_or_else(|| win_error("Map recovery state"))?,
        })
    }
    fn state(&self) -> &RecoveryState {
        // SAFETY: 父进程先初始化再启动子进程；两者只使用原子字段，映射由本对象持有。
        unsafe { self.address.as_ref() }
    }
}
impl Drop for View {
    fn drop(&mut self) {
        // SAFETY: 独占的映射视图只解除一次；所有 state 借用已结束。
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.address.as_ptr().cast(),
            });
        }
    }
}

struct Attributes {
    storage: Vec<usize>,
}
impl Attributes {
    fn new() -> Result<Self> {
        let mut bytes = 0;
        // SAFETY: 首次调用查询大小；第二次提供按 usize 对齐且足够大的缓冲区。
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut bytes);
            if bytes == 0 || bytes > 64 * 1024 {
                return Err("Invalid process attribute size.".into());
            }
            let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
            if InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 1, 0, &mut bytes) == 0
            {
                return Err(win_error("Initialize recovery attributes"));
            }
            Ok(Self { storage })
        }
    }
    fn ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: 缓冲区在 DeleteProcThreadAttributeList 返回后才释放。
        unsafe {
            DeleteProcThreadAttributeList(self.ptr());
        }
    }
}

pub struct Guardian {
    // 字段按声明顺序释放；映射视图先于映射句柄释放。
    view: View,
    _mapping: Handle,
    changed: Handle,
    _ready: Handle,
    _parent: Handle,
    process: Handle,
    id: u32,
    marker: PathBuf,
}

impl Guardian {
    pub fn new(directory: &Path) -> Result<Self> {
        fs::create_dir_all(directory)?;
        let marker = directory.join(MARKER_FILE);
        if marker.try_exists()? {
            if !reload_cursors() {
                return Err("Cannot recover previous cursor change.".into());
            }
            fs::remove_file(&marker)?;
        }
        // SAFETY: 所有内核句柄立即交给 RAII；只允许指定的四个句柄继承。
        unsafe {
            let security = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: null_mut(),
                bInheritHandle: 1,
            };
            let mapping = Handle::owned(CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                &security,
                PAGE_READWRITE,
                0,
                size_of::<RecoveryState>() as u32,
                null(),
            ))?;
            let changed = Handle::owned(CreateEventW(&security, 0, 0, null()))?;
            let ready = Handle::owned(CreateEventW(&security, 1, 0, null()))?;
            let parent = Handle::owned(OpenProcess(
                0x00100000 | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                1,
                GetCurrentProcessId(),
            ))?;
            let view = View::map(&mapping)?;
            view.address.as_ptr().write(RecoveryState::default());
            let exe = std::env::current_exe()?;
            // 路径来自 Windows，不能包含双引号；通过 OsString 保留非 Unicode 文件名。
            let mut command = std::ffi::OsString::from("\"");
            command.push(&exe);
            command.push(format!(
                "\" --guardian {} {} {} {} \"",
                parent.get() as usize,
                mapping.get() as usize,
                changed.get() as usize,
                ready.get() as usize
            ));
            command.push(&marker);
            command.push("\"");
            let mut command = wide(command);
            let inherit = [parent.get(), mapping.get(), changed.get(), ready.get()];
            let mut attributes = Attributes::new()?;
            check(
                UpdateProcThreadAttribute(
                    attributes.ptr(),
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                    inherit.as_ptr().cast(),
                    size_of_val(&inherit),
                    null_mut(),
                    null(),
                ),
                "Set inherited handle list",
            )?;
            let mut startup = STARTUPINFOEXW::default();
            startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
            startup.StartupInfo.dwFlags = STARTF_USESHOWWINDOW | STARTF_FORCEOFFFEEDBACK;
            startup.StartupInfo.wShowWindow = SW_HIDE as u16;
            startup.lpAttributeList = attributes.ptr();
            let mut process = PROCESS_INFORMATION::default();
            check(
                CreateProcessW(
                    wide(exe).as_ptr(),
                    command.as_mut_ptr(),
                    null(),
                    null(),
                    1,
                    EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
                    null(),
                    null(),
                    &startup.StartupInfo,
                    &mut process,
                ),
                "Start recovery process",
            )?;
            let process_handle = Handle::owned(process.hProcess)?;
            let _thread = Handle::owned(process.hThread)?;
            if WaitForSingleObject(ready.get(), 3000) != WAIT_OBJECT_0 {
                view.state().stopping.store(1, SeqCst);
                SetEvent(changed.get());
                if WaitForSingleObject(process_handle.get(), 1000) == WAIT_TIMEOUT {
                    TerminateProcess(process_handle.get(), 3);
                }
                return Err("Recovery process not ready; cursor changes disabled.".into());
            }
            Ok(Self {
                view,
                _mapping: mapping,
                changed,
                _ready: ready,
                _parent: parent,
                process: process_handle,
                id: process.dwProcessId,
                marker,
            })
        }
    }
    pub fn process(&self) -> HANDLE {
        self.process.get()
    }
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn alive(&self) -> bool {
        // SAFETY: process 句柄保持有效，零超时不阻塞。
        unsafe { WaitForSingleObject(self.process(), 0) == WAIT_TIMEOUT }
    }
    pub fn dirty(&self) -> bool {
        self.view.state().dirty.load(SeqCst) != 0
    }
    pub fn heartbeat(&self) {
        // SAFETY: GetTickCount64 无指针参数；同一系统的单调计时在两进程间可比较。
        self.view
            .state()
            .heartbeat
            .store(unsafe { GetTickCount64() }, SeqCst);
    }
    fn signal(&self) {
        // SAFETY: changed 事件的生命周期覆盖本调用。
        unsafe {
            SetEvent(self.changed.get());
        }
    }
    pub fn begin(&self) -> Result<()> {
        if !self.alive() {
            return Err("Recovery process unavailable.".into());
        }
        self.heartbeat();
        if !self.dirty() {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .custom_flags(FILE_FLAG_WRITE_THROUGH)
                .open(&self.marker)?;
            file.write_all(b"ShakeSpot cursor recovery required\r\n")?;
            file.sync_all()?;
            // 落盘之后才允许修改系统光标；崩溃时至少有恢复标记或运行中的监护进程。
            self.view.state().dirty.store(1, SeqCst);
            self.signal();
        }
        Ok(())
    }
    pub fn restore(&self) -> Result<()> {
        if !self.dirty() {
            return Ok(());
        }
        if !reload_cursors() {
            return Err("System cursor restoration failed.".into());
        }
        self.view.state().dirty.store(0, SeqCst);
        let _ = fs::remove_file(&self.marker);
        self.signal();
        Ok(())
    }
}
impl Drop for Guardian {
    fn drop(&mut self) {
        let _ = self.restore();
        self.view.state().stopping.store(1, SeqCst);
        self.signal();
        // SAFETY: 等待独占持有的子进程，字段在等待后才释放。
        unsafe {
            WaitForSingleObject(self.process(), 3000);
        }
    }
}

pub fn run(
    parent: Handle,
    mapping: Handle,
    changed: Handle,
    ready: Handle,
    marker: &Path,
) -> Result<i32> {
    let view = View::map(&mapping)?;
    // SAFETY: 本入口只由父进程通过限制继承列表启动；映射已经初始化。
    unsafe {
        check(SetEvent(ready.get()), "Signal recovery readiness")?;
        let waits = [parent.get(), changed.get()];
        loop {
            let state = view.state();
            let dirty = state.dirty.load(SeqCst) != 0;
            let result =
                WaitForMultipleObjects(2, waits.as_ptr(), 0, if dirty { 250 } else { INFINITE });
            let mut dead = result == WAIT_OBJECT_0;
            let stopping = state.stopping.load(SeqCst) != 0;
            let stale = state.dirty.load(SeqCst) != 0
                && GetTickCount64().saturating_sub(state.heartbeat.load(SeqCst)) > 1500;
            if stale && !dead && !stopping {
                TerminateProcess(parent.get(), 3);
                dead = WaitForSingleObject(parent.get(), 3000) == WAIT_OBJECT_0;
            }
            if dead || stopping || result == WAIT_FAILED {
                if state.dirty.load(SeqCst) != 0 && reload_cursors() {
                    state.dirty.store(0, SeqCst);
                    let _ = fs::remove_file(marker);
                }
                return Ok(if result == WAIT_FAILED { 3 } else { 0 });
            }
        }
    }
}
