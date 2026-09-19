#pragma once
#include "platform.h"
#include <iomanip>

namespace shakespot {
inline Settings loadSettings(const std::wstring& directory) {
    Settings result;
    std::wifstream input(std::filesystem::path(directory) / L"native-settings.ini");
    std::wstring key;
    while (std::getline(input, key)) {
        auto separator = key.find(L'=');
        if (separator == std::wstring::npos) continue;
        auto value = key.substr(separator + 1); key.resize(separator);
        try {
            if (key == L"sensitivity") result.sensitivity = std::stoi(value);
            else if (key == L"maximumScale") result.maximumScale = std::stod(value);
            else if (key == L"durationMs") result.durationMs = std::stoi(value);
            else if (key == L"enabled") result.enabled = std::stoi(value) != 0;
            else if (key == L"startup") result.startup = std::stoi(value) != 0;
        } catch (...) {}
    }
    result.normalize(); return result;
}
inline bool saveSettings(const std::wstring& directory, const Settings& settings) {
    std::filesystem::create_directories(directory);
    auto file = std::filesystem::path(directory) / L"native-settings.ini";
    auto temporary = file; temporary += L".tmp";
    std::ofstream out(temporary, std::ios::trunc);
    out << "sensitivity=" << settings.sensitivity << "\nmaximumScale=" << settings.maximumScale <<
        "\ndurationMs=" << settings.durationMs << "\nenabled=" << settings.enabled << "\nstartup=" << settings.startup << '\n';
    out.close();
    return out.good() && MoveFileExW(temporary.c_str(), file.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH);
}
inline bool startupEnabled() {
    HKEY key = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Run", 0, KEY_QUERY_VALUE, &key) != ERROR_SUCCESS) return false;
    wchar_t value[32768]{}; DWORD size = sizeof(value), type = 0;
    LSTATUS result = RegQueryValueExW(key, L"ShakeSpotNative", nullptr, &type, reinterpret_cast<BYTE*>(value), &size);
    RegCloseKey(key);
    return result == ERROR_SUCCESS && type == REG_SZ && quote(executablePath()) == value;
}
inline bool setStartup(bool enabled) {
    HKEY key = nullptr;
    if (RegCreateKeyExW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Run", 0, nullptr, 0, KEY_SET_VALUE, nullptr, &key, nullptr) != ERROR_SUCCESS) return false;
    auto path = quote(executablePath());
    LSTATUS result = enabled ? RegSetValueExW(key, L"ShakeSpotNative", 0, REG_SZ, reinterpret_cast<const BYTE*>(path.c_str()),
        static_cast<DWORD>((path.size() + 1) * sizeof(wchar_t))) : RegDeleteValueW(key, L"ShakeSpotNative");
    RegCloseKey(key);
    return result == ERROR_SUCCESS || (!enabled && result == ERROR_FILE_NOT_FOUND);
}
}
