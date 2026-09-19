# 发布到 GitHub

仓库为 `CELLOPOI/ShakeSpot`，默认分支为 `main`。首次公开版本采用预发布形式供朋友试用。下面的创建仓库和推送命令会公开源码，请在准备正式上传时执行。它们不由构建脚本或 CI 自动运行。

**发布内容**

仓库包含源码、锁定依赖文件、测试、构建脚本、文档和许可证。`target/`、`artifacts/`、`work/`、本机工具和缓存、环境变量文件、证书和日志由 `.gitignore` 排除。历史 C++ 与 C# 源码保留用于对照，当前产品使用 Rust。

用户下载文件放在 GitHub Release：

- `ShakeSpot-0.4.1-win-x64.zip`：程序、中文使用说明、MIT 和依赖许可、构建信息；开发说明和验证记录保留在仓库。
- `ShakeSpot-0.4.1-win-x64.zip.sha256`：整个 ZIP 的 SHA-256。

ZIP 内的 `SHA256SUMS.txt` 记录每个打包文件的 SHA-256；`BUILD-INFO.txt` 记录实际工具链和 EXE 哈希。发布包只从明确的文件清单生成。

**本机验证与打包**

在仓库根目录的 PowerShell 7 中执行：

```powershell
./scripts/rust.ps1 check
./scripts/rust.ps1 test
./scripts/rust.ps1 publish
```

涉及输入、识别或绘制性能时增加 `./scripts/rust.ps1 bench`。涉及运行状态或资源生命周期时，在可交互桌面执行 `./scripts/rust.ps1 desktop-checks`；它会移动鼠标，运行前停止手动输入。正式发布的 EXE 还应运行 `./scripts/smoke-rust.ps1`。纯文档和打包清单变更，若 EXE 哈希与已验收文件相同，可复用对应桌面结果。

在发布包所在目录核对 ZIP：

```powershell
$zipName = 'ShakeSpot-0.4.1-win-x64.zip'
$expected = ((Get-Content -LiteralPath "$zipName.sha256" -Raw).Trim() -split '\s+')[0]
if ((Get-FileHash -LiteralPath $zipName -Algorithm SHA256).Hash -ne $expected) { throw 'ZIP checksum mismatch' }
```

GitHub Actions 在 `windows-2022` 上执行格式、Clippy、逻辑测试、基准和打包，产物保留 14 天。云端基准只用于检查执行和分配约束，不能与本机耗时直接比较；桌面检查由维护者本机完成。每次发布应核对对应提交的 Actions 结果。

**首次上传**

确认 `gh auth status` 对应自己的账号，在仓库根目录查看 `git status` 和 `git diff --cached --stat`。本地已初始化时不用再次执行 `git init`；重新解压源码且没有 `.git` 时，先执行 `git init -b main`。

确认 Git 的提交用户名和邮箱是希望公开的身份；可使用 GitHub 的 noreply 邮箱。随后执行：

```powershell
git add .
git commit -m "Prepare ShakeSpot 0.4.1 for GitHub"
gh repo create CELLOPOI/ShakeSpot --public --source . --remote origin --push --description "Windows 鼠标定位工具：晃动鼠标放大真实指针，Rust + Win32，低占用。"
```

若目标仓库已经存在，先核对其内容，再连接对应 remote；不要覆盖已有历史。仓库名称改变时同步更新 README 的 Releases 链接。

**创建版本发布**

等待 Actions 通过，检查 `Cargo.toml`、资源版本、清单和更新记录一致，确认用于上传的 ZIP 对应已验收程序，然后创建标签：

```powershell
git tag -a v0.4.1 -m "ShakeSpot 0.4.1"
git push origin v0.4.1
gh release create v0.4.1 artifacts/ShakeSpot-0.4.1-win-x64.zip artifacts/ShakeSpot-0.4.1-win-x64.zip.sha256 --repo CELLOPOI/ShakeSpot --verify-tag --draft --prerelease --title "ShakeSpot 0.4.1 试用版" --notes-file docs/releases/v0.4.1.md
```

这会先创建预发布草稿。检查附件、说明和校验文件后，在 GitHub 的 Release 页面发布草稿，或执行：

```powershell
gh release edit v0.4.1 --repo CELLOPOI/ShakeSpot --draft=false --prerelease
```

发布后将 `https://github.com/CELLOPOI/ShakeSpot/releases/tag/v0.4.1` 发给朋友，让对方下载 ZIP 并解压运行。不要重新编译 EXE 后沿用旧哈希或未经核对的性能数据。

后续升级需同步更新 `Cargo.toml` / `Cargo.lock`、`rust/resources.rc`、`rust/app.manifest`、README、更新记录、`packaging/使用说明.txt` 和对应的 `docs/releases/v<版本>.md`。打包文件名从 Cargo 元数据读取。
