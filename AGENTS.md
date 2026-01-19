# AGENTS.md（XNote）

本文件用于指导在本仓库中工作的 AI/自动化 Agent（以及人类协作者），让改动保持一致、可验证、可回滚。

## 项目概览
- XNote：桌面端 Markdown 笔记应用（Obsidian-like），技术栈为 Electron + React + TypeScript。
- 构建/工具链：`electron-vite`、`vite`、`typescript`、`vitest`。
- 编辑/渲染：`@uiw/react-codemirror`（CodeMirror 6）、`react-markdown`、`remark-gfm`。

## 目录结构（重要）
- `src/main/`：Electron 主进程（IPC、vault 文件系统、索引等）。
- `src/preload/`：安全桥接（`contextBridge` 暴露 `window.xnote`）。
- `src/renderer/src/`：React UI（编辑/阅读/侧边栏/设置等）。
- `src/shared/`：主进程与渲染进程共享的 types/utils（尽量保持纯函数、无 Electron 依赖）。
- `docs/obsidian_feature_map.csv`：功能对齐/优先级/状态（推进功能的主要清单）。
- `tests/`：Vitest 单元/集成测试。

## 默认工作流（按清单推进）
当用户让你“继续推进/按 P1 顺序”等类似需求时，按以下顺序执行：
1. 打开 `docs/obsidian_feature_map.csv`，找到目标优先级（通常是 P1）里**下一个未完成**条目。
2. 小步实现端到端：优先打通 `main → preload → renderer`，再补齐 UI/交互。
3. 完成后更新 CSV 的 `状态` 与 `备注`（`todo/partial/done`），保证可追踪。
4. 跑 `npm run typecheck` 与 `npm test` 做回归验证。

备注：该 CSV 可能带 UTF-8 BOM；在 PowerShell 下如遇乱码，读取时使用 `-Encoding utf8`。

## 代码约定与注意事项
- **Vault 内路径**：统一使用 POSIX 分隔符（`a/b.md`）；涉及文件系统必须做路径规范化与越权/路径穿越防护。
- **分层边界**：Electron/Node 能力只放在 `main/preload`；`shared` 仅放类型与纯逻辑；`renderer` 不直接访问 Node API。
- **IPC 设计**：只在 preload 暴露必要能力；命名保持 `vault:*`、`window:*`、`shell:*` 等风格一致。
- **React 稳定性**：订阅类数据（如 `useSyncExternalStore`）要返回稳定快照，避免渲染循环或黑屏。
- **变更范围**：不做无关重构；不改动未被需求覆盖的行为；除非明确要求，不执行 `git commit`/`git reset` 等操作。
- **UI 文案**：当前界面以英文为主（保持一致性）；面向用户的说明可以中文。

## 常用命令
- 开发：`npm run dev`
- 构建：`npm run build`
- 预览：`npm run preview`
- 类型检查：`npm run typecheck`
- 测试：`npm test`

