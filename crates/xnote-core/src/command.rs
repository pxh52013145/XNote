#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandId {
    OpenVault,
    QuickOpen,
    CommandPalette,
    Settings,
    ReloadVault,
    NewNote,
    SaveFile,
    Undo,
    Redo,
    ToggleSplit,
    FocusExplorer,
    FocusSearch,
    AiRewriteSelection,
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
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::ToggleSplit => "toggle_split",
            Self::FocusExplorer => "focus_explorer",
            Self::FocusSearch => "focus_search",
            Self::AiRewriteSelection => "ai_rewrite_selection",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        input.parse().ok()
    }
}

impl std::str::FromStr for CommandId {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim() {
            "open_vault" => Ok(Self::OpenVault),
            "quick_open" => Ok(Self::QuickOpen),
            "command_palette" => Ok(Self::CommandPalette),
            "settings" => Ok(Self::Settings),
            "reload_vault" => Ok(Self::ReloadVault),
            "new_note" => Ok(Self::NewNote),
            "save_file" => Ok(Self::SaveFile),
            "undo" => Ok(Self::Undo),
            "redo" => Ok(Self::Redo),
            "toggle_split" => Ok(Self::ToggleSplit),
            "focus_explorer" => Ok(Self::FocusExplorer),
            "focus_search" => Ok(Self::FocusSearch),
            "ai_rewrite_selection" => Ok(Self::AiRewriteSelection),
            _ => Err(()),
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
        id: CommandId::Undo,
        label_key: "cmd.undo.label",
        detail_key: "cmd.undo.detail",
        default_shortcut: "Ctrl+Z",
    },
    CommandSpec {
        id: CommandId::Redo,
        label_key: "cmd.redo.label",
        detail_key: "cmd.redo.detail",
        default_shortcut: "Ctrl+Y",
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
    CommandSpec {
        id: CommandId::AiRewriteSelection,
        label_key: "cmd.ai_rewrite_selection.label",
        detail_key: "cmd.ai_rewrite_selection.detail",
        default_shortcut: "",
    },
];

pub fn command_specs() -> &'static [CommandSpec] {
    COMMAND_SPECS
}
