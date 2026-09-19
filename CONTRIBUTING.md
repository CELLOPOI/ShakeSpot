# 参与开发

当前维护 Rust + Win32 实现，代码位于 `rust/`；`native/`、`src/` 和对应子目录测试保留历史实现。独立桌面验证驱动位于 `validation/`。

从仓库根目录在 PowerShell 7 中执行：

```powershell
./scripts/rust.ps1 check
./scripts/rust.ps1 test
./scripts/rust.ps1 build
```

修改输入、手势或栅格化时执行 `./scripts/rust.ps1 bench`，保存同一环境下修改前后的数据。性能改动需要同时检查识别准确性、触发延迟、分配次数和资源回收，约束见 [AGENTS.md](AGENTS.md)。

修改输入注册、运行状态、动画、光标或恢复流程时，在可交互的 Windows 桌面执行 `./scripts/rust.ps1 desktop-checks`。测试会移动鼠标、操作自建窗口并模拟测试进程异常退出；运行前退出常用实例并停止手动输入。CI 运行逻辑测试和构建，桌面检查需在本机完成。

提交问题时请提供版本、Windows 版本、缩放比例、鼠标 DPI、最短复现步骤，以及暂停或退出后是否恢复。日志位于 `%LOCALAPPDATA%\ShakeSpot\rust-error.log`，分享前检查其中的本机路径。提交改动时说明用户可见的结果、验证方式和性能影响。

原创代码使用 [MIT 许可证](LICENSE)，第三方组件保留各自许可。引入依赖或外部代码时同步更新 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 和相关许可文本。
