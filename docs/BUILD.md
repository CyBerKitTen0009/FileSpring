# 构建与开发

Rust 1.95+，Cargo，原生 C 编译器；打包工具需要 Python 3.11+。SQLite 使用 bundled feature，不依赖用户安装的 SQLite 运行时。GUI 为可选 feature。当前优先交付 macOS 与 Windows 10/11 x64，Linux/WSL 暂缓。

```sh
export CARGO_HOME="$PWD/.cache/cargo"
cargo fetch --locked
cargo build --locked
cargo build --locked --features desktop
cargo test --locked --features desktop
```

后续无网络可加 `--offline`。首次 fetch 需要网络。只在本项目的 `.cache` 和 `target` 下写入构建依赖。

## 平台

- macOS：Xcode Command Line Tools。`cargo build --release --features desktop` 后运行 `python3 tools/package_macos.py`，在 `artifacts/FileSpring.app` 得到未签名开发包。`--debug` 可打包 debug 构建。尚未签名、公证、生成 DMG 或验证最低系统版本；Info.plist 的 11.0 仅为目标配置。
- Windows：MSVC Rust 工具链、Visual Studio C++ Build Tools/Windows SDK 和 Python 3.11+。运行 `python tools/build_windows.py`，生成 `artifacts/FileSpring-0.1.0-windows-x64-portable.zip`。EXE 静态链接 CRT、嵌入 Win10/11 清单；GUI 隐藏控制台并显示启动错误。见 [Windows 专项说明](WINDOWS.md)。未签名，不含安装器。
- Linux：暂缓开发、打包和 CI，不作为当前交付目标。现有跨平台代码暂保留，后续恢复时再验证依赖与兼容性。

## 隔离演示

```sh
python3 tools/make_demo.py
./target/debug/filespring-desktop --root "$PWD/demo" --state "$PWD/demo/state"
```

脚本只在项目下的 `demo/` 创建缺失样本，不覆盖已有同名文件。应用实际文件变更仍需点击预演里的确认按钮。

## 测试与基准

```sh
cargo fmt --all --check
cargo clippy --all-targets --features desktop -- -D warnings
cargo test --features desktop -- --nocapture
cargo run --release --example scan_benchmark -- 1000 10000 100000
```

基准在自动创建的独立临时目录中生成样本，三次扫描，退出后释放样本。测试也只使用 `tempfile` 目录。不要将测试路径替换为个人文件夹。测试进程中断使用专用子测试进程在首个文件完成后直接退出，不在主程序添加故障后门。

`--smoke-seconds N` 可让原生窗口在 N 秒后退出（任务进行中时拒绝关闭）。这只能验证窗口存活与退出，不能代替交互测试。
