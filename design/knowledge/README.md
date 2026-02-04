# Knowledge Module (Vault) — Design

目标：提供类 Obsidian 的笔记体验，并补齐 Obsidian 的关键短板：**文件树可手动排序（索引式/过程式）**，同时与 Resource/Info 完全互通。

> 长期目标：对齐 Zed 的轻量与性能：**Rust Core + GPUI 原生 UI**。本模块设计必须保持“命令边界清晰”，以便从 Electron/React 平滑迁移。

## 1. 模块根与文件组织

- 模块根：`*.vault/`
- 笔记：普通 Markdown 文件（人类可读、可被外部编辑器直接打开）
- 附件：建议约定 `attachments/`（或兼容 Obsidian 的附件策略）

> 约束：Vault 内部路径统一用 POSIX 分隔符（`a/b.md`）。

## 2. 交互布局（基线）

- 左侧：文件树（支持拖拽排序、搜索过滤）
- 中间：编辑器（当前 Electron 版为 CodeMirror 6；GPUI 目标态为原生文本编辑器）
- 右侧：预览/反链/大纲/关系面板（可切换）
- 底部：模块切换栏（默认 Knowledge）

多窗口：
- 新窗口默认打开 Knowledge，并复用同一工作区索引
- 可在同一工作区中开多个 Knowledge 窗口（不同笔记并排）

## 3. 自定义排序（核心能力）

### 3.1 需求解释

“过程式学习/索引式写作”要求在同一文件夹内：
- “介绍/目录”应排在前面
- 排序由用户拖拽决定
- 不通过重命名文件实现

### 3.2 Source-of-truth：Folder Order File

每个文件夹一个 order 文件（推荐放在该 vault 的 `.xnote/order/` 下）：

```text
Knowledge.vault/
  .xnote/
    order/
      notes.order.md      # 对 notes/ 文件夹生效（推荐）
```

内容格式推荐 Markdown 列表（可读、可 diff；并且放在 `.xnote/` 下不会污染 vault 的 graph/backlinks）：

```md
# Order for notes/
- [[path:notes/Intro.md]]
- [[path:notes/Basics.md]]
- [[path:notes/Pointers.md]]
```

说明：
- MVP 阶段以 `path:` 作为权威键；引入 note `id` 后可升级为 `id:`（更稳健）
- 重命名/移动后由 XNote 自动更新 order 引用（或提供一键修复）

注意：order 文件属于**用户态元数据（Source-of-truth）**，不应被当作缓存删除；建议纳入版本控制。

渲染规则：
1. order 列表中出现的条目按列表顺序显示
2. 文件夹中新出现但不在 order 的条目：追加在末尾（可配置为按字母/按创建时间）
3. 被删除/移动的条目：渲染时忽略；在 UI 提供“清理 order”操作

### 3.3 与 Link Graph 的关系

排序不是链接，但它是“结构关系”的一部分。实现上可：
- order 文件作为权威
- 索引层把 order 解析为 `graph` 中的一种关系（例如 `rel:order` 或 `rel:next`），用于 AI/查询

## 4. 双链与跨模块跳转

支持：
- 标准 Markdown 链接（推荐）：`[Title](relative/path.md)`
- Obsidian 风格 wiki link（可选兼容）：`[[Title]]`
- 跨模块/深链接：`xnote://resource/<id>`、`xnote://info/<id>`

> XNote 内部会用 frontmatter `id` 把上述“可移植链接”解析到稳定实体；不建议在正文里推广 `[[id:...]]` 这种形式（Obsidian 会当作普通 wiki link，兼容性差）。

### 4.1 Note `id`（逐步引入，避免“路径绑定”）

为了实现跨模块稳定引用、以及未来 AI/索引层的鲁棒性，建议逐步给 Knowledge 笔记引入 frontmatter：

```yaml
---
id: 01H...
kind: knowledge
createdAt: 2026-02-01T12:00:00Z
---
```

迁移策略：
- 初期：允许无 `id` 的旧笔记继续工作（按 path/title 解析双链）
- 逐步：在笔记首次被创建/打开/被引用时补齐 `id`
- 完成后：可把更多引用与排序从 `path:` 升级为 `id:`

## 5. 增量元数据（NoteMeta）：更激进连接表达的载体

为最大化兼容性，XNote 不建议把“typed relations/序列关系/来源关系/pins”等强语义写进正文语法。

替代方案：每篇笔记一个增量文件：

```text
Knowledge.vault/
  .xnote/
    meta/
      <noteId>.json
```

职责（V1）：
- `relations[]`：类型化关系（`xnote.*` 点分层命名，如 `xnote.source`、`xnote.explains`）
- `pins`：把某些 Resource/Info/Note 固定到该笔记的“关联面板”
- `ext`：插件/AI 扩展字段（不影响核心 schema）

> 备注：正文依然可以包含普通链接（markdown link / `[[...]]`），那是“可移植语义”；NoteMeta 是“XNote 增强语义”。

UI 行为：
- 悬浮预览（Hover preview）
- Ctrl/⌘+Click 跳转
- 右侧“关联”面板展示：
  - 反链（谁引用了我）
  - 关联资源（资源 sidecar/摘要）
  - 关联信息（来源、时间线条目）

## 6. AI 在 Knowledge 的最小可用切入点（不先做“中枢”也能落地）

最小可用 AI 功能应围绕“选区”：
- 选中文本 → `rewrite`（基于上下文的重写）
- 选中文本 → `expand`（补充知识点/例子/反例）
- 生成图片 → 作为 Resource 资产导入，并在笔记插入引用（保持最小耦合）

AI 相关细节见 `design/ai/README.md`（命令通道与策略守卫）。

## 7. MVP 与 GPUI 目标态（入口）

- MVP 成功基线与裁剪：`design/knowledge/MVP.md`
- GPUI UI 视图树与状态：`design/knowledge/GPUI.md`
- 索引与 watcher：`design/knowledge/Indexing.md`
