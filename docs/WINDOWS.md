# Windows 10 / 11 开发包

当前目标：Windows 10 / 11 **64 位 x86-64**，优先验证 Win10 22H2 与 Win11。32 位 Windows、Windows ARM64 尚未纳入当前交付。Linux 暂缓。与 macOS 共用 Rust 文件引擎和原生 GUI，不使用 WebView。

此包为未签名的 Phase 1 开发原型。Windows EXE 已可在 Mac 上交叉编译，PE 架构、桌面/控制台子系统、嵌入清单和静态 CRT 依赖会在打包时检查；**交叉编译不代表 Win10/11 已运行通过**。Windows Server 2022 云端构建、39 项自动测试、CLI 和便携包打包已通过；Windows 10/11 桌面验证结果需另行记录。

## 解压与验证（不需要安装 Rust、Python 或管理员权限）

1. 将整个 ZIP 解压到本机 NTFS 上可写的新目录，保留同目录的文件。不要在 ZIP 预览里直接运行，也不要先用个人重要文件测试。
2. 双击 `Verify-Windows.cmd`。脚本仅在系统临时目录新建 `FileSpring-verify-...` 样本目录，验证复制、移动、预演、冲突拒绝、撤销、日志排他锁与持久化重开，再打开原生窗口约 3 秒。不会清理样本或修改系统配置。
3. 将控制台结果、打印路径中的 `verification.json`、`window-smoke.txt` 和 `winver` 显示的系统版本保留下来。样本保留以便检查；若失败，保留错误文字。
4. 双击 `Launch-Demo.cmd` 进入包内自动生成的演示目录。左栏打开 Inbox、右栏 Library，分别测试复制和移动的预演→确认→历史→确认撤销。另测 Ctrl 多选、Shift 区间、Ctrl+A/C/X/V/L、中文文本预览、目录选择器、深浅主题和 100%/150%/200% 缩放。
5. 正常启动可双击 `FileSpring.exe`，首先选择工作目录。启动错误会显示对话框；GUI 不附带控制台黑窗。CLI 位于 `FileSpring-cli.exe`。

验证脚本不会修改执行策略，也不需要 PowerShell 脚本权限。若组织策略或系统安全提示阻止运行，保留提示并反馈；不要为了本原型关闭系统防护。

## 当前 Windows 限制

- 目前以本地 NTFS 普通文件为验证基线，目标卷需支持硬链接。FAT/exFAT、NAS/SMB、真实跨卷及断连未验证。
- 只读源文件的修改操作会在预演时拒绝。不会自动清除只读属性；Windows ACL、ADS、稀疏属性等尚未完整保留。
- 拒绝 NTFS 流路径、DOS 设备别名、尾随空格/点及重解析点遍历。Junction、独占锁、超 260 字符路径的集成测试已在 Windows Server 2022 CI 上运行通过，Windows 10/11 仍需验证。
- 清单声明 Windows 10/11 兼容、普通用户权限和长路径感知，不会更改系统长路径策略；不能据此宣称所有长路径组合都兼容。
- 系统文件剪贴板与资源管理器互通、原生拖放、旧机器显卡驱动和低配性能仍需实测。当前使用 Glow/OpenGL，需要可用的 OpenGL 驱动。
- 未代码签名、未生成安装器、尚未完成正式发行所需的许可与兼容性审核。

## 在 Windows 上构建

开发环境需要 Rust 1.95+ 的 MSVC 工具链、Visual Studio C++ Build Tools/Windows SDK，以及 Python 3.11+。这些是开发依赖，便携包使用者不需要安装。

```powershell
python tools/build_windows.py
```

该脚本执行格式、Clippy、测试、Release、真实 CLI 子进程测试和打包；已缓存全部依赖时可加 `--offline`。成品位于 `artifacts/FileSpring-0.1.0-windows-x64-portable.zip`，另有 SHA-256 文件。构建不会更改系统级配置，也不会自动安装上述开发依赖。

原生 Windows 构建使用 `.cargo/config.toml` 静态链接 CRT；`build.rs` 将清单嵌入 GUI 和 CLI。GitHub Actions 会构建并上传 Windows ZIP 与 macOS 包，但 Server 2022 CI 不能替代 Win10/11 的桌面交互检查，已通过的远端记录见 [GitHub Actions](https://github.com/CyBerKitTen0009/FileSpring/actions/runs/42546291161)。

## Mac 上复现交叉构建

使用已经配置的 Rust Windows MSVC target、Clang/LLVM 和具有 `crt/`、`sdk/` 目录的 SDK 缓存：

```sh
python3 tools/cross_windows.py --sdk /path/to/sdk --llvm /path/to/llvm/bin --linker /path/to/lld-link --rust-bin /path/to/rust/bin
python3 tools/package_windows.py
```

脚本不安装工具、不下载 SDK、不接受新许可；Windows 测试可用 `--tests` 编译，Clippy 可用 `--check` 检查，均不会冒充运行测试。交叉构建用 LLVM 资源编译器嵌入同一清单，避免依赖 Windows `mt.exe`。

清单设置依据 [Microsoft 应用清单文档](https://learn.microsoft.com/en-us/windows/win32/sbscs/application-manifests)，文件名限制依据 [Microsoft 文件与路径命名文档](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)。这些文档说明接口语义，不能替代实机测试。
