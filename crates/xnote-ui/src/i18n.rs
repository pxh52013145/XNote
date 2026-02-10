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
        ("settings.language.chinese", "简体中文"),
        ("settings.colors.accent", "Accent color"),
        (
            "settings.colors.accent.hint",
            "Used for buttons, links and highlights.",
        ),
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
        ("palette.placeholder.commands", "Type a command…"),
        ("palette.placeholder.quick_open", "Type to search…"),
        ("palette.group.navigation", "NAVIGATION"),
        ("palette.group.files", "FILES"),
        ("palette.title.search", "SEARCH"),
        (
            "palette.placeholder.search",
            "Type keywords to search content…",
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
            "选择文件夹并在新应用窗口中打开",
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
        ("cmd.undo.detail", "撤销上一次编辑操作"),
        ("cmd.redo.label", "重做"),
        ("cmd.redo.detail", "重做被撤销的编辑操作"),
        ("cmd.toggle_split.label", "切换分栏"),
        ("cmd.toggle_split.detail", "切换编辑区分栏"),
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
        ("settings.colors.accent.hint", "用于按钮、链接和高亮。"),
        ("status.ready", "就绪"),
        ("hint.open_vault", "打开仓库（Ctrl+O）"),
        ("hint.vault_error", "仓库错误（Ctrl+O）"),
        ("prompt.enter_vault_path", "请输入仓库文件夹路径。"),
        ("prompt.vault_path_not_folder", "仓库路径不是文件夹。"),
        ("palette.empty_commands", "无可用命令"),
        ("palette.title.commands", "命令面板"),
        ("palette.title.quick_open", "快速打开"),
        ("palette.placeholder.commands", "输入命令…"),
        ("palette.placeholder.quick_open", "输入以搜索…"),
        ("palette.group.navigation", "导航"),
        ("palette.group.files", "文件"),
        ("palette.title.search", "搜索"),
        ("palette.placeholder.search", "输入关键词搜索内容…"),
        ("palette.group.search", "搜索结果"),
    ])
}
