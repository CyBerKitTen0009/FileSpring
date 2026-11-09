# 界面设计

2026-11-09 重写原生桌面界面。视觉参考 Apple 的窗口、侧栏和工具栏结构：中性灰背景、系统蓝强调、明确的文本层级、紧凑的工具栏、细分隔线和连续的文件列表。默认深色，可切换浅色；macOS 与 Windows 使用相同的信息结构，保留各自原生窗口控制和系统目录选择器。

- **侧栏**：文件、操作历史、已授权的工作目录，底部提供语言和主题切换。
- **工具栏**：当前两栏的目录名称、复制和移动入口。选择文件后才启用，点击后生成预演。
- **双栏**：独立的返回、上一级、刷新、路径输入和名称筛选；32 pt 行高，整行选中、轻微交替底色、名称与大小对齐。文件列表保留虚拟滚动。
- **操作预演**：底部可调整高度的面板，显示状态、文件数、大小和源/目标路径；执行、恢复与撤销仍需要明确确认。冲突和错误可滚动阅读，不能执行有冲突的计划。
- **历史**：状态、批次摘要与查看/恢复/撤销入口；撤销使用独立确认弹窗。

颜色与控件样式集中在 `src/bin/app/theme.rs`；原始矢量图标在 `icons.rs`；布局在 `view.rs`。图标由项目自行绘制，没有打包 SF Symbols 或 Apple 字体。自绘按钮、文件行和预演行提供辅助功能标签；完整屏幕阅读器和 Windows 缩放仍需实机回归。

设计参考：[Apple Windows](https://developer.apple.com/design/human-interface-guidelines/windows)、[Sidebars](https://developer.apple.com/design/human-interface-guidelines/sidebars)、[Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars)。这是 FileSpring 自己的跨平台实现。

验证范围：macOS 原生窗口实际检查深/浅色、中/英文、名称筛选、历史页，完成生成样本的复制、预演确认与撤销；headless 完整布局测试覆盖两种主题、两种语言、960×640 和 1240×800、文件页与历史页。Windows 10/11 桌面运行仍待实机验证。
