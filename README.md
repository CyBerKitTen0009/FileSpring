# FileSpring

离线优先的原生文件工作台。项目根据 [prompt1.md](prompt1.md) 开发，采用 Rust、SQLite 和 egui 原生窗口，不包含 Electron、Chromium 或 WebView。

**当前是 Phase 1 开发原型，不是完整产品或正式发行版。** 已有真实文件浏览、复制、移动、预演、持久化日志、显式恢复与撤销。每个修改都会经过同一个操作引擎。目前仅支持普通文件，不支持目录递归操作、覆盖、删除、同步或备份。

当前交付目标为 **macOS + Windows 10/11 x64**，Linux 暂缓。Windows 提供原生 EXE 便携 ZIP、演示目录和无需额外运行时的 `Verify-Windows.cmd`，见 [Windows 使用与验证](docs/WINDOWS.md)。Windows 已完成交叉构建和二进制检查，实机运行尚待验证。

## 本地运行

开发需要 Rust 1.95+ 和平台原生编译工具，打包脚本需要 Python 3.11+；便携包使用者无需这些开发工具，参见 [构建指南](docs/BUILD.md)。不需要全局安装任何 Cargo 工具。

```sh
export CARGO_HOME="$PWD/.cache/cargo"
cargo build --features desktop --locked
python3 tools/make_demo.py
cargo run --features desktop --bin filespring-desktop -- --root "$PWD/demo" --state "$PWD/demo/state"
```

默认中文，可在侧栏切换英文或主题。左、右栏分别打开 `demo/Inbox` 和 `demo/Library`，选择普通文件后点击复制或移动，查看计划并确认。历史页面可以撤销。所有示例文件均由项目生成；不要用真实重要文件验证开发原型。

不传 `--root` 时先弹出系统目录选择器，取消就退出，不会自动扫描个人目录。原生应用默认在所选目录的 `.filespring` 中保存操作记录。用 `--root` 可以授权多个根目录，状态路径可用 `--state` 单独指定。核心拒绝越过这些根目录和跟随符号链接；该边界是应用级校验，并非针对恶意进程的 OS 沙箱。

## CLI

```sh
cargo run -- list demo/Inbox
cargo run -- plan copy 'demo/Inbox/项目笔记 01.txt' --to demo/Library
cargo run -- show PLAN_ID
cargo run -- apply PLAN_ID --confirm
cargo run -- events PLAN_ID
cargo run -- undo PLAN_ID --confirm
```

`plan` 只读取文件并保存预演，不改变目标文件。`apply`、`resume` 和 `undo` 必须带 `--confirm`。CLI 默认只授权当前工作目录；跨目录传输需要重复指定 `--root`。操作只能在原授权范围内继续。

`filespring self-test` 只在新建的临时样本目录执行诊断并保存 `verification.json`，不会使用 `--root`/`--state`。可用 `--fixture NEW_DIRECTORY` 指定一个尚不存在的目录；目录已存在时立即拒绝，诊断后保留样本。

GUI 原型提供双栏、路径输入、目录内名称筛选、Ctrl/Cmd 多选、Shift 区间选择、方向键、Enter 打开目录/文本预览、F5 刷新和应用内 Ctrl/Cmd+C/X/V。复制、剪切会将路径文本写入系统剪贴板；在应用内粘贴时，仅当文本仍匹配才使用原始文件路径生成预演，仍需确认执行。系统文件剪贴板协议尚未实现，不能与 Finder/资源管理器交换文件。拖放接口已接入，原生拖放交互尚待人工验证。

## 检查与证据

```sh
cargo fmt --all --check
cargo clippy --all-targets --features desktop -- -D warnings
cargo test --features desktop -- --nocapture
cargo run --release --example scan_benchmark
```

参见 [测试记录](docs/TEST-REPORT.md)、[原始运行日志](docs/evidence/)、[架构与恢复协议](docs/ARCHITECTURE.md)、[GUI 选型验证](docs/GUI-EVALUATION.md)、[兼容性与限制](docs/COMPATIBILITY.md) 和 [阶段进度](docs/ROADMAP.md)。

## 关键限制

- 目标文件已存在即停止，不提供覆盖或自动改名；同名冲突需要人工处理。
- 发布目标使用不覆盖的硬链接，再移除暂存名；目标卷必须支持硬链接。FAT32/exFAT 等不支持该机制的文件系统会安全失败，尚未达到完整跨文件系统兼容目标。
- 移动采用校验复制后移除源文件，不宣称整个批次原子。进程退出后不会自动继续，需显式恢复。
- 撤销只处理文件标识、长度、修改时间和内容哈希都符合记录的文件；外部改动或原路径被占用时会停止。日志不是备份。
- 仅保留文件内容、基础权限和修改时间；ACL、扩展属性、资源叉、稀疏特性、硬链接拓扑尚未完整保留。对这些元数据有要求的文件不应使用本原型迁移。
- 不承诺抵御外部恶意进程同时替换路径的竞态，不应在不可信共享目录中用于重要数据操作。

GPL-3.0-only；依赖许可信息见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
