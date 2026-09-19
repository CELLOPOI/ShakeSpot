> 历史记录：本文描述 0.2.0 C++ 版本。当前产品已迁移到 Rust，请从 README.md 和 RUST.md 开始。

# ShakeSpot

Windows 11 x64 的鼠标定位工具。快速左右或上下往返晃动鼠标，实际系统指针会变为黑色、白色描边的大箭头，平滑放大、停留并缩小恢复。平时驻留系统托盘，普通用户权限运行，无需网络或账户。

当前版本采用 **C++20 / Win32**：Raw Input 接收鼠标事件，`SetSystemCursor` 临时替换当前标准系统指针，原生对话框提供设置。鼠标移动由 Windows 自己绘制，没有额外跟随箭头窗口。另有一个独立恢复进程，处理主程序崩溃或动画期间卡死；两者使用同一个 EXE。

这是针对低占用和单一真实指针目标的原生实现。最初的 C# / WPF 覆盖层版本保留在 `src/`、`tests/`，其说明见 [LEGACY.md](LEGACY.md)，不需要同时运行两个版本。

**直接运行**

双击 `artifacts/publish/native-win-x64/ShakeSpot.Native.exe`。发布包为 `artifacts/ShakeSpot-0.2.0-native-win-x64.zip`，无需安装 .NET 或 VC++ 运行时。托盘右键提供暂停、设置、预览、恢复原系统指针和退出；双击打开设置。

默认灵敏度 3/5、最大倍率 4、总持续时间 1.1 秒，开机启动关闭。按住任意鼠标键时不会触发；点击或拖拽开始会结束当前效果。持续晃动延长同一次动画。设置保存在 `%LOCALAPPDATA%\ShakeSpot\native-settings.ini`。

**可复制的单行命令**

构建需要 Windows x64、PowerShell 7、Visual Studio 2022 Build Tools 的“使用 C++ 的桌面开发”和 Windows SDK。脚本自动寻找本机工具链，不修改持久环境变量。

构建：

```powershell
& '.\scripts\native.ps1' build
```

识别与动画测试：

```powershell
& '.\scripts\native.ps1' test
```

构建并运行：

```powershell
& '.\scripts\native.ps1' run
```

发布 EXE 和 ZIP：

```powershell
& '.\scripts\native.ps1' publish
```

直接运行已发布文件：

```powershell
Start-Process -FilePath '.\artifacts\publish\native-win-x64\ShakeSpot.Native.exe' -WindowStyle Hidden
```

真实桌面测试会短暂移动鼠标、点击和拖拽专用测试窗口，请在可交互桌面、停止手动操作时运行：

```powershell
& '.\scripts\native.ps1' desktop-checks
```

异常恢复入口：

```powershell
& '.\artifacts\publish\native-win-x64\ShakeSpot.Native.exe' --restore
```

构建 EXE 位于 `artifacts/native/ShakeSpot.Native.exe`。以上命令从仓库根目录执行。

最终发布文件也可使用 `scripts/smoke-native.ps1` 单独检查启动、暂停、恢复启用和退出。它使用隔离配置，不打开设置、不主动触发放大效果。

**验收与限制**

1. 启动后确认只有托盘图标，快速左右、上下往返各 2–3 次，检查放大、尖端对齐和自动恢复；持续晃动检查动画接续。
2. 普通移动、快速单向移动、小幅抖动、点击和拖拽不应误触。效果显示时继续点击、滚动、拖动，确认应用焦点和输入正常。
3. 暂停后晃动无效果；重新启用后可触发。保存设置、退出重启，检查配置保留；开机启动需自行重新登录验收。
4. 在每块物理显示器和不同缩放比例下验证，特别是负坐标副屏、跨屏和四角。右侧、底部边缘箭身会翻向屏内，热点仍在真实鼠标位置。
5. 动画显示时退出，再测试仅结束主进程，确认原指针恢复。结束整个进程树会同时停止恢复进程，此时重新启动或执行 `--restore`。

支持标准系统指针；应用私有光标会跳过效果。原生指针暂时限制为 256 物理像素，高 DPI 或高倍率会达到上限。独占全屏游戏、安全桌面不在支持范围，玩游戏可从托盘或 `--pause` 暂停。不要同时运行其他会替换系统指针的工具。

技术选型、异常恢复边界和代码结构见 [NATIVE.md](NATIVE.md)。实测结果、资源统计口径与待验证事项见 [NATIVE-VALIDATION.md](NATIVE-VALIDATION.md)。来源与第三方声明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
