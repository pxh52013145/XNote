# 2026-02-08 Editor Gutter + Layout Persistence Hardening

## 阶段目标
- 将编辑器行号栏压窄到更接近 VSCode 的紧凑观感。
- 保证 `1~9999` 行范围内 gutter 宽度稳定，不因位数变化引发布局抖动。
- 持久化窗口与分栏布局（窗口尺寸/位置、Explorer/Workspace 宽度、折叠状态），并在下次启动恢复。

## 实施步骤
1. 收敛 gutter 常量（base/digit/text padding）并引入稳定位数策略。
2. 在 editor layout 计算中强制 `max(line_digits, 4)`，直到 `10000+` 才扩展宽度。
3. 在 `xnote-core::settings::WindowLayoutSettings` 已有结构上补齐 UI 读写流程。
4. 启动时恢复分栏状态与窗口 bounds；运行时通过 debounce 持久化。
5. 增加单测并执行 core/ui 回归校验。

## 关键落地
- `crates/xnote-ui/src/main.rs`
  - 行号栏紧凑化：
    - `EDITOR_GUTTER_BASE_WIDTH = 3.0`
    - `EDITOR_GUTTER_DIGIT_WIDTH = 5.0`
    - `EDITOR_TEXT_LEFT_PADDING = 5.0`
  - 宽度稳定策略：
    - `line_number_digits(max_line).max(EDITOR_GUTTER_STABLE_DIGITS_MAX_9999)`
  - 布局持久化：
    - 新增窗口默认/最小尺寸常量
    - 新增 `apply_persisted_split_layout`
    - 新增 `window_layout_snapshot`
    - 新增 `schedule_window_layout_persist_if_changed`（320ms debounce）
    - 在 `render` 主流程注入布局变化持久化调度
    - `main()` 启动时基于 settings 恢复 `WindowBounds`
  - 新增测试：`editor_gutter_digits_are_stable_for_1_to_9999_lines`

## 当前状态
- 状态：**done（本里程碑完成）**
- 已满足：
  - 行号区域更紧凑
  - 1~9999 行无位数抖动
  - 窗口与分栏布局可跨重启恢复

## 回归验证
- `cargo test -p xnote-core` ✅
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-ui --no-run` ✅

## 下一步
- 在真实使用中观察多显示器与 DPI 缩放下的窗口恢复体验。
- 如需进一步对标 VSCode，可追加“窗口最大化/全屏状态持久化”显式字段（当前优先持久化 bounds + split）。

---

## 2026-02-08 Explorer 交互紧凑化补丁

### 目标
- Explorer 头部按钮更紧凑，减少视觉稀疏。
- Explorer / Workspace 可收缩到更实用的最小宽度。
- Explorer 行项悬停显示完整名称（含深层路径），对齐 VSCode 使用习惯。

### 落地
- `crates/xnote-ui/src/main.rs`
  - 新增紧凑常量：
    - `PANEL_SHELL_MIN_WIDTH = 150.0`（原有效最小 180）
    - `WORKSPACE_MIN_WIDTH = 180.0`（原有效最小 220）
    - `EXPLORER_HEADER_ACTION_SIZE = 20.0`
    - `EXPLORER_HEADER_ICON_SIZE = 14.0`
  - Explorer 头部按钮组：收缩 hitbox、图标尺寸与间距。
  - 侧栏最小宽度约束：统一改为常量，覆盖恢复/拖拽/响应式计算路径。
  - 新增 `TooltipPreview`，并为 Explorer 行项接入 `.tooltip(...)`：
    - Filter note 行
    - Vault root 行
    - Hint 行
    - Folder 行
    - Note 行（使用完整 path）

### 验证
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-ui --no-run` ✅
