# 2026-02-08 Editor Split Direction + Per-Tab View State

## 阶段目标
- 将 split 从单一 `Split Right` 升级为 `Split Right / Split Down` 双方向。
- 增加“每个 tab 独立记忆编辑视图状态”（Edit/Preview/Split + split ratio + split direction）。
- 保持跨重启持久化：split 比例与方向可恢复。

## 实施步骤
1. 扩展 split 方向模型与拖拽主轴抽象。
2. 扩展窗口布局设置字段，保存 split direction。
3. 增加 tab 级 editor view state 存储与切换恢复。
4. 分屏渲染改为“按方向”布局与分隔条拖拽（横向/纵向）。
5. 回归编译与测试，确认无回归。

## 关键改动
- `crates/xnote-ui/src/main.rs`
  - 新增：
    - `EditorSplitDirection::{Right, Down}`
    - `EditorTabViewState`
    - `editor_tab_view_state: HashMap<String, EditorTabViewState>`
    - `editor_split_direction`
  - `open_note/close_editor`：
    - 切换 tab 前保存当前 tab 视图状态。
    - 打开 tab 时恢复该 tab 视图状态（无记录则默认 Edit）。
  - 监听外部 rename/remove：
    - 同步迁移/清理 `editor_tab_view_state`，避免悬挂状态。
  - split 渲染：
    - 支持按方向 `flex_row/flex_col`。
    - 分隔条根据方向切换 `col-resize/row-resize`。
  - split 拖拽：
    - 拖拽主轴改为 `x/y` 自适应方向。
    - 垂直 split 使用独立最小高度约束。

- `crates/xnote-core/src/settings.rs`
  - `WindowLayoutSettings` 新增：`editor_split_direction: Option<String>`
  - `Default/merge_overlay` 与相关测试同步扩展。

## 当前状态
- 状态：`done`
- 已达成：
  - `Split Right / Split Down` 已可切换并拖拽。
  - 每个 tab 记忆编辑视图状态。
  - split 比例 + 方向持久化可恢复。

## 验证
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo test -p xnote-ui --no-run` ✅

## 下一步建议
- 如需更进一步对标 VSCode，可继续：
  - tab 关闭后“回退到最近激活历史栈”；
  - split 方向按钮替换为更明确图标（新增 `split-horizontal/split-vertical` 资源）；
  - preview 与 edit 的滚动同步（可配置开关）。

