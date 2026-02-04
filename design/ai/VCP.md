# VCP Integration — Variable & Command Protocol Adapter

目标：把 VCP（Variable & Command Protocol）的关键思想融入 XNote 的 AI 引擎蓝图，作为 **“AI 友好的工具调用/上下文注入层”**，同时保持我们既定的 **Command-first + Schema + Policy Gate + Audit** 基座不变。

本文件的定位：**协议/适配层设计**（不是直接复用 VCPToolBox 的实现）。

## 1. 为什么要参考 VCP

VCP 的核心价值对 XNote 很契合：
- **变量（Variable）**：把动态状态（时间、天气、当前工作区、当前选区、关联资源摘要等）做成占位符，注入系统提示词/上下文。
- **命令（Command/Tool）**：把可执行能力做成“对 AI 友好的文本调用协议”，避免过度依赖严格 JSON function calling。
- **鲁棒性与容错**：对 AI 输出的小错更宽容（键名大小写、分隔符等），提升“工具调用成功率”。

在 XNote 中，这些能力对应：
- `Context Pack` 的结构化输出 + 模板化注入
- `Tool Bridge` 的命令暴露与解析（并受 allowlist/policy gate 控制）

## 2. 与 XNote 现有蓝图的关系（不替代 Core 基座）

XNote 的“真相层”仍然是：
- **内部命令总线（Command Schema）**：强类型、可校验、可审计、可回滚
- **风险治理（Policy Gate）**：高风险必须 UI 确认；写操作默认 diff-first

VCP 适配层是“外观协议”之一：
- VCP（AI 友好文本协议） ←→ **命令总线**
- MCP（模型上下文协议） ←→ **命令总线**
- 未来还可加：JSON-RPC/gRPC（开发者/插件） ←→ **命令总线**

结论：**我们不把 VCP 当作核心架构，而是把它当作一个“协议皮肤/适配器”。**

## 3. 在 XNote 中采用的 VCP 核心概念

### 3.1 Variable Registry（变量注册表）

为 AI 提供可组合、可按需暴露的变量：
- 全局变量：`Date/Time`、`WorkspaceName`、`ActiveModule`
- Knowledge 变量：`CurrentNoteTitle`、`CurrentNotePath`、`SelectionText`、`BacklinksSummary`
- Resource 变量（后续）：`SelectedAssetType`、`PreviewState`

变量值允许引用其他变量（模板），并支持“渐进式披露”：
- 当前在 Knowledge：只提供 Knowledge 相关变量，减少上下文噪声

### 3.2 Command/Tool Registry（命令/工具注册表）

每个工具来自内部命令：
- `name`（稳定命名）
- `inputSchema/outputSchema`
- `risk`（read-only / write-safe / destructive）
- `requiresUserApproval`
- `examples`（可选）

工具注册表可以自动生成两类“给模型看的描述”：
- **MCP Tool Definition**
- **VCP Tool Placeholder / Instruction Snippet**

### 3.3 VCP-style Tool Invocation Parsing（文本工具调用解析）

参考 VCP 的思路：让模型用一个明确的文本块触发工具调用，并允许一定容错。

XNote 适配器建议支持：
- `<<<[TOOL_REQUEST]>>> ... <<<[END_TOOL_REQUEST]>>>` 块标记（用于从模型输出中提取调用意图）
- 键值参数使用“包裹符”（避免多行/代码块破坏解析）
- 可选的“串语法”：一次请求内执行多步子命令（在 XNote 中应限制在同一 tool 的安全子命令集合内）

示例（XNote 兼容形态）：

```text
<<<[TOOL_REQUEST]>>>
tool_name:「始」vault.read「末」,
path:「始」notes/Intro.md「末」
<<<[END_TOOL_REQUEST]>>>
```

> 安全约束：即使解析成功，也必须经过 schema 校验 + allowlist + policy gate 才能执行。

### 3.4 Result Payload（结果载荷）

VCP 强调“返回 Markdown 文档 + 可选图片 base64/url”的可读性。
XNote 可沿用该思路：
- 默认返回 Markdown（便于模型复盘与引用）
- 对于图片生成：返回资源条目 `id` + 插入到笔记的引用建议（不直接把大图塞上下文）

## 4. 渐进式落地路线（建议）

### Phase 0（设计完成即可）
- 明确变量/命令的分层与命名规范
- 定义 VCP 适配器的输入输出形状（解析/渲染）

### Phase 1（最小可用）
- 在 `xnote-core` 增加：
  - `VariableRegistry`（可从 UI state / core state 获取快照）
  - `VcpToolRequestParser`（只支持少量安全工具：read/search/suggest）
- 在 UI 中把 VCP 的“工具描述片段”注入到 prompt（按需）

### Phase 2（可控写入）
- 支持 `vault.applyPatch` 走 diff-first
- policy gate：写入必须用户确认
- audit log：记录每次工具调用

### Phase 3（Agent 集群 / 分布式）
- 为 agent 增加 tool allowlist 与 skills
- 可选：提供本地 socket/websocket 作为工具通道（对齐 VCP 的分布式思想）

## 5. Reference Sources（本仓库已归档）

- 后端中间层参考：`reference/VCPToolBox/`
- 前端客户端参考：`reference/VCPChat/`

建议阅读顺序：
1. `reference/VCPToolBox/README.md`（变量/命令/插件/分布式概览）
2. `reference/VCPToolBox/WebSocketServer.js`（多通道与工具路由）
3. `reference/VCPToolBox/Plugin.js`（插件与占位符生成思路）
