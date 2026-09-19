# 第三方声明

ShakeSpot 原创代码采用根目录 [MIT 许可证](LICENSE)。依赖和历史运行库仍适用各自许可。

当前 Rust 实现的原始输入处理、轨迹长度与包围盒筛选、运行状态处理和减少重复判定的思路参考了微软 [PowerToys Find My Mouse](https://github.com/microsoft/PowerToys/blob/cb25632f5b61eb2e57a3b0b52053409403ae9e98/src/modules/MouseUtils/FindMyMouse/FindMyMouse.cpp)，参考版本为 `cb25632f5b61eb2e57a3b0b52053409403ae9e98`。PowerToys 使用 [MIT 许可证](https://github.com/microsoft/PowerToys/blob/cb25632f5b61eb2e57a3b0b52053409403ae9e98/LICENSE)。ShakeSpot 在 Rust 中独立实现，保留自己的固定容量采样、主方向反向幅度判定和系统指针恢复流程，没有把 PowerToys 的 C++ 或渲染资源加入产品。

也查阅了 [MrBeanCpp/CursorFinder](https://github.com/MrBeanCpp/CursorFinder) 的功能和设计。其 [许可证](https://github.com/MrBeanCpp/CursorFinder/blob/master/LICENSE) 为 GPL-2.0；本仓库未引入其 C++/Qt 源码或图片。参考来源的许可证不替代本项目依赖自身的声明。

当前 0.4.1 产品使用 Rust 标准库，以及微软维护的 windows-sys 0.61.2 / windows-link 0.2.1。依赖由 Cargo.lock 固定，未引入 GUI 框架或垃圾回收运行时。Windows API 绑定来自 [microsoft/windows-rs](https://github.com/microsoft/windows-rs)，采用 MIT 或 Apache-2.0；随包提供从实际 Cargo 下载包复制的许可证。Rust 标准库声明来自本机 1.93.1 工具链的 COPYRIGHT-library.html，涵盖标准库及其依赖，原文保存在 licenses/Rust-1.93.1-COPYRIGHT-library.html。

发布脚本还会把实际编译工具链对应的标准库声明复制为发布包中的 licenses/Rust-COPYRIGHT-library.html，并写入 BUILD-INFO.txt，避免升级工具链后仍沿用旧版本声明。Release 静态链接 CRT，Microsoft 编译工具及运行库适用其自身许可。未签名的程序仍可能触发 Windows 的下载来源或信誉提示。

原生接口行为参考微软文档，独立编写业务实现：

- [Raw Input](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input)
- [SetSystemCursor](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setsystemcursor)
- [CreateIconIndirect](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createiconindirect)
- [SystemParametersInfoW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow)

以下保留早期 C++ / C# 版本的来源说明，仅用于对应历史源码和历史发布物。

# 来源与第三方声明

本项目代码、界面和箭头矢量轮廓独立编写。没有引入第三方 NuGet 包，没有复制其他项目的源代码或视觉资源。

查阅了 [MrBeanCpp/CursorFinder](https://github.com/MrBeanCpp/CursorFinder) 的 README，了解其“晃动定位”和透明窗口方案。该仓库的 [LICENSE](https://github.com/MrBeanCpp/CursorFinder/blob/master/LICENSE) 为 GPL-2.0。本项目未使用其中的 C++/Qt 源码、图片或派生代码，因此没有把该仓库代码纳入本次交付。

Win32 行为依据微软官方文档：

- [Layered Windows](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows)：分层窗口的透明命中测试与鼠标事件穿透。
- [UpdateLayeredWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-updatelayeredwindow)：更新覆盖层的位置、尺寸和逐像素透明内容。
- [CURSORINFO](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-cursorinfo)：识别光标可见、隐藏与触控/触笔抑制状态。
- [GetPhysicalCursorPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getphysicalcursorpos)：获取物理屏幕坐标。
- [Setting process DPI awareness](https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process)：在清单中声明 PerMonitorV2。
- [WFO0003](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/compiler-messages/wfo0003)：WinForms 的 DPI 分析器。此项目入口为 WPF，保留清单声明并定向关闭这条 WinForms 警告。

旧版 C# 自包含发布附带 Microsoft .NET / Windows Desktop 运行时。该发布目录保留运行时的 LICENSE / ThirdPartyNotices 文件（如 SDK 输出），另外随包提供微软官方的 .NET 许可证及第三方声明。它们的许可条款继续适用于所包含的运行时组件，不因本项目的原创实现而改变。

历史 C++ 0.2.0 版本使用 MSVC C++20，以 `/MT` 静态链接 Release C/C++ 运行库，运行时调用 Windows 自带 DLL，不包含 .NET、Rust、Qt 或第三方 GUI 框架。Microsoft C++ Standard Library 的标注为 `Apache-2.0 WITH LLVM-exception`，随包保留 [Microsoft STL LICENSE](licenses/Microsoft-STL-LICENSE.txt)，文本取自本机 Visual Studio 2022 Build Tools 的 `Licenses/BuildTools/2052/ThirdPartyNotices.txt` 中 STL 独立段落，并核对了 [微软官方 STL 许可证](https://github.com/microsoft/STL/blob/main/LICENSE.txt)。编译器、Windows SDK 和运行库仍适用各自的微软许可条件；此说明不为其重新授予许可。

原生实现另查阅以下官方文档，代码独立编写：

- [Raw Input](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input)：按鼠标事件接收输入，后台注册及消息生命周期。
- [SetSystemCursor](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setsystemcursor)：替换标准系统指针，以及传入句柄被系统销毁的所有权要求。
- [CreateIconIndirect](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createiconindirect)：颜色位图、透明掩码和光标热点。
- [CopyImage](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-copyimage)：复制光标帧。
- [SystemParametersInfoW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow)：`SPI_SETCURSORS` 重新加载已配置的系统指针。

语言选型查阅 [Rust 官方介绍](https://rust-lang.org/) 和 [微软 windows-rs](https://github.com/microsoft/windows-rs)，历史 0.2.0 原生程序没有引入这些项目的代码或依赖。
