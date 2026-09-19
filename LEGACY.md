# ShakeSpot

Windows 11 x64 的鼠标定位小工具。快速左右或上下往返晃动鼠标，指针位置就会出现黑色、白色描边的大箭头，平滑放大、停留并缩小消失。程序平时只在系统托盘运行，无需管理员权限、账户或网络。

第一版采用 C# / .NET 10：WPF 提供设置窗口，Windows Forms 提供托盘，Win32 分层窗口绘制箭头。这样可以直接控制物理像素坐标、透明命中测试和窗口激活行为，避免把 WPF 的 DIP 坐标与跨屏物理坐标混用。运行时没有第三方 NuGet 依赖。

**快速运行**

直接双击 `artifacts/publish/win-x64/ShakeSpot.exe`。发布版本自带 .NET 运行时，单个 EXE 可放到固定目录使用。首次运行后，在任务栏右下角的隐藏图标区域寻找箭头图标；双击打开设置，右键可暂停、启用、预览或退出。

默认灵敏度为 3/5，最大倍数为 4，效果总时长约 1.1 秒，开机启动关闭。建议在半秒内左右往返 2–3 次，每次移动约 1–2 厘米，再根据鼠标 DPI 和使用习惯调整灵敏度。按住任意鼠标键时不触发；正在显示的效果会在点击或拖拽开始时收起。

**构建、测试和发布**

需要 Windows x64、PowerShell 7、.NET 10 SDK。开发脚本优先使用 `.tools/dotnet/dotnet.exe`，不存在时使用 PATH 中的 SDK，不修改持久环境变量。

以下命令从仓库根目录执行：

```powershell
& '.\scripts\dev.ps1' build
```

```powershell
& '.\scripts\dev.ps1' test
```

```powershell
& '.\scripts\dev.ps1' run
```

```powershell
& '.\scripts\dev.ps1' publish
```

没有 SDK 时，可执行项目内的 `scripts/bootstrap-sdk.ps1`，它从微软官方元数据选择 .NET SDK 10.0.401，下载到 `.tools/dotnet` 并核验 SHA512；仅安装和首次发布需要联网。

已安装 SDK 的开发者也可在项目目录使用标准命令：

```powershell
dotnet build ShakeSpot.slnx -c Release
```

```powershell
dotnet run --project src/ShakeSpot -c Release
```

```powershell
dotnet run --project tests/ShakeSpot.Tests -c Release
```

```powershell
dotnet publish src/ShakeSpot/ShakeSpot.csproj -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:EnableCompressionInSingleFile=false -o artifacts/publish/win-x64
```

构建输出为 `src/ShakeSpot/bin/Release/net10.0-windows/win-x64/ShakeSpot.exe`，它依赖 .NET 10 Windows Desktop Runtime。发布输出为 `artifacts/publish/win-x64/ShakeSpot.exe`，包含运行时；首次启动会将捆绑的本机库解压到 .NET 管理的用户临时目录。

`dev.ps1 publish` 还会将 README、验证记录和运行时许可证一起打包为 `artifacts/ShakeSpot-0.1.0-win-x64.zip`。可用 `scripts/smoke-published.ps1` 单独检查发布 EXE 的启动、采样与正常退出。

发布 EXE 约 165 MiB。内部程序集不压缩，以降低启动内存，分发时由外层 ZIP 压缩。最终发布文件启动实测工作集约 69.7 MiB；设置窗口加载后的占用见验证记录。

**设置与运行行为**

- 灵敏度：1–5。数值越高，要求的移动幅度和累计路程越小。
- 最大放大倍数：2–8，步长 0.5。倍率相对于内置的 35 DIP 高箭头，并随当前显示器 DPI 缩放。
- 效果持续时间：0.5–3 秒，包含约 150 毫秒放大和 260 毫秒缩小。持续晃动延长当前效果；缩小途中再次触发从当前大小、透明度接续。
- 登录 Windows 时启动：默认关闭。用户保存勾选后，写入当前用户 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 下的 `ShakeSpot` 项；取消勾选删除该项。移动 EXE 后应重新关闭并开启此选项。任务管理器中禁用启动项可能覆盖此设置。
- 暂停状态会保存，重启后继续保持。锁屏或挂起时暂停采样，恢复后清空旧轨迹。

配置位于 `%LOCALAPPDATA%\ShakeSpot\settings.json`。保存使用临时文件替换；无效 JSON 会退回默认值并显示托盘提示，不会直接覆盖损坏文件。最后一次错误写入同目录的 `last-error.log`。不记录鼠标轨迹或按键内容。

重复启动会打开现有实例的设置。也可使用以下控制参数，作用于已运行的同一配置实例：`--settings`、`--pause`、`--resume`、`--preview`、`--exit`。除 `--settings` 外，如果没有运行中的实例，会返回退出码 2。

```powershell
& '.\artifacts\publish\win-x64\ShakeSpot.exe' --pause
```

**实现与系统状态**

识别逻辑在 `src/ShakeSpot.Core/ShakeDetector.cs`，完全独立于界面。固定容量轨迹缓冲区结合时间窗口、至少三次有效反向、最小幅度、累计路程、往返路程与范围的比值，以及净位移占比判断。普通单向快速移动不会仅因速度触发。采样间断、光标跳转、暂停、光标不可见和鼠标按键状态会使旧轨迹失效；松开按键后有短暂冷却。已触发的证据会被消耗，后续刷新必须来自新的往返。

启用时请求 15 毫秒的 DispatcherTimer 间隔，实际频率受 Windows 调度影响，目标约 60–67 次/秒。没有忙循环、全局鼠标钩子、Raw Input 注册、系统计时器精度修改或输入重放。暂停时停止计时器。识别路径不按帧创建集合；绘图复用 DIB、Graphics 和矢量路径，停留时只移动窗口，不重绘箭头。

覆盖层使用 `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`，通过 `UpdateLayeredWindow` 绘制预乘 Alpha 位图，并用 `SetWindowPos` 置顶。箭头尖端固定在 `GetPhysicalCursorPos` 返回的位置，窗口与绘图共同使用物理像素。清单启用 PerMonitorV2；隐藏的 1px DPI 探针在目标显示器内通过 `GetDpiForWindow` 获取缩放。右侧、底部边缘将箭身翻向屏内，保持尖端位置，使用滞回避免边界附近反复翻转。

正常应用代码不调用 `ShowCursor`、`SetCursor`、`SetSystemCursor`，不修改系统指针方案，不拦截、吞掉或合成输入。退出时销毁窗口、托盘、计时器和 GDI 资源；异常结束后 Windows 回收覆盖层，不需要恢复全局光标。光标隐藏、触控/触笔抑制或查询失败时立即撤下效果。自动桌面测试程序会在其自建测试窗口中临时隐藏光标及合成输入，这些操作不属于常驻应用。

**测试与验收**

`tests/ShakeSpot.Tests` 是无外部测试框架依赖的控制台测试程序，失败返回非零退出码。覆盖水平/垂直/斜向晃动、普通移动、快速单向移动、小幅抖动、缓慢往返、单次转向、大范围移动、拖拽、重复触发、采样中断、灵敏度、动画接续、边缘热点和配置损坏恢复。请使用上面的 `test` 命令，`dotnet test` 不会运行这个控制台测试程序。

真实桌面验证命令：

```powershell
& '.\scripts\dev.ps1' desktop-checks
```

它需要可交互的 Windows 桌面，会创建独立测试进程，短暂移动鼠标、点击、滚动和拖拽，然后恢复鼠标位置并关闭测试进程。运行时请先停下鼠标操作。测试使用 `artifacts/desktop-checks/profile` 的独立配置，不操作正常配置或开机启动项。结果、资源测量和截图写入 `artifacts/desktop-checks`。在禁止 SendInput 的沙箱、锁屏或非交互会话中运行会失败，应从普通桌面终端运行，无需管理员权限。

本次机器上的实测记录见 `VALIDATION.md`。负坐标布局测试通过不代表多显示器硬件验收通过，构建成功也不代表游戏或全部 DPI 场景已验证。

手动验收建议：

1. 启动发布 EXE，确认没有主窗口或控制台，托盘可见。左右、上下快速往返晃动，确认只出现一个箭头、尖端对齐，停止后自动消失。
2. 普通移动、跨屏快速单向移动、轻微抖动、点击文字和拖动窗口，检查误触。连续晃动 3 秒，确认效果延长而不是闪烁重启。
3. 在记事本或浏览器中触发后点击、滚动、拖选，确认焦点和输入正常。在应用自行隐藏光标时，确认没有额外箭头。
4. 在每块显示器上测试 100%、150%、175%、200% 等实际缩放，跨屏移动，包括副屏位于主屏左侧/上方的布局；靠近四角检查箭身可见且尖端没有偏移。
5. 托盘暂停后晃动应无效果，重新启用应恢复。修改设置、保存、退出再启动，确认参数保留。勾选开机启动后重新登录验证，再取消并再次验证。
6. 在动画显示时正常退出，再启动后用任务管理器结束进程，确认鼠标仍可见、输入正常。托盘缓存图标可能需悬停或等待 Explorer 刷新后消失。

**已知限制**

- 保留真实系统光标，因此白色、彩色、大号箭头或文本插入光标可能叠在黑色箭头上。高速移动时覆盖层可能落后一次采样；硬件光标本身始终正常工作。第一版不保证完全消除重影。
- 内置箭头固定为黑色白描边，不复制当前主题、文本指针或应用自定义光标。临近右/下边缘会改变朝向；跨屏时大小按新屏幕 DPI 调整。
- 独占全屏游戏、安全桌面、受保护的内容及某些特殊置顶窗口不在支持范围。玩游戏前可在托盘暂停；本程序不会捕获游戏输入。
- 定频采样可能漏掉采样间隔以内的极短按键或极快往返，不能保证所有使用习惯下零误触。实际手感仍需用物理鼠标调整。
- 尚未提供安装器、代码签名和自动更新；Windows 可能对未签名的新下载文件显示信誉提示。运行过程本身不联网。

参考资料及许可证核查见 `THIRD_PARTY_NOTICES.md`。本项目代码和箭头轮廓独立编写，没有复制 CursorFinder 的源码或图片。
