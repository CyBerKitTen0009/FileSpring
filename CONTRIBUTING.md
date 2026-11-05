# Contributing

请先阅读 `prompt1.md`、`docs/ARCHITECTURE.md` 与阶段进度。当前优先完成 Phase 1 的安全与兼容性门槛。贡献代码使用 GPL-3.0-only。

文件操作只能由统一引擎执行。默认拒绝覆盖、符号链接跟随和永久删除；新增操作前必须说明预演、持久化、失败恢复与撤销策略，并加入真实临时文件测试。所有危险测试都必须使用独立 `tempfile` 或生成的模拟目录。不得扫描或修改开发者个人数据。

提交前运行 `cargo fmt --all --check`、`cargo clippy --all-targets --features desktop -- -D warnings` 和 `cargo test --features desktop`。跨平台 CI 配置并不表示平台已验证；提交说明中列出实际平台和未验证场景。

不要提交 `.cache`、`target`、操作数据库、真实个人文件或合成演示目录。依赖变更应提交 `Cargo.lock`，更新第三方许可证清单。开 PR 时说明用户可见行为、测试证据与限制，不以规划功能作为完成项。
