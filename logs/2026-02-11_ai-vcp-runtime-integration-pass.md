# 2026-02-11 AI VCP Runtime Integration Pass

## 阶段目标
- 将 XNote AI Hub 从“本地占位探测”升级为“VCP 双端点运行时探测 + 管理面快照同步”基座。
- 修复设置页 AI runtime 编辑/交互一致性问题，确保未连接时 UI 行为真实、可解释。

## 实施步骤
1. 在 `xnote-core` 新增 `vcp` 模块并抽象 endpoint 规范化、health probe、admin snapshot。
2. 将 `xnote-ui` 的连接检查接入 `xnote_core::vcp::probe_vcp_runtime`，统一状态映射。
3. AI Hub 增加 pane/meta/runtime 数据展示（agents/plugins/rag/schedules/metrics/warnings）。
4. 清理 UI 旧探测实现，避免重复逻辑与编译冲突。
5. 修复设置编辑态输入与按钮交互（含 timeout 仅数字输入）。
6. 校验 i18n key 覆盖并完成回归编译测试。

## 当前状态
- `cargo check -p xnote-ui`：通过。
- `cargo test -p xnote-core`：通过（112 + 10 tests）。
- AI Chat 在未连接 VCP 时禁用并弹连接提示；不再误导“已就绪”。
- Settings AI Runtime 支持：admin endpoint / auth / timeout / ws sync 的编辑与持久化。

## 下一步
- 对接 VCP admin 实时变更（WebSocket 增量同步），减少轮询依赖。
- 按 VCPChat 模块拆分 AI Hub 子页面（会话、代理、插件、任务、知识）并加操作入口。
- 增加“连接诊断详情”页（chat/admin 分状态、HTTP 码、auth 提示、端口建议）。
