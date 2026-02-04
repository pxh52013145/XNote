# QuickLook Reference Notes (for Resource Preview)

QuickLook（QL-Win/QuickLook）是 Windows 上非常成熟的“按空格快速预览文件”实现，核心价值在于：

- **极快的预览启动**：用户选中文件→按空格→立即预览（体验上接近“系统能力”）。
- **Viewer/Plugin 架构**：按文件类型匹配 Viewer；不同类型用不同渲染器（图片/视频/PDF/Markdown/Office/3D…）。
- **预览与主编辑解耦**：预览只做“看”，编辑交给外部专业软件。

XNote 的 Resource 模块目标是“资产工作站”，与 QuickLook 的共同点是：**核心体验取决于预览链路**（类型识别 → 派生 → 渲染 → 缓存 → 刷新）。

> 许可证：QuickLook 为 GPL-3.0。这里只做架构/交互参考，不复用其源码实现。

## 1) 我们要从 QuickLook 学什么（抽象层）

### 1.1 Viewer 的职责边界

在 QuickLook 里，Viewer 通常具备这些职责：
- `CanHandle(path)`：判断能否处理该文件（类型匹配）
- `Prepare(path, context)`：声明预览窗口需求（大小/标题/是否可缩放…）
- `View(path, context)`：真正渲染预览内容（并支持异步/忙碌态）
- `Cleanup()`：释放资源（播放器、解码器、临时文件等）

映射到 XNote（Rust + GPUI）：
- **ResourceRenderer**（trait）只负责“如何预览”，不负责库管理/索引更新/导入。
- 预览所需的派生物（缩略图、抽帧、字幕、OCR）由后台任务队列生成，Renderer 只读取“已就绪的数据”。

### 1.2 Plugin/Viewer Registry（类型驱动）

QuickLook 的重要经验是：**用 registry 把“类型 → viewer” 固化为可扩展机制**，而不是写死在 UI 里。

映射到 XNote：
- `ResourceRendererRegistry`：`mime/extension/kind → rendererId`
- `ResourceDerivedRegistry`：`mime/extension/kind → derivedJobs[]`（可选，后续）

### 1.3 Preview Cache 与刷新策略

预览是典型的“高频读 + 可重建派生物”：
- 缩略图/抽帧/文本提取：可重建，但重建成本高 → 需要缓存与失效策略
- 外部编辑回写：watch 文件变化 → 触发派生刷新 → UI 热更新

映射到 XNote：
- Source-of-truth 仍是 `Resources.asset/items/<id>/original/*`
- 可重建派生物默认放在 `Workspace/.xnote/derived/` 或 `Resources.asset/.xnote/derived/`（见 `design/resources/README.md` 的空间策略）

## 2) 对 Resource 模块的具体落地建议（MVP → V2）

### MVP（先做“选中即预览”）
- 资产列表（虚拟滚动）选中条目
- 右侧预览区根据 `type/extension` 选择 Renderer
- 先支持 3 类：`image / video / text`（文本包括 markdown/plain）
- “Open With…”：调用外部编辑器打开 `original/*`
- Watcher：检测 `original/*` 变更 → 刷新缩略图/重新加载预览

### V2（工作站能力）
- 视频：时间轴标注、抽帧与字幕 sidecar
- 音频：波形预览、片段标注
- 电子书/PDF：目录导航、页面缩略图
- 3D：模型预览（viewer 插件化，避免把复杂渲染塞进主 UI）

## 3) 建议阅读点（在 reference/ 中）

- QuickLook 主项目（入口/匹配/窗口管理）：
  - `reference/QuickLook/QuickLook/PluginManager.cs`
  - `reference/QuickLook/QuickLook/ViewWindowManager.cs`
- QuickLook Viewer 接口（子模块）：
  - `reference/QuickLook.Common/Plugin/IViewer.cs`

