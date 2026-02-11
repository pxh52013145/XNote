use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    EnUs,
    ZhCn,
}

impl Locale {
    pub fn from_tag(input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" => Some(Self::EnUs),
            "zh" | "zh-cn" => Some(Self::ZhCn),
            _ => None,
        }
    }

    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::ZhCn => "zh-CN",
        }
    }
}

pub struct I18n {
    locale: Locale,
    en_us: HashMap<&'static str, &'static str>,
    zh_cn: HashMap<&'static str, &'static str>,
}

impl I18n {
    pub fn new(locale: Locale) -> Self {
        Self {
            locale,
            en_us: en_us_dict(),
            zh_cn: zh_cn_dict(),
        }
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
    }

    pub fn text(&self, key: &str) -> String {
        match self.locale {
            Locale::EnUs => self
                .en_us
                .get(key)
                .copied()
                .or_else(|| self.zh_cn.get(key).copied())
                .unwrap_or(key)
                .to_string(),
            Locale::ZhCn => self
                .zh_cn
                .get(key)
                .copied()
                .or_else(|| self.en_us.get(key).copied())
                .unwrap_or(key)
                .to_string(),
        }
    }
}

fn en_us_dict() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("cmd.open_vault.label", "Open vault"),
        ("cmd.open_vault.detail", "Choose a folder as a vault"),
        (
            "cmd.open_vault_in_new_window.label",
            "Open vault in new window",
        ),
        (
            "cmd.open_vault_in_new_window.detail",
            "Choose a folder and open it in a new app window",
        ),
        ("cmd.quick_open.label", "Quick Open"),
        ("cmd.quick_open.detail", "Search and open a note by path"),
        ("cmd.command_palette.label", "Command palette"),
        ("cmd.command_palette.detail", "Search and run commands"),
        ("cmd.settings.label", "Settings"),
        ("cmd.settings.detail", "Preferences, hotkeys, and more"),
        ("cmd.reload_vault.label", "Reload vault"),
        (
            "cmd.reload_vault.detail",
            "Rescan notes and apply folder order",
        ),
        ("cmd.new_note.label", "New note"),
        (
            "cmd.new_note.detail",
            "Create a note in the selected folder",
        ),
        ("cmd.save_file.label", "Save file"),
        ("cmd.save_file.detail", "Write the current note to disk"),
        ("cmd.undo.label", "Undo"),
        ("cmd.undo.detail", "Undo the last edit operation"),
        ("cmd.redo.label", "Redo"),
        ("cmd.redo.detail", "Redo the last undone edit operation"),
        ("cmd.toggle_split.label", "Toggle split"),
        ("cmd.toggle_split.detail", "Split the editor view"),
        ("cmd.focus_explorer.label", "Explorer"),
        ("cmd.focus_explorer.detail", "Show Explorer panel"),
        ("cmd.focus_search.label", "Search"),
        ("cmd.focus_search.detail", "Show Search panel"),
        ("cmd.ai_rewrite_selection.label", "AI: Rewrite Selection"),
        (
            "cmd.ai_rewrite_selection.detail",
            "Draft a rewrite proposal for selected text",
        ),
        ("settings.title", "Settings"),
        ("settings.nav.about", "About"),
        ("settings.nav.appearance", "Appearance"),
        ("settings.nav.editor", "Editor"),
        ("settings.nav.ai", "AI"),
        ("settings.nav.files", "Files & Links"),
        ("settings.nav.hotkeys", "Hotkeys"),
        ("settings.nav.advanced", "Advanced"),
        ("settings.theme.dark", "Dark"),
        ("settings.theme.light", "Light"),
        ("settings.accent.default", "Default"),
        ("settings.accent.blue", "Blue"),
        ("settings.section.language", "LANGUAGE"),
        ("settings.section.theme", "THEME"),
        ("settings.section.colors", "COLORS"),
        ("settings.language.hint", "Change the UI language."),
        ("settings.language.english", "English"),
        ("settings.language.chinese", "Simplified Chinese"),
        ("settings.colors.accent", "Accent color"),
        (
            "settings.colors.accent.hint",
            "Used for buttons, links and highlights.",
        ),
        ("settings.common.none", "(none)"),
        ("settings.common.enabled", "Enabled"),
        ("settings.common.disabled", "Disabled"),
        ("settings.about.runtime_status.idle", "idle"),
        ("settings.about.runtime_status.activating", "activating"),
        ("settings.about.runtime_status.ready", "ready"),
        ("settings.about.runtime_status.error", "error"),
        ("settings.about.card.workspace.title", "Workspace"),
        (
            "settings.about.card.workspace.desc",
            "Current Knowledge workspace and content stats.",
        ),
        ("settings.about.workspace.notes", "Vault notes"),
        ("settings.about.workspace.open_note", "Open note"),
        ("settings.about.workspace.bookmarks", "Bookmarks"),
        ("settings.about.card.runtime.title", "Runtime"),
        (
            "settings.about.card.runtime.desc",
            "Plugin runtime activation and mode summary.",
        ),
        ("settings.about.runtime.plugins", "Plugins"),
        ("settings.about.runtime.mode", "Runtime mode"),
        ("settings.about.runtime.status", "Runtime status"),
        (
            "settings.editor.status.autosave_set",
            "Autosave delay set to",
        ),
        ("settings.editor.unit.ms", "ms"),
        ("settings.editor.card.autosave.title", "Autosave"),
        (
            "settings.editor.card.autosave.desc",
            "Controls delayed writeback while editing markdown notes.",
        ),
        ("settings.editor.card.behavior.title", "Editor behavior"),
        (
            "settings.editor.card.behavior.desc",
            "Core editing features are active: selection, IME, clipboard and save.",
        ),
        (
            "settings.editor.card.behavior.hint",
            "Use Ctrl+S to force-save. Autosave applies after idle delay.",
        ),
        ("settings.ai.status.provider_set", "AI provider set to"),
        ("settings.ai.status.endpoint_set", "AI endpoint set to"),
        ("settings.ai.status.model_set", "AI model set to"),
        (
            "settings.ai.status.tool_injection_enabled",
            "VCP tool injection enabled",
        ),
        (
            "settings.ai.status.tool_injection_disabled",
            "VCP tool injection disabled",
        ),
        ("settings.ai.toggle.tool_injection_on", "Tool Injection: On"),
        (
            "settings.ai.toggle.tool_injection_off",
            "Tool Injection: Off",
        ),
        ("settings.ai.status.endpoint_ready", "AI endpoint ready:"),
        (
            "settings.ai.status.endpoint_checking",
            "Checking AI endpoint...",
        ),
        ("settings.ai.status.endpoint_ok", "AI endpoint reachable:"),
        (
            "settings.ai.status.endpoint_failed",
            "AI endpoint unreachable:",
        ),
        ("settings.ai.button.apply_check", "Apply & Check"),
        ("settings.ai.card.provider.title", "AI Provider"),
        (
            "settings.ai.card.provider.desc",
            "Switch between local mock and VCP backend compatibility mode.",
        ),
        ("settings.ai.card.endpoint.title", "VCP Endpoint"),
        (
            "settings.ai.card.endpoint.desc",
            "Choose target VCPToolBox server endpoint for AI requests.",
        ),
        ("settings.ai.card.model.title", "VCP Model"),
        (
            "settings.ai.card.model.desc",
            "Pick default model name used in VCP-compatible requests.",
        ),
        ("settings.ai.card.tool_loop.title", "Tool Loop"),
        (
            "settings.ai.card.tool_loop.desc",
            "Control whether VCP tool-injection endpoint is used.",
        ),
        ("settings.ai.card.connection.title", "Connection"),
        (
            "settings.ai.card.connection.desc",
            "Apply current AI settings and run live endpoint check.",
        ),
        (
            "settings.ai.connection.not_checked",
            "No connection check yet. Click 'Apply & Check'.",
        ),
        (
            "settings.files.status.external_sync_enabled",
            "External sync enabled",
        ),
        (
            "settings.files.status.external_sync_disabled",
            "External sync disabled",
        ),
        (
            "settings.files.status.prefer_wikilink_enabled",
            "Prefer wikilink titles enabled",
        ),
        (
            "settings.files.status.prefer_wikilink_disabled",
            "Prefer wikilink titles disabled",
        ),
        (
            "settings.files.card.external_sync.title",
            "External changes sync",
        ),
        (
            "settings.files.card.external_sync.desc",
            "Whether to sync external editor updates via watcher.",
        ),
        (
            "settings.files.card.wiki_pref.title",
            "Wiki link preference",
        ),
        (
            "settings.files.card.wiki_pref.desc",
            "Prefer wiki title resolution over strict path matching.",
        ),
        ("settings.hotkeys.button.cancel", "Cancel"),
        ("settings.hotkeys.button.edit", "Edit"),
        ("settings.hotkeys.button.reset", "Reset"),
        ("settings.hotkeys.button.reset_all", "Reset all to defaults"),
        ("settings.hotkeys.placeholder.press_keys", "Press keys..."),
        ("settings.hotkeys.status.reset_one", "Reset shortcut"),
        (
            "settings.hotkeys.status.reset_one_failed",
            "Reset shortcut failed",
        ),
        (
            "settings.hotkeys.status.shortcut_already_default",
            "Shortcut already default",
        ),
        (
            "settings.hotkeys.status.reset_all",
            "Reset all shortcuts to defaults",
        ),
        (
            "settings.hotkeys.status.reset_all_failed",
            "Reset all shortcuts failed",
        ),
        (
            "settings.hotkeys.status.all_shortcuts_already_default",
            "All shortcuts already default",
        ),
        ("settings.hotkeys.card.shortcuts.title", "Command shortcuts"),
        (
            "settings.hotkeys.card.shortcuts.desc",
            "Click Edit then press a new key chord in this window to override.",
        ),
        (
            "settings.hotkeys.tip",
            "Tip: Esc exits editing mode. Overrides are persisted to settings.",
        ),
        ("settings.advanced.runtime_mode.process", "process"),
        ("settings.advanced.runtime_mode.in_process", "in_process"),
        (
            "settings.advanced.status.runtime_mode_set",
            "Plugin runtime mode set to",
        ),
        ("settings.advanced.watcher.enabled", "Watcher Enabled"),
        ("settings.advanced.watcher.disabled", "Watcher Disabled"),
        (
            "settings.advanced.card.runtime_mode.title",
            "Plugin runtime mode",
        ),
        (
            "settings.advanced.card.runtime_mode.desc",
            "Switch between in-process and process-isolated runtime.",
        ),
        ("settings.advanced.card.file_watcher.title", "File watcher"),
        (
            "settings.advanced.card.file_watcher.desc",
            "Enable or disable external file synchronization watcher.",
        ),
        ("ai.hub.system.ready", "AI Hub is ready."),
        ("ai.hub.timestamp.now", "now"),
        ("ai.hub.session.title", "VCP Chat Session"),
        ("ai.hub.status.chat_in_progress", "AI chat in progress..."),
        (
            "ai.hub.status.chat_empty_response",
            "AI returned an empty response.",
        ),
        (
            "ai.hub.status.chat_completed_empty",
            "AI chat completed with empty response.",
        ),
        ("ai.hub.status.chat_done", "AI chat done"),
        ("ai.hub.status.chat_failed", "AI chat failed"),
        (
            "ai.hub.status.rewrite_in_progress",
            "AI rewrite in progress...",
        ),
        ("ai.hub.status.rewrite_applied", "AI draft applied"),
        ("ai.hub.status.rewrite_failed", "AI rewrite failed"),
        (
            "ai.hub.error.vault_not_opened",
            "AI chat failed: vault is not opened.",
        ),
        (
            "ai.hub.error.read_vault_state",
            "AI chat failed: cannot read vault state",
        ),
        ("ai.hub.error.input_empty", "AI chat input is empty."),
        ("ai.hub.error.no_open_note", "AI unavailable: no open note."),
        (
            "ai.hub.error.empty_selection",
            "AI rewrite unavailable: empty selection.",
        ),
        (
            "ai.hub.error.invalid_selection_range",
            "AI rewrite failed: invalid selection range.",
        ),
        (
            "ai.hub.error.rewrite_vault_not_opened",
            "AI rewrite failed: vault is not opened.",
        ),
        (
            "ai.hub.error.rewrite_read_vault_state",
            "AI rewrite failed: cannot read vault state",
        ),
        (
            "ai.hub.chat.instruction.base",
            "You are XNote AI assistant. Keep answers concise, practical, and note-oriented.",
        ),
        (
            "ai.hub.chat.instruction.context_header",
            "Recent conversation context:",
        ),
        ("ai.hub.chat.instruction.user_prompt", "User prompt:"),
        ("ai.hub.trace.calls", "Calls"),
        ("ai.hub.trace.rounds", "Rounds"),
        ("ai.hub.trace.stop", "Stop"),
        ("ai.hub.trace.calls_suffix", "calls"),
        ("ai.hub.trace.args.none", "no args"),
        ("ai.hub.trace.stop.final_response", "final response"),
        ("ai.hub.trace.stop.max_rounds", "max rounds reached"),
        ("ai.hub.trace.stop.unknown", "unknown"),
        ("ai.hub.ui.role.user", "You"),
        ("ai.hub.ui.role.assistant", "Assistant"),
        ("ai.hub.ui.role.system", "System"),
        (
            "ai.hub.ui.input.placeholder",
            "Type message... (Shift+Enter newline)",
        ),
        (
            "ai.hub.ui.input.disabled_placeholder",
            "VCP disconnected: run Apply & Check in Settings > AI",
        ),
        (
            "ai.hub.error.vcp_not_checked",
            "VCP connection is not checked yet. Go to Settings > AI and click Apply & Check.",
        ),
        (
            "ai.hub.error.vcp_not_connected",
            "VCP is not connected. Go to Settings > AI and fix endpoint/key, then Apply & Check.",
        ),
        ("ai.hub.popup.connection.title", "VCP Connection Required"),
        (
            "ai.hub.popup.connection.action_settings",
            "Open Settings > AI",
        ),
        ("ai.hub.popup.connection.action_close", "Close"),
        ("status.ready", "Ready"),
        ("hint.open_vault", "Open a vault (Ctrl+O)"),
        ("hint.vault_error", "Vault error (Ctrl+O)"),
        ("prompt.enter_vault_path", "Enter a path to a vault folder."),
        (
            "prompt.vault_path_not_folder",
            "Vault path is not a folder.",
        ),
        ("palette.empty_commands", "No commands"),
        ("palette.title.commands", "COMMAND PALETTE"),
        ("palette.title.quick_open", "QUICK OPEN"),
        ("palette.placeholder.commands", "Type a command..."),
        ("palette.placeholder.quick_open", "Type to search..."),
        ("palette.group.navigation", "NAVIGATION"),
        ("palette.group.files", "FILES"),
        ("palette.title.search", "SEARCH"),
        (
            "palette.placeholder.search",
            "Type keywords to search content...",
        ),
        ("palette.group.search", "SEARCH RESULTS"),
    ])
}

fn zh_cn_dict() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("cmd.open_vault.label", "打开仓库"),
        ("cmd.open_vault.detail", "选择一个文件夹作为仓库"),
        ("cmd.open_vault_in_new_window.label", "在新窗口打开仓库"),
        (
            "cmd.open_vault_in_new_window.detail",
            "选择文件夹并在新窗口中打开",
        ),
        ("cmd.quick_open.label", "快速打开"),
        ("cmd.quick_open.detail", "按路径搜索并打开笔记"),
        ("cmd.command_palette.label", "命令面板"),
        ("cmd.command_palette.detail", "搜索并执行命令"),
        ("cmd.settings.label", "设置"),
        ("cmd.settings.detail", "偏好设置、快捷键等"),
        ("cmd.reload_vault.label", "重载仓库"),
        ("cmd.reload_vault.detail", "重新扫描笔记并应用排序"),
        ("cmd.new_note.label", "新建笔记"),
        ("cmd.new_note.detail", "在当前文件夹创建笔记"),
        ("cmd.save_file.label", "保存文件"),
        ("cmd.save_file.detail", "将当前笔记写入磁盘"),
        ("cmd.undo.label", "撤销"),
        ("cmd.undo.detail", "撤销上一次编辑"),
        ("cmd.redo.label", "重做"),
        ("cmd.redo.detail", "重做被撤销的编辑"),
        ("cmd.toggle_split.label", "切换分栏"),
        ("cmd.toggle_split.detail", "切换编辑区域分栏"),
        ("cmd.focus_explorer.label", "资源管理器"),
        ("cmd.focus_explorer.detail", "显示资源管理器面板"),
        ("cmd.focus_search.label", "搜索"),
        ("cmd.focus_search.detail", "显示搜索面板"),
        ("cmd.ai_rewrite_selection.label", "AI：重写选区"),
        ("cmd.ai_rewrite_selection.detail", "为选中文本生成重写建议"),
        ("settings.title", "设置"),
        ("settings.nav.about", "关于"),
        ("settings.nav.appearance", "外观"),
        ("settings.nav.editor", "编辑器"),
        ("settings.nav.ai", "AI"),
        ("settings.nav.files", "文件与链接"),
        ("settings.nav.hotkeys", "快捷键"),
        ("settings.nav.advanced", "高级"),
        ("settings.theme.dark", "深色"),
        ("settings.theme.light", "浅色"),
        ("settings.accent.default", "默认"),
        ("settings.accent.blue", "蓝色"),
        ("settings.section.language", "语言"),
        ("settings.section.theme", "主题"),
        ("settings.section.colors", "颜色"),
        ("settings.language.hint", "切换界面语言。"),
        ("settings.language.english", "English"),
        ("settings.language.chinese", "简体中文"),
        ("settings.colors.accent", "强调色"),
        ("settings.colors.accent.hint", "用于按钮、链接与高亮。"),
        ("settings.common.none", "（无）"),
        ("settings.common.enabled", "已启用"),
        ("settings.common.disabled", "已禁用"),
        ("settings.about.runtime_status.idle", "空闲"),
        ("settings.about.runtime_status.activating", "启动中"),
        ("settings.about.runtime_status.ready", "就绪"),
        ("settings.about.runtime_status.error", "错误"),
        ("settings.about.card.workspace.title", "工作区"),
        (
            "settings.about.card.workspace.desc",
            "当前 Knowledge 工作区与内容统计。",
        ),
        ("settings.about.workspace.notes", "仓库笔记"),
        ("settings.about.workspace.open_note", "当前打开"),
        ("settings.about.workspace.bookmarks", "书签"),
        ("settings.about.card.runtime.title", "运行时"),
        (
            "settings.about.card.runtime.desc",
            "插件运行时模式与激活状态概览。",
        ),
        ("settings.about.runtime.plugins", "插件"),
        ("settings.about.runtime.mode", "运行时模式"),
        ("settings.about.runtime.status", "运行状态"),
        (
            "settings.editor.status.autosave_set",
            "自动保存延时已设置为",
        ),
        ("settings.editor.unit.ms", "ms"),
        ("settings.editor.card.autosave.title", "自动保存"),
        (
            "settings.editor.card.autosave.desc",
            "控制编辑 Markdown 时的延迟写回。",
        ),
        ("settings.editor.card.behavior.title", "编辑器行为"),
        (
            "settings.editor.card.behavior.desc",
            "已支持选中、IME、剪贴与保存等核心编辑功能。",
        ),
        (
            "settings.editor.card.behavior.hint",
            "可使用 Ctrl+S 强制保存，自动保存会在停止输入后触发。",
        ),
        ("settings.ai.status.provider_set", "AI Provider 已设置为"),
        ("settings.ai.status.endpoint_set", "AI 端点已设置为"),
        ("settings.ai.status.model_set", "AI 模型已设置为"),
        (
            "settings.ai.status.tool_injection_enabled",
            "已启用 VCP Tool Injection",
        ),
        (
            "settings.ai.status.tool_injection_disabled",
            "已禁用 VCP Tool Injection",
        ),
        ("settings.ai.toggle.tool_injection_on", "Tool Injection：On"),
        (
            "settings.ai.toggle.tool_injection_off",
            "Tool Injection：Off",
        ),
        ("settings.ai.status.endpoint_ready", "AI 端点已就绪："),
        (
            "settings.ai.status.endpoint_checking",
            "正在检查 AI 端点...",
        ),
        ("settings.ai.status.endpoint_ok", "AI 端点可达："),
        ("settings.ai.status.endpoint_failed", "AI 端点不可达："),
        ("settings.ai.button.apply_check", "应用并检查"),
        ("settings.ai.card.provider.title", "AI Provider"),
        (
            "settings.ai.card.provider.desc",
            "在本地 mock 与 VCP 后端兼容模式间切换。",
        ),
        ("settings.ai.card.endpoint.title", "VCP Endpoint"),
        (
            "settings.ai.card.endpoint.desc",
            "选择 AI 请求的目标 VCPToolBox 服务端点。",
        ),
        ("settings.ai.card.model.title", "VCP Model"),
        (
            "settings.ai.card.model.desc",
            "选择 VCP 兼容请求使用的默认模型名。",
        ),
        ("settings.ai.card.tool_loop.title", "Tool Loop"),
        (
            "settings.ai.card.tool_loop.desc",
            "控制是否使用 VCP tool-injection endpoint。",
        ),
        ("settings.ai.card.connection.title", "Connection"),
        (
            "settings.ai.card.connection.desc",
            "将当前 AI 设置应用到运行时环境变量。",
        ),
        (
            "settings.files.status.external_sync_enabled",
            "已启用外部同步",
        ),
        (
            "settings.files.status.external_sync_disabled",
            "已禁用外部同步",
        ),
        (
            "settings.files.status.prefer_wikilink_enabled",
            "已启用优先使用 Wiki Link 标题",
        ),
        (
            "settings.files.status.prefer_wikilink_disabled",
            "已禁用优先使用 Wiki Link 标题",
        ),
        ("settings.files.card.external_sync.title", "外部修改同步"),
        (
            "settings.files.card.external_sync.desc",
            "是否通过 watcher 同步外部编辑器的更新。",
        ),
        ("settings.files.card.wiki_pref.title", "Wiki 链接偏好"),
        (
            "settings.files.card.wiki_pref.desc",
            "优先使用 wiki 标题解析，而非严格路径匹配。",
        ),
        ("settings.hotkeys.button.cancel", "取消"),
        ("settings.hotkeys.button.edit", "编辑"),
        ("settings.hotkeys.button.reset", "重置"),
        ("settings.hotkeys.button.reset_all", "全部恢复默认"),
        ("settings.hotkeys.placeholder.press_keys", "按下按键..."),
        ("settings.hotkeys.status.reset_one", "已重置快捷键"),
        ("settings.hotkeys.status.reset_one_failed", "重置快捷键失败"),
        (
            "settings.hotkeys.status.shortcut_already_default",
            "快捷键已是默认",
        ),
        (
            "settings.hotkeys.status.reset_all",
            "已将所有快捷键恢复为默认",
        ),
        (
            "settings.hotkeys.status.reset_all_failed",
            "全部快捷键重置失败",
        ),
        (
            "settings.hotkeys.status.all_shortcuts_already_default",
            "所有快捷键均为默认",
        ),
        ("settings.hotkeys.card.shortcuts.title", "命令快捷键"),
        (
            "settings.hotkeys.card.shortcuts.desc",
            "点击 Edit 后，在窗口中按新的按键组合以覆盖。",
        ),
        (
            "settings.hotkeys.tip",
            "提示：Esc 可退出编辑模式，覆盖项会持久化到设置。",
        ),
        ("settings.advanced.runtime_mode.process", "process"),
        ("settings.advanced.runtime_mode.in_process", "in_process"),
        (
            "settings.advanced.status.runtime_mode_set",
            "插件运行时模式已设置为",
        ),
        ("settings.advanced.watcher.enabled", "Watcher 已启用"),
        ("settings.advanced.watcher.disabled", "Watcher 已禁用"),
        (
            "settings.advanced.card.runtime_mode.title",
            "插件运行时模式",
        ),
        (
            "settings.advanced.card.runtime_mode.desc",
            "在 in-process 与 process-isolated 运行时之间切换。",
        ),
        ("settings.advanced.card.file_watcher.title", "文件 watcher"),
        (
            "settings.advanced.card.file_watcher.desc",
            "启用或禁用外部文件同步 watcher。",
        ),
        ("ai.hub.system.ready", "AI Hub 已就绪。"),
        ("ai.hub.timestamp.now", "刚刚"),
        ("ai.hub.session.title", "VCP 会话"),
        ("ai.hub.status.chat_in_progress", "AI 对话处理中..."),
        ("ai.hub.status.chat_empty_response", "AI 返回了空响应。"),
        (
            "ai.hub.status.chat_completed_empty",
            "AI 对话完成（空响应）。",
        ),
        ("ai.hub.status.chat_done", "AI 对话完成"),
        ("ai.hub.status.chat_failed", "AI 对话失败"),
        ("ai.hub.status.rewrite_in_progress", "AI 改写处理中..."),
        ("ai.hub.status.rewrite_applied", "AI 草稿已应用"),
        ("ai.hub.status.rewrite_failed", "AI 改写失败"),
        ("ai.hub.error.vault_not_opened", "AI 对话失败：未打开仓库。"),
        (
            "ai.hub.error.read_vault_state",
            "AI 对话失败：无法读取仓库状态",
        ),
        ("ai.hub.error.input_empty", "AI 对话输入为空。"),
        ("ai.hub.error.no_open_note", "AI 不可用：当前没有打开笔记。"),
        ("ai.hub.error.empty_selection", "AI 改写不可用：选区为空。"),
        (
            "ai.hub.error.invalid_selection_range",
            "AI 改写失败：选区范围无效。",
        ),
        (
            "ai.hub.error.rewrite_vault_not_opened",
            "AI 改写失败：未打开仓库。",
        ),
        (
            "ai.hub.error.rewrite_read_vault_state",
            "AI 改写失败：无法读取仓库状态",
        ),
        (
            "ai.hub.chat.instruction.base",
            "你是 XNote 的 AI 助手。请保持回答简洁、可执行，并围绕笔记上下文。",
        ),
        ("ai.hub.chat.instruction.context_header", "最近对话上下文："),
        ("ai.hub.chat.instruction.user_prompt", "用户问题："),
        ("ai.hub.trace.calls", "调用数"),
        ("ai.hub.trace.rounds", "轮次"),
        ("ai.hub.trace.stop", "停止原因"),
        ("ai.hub.trace.calls_suffix", "次调用"),
        ("ai.hub.trace.args.none", "无参数"),
        ("ai.hub.trace.stop.final_response", "最终响应"),
        ("ai.hub.trace.stop.max_rounds", "达到最大轮次"),
        ("ai.hub.trace.stop.unknown", "未知"),
        ("ai.hub.ui.role.user", "你"),
        ("ai.hub.ui.role.assistant", "助手"),
        ("ai.hub.ui.role.system", "系统"),
        (
            "ai.hub.ui.input.placeholder",
            "输入消息...（Shift+Enter 换行）",
        ),
        ("status.ready", "就绪"),
        ("hint.open_vault", "打开仓库（Ctrl+O）"),
        ("hint.vault_error", "仓库错误（Ctrl+O）"),
        ("prompt.enter_vault_path", "请输入仓库文件夹路径。"),
        ("prompt.vault_path_not_folder", "仓库路径不是文件夹。"),
        ("palette.empty_commands", "无可用命令"),
        ("palette.title.commands", "命令面板"),
        ("palette.title.quick_open", "快速打开"),
        ("palette.placeholder.commands", "输入命令..."),
        ("palette.placeholder.quick_open", "输入以搜索..."),
        ("palette.group.navigation", "导航"),
        ("palette.group.files", "文件"),
        ("palette.title.search", "搜索"),
        ("palette.placeholder.search", "输入关键词搜索内容..."),
        ("palette.group.search", "搜索结果"),
        (
            "settings.ai.connection.not_checked",
            "No connection check yet. Click 'Apply & Check'.",
        ),
        (
            "ai.hub.ui.input.disabled_placeholder",
            "VCP disconnected: run Apply & Check in Settings > AI",
        ),
        (
            "ai.hub.error.vcp_not_checked",
            "VCP connection is not checked yet. Go to Settings > AI and click Apply & Check.",
        ),
        (
            "ai.hub.error.vcp_not_connected",
            "VCP is not connected. Go to Settings > AI and fix endpoint/key, then Apply & Check.",
        ),
        ("ai.hub.popup.connection.title", "需要 VCP 连接"),
        (
            "ai.hub.popup.connection.action_settings",
            "打开 设置 > AI",
        ),
        ("ai.hub.popup.connection.action_close", "关闭"),
    ])
}
