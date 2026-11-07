# 测试与性能记录

自动化复验：2026-11-08；原生 GUI 与扫描基准：2026-11-06（Asia/Shanghai）。环境：macOS 26.6.2、Apple Silicon/aarch64、原生 Rust/Cargo 1.98.0。Windows 交叉构建使用已有的 Rust 1.97.1 MSVC target、Clang 22 和 SDK 缓存，未全局安装依赖。当前本地卷支持硬链接；沙箱内无法使用 DiskManagement 查询确切卷格式。未测机械硬盘、旧双核机器、第二卷或其他操作系统。

## 自动化结果

`python3 tools/check.py` 的四项检查全部返回 0：

| 检查 | 结果与原始证据 |
| --- | --- |
| rustfmt | [通过](evidence/format.log) |
| Clippy（全部 targets、desktop、warnings as errors） | [通过](evidence/clippy.log) |
| 测试 | [36 项通过](evidence/tests.log)：8 项恢复边界、1 项 Windows 名称纯校验、5 项 GUI/预览/剪贴板、22 项集成；包含一个仅用于子进程中断测试的辅助入口 |
| Debug CLI 和原生桌面构建 | [通过](evidence/build.log) |
| Release CLI 和原生桌面构建 | [通过](evidence/release-build.log)；本地 macOS `.app` 约 13 MB，包含许可证材料，不代表运行内存 |
| CLI 真实子进程端到端 | [11 次命令调用通过](evidence/cli-smoke.json)：复制/移动、拒绝缺少确认、查看、历史和撤销 |

所有修改和危险场景均在自动生成的 `tempfile` 目录或 `demo/` 中进行。没有扫描、整理或修改个人文件。

集成测试覆盖：预演不修改文件、复制/移动及撤销、持久化重开、目标冲突、预演后源内容变化、目标被占用、撤销目标被编辑/替换、原路径被重建、重复目标、链式重命名、越界与状态目录保护、普通目录拒绝、中文与特殊字符、长名称、零字节文件、文件锁、批扫描与取消、坏链接、预演后父目录被换成链接、权限不足、不可访问路径不得被误认成不存在。

恢复测试覆盖：部分暂存、身份未知暂存保留、暂存后目标冲突、发布后外部编辑、目标发布尚未更新日志、源移除尚未写 Completed、撤销发布尚未记日志、撤销删除尚未记日志。另有真实测试子进程不执行析构即退出，父进程重新打开 SQLite 和 OS 锁，观察 Running/部分完成状态，显式继续后再撤销。

非 Unicode 名称的写操作拒绝测试仅在 Linux 启用，因为本机本地卷拒绝创建测试名称；Windows 独占文件锁测试也仅在 Windows 启用。本次未执行它们。测试过程修正过 headless 纹理 delta 释放方式和平台测试条件；所附日志是最终源码的通过记录。

## Windows 10/11 x64 构建与待验证项（2026-11-08）

- [Windows Release 交叉构建](evidence/windows-cross-build.log)、[跨目标 Clippy](evidence/windows-cross-clippy.log)、[Windows 测试编译](evidence/windows-test-build.log) 均通过。测试使用 `--no-run`，未执行 Windows 测试 EXE。
- 交叉链接保留了 SDK 静态 CRT 缺少上游调试 PDB 的 LNK4099 警告；缺少的是调试符号，不是运行时 DLL。没有隐藏这些警告，也没有把构建成功当成运行成功。
- GUI 为 x86-64 PE32+ Windows 子系统，CLI 为控制台子系统；两者嵌入 Windows 10/11、asInvoker 和 longPathAware 清单。打包检查 [DLL 与清单记录](evidence/windows-pe-audit.json)，未发现外部 VC 运行库或 SQLite DLL 依赖。清单与长路径声明不代表运行兼容性已验证。
- 便携包附 `Verify-Windows.cmd`、`Launch-Demo.cmd` 和生成样本。`self-test` 在 Mac 实际验证 9 项核心行为，并通过“既有目录拒绝复用、全局 state 不被使用”的真实 CLI 子进程回归；Windows 上仍待运行。
- Windows 专项测试包括独占锁（预演前/预演后）、只读源、设备别名与 ADS、超过 260 字符的路径、预演后 Junction 替换。以上仅编译通过，没有冒充运行结果。
- 按用户安排，下一步在 Windows 10/11 实机执行包内验证脚本及 [Windows 手动回归](WINDOWS.md)。CI 仅保留 macOS 和 Windows，Linux 暂缓；远端 CI 尚未执行。

## 原生窗口实际操作

本地原生 macOS 开发包已打开，辅助功能树可读取中文控件。窗口截图检查过双栏、状态栏、文件筛选与选择状态。通过原生 UI 完成以下流程：

1. 左栏打开 `demo/Library`，右栏打开 `demo/Inbox`。
2. 选择生成的 `项目笔记 09.txt`，点击复制，观察 Prepared 预演及完整源/目标路径。
3. 点击确认执行，观察 Completed、100% 进度和目标栏新增文件。
4. 在操作历史中点击撤销并确认，观察 RolledBack，回到文件栏后目标目录只剩原有样本。

对应计划短 ID：`6d579bd0`，持久化记录在本地演示状态数据库，不提交数据库。

用户解锁后继续执行原生回归：

- 左栏 Inbox、右栏 Library，移动 `项目笔记 08.txt`：确认后源 16 项、目标 2 项；确认撤销后源 17 项、目标 1 项，计划 `13e6b0d5` 为 RolledBack。
- Cmd 多选得到 2 项、Shift 区间得到 5 项、Cmd+A 得到 17 项；右栏 Cmd+L 输入路径后导航正确，连续后退依次回到 Library 和 demo。
- 系统目录选择器选择 Library 后正确导航；原生右击显示查看、复制路径、复制预演菜单。
- 中文文本预览内容正确，读取后状态回到就绪；英文及浅色主题显示后恢复中文深色主题。
- 修正原生框架将 Cmd+C/X/V 转换成剪贴板事件后的处理：Cmd+C/V 产生复制预演 `fa62d7f1`，Cmd+X/V 产生移动预演 `309f3cdc`，均未执行。增加事件回归及外部文本不匹配时拒绝使用旧文件选择的测试。

原生拖放未验证通过。自动化拖动没有触发预演，临时事件诊断只观察到松键，没有观察到按下或 drag_started；诊断代码已移除。这一结果不能证明真实手动拖放正常，也不能单独证明应用拖放有故障，保留为人工验证项。代码现已开启行拖动感应、携带已选文件集合，并覆盖子控件上的目标区域。观察记录见 [原生回归证据](evidence/native-gui-regression.json)。

## 合成扫描基准

命令：`cargo run --offline --release --example scan_benchmark`。每次在临时目录生成 28 字节普通文件，再运行三次非递归目录与元数据扫描。扫描结果按批丢弃，不包含 GUI 全量条目缓存成本；数据刚创建，缓存为热。以下为本机真实执行值，见 [原始数值](evidence/scan-benchmark.json)。

| 文件数 | 样本创建 ms | 扫描 1 ms | 扫描 2 ms | 扫描 3 ms |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 65.202 | 3.422 | 2.808 | 2.892 |
| 10,000 | 648.060 | 36.688 | 34.767 | 34.679 |
| 100,000 | 8,545.935 | 388.338 | 372.405 | 357.731 |

GUI 无窗口测试在 100,000 个逻辑条目中每帧最多建立 23 行，20 帧耗时见最终 [测试日志](evidence/tests.log)，debug 构建。该值只验证虚拟化行为和 headless 控件构造，不能称为实际 GUI FPS。

## 尚未验证

冷启动、空闲/峰值内存、真实窗口帧率/输入延迟、复制吞吐、独立哈希吞吐、缩略图生成、索引大小、100 万条索引与搜索、真实跨卷、文件系统断连、硬件断电、真实 Windows/Linux、NTFS/FAT/exFAT/ext4/Btrfs/NAS、完整元数据保留和恶意并发路径替换。索引和搜索尚未实现，因此没有编造百万条结果。

本报告支持“Phase 1 可运行原型”的结论，不支持“完整产品”“全部平台兼容”或“150 MB 内存目标已达成”的结论。跨平台 CI 方案使用 [GitHub 官方列出的运行器标签](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)；本次没有远端 CI 运行记录。
