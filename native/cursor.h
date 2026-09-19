#pragma once
#include "platform.h"

namespace shakespot {
inline constexpr std::array<UINT, 13> CursorIds{OCR_NORMAL, OCR_IBEAM, OCR_WAIT, OCR_CROSS, OCR_UP,
    OCR_SIZENWSE, OCR_SIZENESW, OCR_SIZEWE, OCR_SIZENS, OCR_SIZEALL, OCR_NO, OCR_HAND, OCR_APPSTARTING};

// 通过超采样的原生 GDI 多边形生成带 Alpha 的真光标，不创建覆盖层窗口。
inline HCURSOR makeArrow(int height, bool flipX = false, bool flipY = false, bool icon = false) {
    constexpr int supersample = 3, padding = 2;
    double scale = (height - padding * 2) / 35.0;
    int width = static_cast<int>(std::ceil(26 * scale)) + padding * 2;
    int sw = width * supersample, sh = height * supersample;
    BITMAPINFO info{};
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = sw; info.bmiHeader.biHeight = -sh;
    info.bmiHeader.biPlanes = 1; info.bmiHeader.biBitCount = 32;
    uint32_t* large = nullptr;
    HBITMAP source = CreateDIBSection(nullptr, &info, DIB_RGB_COLORS, reinterpret_cast<void**>(&large), nullptr, 0);
    if (!source) return nullptr;
    std::fill_n(large, static_cast<size_t>(sw) * sh, 0x00ff00ffu);
    HDC dc = CreateCompatibleDC(nullptr);
    if (!dc) { DeleteObject(source); return nullptr; }
    HGDIOBJ previous = SelectObject(dc, source);
    HGDIOBJ oldPen = SelectObject(dc, GetStockObject(NULL_PEN));
    HGDIOBJ oldBrush = SelectObject(dc, GetStockObject(WHITE_BRUSH));
    constexpr double outer[7][2]{{0,0},{0,29},{8,22},{14,35},{21,31},{14.5,19},{26,19}};
    constexpr double inner[7][2]{{1.6,3.4},{1.6,25.5},{8.5,19.5},{14.8,32.7},{18.7,30.3},{11.9,17.4},{21.3,17.4}};
    POINT points[7]{};
    auto polygon = [&](const double (&vertices)[7][2]) {
        for (int i = 0; i < 7; ++i) {
            points[i] = {static_cast<LONG>(std::lround((padding + vertices[i][0] * scale) * supersample)),
                static_cast<LONG>(std::lround((padding + vertices[i][1] * scale) * supersample))};
        }
        Polygon(dc, points, 7);
    };
    polygon(outer);
    SelectObject(dc, GetStockObject(BLACK_BRUSH)); polygon(inner);
    GdiFlush();
    SelectObject(dc, oldBrush); SelectObject(dc, oldPen); SelectObject(dc, previous); DeleteDC(dc);

    info.bmiHeader.biWidth = width; info.bmiHeader.biHeight = -height;
    uint32_t* pixels = nullptr;
    HBITMAP color = CreateDIBSection(nullptr, &info, DIB_RGB_COLORS, reinterpret_cast<void**>(&pixels), nullptr, 0);
    if (!color) { DeleteObject(source); return nullptr; }
    for (int y = 0; y < height; ++y) for (int x = 0; x < width; ++x) {
        unsigned alpha = 0, white = 0;
        for (int dy = 0; dy < supersample; ++dy) for (int dx = 0; dx < supersample; ++dx) {
            uint32_t pixel = large[(y * supersample + dy) * sw + x * supersample + dx] & 0xffffff;
            if (pixel != 0xff00ff) { alpha += 255; white += pixel == 0xffffff ? 255 : 0; }
        }
        alpha /= supersample * supersample; white /= supersample * supersample;
        int tx = flipX ? width - 1 - x : x, ty = flipY ? height - 1 - y : y;
        pixels[ty * width + tx] = (alpha << 24) | (white << 16) | (white << 8) | white;
    }
    DeleteObject(source);
    std::vector<unsigned char> maskBits(static_cast<size_t>((width + 15) / 16) * 2 * height, 0);
    HBITMAP mask = CreateBitmap(width, height, 1, 1, maskBits.data());
    ICONINFO cursor{}; cursor.fIcon = icon;
    cursor.xHotspot = flipX ? width - 1 - padding : padding;
    cursor.yHotspot = flipY ? height - 1 - padding : padding;
    cursor.hbmColor = color; cursor.hbmMask = mask;
    HCURSOR result = mask ? reinterpret_cast<HCURSOR>(CreateIconIndirect(&cursor)) : nullptr;
    if (mask) DeleteObject(mask); DeleteObject(color);
    return result;
}

class CursorFrames {
    struct Entry { HCURSOR cursor = nullptr; int key = -1; uint64_t used = 0; };
    // 不同 DPI 和边缘朝向可能产生大量组合；限定缓存，避免长期晃动积累资源。
    std::array<Entry, 32> frames_{};
    uint64_t clock_ = 0;
public:
    ~CursorFrames() { clear(); }
    void clear() { for (auto& entry : frames_) { if (entry.cursor) DestroyCursor(entry.cursor); entry = {}; } clock_ = 0; }
    HCURSOR get(int height, bool flipX, bool flipY) {
        height = std::clamp((height + 1) / 2 * 2, 24, 256);
        int orientation = (flipX ? 1 : 0) | (flipY ? 2 : 0);
        int key = orientation * 129 + height / 2;
        for (auto& entry : frames_) if (entry.key == key && entry.cursor) { entry.used = ++clock_; return entry.cursor; }
        auto& oldest = *std::min_element(frames_.begin(), frames_.end(), [](const Entry& a, const Entry& b) { return a.used < b.used; });
        HCURSOR created = makeArrow(height, flipX, flipY);
        if (!created) return nullptr;
        if (oldest.cursor) DestroyCursor(oldest.cursor);
        oldest = {created, key, ++clock_}; return created;
    }
};

class CursorRoles {
    std::array<HCURSOR, CursorIds.size()> roles_{};
public:
    CursorRoles() { refresh(); }
    void refresh() {
        for (size_t i = 0; i < roles_.size(); ++i) roles_[i] = LoadCursorW(nullptr, MAKEINTRESOURCEW(CursorIds[i]));
    }
    int identify(HCURSOR cursor) const {
        for (size_t i = 0; i < roles_.size(); ++i) if (roles_[i] && roles_[i] == cursor) return static_cast<int>(i);
        return -1;
    }
};
}
