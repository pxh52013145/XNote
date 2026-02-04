# NoteMeta v1 — Knowledge 笔记的增量元数据

目标：让 `*.md` 笔记保持最大兼容（Obsidian + 其他 Markdown 软件可读/尽量可用），同时让 XNote 通过**增量文件**渲染更强的“连接导向”语义（typed relations / pins 等）。

## 1. 存放位置与命名

在 vault 内：

```text
Knowledge.vault/
  .xnote/
    meta/
      <noteId>.json
```

其中 `<noteId>` 来自笔记 frontmatter 的稳定 `id`。

备注：
- 本文件是 typed relations / pins 的 **Source-of-truth**。
- 必须可 Git diff/merge。
- 必须避免被 Obsidian 当作“笔记”解析（因此使用 JSON，而不是 `.md`）。

## 2. 关系类型命名

- 使用**点分层**：`xnote.<namespace>.<verb>`（简单场景可用 `xnote.<verb>`）
- 例子：
  - `xnote.source`
  - `xnote.explains`
  - `xnote.sequence.next`
  - `plugin.myPlugin.relatesTo`

规则：未知 `type` 必须保留（前向兼容）。

## 3. JSON 结构（v1）

最小结构：

```json
{
  "version": 1,
  "id": "01H...",
  "updatedAt": "2026-02-01T12:00:00Z",
  "relations": [],
  "pins": {
    "resources": [],
    "infos": [],
    "notes": []
  },
  "ext": {}
}
```

### 3.1 `relations[]`

每条 relation 是一条**有向** typed edge。

```json
{
  "type": "xnote.explains",
  "to": { "kind": "knowledge", "id": "01H..." },
  "note": "optional human note",
  "createdAt": "2026-02-01T12:00:00Z",
  "createdBy": "user"
}
```

字段：
- `type`（string，必填）：关系类型 id
- `to`（object，必填）：目标实体引用
  - `kind`：`knowledge | resource | info`
  - `id`：目标 id
  - `anchor`（可选）：稳定锚点提示（建议 V2+ 再强化）
- `note`（string，可选）：人类注释
- `createdAt`（ISO string，可选）
- `createdBy`（`user | ai | system`，可选）

允许出现更多字段（前向兼容）。

### 3.2 `pins`

Pins 是 UI 提示：把这些实体固定展示在笔记的关联面板中。

```json
"pins": {
  "resources": ["01H..."],
  "infos": ["01H..."],
  "notes": ["01H..."]
}
```

Pins **不是** relations 的替代；它是“注意力快捷方式”。

### 3.3 `ext`

`ext` 是插件/AI 的自由扩展区。

Example:

```json
"ext": {
  "xnote.ai": {
    "suggestedRelations": [
      {
        "type": "xnote.explains",
        "to": { "kind": "knowledge", "id": "01H..." },
        "confidence": 0.78,
        "status": "proposed"
      }
    ]
  }
}
```

规则：XNote core 必须忽略未知 key，但不能丢数据。

## 4. Canonical JSON 规则（Git 友好）

为了减少噪音 diff、提高 merge 可预测性：

1. UTF-8, LF line endings
2. 2-space indentation, trailing newline
3. key 顺序（推荐）：
   - `version`, `id`, `updatedAt`, `relations`, `pins`, `ext`
4. 数组：
   - `relations` 默认 append-only，保留顺序
   - `pins.*` 保留顺序（用户偏好）
5. 写入策略：
   - 小改动不要重写整个文件（避免 churn）

## 5. Source-of-truth 边界

- **MD note**：内容 + 可移植链接（Markdown links / 可选 `[[...]]`）
- **NoteMeta JSON**：typed relations、pins、AI suggestions
- **Index DB**：纯缓存

不要让 NoteMeta 成为“读懂笔记内容”的必要条件。
