#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandId {
    OpenVault,
    QuickOpen,
    CommandPalette,
    Settings,
    ReloadVault,
    NewNote,
    SaveFile,
    ToggleSplit,
    FocusExplorer,
    FocusSearch,
}

impl CommandId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenVault => "open_vault",
            Self::QuickOpen => "quick_open",
            Self::CommandPalette => "command_palette",
            Self::Settings => "settings",
            Self::ReloadVault => "reload_vault",
            Self::NewNote => "new_note",
            Self::SaveFile => "save_file",
            Self::ToggleSplit => "toggle_split",
            Self::FocusExplorer => "focus_explorer",
            Self::FocusSearch => "focus_search",
        }
    }

    pub fn from_str(input: &str) -> Option<Self> {
        match input.trim() {
            "open_vault" => Some(Self::OpenVault),
            "quick_open" => Some(Self::QuickOpen),
            "command_palette" => Some(Self::CommandPalette),
            "settings" => Some(Self::Settings),
            "reload_vault" => Some(Self::ReloadVault),
            "new_note" => Some(Self::NewNote),
            "save_file" => Some(Self::SaveFile),
            "toggle_split" => Some(Self::ToggleSplit),
            "focus_explorer" => Some(Self::FocusExplorer),
            "focus_search" => Some(Self::FocusSearch),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CommandSpec {
    pub id: CommandId,
    pub label_key: &'static str,
    pub detail_key: &'static str,
    pub default_shortcut: &'static str,
}

const COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        id: CommandId::OpenVault,
        label_key: "cmd.open_vault.label",
        detail_key: "cmd.open_vault.detail",
        default_shortcut: "Ctrl+O",
    },
    CommandSpec {
        id: CommandId::QuickOpen,
        label_key: "cmd.quick_open.label",
        detail_key: "cmd.quick_open.detail",
        default_shortcut: "Ctrl+P",
    },
    CommandSpec {
        id: CommandId::CommandPalette,
        label_key: "cmd.command_palette.label",
        detail_key: "cmd.command_palette.detail",
        default_shortcut: "Ctrl+K",
    },
    CommandSpec {
        id: CommandId::Settings,
        label_key: "cmd.settings.label",
        detail_key: "cmd.settings.detail",
        default_shortcut: "Ctrl+,",
    },
    CommandSpec {
        id: CommandId::ReloadVault,
        label_key: "cmd.reload_vault.label",
        detail_key: "cmd.reload_vault.detail",
        default_shortcut: "Ctrl+R",
    },
    CommandSpec {
        id: CommandId::NewNote,
        label_key: "cmd.new_note.label",
        detail_key: "cmd.new_note.detail",
        default_shortcut: "Ctrl+N",
    },
    CommandSpec {
        id: CommandId::SaveFile,
        label_key: "cmd.save_file.label",
        detail_key: "cmd.save_file.detail",
        default_shortcut: "Ctrl+S",
    },
    CommandSpec {
        id: CommandId::ToggleSplit,
        label_key: "cmd.toggle_split.label",
        detail_key: "cmd.toggle_split.detail",
        default_shortcut: "Ctrl+\\",
    },
    CommandSpec {
        id: CommandId::FocusExplorer,
        label_key: "cmd.focus_explorer.label",
        detail_key: "cmd.focus_explorer.detail",
        default_shortcut: "Alt+1",
    },
    CommandSpec {
        id: CommandId::FocusSearch,
        label_key: "cmd.focus_search.label",
        detail_key: "cmd.focus_search.detail",
        default_shortcut: "Alt+2",
    },
];

pub fn command_specs() -> &'static [CommandSpec] {
    COMMAND_SPECS
}
