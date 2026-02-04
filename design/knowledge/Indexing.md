# Knowledge Indexing — Fast Path First

目标：索引必须服务“流畅交互”，而不是一次性把所有东西做全。优先保证启动快、滚动快、打开快。

## 1. 两段式扫描（建议）

### Stage A: Fast scan（立即可用）
- 只收集：`path, mtime, size, isDir`
- 生成：文件树结构（用于 Explorer）
- 不读取内容

### Stage B: Content parse（后台渐进）
- 读取内容（可分批、可优先当前打开文件）
- 提取：
  - title（H1 或文件名）
  - frontmatter（YAML）
  - links（`[[...]]`）
  - tags（可选）
- 构建：关键词索引（FTS 或内存倒排）

## 2. Watcher 策略

- 统一 watcher 在 Core
- 事件去抖（例如 200–500ms）
- 只增量更新受影响路径
- 对“频繁保存”的文件（编辑器打开的那份）特殊处理：以编辑器内容为准，不重复读盘

## 3. 数据落盘（缓存，可删除）

建议缓存：
- `db.sqlite`（notes 表、order 表、links 表、fts）
- content hash（避免重复解析）

约束：
- 删除缓存不影响数据正确性（只会慢一点）
- schema 版本化（便于升级/回滚）

## 4. 查询路径（优先级）

1. Quick open：path/title fuzzy（优先内存索引）
2. Search：关键词（FTS / 倒排）
3. Backlinks：links 表反查（增量维护）

语义/向量检索属于后续，不进入 MVP。

