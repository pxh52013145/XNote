# Information Module (Inbox/Timeline) — Design

目标：提供一个“缓存工作区域”：强时间标识、碎片化、多模态快速编辑，并可一键转化为 Knowledge 或归档为 Resource。

## 1. 模块根与条目组织

- 模块根：`*.info/`
- 条目按时间组织（append-only 更利于性能与审计）

推荐结构：

```text
Inbox.info/
  entries/
    2026-02-01T120000Z-01H...md
  attachments/
    01H.../screenshot.png
```

条目文件（Markdown + frontmatter）包含：
- `id/kind=info/createdAt/source`
- 可扩展字段：`tags`、`status`、`rel:*`

## 2. 交互布局（基线）

- 主视图：Timeline/IM 风格（按时间分组）
- 支持快速插入：
  - 粘贴文本/图片
  - 录音（后续）
  - 网页剪藏（后续）

## 3. 转化与归档（Info → Knowledge / Resource）

Info 的价值在于“加工与分流”：

- `info:promoteToKnowledge(entryId, targetFolder?)`
  - 生成新的 Knowledge 笔记（保留来源关系）
  - 可选择“复制正文/拆分成多个原子笔记”（后续可由 AI 辅助）

- `info:archiveAsResource(entryId, mode)`
  - `mode=image/video/file/...`：将附件导入 Resources.asset
  - 在 Resource sidecar 中写入 `rel:source` 指向该 Info 条目

## 4. 与 Knowledge/Resource 的互通

要求：
- 从 Knowledge 可快速跳回“来源 Info”（追溯）
- 从 Resource 可看到“导入来源”（Info）与“被哪些笔记引用”

实现上都通过 `graph:*` 查询，不依赖路径。

