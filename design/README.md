# XNote Super Workstation — Design Blueprint

本目录用于沉淀 XNote 的整体蓝图设计（不是实现细节）。目标是：**最小耦合、组件化可扩展、Local-first、AI-native**，并以 **Zed 级别的轻量与性能**为导向（Rust + GPUI 原生渲染）。

> 约定：仓库当前实现是 Electron + React + TypeScript，但本设计的“接口/命令/数据”边界应能无痛迁移到 **Rust Core + GPUI UI**。也就是说：**先定边界，再换实现**，避免被框架锁死。

## 0. 核心目标（必须同时满足）

1. **三模块 IRK**：Information / Resource / Knowledge 三个模块完全不同的操作逻辑与布局，但**共享同一工作区与同一关联图谱**。
2. **多窗口**：可按需打开多个窗口；默认打开 **Knowledge（笔记）** 模块。
3. **自定义排序**：Knowledge 的文件树支持“索引式/过程式”的手动排序（拖拽即可），不依赖文件名 `01-02`。
4. **导入式资产库**：Resource 模块像 Eagle 一样导入、处理并纳入库（不依赖外部路径），支持预览与外部编辑回写刷新。
5. **AI 中枢可控**：AI 通过“工具/命令通道”操作工作站（类似 MCP 打通 Chrome DevTools 的方式），并具备权限、审计、可回滚。
6. **最小耦合与可迁移**：卸载软件后，数据仍是人类可读/可迁移的文件；数据库/索引只做缓存，可重建。
7. **极致性能**：启动快、低内存、低后台常驻；大库（10 万+实体）操作仍保持流畅（虚拟列表、增量索引、后台队列）。

## 1. 术语与边界

- **Workspace（工作区）**：包含 IRK 三模块根目录的集合，以及 `.xnote/` 系统目录。
- **Module Root（模块根）**：
  - `*.vault/`：Knowledge（类似 Obsidian vault）
  - `*.asset/`：Resource（类似 Eagle library）
  - `*.info/`：Information（Inbox/Timeline）
- **Entity（实体）**：可被引用的最小对象（笔记、资源、信息条目等），拥有稳定 `id`（建议 ULID/UUIDv7）。
- **Link Graph（链接图谱）**：所有实体之间的引用与关系（双链、反链、序列关系、来源关系等）。
- **Index Cache（索引缓存）**：SQLite/FTS/向量索引等，仅作加速，不作为权威数据源。

边界原则（强制）：
- `renderer` 不直接访问 Node/Electron；所有能力通过 `preload` 暴露。
- `shared` 只放纯类型/纯逻辑（无 Electron/Node 依赖）。
- 文件系统与索引更新在 `main`；UI 只消费稳定快照与事件流。

## 2. 工作区磁盘结构（Source-of-truth 在文件）

示例（一个工作区目录）：

```text
MyWorkspace/
  Knowledge.vault/
    notes/
      C Language.md
      Pointers.md
    attachments/
      image-1.png
    .xnote/               # vault 私有元数据（建议纳入版本控制）
      order/
        notes.order.md    # 自定义排序（Source-of-truth）
      meta/
        01H...json        # per-note 增量元数据（typed relations/pins 等，Source-of-truth）

  Resources.asset/
    items/
      01H...-SomeVideo/
        asset.md          # 资源 sidecar（可扩展，Markdown + frontmatter）
        original/         # 原始导入文件（或内容寻址存储引用）
        previews/         # 缩略图/抽帧（可重建，但可放这里方便迁移）
    .xnote/               # asset 库私有缓存/派生（可删除重建）
      derived/            # OCR/转写/字幕等衍生文本（建议放缓存）

  Inbox.info/
    entries/
      2026-02-01T120000Z-01H...md
    .xnote/
      derived/

  .xnote/                 # 工作区级（推荐）：索引/缓存/日志/AI 配置
    db.sqlite             # id↔path、link graph、fts 等（可删除重建）
    thumbnails/
    logs/
    ai/
      agents/             # agent 配置（可版本化）
      sessions/
```

说明：
- `*.vault/*.asset/*.info` 是**对人类友好**的“可识别容器”；内部结构可逐步演进。
- `.xnote/` 是 XNote 预留目录（XNote-managed）。其中既可能包含**可重建缓存**，也可能包含**用户态元数据/配置**（例如排序、per-note meta、agent 配置）。
  因此不要把“删掉 `.xnote/` 就能完全重建”当作默认假设——必须按子目录粒度判断。

### 2.1 `.xnote/` 的可删除性约定（强制）

| 位置 | 内容示例 | 类型 | 是否 Source-of-truth | 是否可删除重建 |
| --- | --- | --- | --- | --- |
| `Knowledge.vault/.xnote/order/` | `notes.order.md` | **用户态元数据** | ✅ | ❌ |
| `Knowledge.vault/.xnote/meta/` | `01H...json` | **用户态元数据** | ✅ | ❌ |
| `*/.xnote/ai/agents/` | `coordinator.json` | **配置（可版本化）** | ✅（若你选择版本化） | ⚠️（删了会丢配置） |
| `Workspace/.xnote/db.sqlite` | 索引库 | 缓存 | ❌ | ✅ |
| `Workspace/.xnote/thumbnails/` | 缩略图 | 缓存 | ❌ | ✅ |
| `Resources.asset/.xnote/derived/` | OCR/转写等 | 缓存 | ❌ | ✅ |
| `Inbox.info/.xnote/derived/` | 转写/派生 | 缓存 | ❌ | ✅ |
| `Workspace/.xnote/logs/` | 审计/运行日志 | 日志 | ❌ | ✅（但会丢审计） |

> 设计原则：**Source-of-truth 永远在可读文件中**（笔记/sidecar/order/config），数据库与派生物只做加速与体验增强。

## 3. 链接与可扩展元数据（不写死 JSON Schema）

### 3.1 可扩展 sidecar：Markdown + frontmatter

所有实体（尤其 Resource/Info）都应有一个“侧车描述文件”，推荐：
- `asset.md`（资源）
- `entry.md`（信息）
- Knowledge 直接是 `*.md` 笔记本体

使用 YAML frontmatter 保存少量关键字段（稳定、可索引）：
- `id`：稳定 id（ULID/UUIDv7）
- `kind`：`knowledge | resource | info`
- `type`：更细粒度类型（例如 `resource/video`、`resource/image`）
- `createdAt/updatedAt`

正文使用 Markdown 表达“可移植的链接语义”（兼容优先）：
- 标准 Markdown 链接（推荐）：`[Title](relative/path.md)`
- Obsidian 风格 wiki link（可选兼容）：`[[Some Note]]`
- 跨模块/深链接：`xnote://resource/<id>`、`xnote://info/<id>`（其他软件可显示为链接但未必可打开）

更激进的“强连接表达”（typed relations/pins/来源/序列等）建议不要写进正文语法，改为存放在增量文件：
- `Knowledge.vault/.xnote/meta/<id>.json`（每篇笔记一个，Source-of-truth）
- 关系类型命名采用 `xnote.*` 点分层（例如 `xnote.source`、`xnote.explains`）

### 3.2 “自定义排序”作为一等公民关系

Knowledge 的“索引/过程”排序本质是“序列关系”，必须：
- 不依赖文件名
- 可拖拽编辑
- 可在 Git diff 中查看变更

推荐每个文件夹一个 `*.order.md`（Source-of-truth）：

```md
# Order for notes/
- [[path:notes/Intro.md]]
- [[path:notes/Basics.md]]
```

说明：
- MVP 阶段以 `path:` 为主；引入 note `id` 后可升级为 `[[id:01H...]]` 形式
- 由于 order 文件位于 `.xnote/`，默认不会被当作“笔记”参与 graph/backlinks
- 如确实需要结构化格式，也可选用 `*.order.json`，但不建议把“强 schema”扩散到其他元数据

## 4. UI/窗口模型（多窗口 + 模块切换）

- 每个窗口有：
  - `workspaceId`
  - `activeModule`（默认 `knowledge`）
  - `selection/context`（给 AI/命令系统使用）
- 同一工作区可开多个窗口（例如一个窗口看知识、另一个窗口看资源）。
- 模块切换只切“视图与命令集”，不切数据源；三模块共享：
  - 全局搜索（关键词/语义）
  - Link Graph（反链、关联跳转）

## 5. 命令总线（Command-first，AI 与 UI 共用）

核心思想：所有能力做成“确定性命令”，UI 调用命令，AI 未来也调用同一命令。

命令分层：
- `vault:*`：知识库文件与排序（读/写/移动/排序）
- `resource:*`：导入、预览、外部编辑、派生生成
- `info:*`：收件箱写入、时间线查询、转化/归档
- `graph:*`：links/backlinks/query
- `ai:*`：上下文打包、草稿生成、应用 diff（受策略守卫）

命令通道：
- renderer → preload：`window.xnote.*`（最小暴露）
- preload → main：IPC（输入校验 + 权限/策略守卫）
- main：执行文件系统/索引/队列任务，广播事件给所有窗口

> Rust + GPUI 目标态：命令总线仍然存在，但“通道”变为 **进程内调用**（UI 线程 → Core 服务层），或保留可选的本地 socket 以支持外部工具/MCP 复用。

命名约定（建议统一）：
- **命令 ID**：`namespace:verbNoun`（例如 `vault:updateNote`、`graph:backlinks`）
- **preload API**：`window.xnote.<namespace>.<verbNoun>(...)`（由 preload 映射到同一命令并做 schema 校验）

## 6. AI 中枢（MCP-like Tool Bridge）

AI 不是“读文件猜一切”，而是通过工具通道操作工作站：
- **Context Pack**：把“选区 + 局部上下文 + 关联实体摘要 + 当前模块状态”打包给模型
- **Tool/Command Allowlist**：按模块渐进暴露工具；高风险操作必须 UI 确认
- **Audit + Undo**：记录每次 AI 调用的命令与影响范围，支持回滚

详见：
- `design/ai/README.md`

## 7. 子设计文档入口

- 架构（Electron → Rust+GPUI 迁移/分层/性能原则）：`design/architecture/README.md`
- Knowledge（笔记）：`design/knowledge/README.md`
- Resource（资产）：`design/resources/README.md`
- Information（信息）：`design/info/README.md`
- AI Hub（中枢）：`design/ai/README.md`
  - VCP（变量与命令协议）适配：`design/ai/VCP.md`

## 8. 工程推进清单（CSV）

工程推进采用 CSV 清单的同款方法：**小步端到端 + 状态可追踪**（历史参考：`archive/electron/app/docs/obsidian_feature_map.csv`）。

- Rust + GPUI 新架构的推进清单：`docs/workstation_feature_map.csv`
