# ShakeSpot

晃动鼠标，找到指针。ShakeSpot 是一个 **Windows 11 x64** 鼠标定位工具：快速左右、上下或斜向往返晃动，真实系统指针会平滑放大为黑色、白色描边的大箭头，停留后恢复。

当前源码版本 **0.4.2**，使用 Rust + Win32。普通用户权限运行，驻留系统托盘，无需网络、账户或额外运行时。源码采用 [MIT 许可证](LICENSE)。

**下载与使用**

从仓库的 [Releases 页面](https://github.com/CELLOPOI/ShakeSpot/releases) 下载已发布的 Windows x64 压缩包，解压到固定目录，双击 `ShakeSpot.exe`。GitHub 自动提供的 Source code 压缩包是源码，需要自行编译。当前作为预发布版提供给朋友试用，包内附有“使用说明.txt”。

- 快速往返晃动即可触发；持续晃动会延长效果。
- 默认灵敏度 3/5、最大倍率 4、持续时间 1.1 秒，开机启动默认关闭。
- 托盘右键可暂停、设置、预览、恢复指针或退出；双击打开设置。
- 按住鼠标键时不触发，点击或拖拽开始会结束效果。
- “排除应用”可每行填写一个程序名（如 `notepad.exe`）或完整 EXE 路径，最多 32 项。匹配程序位于前台时临时停止识别，手动暂停状态独立保留。

原始移动量用于手势识别，屏幕坐标用于显示器定位和绘制；指针到达屏幕边缘后仍可识别相对甩动。更换鼠标硬件 DPI 后，可用灵敏度选项调整手感。

设置保存在 `%LOCALAPPDATA%\ShakeSpot\native-settings.ini`，错误日志为同目录的 `rust-error.log`。升级前退出旧实例；移动 EXE 后，如已启用开机启动，请在新位置重新保存启动选项。也可在 EXE 所在目录运行 `./ShakeSpot.exe --restore` 恢复系统指针。

**占用与验证**

输入、识别和事件队列采用固定容量；静止且无效果时等待系统消息，光标缓存有数量和位图预算并在闲置后释放。主程序和独立恢复进程使用同一个 EXE。

0.4.2 修复悬浮按钮、链接和文本区域切换时短暂闪回小光标的问题。尺寸或方向变化时同步标准光标角色，角色切换本身不重建位图；动画期间会增加系统光标替换次数，具体占用见 [本次验证](validation/HOVER-FIX-0.4.2.md)。

0.4.1 增加必要条件预筛，减少无效移动的重复计算。14 轮同机微基准中，无效轨迹的检测耗时降低约 94%～97%，有效水平晃动耗时增加约 4%；检测器增加 8 字节状态，热路径保持零堆分配。整程序 CPU 和内存仍处于旧版量级。

历史 0.4.1 已通过 54 项 Rust 测试、86 项桌面检查，以及 1,161,102 个输入样本的逐点行为对照；0.4.2 的新增悬浮回归和资源结果见上述本次验证。环境、测量口径和未覆盖项见 [验证报告](RUST-VALIDATION.md)，版本变化见 [更新记录](CHANGELOG.md)。这些数据描述已测试环境，不代表所有设备的占用与行为。

**从源码构建**

需要 Windows x64、PowerShell 7、[Rust MSVC 工具链](https://rust-lang.org/tools/install/)，以及 Visual Studio Build Tools 的“使用 C++ 的桌面开发”和 Windows SDK。仓库通过 `rust-toolchain.toml` 选择 stable 工具链并要求 rustfmt、Clippy。

克隆后，在仓库根目录执行：

```powershell
./scripts/rust.ps1 check
./scripts/rust.ps1 test
./scripts/rust.ps1 build
./scripts/rust.ps1 run
```

打包执行 `./scripts/rust.ps1 publish`，输出到 `artifacts/publish/rust-win-x64/`，同时生成版本化 ZIP 和 `.zip.sha256` 校验文件。下载包包含程序、中文使用说明、许可证及校验信息；开发文档和性能记录保留在仓库中。`publish` 只在本地生成文件。

`./scripts/rust.ps1 bench` 测量检测、输入和栅格化热路径。`./scripts/rust.ps1 desktop-checks` 会移动鼠标、操作自建窗口并模拟测试进程异常退出，请先退出常用实例并停止手动输入；它使用独立配置，结束后恢复现场。发布后的启动检查为 `./scripts/smoke-rust.ps1`。

GitHub Actions 配置了 Windows 构建、格式检查、Clippy、逻辑测试、基准和 ZIP 打包。涉及桌面交互的检查在本机进行。开发与性能约束见 [CONTRIBUTING.md](CONTRIBUTING.md)，维护者发布步骤见 [PUBLISHING.md](PUBLISHING.md)。

**使用边界**

支持标准系统光标，应用私有光标跳过放大。最大高度 256 物理像素，高 DPI 或高倍率下可能达不到所选倍率；屏幕右侧和底部会镜像箭身，热点仍对应真实点击位置。

混合 DPI 多屏、物理高回报率鼠标、辅助功能主题、锁屏/睡眠、远程桌面、长期运行与实际重新登录仍需对应环境验收。独占全屏游戏和安全桌面不在支持范围；运行游戏前可暂停。请避免同时使用其他替换系统指针的工具。

主进程崩溃或卡死时，恢复进程会尝试恢复；整个进程树同时结束后，需重新启动或执行 `--restore`。当前为未签名的便携版本，尚无安装器和自动更新。

**源码与参考**

当前产品源码位于 `rust/`，实现说明见 [RUST.md](RUST.md)。历史 C++ 实现在 `native/`，对应 [NATIVE.md](NATIVE.md) 和 [NATIVE-VALIDATION.md](NATIVE-VALIDATION.md)；早期 C#/WPF 实现在 `src/`，对应 [LEGACY.md](LEGACY.md)。它们用于历史对照，不参与当前产品构建。独立桌面验证驱动位于 `validation/`。

输入处理、轨迹筛选和运行状态设计参考了 [PowerToys Find My Mouse](https://github.com/microsoft/PowerToys/tree/main/src/modules/MouseUtils/FindMyMouse)，并查阅了 [CursorFinder](https://github.com/MrBeanCpp/CursorFinder) 的功能与设计。具体来源和依赖许可见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。ShakeSpot 是独立项目。
