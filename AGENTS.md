# AGENTS.md（XNote / Rust + GPUI）

本文件用于指导在本仓库中工作的 AI/自动化 Agent（以及人类协作者），让改动保持一致、可验证、可回滚。

## 项目概览
- XNote：桌面端“笔记为核心”的 IRK（Info/Resource/Knowledge）超级工作站。
- 目标：Zed-like 极致轻量与性能（Rust Core + GPUI 原生渲染）。
- 当前阶段：优先完成 Knowledge（笔记）模块的高性能 MVP（10 万规模门禁）。

## 目录结构（重要）
- `crates/xnote-core/`：核心逻辑（vault 扫描/读写、安全路径、排序 order、索引等）。**不依赖 UI**。
- `crates/xnote-ui/`：GPUI 桌面 UI（Explorer/Editor/Inspector/Bottom bar）。
- `crates/xtask/`：工程任务（后续放 10 万 vault 生成器、基准测试、指标采集）。
- `design/`：蓝图设计（功能/数据/架构/性能门禁的 Source of truth）。
- `docs/workstation_feature_map.csv`：推进清单（P0/P1/状态/备注）。
- `Xnote.pen`：UI 设计画布。
- `archive/electron/app/`：已归档的 Electron 旧实现（仅参考，不再作为主线）。

## 默认工作流（按清单推进）
当用户让你“继续推进/按 P0/P1 顺序”等需求时：
1. 打开 `docs/workstation_feature_map.csv`，找到目标优先级里**下一个未完成**条目。
2. 小步实现（优先打通 `xnote-core → xnote-ui`），保证可运行、可验证。
3. 完成后更新 CSV 的 `状态` 与 `备注`（`todo/partial/done`），保证可追踪。
4. 跑回归验证：
   - `cargo test -p xnote-core`
   - `cargo check -p xnote-ui`（或 `cargo run -p xnote-ui`）

## 代码约定与注意事项
- **Vault 内路径**：统一使用 POSIX 分隔符（`a/b.md`）；必须做路径规范化与越权/路径穿越防护。
- **分层边界**：Core（`xnote-core`）不引用 GPUI；UI 只通过命令/接口调用 Core。
- **性能优先**：任何会在 10 万规模放大的行为（全量读内容、全量排序、全量重建索引）必须改为增量/分页/后台队列。
- **变更范围**：不做无关重构；不改动未被需求覆盖的行为；除非明确要求，不执行 `git commit`/`git reset` 等操作。
- **UI 文案**：界面以英文为主（保持一致性）；面向用户的说明可以中文。

## 常用命令
- Core 测试：`cargo test -p xnote-core`
- UI 检查：`cargo check -p xnote-ui`
- 运行 UI：`cargo run -p xnote-ui`

