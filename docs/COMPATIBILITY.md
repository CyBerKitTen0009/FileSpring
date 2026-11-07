# 兼容性矩阵

| 项目 | 当前状态 |
| --- | --- |
| macOS Apple Silicon / 当前本地卷 | 本地编译、自动化核心测试、原生 GUI 复制/移动与撤销已验证；磁盘格式未独立核实 |
| macOS Intel | 未验证，配置 CI 原生测试 |
| Windows 10/11 x86-64 | 已交叉编译 GUI/CLI、检查 PE 与清单、编译 Windows 测试并通过跨目标 Clippy；便携 ZIP 与验证脚本已提供，尚未在 Win10/11 执行 |
| Linux x86-64 | 按用户要求暂缓，无当前构建/打包 CI |
| Windows ARM64 / Windows 32 位 | 未纳入当前交付 |
| WSL CLI / Linux ARM64 | 暂缓 |
| 第二真实卷上的移动 | 协议已实现，未实测 |
| FAT32/exFAT | 当前发布机制不支持，失败时不覆盖现有目标 |
| SMB/NAS/断连 | 未验证，不作为当前支持能力 |
| 符号链接/Windows Junction | 默认拒绝跟随和修改；macOS 符号链接测试已通过，Windows 未实测 |
| 中文、空格、shell 特殊字符 | 隔离目录核心操作已测试 |
| 非 Unicode 名称 | 浏览时显示替代字符；操作日志 v1 拒绝写操作。Linux 专项测试保留但暂不运行 |
| 长文件名 | 120 个 ASCII 字符加中文/特殊字符已测试；平台最大长路径未验证 |
| ACL/扩展属性/资源叉/稀疏/硬链接拓扑 | 尚未完整保留 |
| Windows 设备名/NTFS ADS/尾随点与空格 | 路径校验拒绝，纯名称测试已在 Mac 执行；Windows 集成测试待实机运行 |
| Windows 只读源 | 预演时拒绝，不自动修改只读属性 |
| 双核/4 GB/机械硬盘 | 未验证 |

CI 文件包含 macOS/Windows 检查和成品上传，尚无远端仓库、没有运行过远端 CI，不将配置视为测试通过。Windows 细节与实机验证步骤见 [WINDOWS.md](WINDOWS.md)。
