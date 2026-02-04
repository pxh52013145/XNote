# Knowledge UI in GPUI — View Tree & State

目标：给 GPUI 实现一个“像 Zed 一样”的工作站式 Knowledge UI：轻量、无 webview、渲染快、输入顺滑。

## 1. View Tree（建议）

`AppRoot`
- `WindowChrome`（可选，自绘标题栏/菜单）
- `MainSplit`
  - `ExplorerPane`
    - `ExplorerHeader`（Vault selector + filter input）
    - `ExplorerTree`（虚拟化树）
  - `EditorPane`
    - `TabStrip`（MVP 可先单 tab）
    - `TextEditorView`
  - `InspectorPane`
    - `InspectorTabs`（Outline / Backlinks / Links）
    - `InspectorContent`（MVP 占位）
- `BottomBar`
  - `ModuleSwitcher`（最左）
  - `WorkspaceStatus`
  - `CursorStatus`

## 2. State Model（避免 UI/索引耦合）

建议把状态拆成三层：

1. **CoreState（线程安全）**
   - 当前 vault root
   - note 元数据缓存（path, mtime, size）
   - order 缓存（folder → orderedPaths）
   - 搜索索引状态（ready/partial）

2. **UIState（UI 线程）**
   - 当前选择（selected folder/note）
   - 编辑器状态（open note, dirty, selection）
   - Explorer 展开状态（展开哪些节点）
   - 过滤器与搜索查询

3. **Async Jobs**
   - 扫描任务（scan vault）
   - 写入任务（save note）
   - 解析任务（frontmatter/links）
   - watcher 事件处理（debounce）

核心约束：UIState 不直接读取文件系统；只通过命令拿快照/订阅事件。

## 3. 事件流（UI 不阻塞）

Core 向 UI 广播的最小事件：
- `VaultOpened(root)`
- `TreeChanged(changes[])`（新增/删除/重命名）
- `OrderChanged(folder)`
- `NoteChanged(path)`（外部保存触发）
- `IndexProgress(percent)`

UI 只做：
- 合并事件（coalesce）
- 刷新虚拟列表的可视区
- 在必要时触发重新读取当前打开文件（debounce）

## 4. Drag & Drop Sorting（关键交互细节）

ExplorerTree 需要支持：
- 同文件夹内的拖拽 reorder
- 拖拽反馈：插入线 + 目标位置预览
- drop 后立即更新 UI 顺序（乐观更新），并异步 `vault.order.set`
- 写入失败时回滚 UI 顺序并 toast

## 5. 渐进增强路线

MVP 之后可以按顺序扩展：
1. 多 tab + 分屏
2. 更强的 `[[` link picker（增量索引 + 预览）
3. Backlinks/Graph（先基于解析 link）
4. Markdown 预览（可选：走轻量渲染，不引入 webview）

