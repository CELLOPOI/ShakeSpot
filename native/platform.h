#pragma once
#ifndef UNICODE
#define UNICODE
#endif
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define OEMRESOURCE
#include <windows.h>
#include <windowsx.h>
#include <shellapi.h>
#include <shlobj.h>
#include <commctrl.h>
#include <wtsapi32.h>
#include <psapi.h>
#include <array>
#include <string>
#include <string_view>
#include <vector>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <memory>
#include <cstdint>
#include <stdexcept>
#include "core.h"

namespace shakespot {
inline constexpr wchar_t WindowClass[] = L"ShakeSpot.Native.Control.v2";
inline constexpr wchar_t MutexName[] = L"Local\\ShakeSpot.Native.SystemCursor.v2";
inline constexpr UINT CommandMessage = WM_APP + 30;
inline constexpr UINT QueryMessage = WM_APP + 31;
enum Command { OpenSettings = 1, Pause, Resume, Preview, Quit, Restore, TestHang };
inline double clockMs() {
    static const double frequency = [] { LARGE_INTEGER v{}; QueryPerformanceFrequency(&v); return double(v.QuadPart) / 1000; }();
    LARGE_INTEGER v{}; QueryPerformanceCounter(&v); return double(v.QuadPart) / frequency;
}
inline std::wstring executablePath() {
    std::wstring path(32768, L'\0');
    DWORD length = GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
    if (!length || length == path.size()) throw std::runtime_error("Executable path unavailable.");
    path.resize(length); return path;
}
inline std::wstring quote(const std::wstring& text) { return L"\"" + text + L"\""; }
inline bool reloadCursors() { return SystemParametersInfoW(SPI_SETCURSORS, 0, nullptr, 0) != FALSE; }
inline bool cursorInfo(CURSORINFO& info) { info = {}; info.cbSize = sizeof(info); return GetCursorInfo(&info) != FALSE; }
inline bool buttonsDown() {
    return (GetAsyncKeyState(VK_LBUTTON) | GetAsyncKeyState(VK_RBUTTON) | GetAsyncKeyState(VK_MBUTTON) |
        GetAsyncKeyState(VK_XBUTTON1) | GetAsyncKeyState(VK_XBUTTON2)) & 0x8000;
}
inline std::wstring profilePath() {
    PWSTR path = nullptr;
    if (FAILED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &path))) throw std::runtime_error("LocalAppData unavailable.");
    std::wstring result(path); CoTaskMemFree(path); return result + L"\\ShakeSpot";
}
inline void logError(const std::wstring& directory, const std::string& error) noexcept {
    try { std::filesystem::create_directories(directory); std::ofstream(std::filesystem::path(directory) / L"native-error.log") << error; }
    catch (...) {}
}
struct Handle {
    HANDLE value = nullptr;
    Handle() = default;
    explicit Handle(HANDLE h) : value(h) {}
    ~Handle() { if (value && value != INVALID_HANDLE_VALUE) CloseHandle(value); }
    Handle(const Handle&) = delete;
    Handle& operator=(const Handle&) = delete;
    HANDLE get() const { return value; }
};
}
