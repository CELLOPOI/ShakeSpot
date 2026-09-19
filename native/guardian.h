#pragma once
#include "platform.h"

namespace shakespot {
struct alignas(8) RecoveryState {
    volatile LONG dirty = 0;
    volatile LONG stopping = 0;
    volatile LONG recovered = 0;
    volatile LONG64 heartbeat = 0;
};

// 恢复进程通过内核等待休眠；只在修改系统光标期间检查心跳。
inline int runGuardian(HANDLE parent, HANDLE mapping, HANDLE changed, HANDLE ready, const std::wstring& marker) {
    auto* state = static_cast<RecoveryState*>(MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, sizeof(RecoveryState)));
    if (!state || !parent || !changed || !ready) return 2;
    SetEvent(ready);
    HANDLE waits[]{parent, changed};
    for (;;) {
        bool dirty = InterlockedCompareExchange(&state->dirty, 0, 0) != 0;
        DWORD result = WaitForMultipleObjects(2, waits, FALSE, dirty ? 250 : INFINITE);
        bool parentDead = result == WAIT_OBJECT_0;
        bool stopping = InterlockedCompareExchange(&state->stopping, 0, 0) != 0;
        auto beat = static_cast<ULONGLONG>(InterlockedCompareExchange64(&state->heartbeat, 0, 0));
        bool stale = InterlockedCompareExchange(&state->dirty, 0, 0) && GetTickCount64() - beat > 1500;
        if (stale && !parentDead && !stopping) {
            // 先终止已挂起的主进程，防止恢复后又被其迟到的动画帧改回。
            TerminateProcess(parent, 3);
            parentDead = WaitForSingleObject(parent, 3000) == WAIT_OBJECT_0;
        }
        if (parentDead || stopping) {
            if (InterlockedCompareExchange(&state->dirty, 0, 0)) {
                bool recovered = reloadCursors();
                if (recovered) {
                    InterlockedExchange(&state->dirty, 0);
                    InterlockedExchange(&state->recovered, 1);
                    DeleteFileW(marker.c_str());
                }
            }
            UnmapViewOfFile(state);
            return 0;
        }
        if (result == WAIT_FAILED) { if (dirty) reloadCursors(); UnmapViewOfFile(state); return 3; }
    }
}

class Guardian {
    struct ViewDeleter { void operator()(RecoveryState* state) const { if (state) UnmapViewOfFile(state); } };
    Handle mapping_, changed_, ready_, parent_;
    std::unique_ptr<RecoveryState, ViewDeleter> view_;
    PROCESS_INFORMATION process_{};
    RecoveryState* state_ = nullptr;
    std::wstring marker_;
public:
    explicit Guardian(const std::wstring& directory) : marker_(directory + L"\\native-recovery.flag") {
        std::filesystem::create_directories(directory);
        if (std::filesystem::exists(marker_)) {
            if (!reloadCursors()) throw std::runtime_error("Cannot recover a previous cursor change.");
            DeleteFileW(marker_.c_str());
        }
        SECURITY_ATTRIBUTES security{sizeof(security), nullptr, TRUE};
        mapping_.value = CreateFileMappingW(INVALID_HANDLE_VALUE, &security, PAGE_READWRITE, 0, sizeof(RecoveryState), nullptr);
        changed_.value = CreateEventW(&security, FALSE, FALSE, nullptr);
        ready_.value = CreateEventW(&security, TRUE, FALSE, nullptr);
        parent_.value = OpenProcess(SYNCHRONIZE | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION, TRUE, GetCurrentProcessId());
        if (!mapping_.get() || !changed_.get() || !ready_.get() || !parent_.get()) throw std::runtime_error("Cannot create recovery handles.");
        state_ = static_cast<RecoveryState*>(MapViewOfFile(mapping_.get(), FILE_MAP_ALL_ACCESS, 0, 0, sizeof(RecoveryState)));
        if (!state_) throw std::runtime_error("Cannot map recovery state.");
        view_.reset(state_);
        std::wstring command = quote(executablePath()) + L" --guardian " +
            std::to_wstring(reinterpret_cast<uintptr_t>(parent_.get())) + L" " +
            std::to_wstring(reinterpret_cast<uintptr_t>(mapping_.get())) + L" " +
            std::to_wstring(reinterpret_cast<uintptr_t>(changed_.get())) + L" " +
            std::to_wstring(reinterpret_cast<uintptr_t>(ready_.get())) + L" " + quote(marker_);
        SIZE_T bytes = 0;
        InitializeProcThreadAttributeList(nullptr, 1, 0, &bytes);
        std::vector<unsigned char> attributes(bytes);
        STARTUPINFOEXW startup{}; startup.StartupInfo.cb = sizeof(startup);
        startup.StartupInfo.dwFlags = STARTF_USESHOWWINDOW | STARTF_FORCEOFFFEEDBACK; startup.StartupInfo.wShowWindow = SW_HIDE;
        startup.lpAttributeList = reinterpret_cast<LPPROC_THREAD_ATTRIBUTE_LIST>(attributes.data());
        if (!InitializeProcThreadAttributeList(startup.lpAttributeList, 1, 0, &bytes)) throw std::runtime_error("Recovery attribute initialization failed.");
        HANDLE inherit[]{parent_.get(), mapping_.get(), changed_.get(), ready_.get()};
        bool success = UpdateProcThreadAttribute(startup.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            inherit, sizeof(inherit), nullptr, nullptr) && CreateProcessW(nullptr, command.data(), nullptr, nullptr, TRUE,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW, nullptr, nullptr, &startup.StartupInfo, &process_);
        DeleteProcThreadAttributeList(startup.lpAttributeList);
        if (!success) throw std::runtime_error("Recovery process startup failed.");
        CloseHandle(process_.hThread); process_.hThread = nullptr;
        if (WaitForSingleObject(ready_.get(), 3000) != WAIT_OBJECT_0) {
            InterlockedExchange(&state_->stopping, 1); SetEvent(changed_.get());
            WaitForSingleObject(process_.hProcess, 1000); CloseHandle(process_.hProcess); process_.hProcess = nullptr;
            view_.reset(); state_ = nullptr;
            throw std::runtime_error("Recovery process did not become ready; cursor changes disabled.");
        }
    }
    HANDLE process() const { return process_.hProcess; }
    DWORD processId() const { return process_.dwProcessId; }
    bool alive() const { return process_.hProcess && WaitForSingleObject(process_.hProcess, 0) == WAIT_TIMEOUT; }
    bool dirty() const { return state_ && InterlockedCompareExchange(&state_->dirty, 0, 0) != 0; }
    void heartbeat() { InterlockedExchange64(&state_->heartbeat, static_cast<LONG64>(GetTickCount64())); }
    bool begin() {
        if (!alive()) return false;
        heartbeat();
        if (!dirty()) {
            Handle file(CreateFileW(marker_.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_WRITE_THROUGH, nullptr));
            if (file.get() == INVALID_HANDLE_VALUE) return false;
            constexpr char text[] = "ShakeSpot native cursor recovery required\r\n";
            DWORD written = 0;
            if (!WriteFile(file.get(), text, sizeof(text) - 1, &written, nullptr) || !FlushFileBuffers(file.get())) return false;
            InterlockedExchange(&state_->dirty, 1);
            SetEvent(changed_.get());
        }
        return true;
    }
    bool restore() {
        if (!dirty()) return true;
        if (!reloadCursors()) return false;
        InterlockedExchange(&state_->dirty, 0);
        DeleteFileW(marker_.c_str());
        SetEvent(changed_.get());
        return true;
    }
    ~Guardian() {
        if (state_) {
            restore();
            InterlockedExchange(&state_->stopping, 1); SetEvent(changed_.get());
            if (process_.hProcess) WaitForSingleObject(process_.hProcess, 3000);
            view_.reset(); state_ = nullptr;
        }
        if (process_.hProcess) CloseHandle(process_.hProcess);
    }
};
}
