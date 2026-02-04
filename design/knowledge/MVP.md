# Knowledge MVP — Rust + GPUI Baseline

目标：先把 **Knowledge（笔记）模块**跑通，并在架构上满足“可扩展、最小耦合、极致性能”的基座要求。其他模块（Resources/Inbox/AI Hub）只保留“可连接”的接口占位，不抢 MVP 资源。

## 0. 成功基线（必须同时达成）

1. **打开 Vault**：选择一个 `*.vault/` 目录，秒级可用（UI 壳先起，索引异步）。
2. **文件树可拖拽排序**：同一文件夹内自由调整顺序，不改文件名；顺序持久化到 order 文件；重启后不丢。
3. **编辑 + 保存**：打开任意笔记，编辑不卡顿；保存不阻塞 UI。
4. **双链跳转（最小版）**：支持 `[[...]]` 基础解析与跳转（按 title/path 匹配即可）。
5. **大库仍流畅**：10 万笔记文件树滚动不卡（虚拟列表）；搜索能用（先关键词）。

## 1. MVP 不做什么（刻意裁剪）

- 不做完整 Obsidian 插件生态
- 不做多模态资源预览（只保留链接占位）
- 不做向量/语义检索（先 BM25/关键词）
- 不做复杂协同/同步
- 不做 AI Agent 集群（只保留命令边界与上下文打包草案）

## 2. 数据与文件约定（MVP 版）

- Vault 根：`<name>.vault/`
- Note 文件：`*.md`（人类可读，外部编辑器可直接打开）
- Vault 私有系统目录：`<name>.vault/.xnote/`
  - `order/`：自定义排序权威文件（source-of-truth）
  - `cache/`：可删除的索引缓存（SQLite/FTS 等）

> 所有 Vault 内相对路径统一使用 POSIX 分隔符：`a/b.md`。

## 3. 自定义排序（MVP 最核心）

### 3.1 Order 文件（Source-of-truth）

每个文件夹一个 order 文件（推荐映射规则）：

- folder：`notes/`
- order file：`.xnote/order/notes.order.md`

内容格式（可读、可 diff）：

```md
# Order for notes/
- [[path:notes/Intro.md]]
- [[path:notes/Basics.md]]
- [[path:notes/Pointers.md]]
```

渲染规则：
1. order 中出现的条目按列表顺序
2. 文件夹中新文件但不在 order：追加到末尾（默认按创建时间，其次按字母）
3. order 中引用不存在的文件：忽略但保留，UI 提供“Clean order”按钮

拖拽规则（稳定、可预测）：
- 拖拽只改变“同级顺序”（跨文件夹移动属于另一个命令）
- 每次拖拽更新对应 folder 的 order 文件（写入去抖动）

### 3.2 与 Graph 的关系

MVP 可先不把排序写入 graph，但 Core 需要暴露：
- `vault.order.get(folder)`
- `vault.order.set(folder, orderedPaths[])`

后续可把 order 映射为一种关系 `rel:order`/`rel:next` 供 AI/查询。

## 4. UI（GPUI）布局与交互（MVP 版）

窗口结构（单窗口单 workspace）：
- Left: Explorer（文件树 + filter）
- Center: Editor（纯文本/Markdown 编辑，先不卡）
- Right: Inspector（Outline/Backlinks/Links 占位）
- Bottom: Module Bar（Knowledge / Resources / Inbox / AI Hub；MVP 仅 Knowledge 可用）

关键交互：
- `Ctrl+P`：Quick open（先按文件名/路径）
- `Ctrl+K`：Command palette（只挂 10 个以内核心命令）
- `[[`：弹出 link picker（MVP 可先用简单 fuzzy 匹配 title/path）

## 5. 性能策略（MVP 必做）

- **UI 壳先起**：打开 vault 后立即显示空文件树骨架与 loading，不等待全量扫描
- **增量扫描**：第一次扫描只收集路径与 mtime/size；内容索引延后
- **虚拟列表**：文件树/搜索结果必须虚拟化
- **后台队列**：解析 frontmatter/links、构建索引都走 worker；UI 只收事件
- **watcher 去抖**：外部编辑器频繁保存不会触发反复全量更新

## 6. 10 万规模性能门禁（可量化验收）

> 目标：在 10 万笔记规模下，保持“Zed 级别的流畅性”。以下指标是 **MVP 的性能门禁**，不达标就不能算完成。

### 6.1 数据规模与测试数据

- **规模**：100,000 个 `*.md`
- **分布**：10~200 层目录随机分布，文件名长度 8–40 字符
- **内容**：每个文件 1–8KB（短笔记为主），10% 文件包含 `[[...]]` 链接

### 6.2 启动与加载

- **冷启动 UI 可见**：≤ 300ms（壳先起）
- **Vault 首次可用**：≤ 1.5s（文件树可滚动/可筛选）
- **后台索引完成**：≤ 60s（允许渐进）

### 6.3 滚动与交互

- **文件树滚动**：稳定 60 FPS（或 ≥ 55 FPS）
- **拖拽排序反馈延迟**：≤ 50ms
- **打开笔记**：≤ 120ms 显示内容（缓存命中 ≤ 40ms）

### 6.4 搜索与跳转

- **Quick Open（模糊匹配）**：≤ 80ms 返回前 20 条
- **关键词搜索**：≤ 200ms 返回前 50 条

### 6.5 资源占用（工作站级目标）

- **常驻内存**：≤ 300MB（不加载大资源）
- **后台 CPU**：Idle 时 ≤ 2%（索引任务完成后）
- **磁盘缓存**：≤ 2GB（100k 笔记 + 索引 + thumbnails）

### 6.6 验收方式（建议）

- 加入可重复的“生成 10 万笔记”的脚本（后续工程阶段）
- 记录并输出：
  - 启动时间
  - 首次可用时间
  - 索引完成时间
  - 搜索延迟分布（P50/P95）
  - 内存峰值

## 6. 命令边界（UI 与 Core 解耦）

MVP 需要的最小命令集：
- `vault.open(rootPath)`
- `vault.list(folder?)`
- `vault.read(notePath)`
- `vault.write(notePath, content)`
- `vault.order.get(folder)`
- `vault.order.set(folder, orderedPaths[])`
- `vault.search(query)`（先关键词）
- `vault.link.resolve(key)`（title/path → path）

这些命令在 Electron 时代走 IPC；GPUI 目标态走进程内调用，但命令形状不变。
