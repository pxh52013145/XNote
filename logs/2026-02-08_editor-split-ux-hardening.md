# 2026-02-08 Editor Split UX Hardening

## 阶段目标
- 将编辑器 `Split` 从静态 50/50 演示升级为 IDE 可用交互：可拖拽、可回退、可持久化。
- 对齐 VSCode 常见体验：切换 split 后能回到上一次单窗模式，不强制回到 Edit。
- 保证渲染稳定：拖拽时不穿透、不抖动、最小宽度可控。

## 实施步骤
1. 在 `xnote-ui` 增加 split 运行时状态：比例、拖拽态、非 split 模式记忆。
2. 重构 `set_editor_view_mode / toggle_editor_split_mode` 行为，补齐模式回退语义。
3. 将 split 主体改为“比例+最小宽度约束”的左右 pane 布局，并加入分隔条拖拽。
4. 复用全局 drag overlay，统一捕获拖拽期间鼠标事件，避免拖动丢失。
5. 扩展 `WindowLayoutSettings`，保存并恢复 split 比例。
6. 回归验证 core/ui 编译与测试。

## 关键改动
- `crates/xnote-ui/src/main.rs`
  - 新增常量：
    - `EDITOR_SPLIT_MIN_RATIO`
    - `EDITOR_SPLIT_MAX_RATIO`
    - `EDITOR_SPLIT_MIN_PANE_WIDTH`
    - `EDITOR_SPLIT_DIVIDER_WIDTH`
    - `EDITOR_SPLIT_RATIO_SCALE`
  - 新增状态：
    - `editor_split_ratio`
    - `editor_split_saved_mode`
    - `editor_split_drag`
  - 新增逻辑：
    - `begin_editor_split_drag`
    - `on_editor_split_drag_mouse_move`
    - `on_editor_split_drag_mouse_up`
    - `on_active_split_drag_mouse_move`
    - `on_active_split_drag_mouse_up`
  - 行为升级：
    - `set_editor_view_mode` 进入 split 时记忆当前模式。
    - `toggle_editor_split_mode` 退出 split 时恢复最近模式（Edit/Preview）。
    - `editor_body` split 区域改为可拖拽分栏，支持最小宽度与比例夹紧。
    - 全局拖拽 overlay 兼容 side splitter 与 editor split divider。
  - 持久化：
    - `apply_persisted_split_layout` 读取 split 比例。
    - `window_layout_snapshot` 写回 split 比例。

- `crates/xnote-core/src/settings.rs`
  - `WindowLayoutSettings` 新增字段：`editor_split_ratio_milli: Option<u16>`。
  - `Default` 与 `merge_overlay` 同步支持该字段。
  - 分层配置与 merge 测试补齐该字段覆盖。

## 当前状态
- 状态：`done`（本阶段完成）
- 已达成：
  - Split 可拖拽、可约束、可稳定渲染。
  - Split 退出恢复到历史模式，交互更接近 VSCode。
  - Split 比例跨重启可恢复。

## 验证
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo test -p xnote-ui --no-run` ✅

## 下一步
- 若要继续对标 VSCode，可在后续加：
  - `Split Right / Split Down` 多分栏策略；
  - 每个 tab 独立记忆 split 状态；
  - 预览 pane 的滚动同步与光标定位联动。

