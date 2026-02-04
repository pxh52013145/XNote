# AI Hub — Design (MCP-like, Agent-ready)

目标：让 AI 成为“可控的智能中枢”，通过工具/命令通道操作工作站，并能逐步升级到 Agent 集群（skills/mcp/专家化）。

本设计不要求一开始就“造完中枢”，而是确保从第一天起架构就是 **AI-ready**：
- UI 调用命令
- AI 未来也调用同一命令
- 命令有 schema、权限、审计、可回滚

## 1. AI 能力分层（从易到难）

### L1：编辑器内的选区能力（最先实现）

- `ai:rewriteSelection`：基于上下文重写选中文本
- `ai:expandSelection`：扩写/补充知识点
- `ai:generateImage`：生成图片并导入 Resource，然后插入引用到笔记

### L2：跨实体的检索与关联

- 语义检索：根据选区/主题召回相关 Knowledge/Resource/Info
- 自动建议链接：给出“可插入的引用/反链建议”，由用户确认

### L3：Agent 集群（open-code/openclaw 风格）

- 一个调度器（Coordinator）+ 多个专家 Agent（每个 Agent 有自己的 skills/tools allowlist）
- 每个 Agent 通过 MCP-like 工具协议调用工作站命令

## 2. Tool Bridge：让 AI “能做事”而不是“猜”

核心：把工作站能力暴露为工具（Tools），并对工具做严格治理。

### 2.1 统一的 Command Schema

每个命令都有：
- `name`：如 `vault:updateNote`
- `inputSchema/outputSchema`：JSON Schema（用于校验与提示模型正确调用）
- `risk`：`read-only | write-safe | destructive`
- `requiresUserApproval`：高风险必须 UI 确认

> 命令命名建议与仓库 IPC 约定保持一致：`namespace:verbNoun`（例如 `vault:updateNote`）。

### 2.2 渐进式披露（Progressive disclosure）

不要一次性暴露全部工具：
- 当前模块=Knowledge：只暴露 `vault:* graph:* ai:*`（最小集合）
- 当前模块=Resource：暴露 `resource:* graph:*`，以及少量 AI 派生工具
- 当前模块=Info：暴露 `info:* graph:*`

这样更省上下文，也更安全。

## 3. Context Pack（上下文打包器）

AI 的输入必须是“结构化上下文”，而不是随意拼文本。

当用户在 Knowledge 里选中文本时，Context Pack 至少包含：
- `selection`：选区文本 + 位置信息（起止行/列）
- `note`：当前笔记标题/路径/id + 选区附近上下文（例如上下各 30 行）
- `links`：当前笔记的 outgoing/incoming links 摘要（可截断）
- `related`：语义检索召回的相关实体摘要（可选）
- `uiState`：当前模块/窗口、当前打开的资源预览状态（可选）

建议：在 Knowledge 场景下，Context Pack 也可包含该笔记的 NoteMeta 摘要（typed relations/pins），以便模型做“连接导向”的改写与关联建议。

## 4. 安全与可回滚（AI-native 必备）

- **Policy Gate**：删除/移动/批量改写等必须 UI 确认
- **Dry-run / Diff-first**：AI 写操作先产出 diff/patch 预览，再由用户应用
- **Audit Log**：记录每次 AI 的工具调用、输入输出摘要、影响的文件列表
- **Undo**：最小可行方案是“写前备份 + 可回滚”（后续可升级为事件溯源）

### 4.1 写操作的最小落地流程（推荐）

1. renderer 组装 `Context Pack` → 调用 `ai:*` 获取 **ProposedChange**（只读阶段）
2. renderer 展示 **diff/patch**（含风险级别、影响文件列表）
3. 用户确认后，renderer 调用对应写命令（例如 `vault:applyPatch` / `resource:updateSidecar`）
4. main 在写入前执行：输入校验 → 权限/策略守卫 → 备份/原子写 → 写入审计日志
5. 若用户撤销：调用 `vault:undoLastChange`（或基于备份回滚）

优先策略（推荐）：
- 能通过更新 NoteMeta（typed relations/pins）达成的 AI 产出，优先落在 `Knowledge.vault/.xnote/meta/<id>.json`
- 需要修改正文时，再走 diff-first 的 `vault:*` 写入命令

## 5. Agent 配置（可版本化、可移植）

建议放在工作区 `.xnote/ai/agents/`：
- `coordinator.json`
- `knowledge-editor.json`
- `resource-curator.json`

每个 agent 定义：
- `name/role`
- `enabledTools`（allowlist）
- `skills`（本地文件路径或内置模板）
- `model`（可替换：本地/云端）

## 6. 与 MCP 的关系（落地方向）

你不需要为每个功能“重新造一套 API”。正确方向是：
1. 先把工作站内部命令做稳（schema + 权限 + 审计）
2. 再把命令桥接到 MCP server（stdio/http 都可）
3. 任意支持 MCP 的 agent/客户端都能“像操作 Chrome DevTools 一样”操作 XNote

## 7. 与 VCP 的关系（Variable & Command Protocol）

VCP（Variable & Command Protocol）强调：
- 用 **变量占位符**把动态上下文注入 prompt
- 用 **对 AI 友好的文本标记协议**承载工具调用（容错更强，减少“严格 JSON function calling”带来的失败）

对 XNote 来说，VCP 更适合作为“协议适配层/外观协议”，而不是替代我们的 Core 基座：
- Core 仍以 `Command Schema + Policy Gate + Audit/Undo` 为真相层
- VCP/MCP/JSON-RPC 等都只是把“同一套命令”翻译成不同生态可用的通道

详见：
- `design/ai/VCP.md`
- 参考实现已归档：`reference/VCPToolBox/`、`reference/VCPChat/`
