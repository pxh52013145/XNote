# Architecture — Rust Core + GPUI UI (Zed-like)

目标：将 XNote 设计成 **极致轻量、性能超强、Local-first** 的“超级工作站”，长期目标对齐 **Rust + GPUI 原生级 UI 渲染**（类似 Zed 的体验）。

本文件描述“目标态架构”，同时约束当前 Electron 实现：**任何新能力都必须先落在稳定的“命令/数据边界”上**，保证未来可迁移。

## 1. 设计原则（硬约束）

1. **Source-of-truth 在文件**：`.vault/.asset/.info` + sidecar markdown；索引/数据库都是缓存。
2. **命令优先（Command-first）**：所有行为都是确定性命令；UI 与 AI 只是命令调用者。
3. **最小耦合**：实体引用用 `id`，不写死路径；外部编辑用 watcher + refresh，不嵌入重型编辑器。
4. **流畅优先**：UI 永远不等 IO/解析；耗时工作全部进入后台队列（可取消、可重试）。
5. **可审计可回滚**：写操作必须记录影响范围；AI 写操作默认 diff-first + policy gate。

## 2. 目标态进程/线程模型（GPUI）

> 注：GPUI 是 Rust 原生 UI 框架，强调高性能渲染与良好的并发模型。细节实现可调整，但边界必须保持。

### 2.1 分层

- **UI Layer（GPUI）**
  - 只负责：渲染、交互、快捷键、窗口管理、虚拟列表/视图状态
  - 不直接做：文件读写、索引、媒体解析、AI 调用

- **Core Layer（Rust）**
  - Workspace 管理：识别 `.vault/.asset/.info`，路径规范化（POSIX）、越权防护
  - File Watcher：增量更新、去抖动、队列化处理
  - Index Cache：SQLite/FTS（可选向量索引），可删可重建
  - Graph Engine：links/backlinks/relations/query
  - Jobs/Workers：缩略图、抽帧、OCR、转写、派生文本生成

- **Bridge Layer（Tool/Command Bridge）**
  - 进程内：UI 直接调用 Core 的命令接口（强类型）
  - 进程外（可选）：暴露本地工具通道（例如 MCP server），给外部 Agent/插件使用

### 2.2 并发与响应

- UI 线程：只做轻量逻辑与状态更新
- Worker 线程池：IO/解析/索引/派生任务
- 任务队列（Job Queue）：
  - 支持优先级（前台相关任务优先）
  - 支持取消（窗口关闭、选择切换时取消旧任务）
  - 支持去重（同一资源重复刷新只保留最新）

## 3. 性能预算（建议作为“门禁”）

这些不是承诺值，但建议作为设计基线的“红线”，方便持续优化：

- **冷启动**：UI 先显示壳（< 300ms），索引/扫描异步加载
- **常驻内存**：默认不加载大资源；缩略图/预览按需加载与缓存
- **大库体验**：10 万笔记实体的列表滚动流畅（虚拟列表 + 分页查询）
- **索引策略**：增量索引 + 内容 hash；避免全量重建

## 4. Electron → Rust+GPUI 的迁移策略（不返工）

当前仓库是 Electron。为了不返工，建议：

1. **先固化“命令接口”**（在 `src/shared/` 定义 types/schema）
2. Electron main 实现这些命令（IPC）
3. 未来 Rust Core 实现同一套命令（直接调用 / 或 local socket）
4. UI 从 React 替换为 GPUI：只替换“视图层”，命令与数据格式不动

换句话说：你现在写的每个能力，都应该能回答这句检查问题：
> “如果 UI 换成 GPUI，这个能力还成立吗？”

## 5. 插件与扩展（轻量优先）

为避免“插件把性能拖垮”，建议采用：

- **Extension API 只暴露命令**：插件不能直接碰文件系统/数据库，只能通过命令调用
- **按需加载**：模块/资源类型渲染器按需加载
- **隔离策略（可选）**：
  - 轻插件：进程内（更快）
  - 重插件：独立进程（更安全，崩溃不带走主进程）

资源子模块扩展（图片/视频/音乐/电子书/3D）建议遵循统一模式：
- `type → renderer`（UI 渲染器）
- `type → derived jobs`（派生生成器）
- `type → external editor mapping`（外部编辑器通道）

## 6. AI 中枢在目标态的落点

AI 不应该“住在 UI 里”，而应是 Core 的一个可替换服务：

- **Context Pack**：由 Core 组装（稳定、可测试）
- **Tool Allowlist + Policy Gate**：由 Core 执行（不被 UI 绕过）
- **MCP Server（可选）**：由 Rust Core 暴露，外部 Agent/集群可直接连接

这样能保证：即使未来 UI 切换为 GPUI，AI 中枢仍保持一致可控。
