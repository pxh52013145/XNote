# Resource Module (Asset Library) — Design

目标：提供“多模态资产工作站”的体验：导入式资产库 + 强预览 + 可外部编辑回写同步 + 与 Knowledge/Info 的强关联。

## 1. 模块根与库结构

- 模块根：`*.asset/`（类似 Eagle 的 `.library`）
- 资产导入后进入库内管理，不依赖外部原路径
- 每个资产一个“条目目录”（可包含原始文件、sidecar、预览、派生）

推荐结构：

```text
Resources.asset/
  items/
    01H...-MyVideo/
      asset.md
      original/
        video.mp4
      previews/
        thumb.jpg
        frame-0001.jpg
```

说明：
- `asset.md` 是该资产的可扩展描述文件（Markdown + frontmatter）
- `original/` 是导入后的原始文件（或未来可升级为内容寻址存储引用）
- `previews/` 可选择“可重建但随库迁移”，或转移到 `.xnote/`（取决于迁移与空间策略）

## 2. 资产导入（Ingest Pipeline）

导入流程（可组件化扩展）：
1. 计算 hash（用于去重/版本识别）
2. 复制/移动到库内 `original/`
3. 生成基础预览（缩略图、媒体信息、时长、尺寸）
4. 生成/更新 `asset.md` frontmatter（`id/kind/type/createdAt/hash`）
5. 写入索引缓存（SQLite）并广播 UI 更新

扩展点：
- 不同 `type`（图片/视频/音频/电子书/文本/3D 模型）有不同的“派生生成器”
- 派生生成器以队列任务运行，避免阻塞 UI

## 3. 预览与“工作站”布局

核心：按 `type` 选择不同预览容器（Renderer）。

建议做成插件式：
- `ResourceRendererRegistry`：`mime/type/extension → rendererId`
- renderer 只关心“读取预览所需的数据”，不要直接操作文件系统（通过命令通道）

示例布局：
- 左侧：资源分类/过滤（text/image/music/video/ebook/…）
- 中间：网格/瀑布流（虚拟滚动 + 懒加载）
- 右侧：预览工作区（播放器/阅读器/文本解析器等）

## 4. 外部编辑与回写同步

原则：
- 专业编辑交给外部软件
- XNote 负责：打开通道 + 监听变化 + 刷新预览/索引

能力拆分：
- `resource:openExternalEditor(id, appId?)`
- `resource:watchChanges(id)`（底层 watcher 统一在 main）
- `resource:refreshDerived(id)`（触发缩略图/转写/OCR 重建）

版本策略（可选，后续再做）：
- 简单模式：只保留最新
- 版本模式：每次外部保存生成版本快照（占用空间大，需策略）

## 5. 与 Knowledge/Info 的互通

资源条目应可被知识笔记引用：
- 在笔记插入 `xnote://resource/<id>` 或 `[[id:<id>]]`

右侧关联面板（资源视角）：
- “相关笔记”：反向查询引用它的 Knowledge
- “来源信息”：它从哪个 Info 条目转化/导入

## 6. 参考实现（Read-only）

- Windows 快速预览（QuickLook）：`design/resources/QuickLook.md`
