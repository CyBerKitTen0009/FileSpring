# GUI 框架原型评估

状态：**候选原型，尚未选择为最终产品框架**。当前使用 egui/eframe 0.37、原生 Winit 窗口、Glow/OpenGL，未启用 WGPU feature。不包含 WebView 或浏览器运行时。CLI 无需 GUI 依赖。

Slint 也保留在候选中。其软件渲染器不依赖 GPU，但官方文档当前列出文字渲染限于西方文字等限制，中文桌面产品需另行实测。参见 [Slint 渲染后端](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/)。egui 的 Glow 是原生渲染方案，选择依据来自 [eframe 官方文档](https://docs.rs/eframe/0.37.0/eframe/)，不能据此推断低配硬件性能达标。

| 能力 | 原型证据 | 尚待验证 |
| --- | --- | --- |
| 大量文件列表 | 100,000 行 headless 测试，每帧最多只建立 23 行；真实目录流按批处理 | 100k 文件原生窗口完整滚动与内存 |
| 虚拟滚动 | `ScrollArea::show_rows` 自动化测试 | Native 帧率、滚轮/触控板、超大偏移 |
| 外部/内部拖拽 | 接入 dropped-files 与 typed drag payload，先生成计划；已选项拖动携带完整选择 | 自动化拖动未触发预演，诊断仅收到松键事件；真实手动拖放、系统拖入、跨窗口拖出尚待验证 |
| 多选 | 单测与 macOS 原生 Cmd 切换、Shift 区间、Cmd+A 均通过 | Windows/Linux 与完整辅助功能 |
| 文件缩略图 | 32x32 纹理上传、绘制自动化测试通过 | 真正图片解码、按需缩略图缓存尚未实现 |
| 右键菜单 | macOS 实际右击显示查看、复制路径、复制预演菜单 | 菜单键盘遍历与其他平台 |
| 快捷键 | macOS 实测 Enter、Cmd+A/C/X/V/L；右栏路径焦点正确 | 其余 Native 按键与焦点组合；可自定义快捷键未实现 |
| 自定义绘图 | painter 圆形绘制测试通过 | 复杂空间图与软件渲染成本 |
| 高 DPI | 2x 测试；当前 Retina 原生窗口显示检查 | 跨屏 DPI、Windows 缩放、Linux 多屏 |
| 剪贴板 | macOS Cmd+C/V 产生复制预演，Cmd+X/V 产生移动预演；系统路径文本必须匹配内部原始路径 | 系统文件剪贴板协议尚未实现，不能与 Finder 交换文件 |
| 原生目录选择器 | macOS 实际打开系统选择器并选择演示 Library，导航成功 | Windows/Linux 与平台权限组合 |
| 中文/英文 | macOS 原生中英文、深浅主题与中文文本预览实测 | 无 CJK 字体系统、IME 与辅助功能全面检查 |
| 关键操作 | macOS 实际完成生成样本的复制、移动、历史和确认撤销；状态栏在历史/文本预览读取后回到就绪 | 其他平台、真实跨卷及中断窗口恢复 |

本轮没有将未验证项写为“支持已完成”。在上述门槛验证前，正式 GUI 技术选型仍开放。尤其需要对无高性能 GPU 的旧电脑、输入法、可访问性和系统文件拖放进行实测。
