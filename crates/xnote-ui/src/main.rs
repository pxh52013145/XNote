mod i18n;

use gpui::{
    div, ease_in_out, fill, point, prelude::*, px, radians, relative, rgb, rgba, size, svg,
    uniform_list, Animation, AnimationExt as _, App, Application, AssetSource, AvailableSpace,
    Bounds, ClickEvent, ClipboardItem, Context, CursorStyle, DragMoveEvent, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, FontWeight, GlobalElementId,
    KeyDownEvent, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, SharedString, Size, Style, Task, TextRun, Timer, Transformation, UTF16Selection,
    UnderlineStyle, Window, WindowBounds, WindowControlArea, WindowOptions,
};
use i18n::{I18n, Locale};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use xnote_core::command::{command_specs, CommandId};
use xnote_core::keybind::KeyContext;
use xnote_core::keybind::Keymap;
use xnote_core::paths::{join_inside, normalize_folder_rel_path, normalize_vault_rel_path};
use xnote_core::plugin::{
    PluginActivationEvent, PluginCapability, PluginLifecycleState, PluginManifest, PluginRegistry,
    PluginRuntimeMode,
};
use xnote_core::settings::{
    load_effective_settings, project_settings_path, save_project_settings, save_settings,
    settings_path, AppSettings,
};
use xnote_core::vault::{NoteEntry, Vault};

const ICON_BOOKMARK: &str = "icons/bookmark.svg";
const ICON_BRUSH: &str = "icons/brush.svg";
const ICON_CHEVRON_DOWN: &str = "icons/chevron-down.svg";
const ICON_CHEVRON_RIGHT: &str = "icons/chevron-right.svg";
const ICON_COLUMNS_2: &str = "icons/columns-2.svg";
const ICON_COMMAND: &str = "icons/command.svg";
const ICON_CORNER_TAG: &str = "icons/corner-tag.svg";
const ICON_DOWNLOAD: &str = "icons/download.svg";
const ICON_FILE_COG: &str = "icons/file-cog.svg";
const ICON_FILE_PLUS: &str = "icons/file-plus.svg";
const ICON_FILE_TEXT: &str = "icons/file-text.svg";
const ICON_FOLDER: &str = "icons/folder.svg";
const ICON_FOLDER_OPEN: &str = "icons/folder-open.svg";
const ICON_FOLDER_PLUS: &str = "icons/folder-plus.svg";
const ICON_FUNNEL: &str = "icons/funnel.svg";
const ICON_GRID_2X2: &str = "icons/grid-2x2.svg";
const ICON_KEYBOARD: &str = "icons/keyboard.svg";
const ICON_LINK_2: &str = "icons/link-2.svg";
const ICON_MINUS: &str = "icons/minus.svg";
const ICON_PANEL_LEFT_CLOSE: &str = "icons/panel-left-close.svg";
const ICON_PANEL_LEFT_OPEN: &str = "icons/panel-left-open.svg";
const ICON_PANEL_RIGHT_CLOSE: &str = "icons/panel-right-close.svg";
const ICON_PANEL_RIGHT_OPEN: &str = "icons/panel-right-open.svg";
const ICON_PLUS: &str = "icons/plus.svg";
const ICON_REFRESH_CW: &str = "icons/refresh-cw.svg";
const ICON_SEARCH: &str = "icons/search.svg";
const ICON_SETTINGS: &str = "icons/settings.svg";
const ICON_SLIDERS_HORIZONTAL: &str = "icons/sliders-horizontal.svg";
const ICON_SQUARE: &str = "icons/square.svg";
const ICON_USER: &str = "icons/user.svg";
const ICON_VAULT: &str = "icons/vault.svg";
const ICON_X: &str = "icons/x.svg";

struct UiAssets {
    base: PathBuf,
}

impl AssetSource for UiAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        fs::read(self.base.join(path))
            .map(|data| Some(Cow::Owned(data)))
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .map_err(|e| e.into())
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(|e| e.into())
    }
}

#[derive(Clone, Debug)]
enum ExplorerRow {
    Vault {
        root_name: String,
        expanded: bool,
    },
    Hint {
        text: SharedString,
    },
    Folder {
        folder: String,
        name: String,
        depth: usize,
        expanded: bool,
        has_children: bool,
    },
    Note {
        folder: String,
        path: String,
        file_name: String,
        depth: usize,
    },
}

#[derive(Clone, Debug)]
struct DraggedNote {
    folder: String,
    path: String,
}

#[derive(Clone, Debug)]
struct DragOver {
    folder: String,
    target_path: String,
    insert_after: bool,
}

#[derive(Clone, Debug)]
enum VaultState {
    NotConfigured,
    Opening {
        path: PathBuf,
    },
    Opened {
        vault: Vault,
        root_name: SharedString,
    },
    Error {
        message: SharedString,
    },
}

#[derive(Clone, Debug)]
enum ScanState {
    Idle,
    Scanning,
    Ready {
        note_count: usize,
        duration_ms: u128,
    },
    Error {
        message: SharedString,
    },
}

#[derive(Clone, Debug)]
enum PluginActivationState {
    Idle,
    Activating,
    Ready { active_count: usize },
    Error { message: SharedString },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanelMode {
    Explorer,
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspaceMode {
    OpenEditors,
    References,
    Bookmarks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitterKind {
    PanelShell,
    Workspace,
}

#[derive(Clone, Copy, Debug)]
struct SplitterDrag {
    kind: SplitterKind,
    start_x: Pixels,
    start_width: Pixels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SidebarState {
    Expanded,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaletteMode {
    Commands,
    QuickOpen,
}

struct PaletteCommandSpec {
    id: CommandId,
    icon: &'static str,
}

#[derive(Clone, Debug)]
enum SearchRow {
    File {
        path: String,
        match_count: usize,
    },
    Match {
        path: String,
        line: usize,
        preview: String,
    },
}

const PALETTE_COMMANDS: &[PaletteCommandSpec] = &[
    PaletteCommandSpec {
        id: CommandId::OpenVault,
        icon: ICON_FOLDER_OPEN,
    },
    PaletteCommandSpec {
        id: CommandId::QuickOpen,
        icon: ICON_SEARCH,
    },
    PaletteCommandSpec {
        id: CommandId::CommandPalette,
        icon: ICON_COMMAND,
    },
    PaletteCommandSpec {
        id: CommandId::Settings,
        icon: ICON_SETTINGS,
    },
    PaletteCommandSpec {
        id: CommandId::ReloadVault,
        icon: ICON_REFRESH_CW,
    },
    PaletteCommandSpec {
        id: CommandId::NewNote,
        icon: ICON_FILE_PLUS,
    },
    PaletteCommandSpec {
        id: CommandId::SaveFile,
        icon: ICON_DOWNLOAD,
    },
    PaletteCommandSpec {
        id: CommandId::ToggleSplit,
        icon: ICON_COLUMNS_2,
    },
    PaletteCommandSpec {
        id: CommandId::FocusExplorer,
        icon: ICON_FOLDER,
    },
    PaletteCommandSpec {
        id: CommandId::FocusSearch,
        icon: ICON_SEARCH,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsSection {
    About,
    Appearance,
    Editor,
    FilesLinks,
    Hotkeys,
    Advanced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsTheme {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsAccent {
    Default,
    Blue,
}

struct XnoteWindow {
    vault_state: VaultState,
    scan_state: ScanState,
    explorer_rows: Vec<ExplorerRow>,
    explorer_filter: String,
    explorer_rows_filtered: Vec<usize>,
    next_filter_nonce: u64,
    pending_filter_nonce: u64,
    explorer_folder_children: HashMap<String, Vec<String>>,
    explorer_expanded_folders: HashSet<String>,
    explorer_all_note_paths: Arc<Vec<String>>,
    explorer_all_note_paths_lower: Arc<Vec<String>>,
    folder_notes: HashMap<String, Vec<String>>,
    selected_note: Option<String>,
    drag_over: Option<DragOver>,
    next_order_nonce: u64,
    pending_order_nonce_by_folder: HashMap<String, u64>,
    open_editors: Vec<String>,
    open_note_path: Option<String>,
    open_note_loading: bool,
    open_note_dirty: bool,
    open_note_content: String,
    editor_focus_handle: FocusHandle,
    vault_prompt_focus_handle: FocusHandle,
    editor_selected_range: Range<usize>,
    editor_selection_reversed: bool,
    editor_marked_range: Option<Range<usize>>,
    editor_is_selecting: bool,
    editor_preferred_x: Option<Pixels>,
    editor_layout: Option<NoteEditorLayout>,
    next_note_open_nonce: u64,
    current_note_open_nonce: u64,
    next_note_save_nonce: u64,
    pending_note_save_nonce: u64,
    status: SharedString,
    app_settings: AppSettings,
    settings_path: PathBuf,
    project_settings_path: Option<PathBuf>,
    i18n: I18n,
    keymap: Keymap,
    plugin_runtime_mode: PluginRuntimeMode,
    plugin_registry: PluginRegistry,
    plugin_activation_state: PluginActivationState,
    panel_mode: PanelMode,
    workspace_mode: WorkspaceMode,
    panel_shell_collapsed: bool,
    panel_shell_tab_toggle_exiting: bool,
    panel_shell_tab_toggle_anim_nonce: u64,
    panel_shell_width: Pixels,
    panel_shell_saved_width: Pixels,
    workspace_collapsed: bool,
    workspace_tab_toggle_exiting: bool,
    workspace_tab_toggle_anim_nonce: u64,
    workspace_width: Pixels,
    workspace_saved_width: Pixels,
    splitter_drag: Option<SplitterDrag>,
    palette_open: bool,
    palette_mode: PaletteMode,
    palette_query: String,
    palette_selected: usize,
    palette_results: Vec<usize>,
    next_palette_nonce: u64,
    pending_palette_nonce: u64,
    vault_prompt_open: bool,
    vault_prompt_needs_focus: bool,
    vault_prompt_value: String,
    vault_prompt_error: Option<SharedString>,
    search_query: String,
    search_selected: usize,
    search_results: Vec<SearchRow>,
    next_search_nonce: u64,
    pending_search_nonce: u64,
    settings_section: SettingsSection,
    settings_open: bool,
    settings_theme: SettingsTheme,
    settings_accent: SettingsAccent,
    settings_language: Locale,
    settings_language_menu_open: bool,
    split_editor: bool,
    open_note_word_count: usize,
    pending_open_note_cursor: Option<(String, usize)>,
}

struct BootContext {
    app_settings: AppSettings,
    settings_path: PathBuf,
    project_settings_path: Option<PathBuf>,
    locale: Locale,
    keymap: Keymap,
    plugin_runtime_mode: PluginRuntimeMode,
}

impl XnoteWindow {
    fn new(cx: &mut Context<Self>) -> Self {
        let boot = load_boot_context();
        let mut plugin_registry = PluginRegistry::with_policy(boot.app_settings.to_plugin_policy());
        let _ = plugin_registry.register_manifest(PluginManifest {
            id: "xnote.builtin.core".to_string(),
            display_name: "XNote Builtin Core".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec![
                PluginCapability::Commands,
                PluginCapability::ReadVault,
                PluginCapability::WriteVault,
            ],
            command_allowlist: command_specs().iter().map(|spec| spec.id).collect(),
            activation_events: vec![
                PluginActivationEvent::OnStartupFinished,
                PluginActivationEvent::OnVaultOpened,
                PluginActivationEvent::OnCommand(CommandId::CommandPalette),
                PluginActivationEvent::OnCommand(CommandId::QuickOpen),
                PluginActivationEvent::OnCommand(CommandId::OpenVault),
                PluginActivationEvent::OnCommand(CommandId::ReloadVault),
                PluginActivationEvent::OnCommand(CommandId::NewNote),
                PluginActivationEvent::OnCommand(CommandId::SaveFile),
                PluginActivationEvent::OnCommand(CommandId::ToggleSplit),
                PluginActivationEvent::OnCommand(CommandId::FocusExplorer),
                PluginActivationEvent::OnCommand(CommandId::FocusSearch),
                PluginActivationEvent::OnCommand(CommandId::Settings),
            ],
        });

        let mut this = Self {
            vault_state: VaultState::NotConfigured,
            scan_state: ScanState::Idle,
            explorer_rows: Vec::new(),
            explorer_filter: String::new(),
            explorer_rows_filtered: Vec::new(),
            next_filter_nonce: 0,
            pending_filter_nonce: 0,
            explorer_folder_children: HashMap::new(),
            explorer_expanded_folders: HashSet::new(),
            explorer_all_note_paths: Arc::new(Vec::new()),
            explorer_all_note_paths_lower: Arc::new(Vec::new()),
            folder_notes: HashMap::new(),
            selected_note: None,
            drag_over: None,
            next_order_nonce: 0,
            pending_order_nonce_by_folder: HashMap::new(),
            open_editors: Vec::new(),
            open_note_path: None,
            open_note_loading: false,
            open_note_dirty: false,
            open_note_content: String::new(),
            editor_focus_handle: cx.focus_handle(),
            vault_prompt_focus_handle: cx.focus_handle(),
            editor_selected_range: 0..0,
            editor_selection_reversed: false,
            editor_marked_range: None,
            editor_is_selecting: false,
            editor_preferred_x: None,
            editor_layout: None,
            next_note_open_nonce: 0,
            current_note_open_nonce: 0,
            next_note_save_nonce: 0,
            pending_note_save_nonce: 0,
            status: SharedString::from("Ready"),
            app_settings: boot.app_settings,
            settings_path: boot.settings_path,
            project_settings_path: boot.project_settings_path,
            i18n: I18n::new(boot.locale),
            keymap: boot.keymap,
            plugin_runtime_mode: boot.plugin_runtime_mode,
            plugin_registry,
            plugin_activation_state: PluginActivationState::Idle,
            panel_mode: PanelMode::Explorer,
            workspace_mode: WorkspaceMode::OpenEditors,
            panel_shell_collapsed: false,
            panel_shell_tab_toggle_exiting: false,
            panel_shell_tab_toggle_anim_nonce: 0,
            panel_shell_width: px(213.),
            panel_shell_saved_width: px(213.),
            workspace_collapsed: false,
            workspace_tab_toggle_exiting: false,
            workspace_tab_toggle_anim_nonce: 0,
            workspace_width: px(260.),
            workspace_saved_width: px(260.),
            splitter_drag: None,
            palette_open: false,
            palette_mode: PaletteMode::Commands,
            palette_query: String::new(),
            palette_selected: 0,
            palette_results: Vec::new(),
            next_palette_nonce: 0,
            pending_palette_nonce: 0,
            vault_prompt_open: false,
            vault_prompt_needs_focus: false,
            vault_prompt_value: String::new(),
            vault_prompt_error: None,
            search_query: String::new(),
            search_selected: 0,
            search_results: Vec::new(),
            next_search_nonce: 0,
            pending_search_nonce: 0,
            settings_section: SettingsSection::Appearance,
            settings_open: false,
            settings_theme: SettingsTheme::Light,
            settings_accent: SettingsAccent::Default,
            settings_language: boot.locale,
            settings_language_menu_open: false,
            split_editor: false,
            open_note_word_count: 0,
            pending_open_note_cursor: None,
        };

        this.status = SharedString::from(this.i18n.text("status.ready"));

        this.activate_plugins(PluginActivationEvent::OnStartupFinished);

        if let Some(vault_path) = resolve_vault_path() {
            this.open_vault(vault_path, cx).detach();
        } else {
            this.open_vault_prompt(cx);
        }

        this
    }

    fn open_vault(&mut self, vault_path: PathBuf, cx: &mut Context<Self>) -> Task<()> {
        self.vault_state = VaultState::Opening {
            path: vault_path.clone(),
        };
        self.scan_state = ScanState::Scanning;
        self.explorer_rows.clear();
        self.explorer_filter.clear();
        self.explorer_rows_filtered.clear();
        self.pending_filter_nonce = 0;
        self.explorer_folder_children.clear();
        self.explorer_expanded_folders.clear();
        self.explorer_all_note_paths = Arc::new(Vec::new());
        self.explorer_all_note_paths_lower = Arc::new(Vec::new());
        self.folder_notes.clear();
        self.selected_note = None;
        self.drag_over = None;
        self.pending_order_nonce_by_folder.clear();
        self.open_editors.clear();
        self.open_note_path = None;
        self.open_note_loading = false;
        self.open_note_dirty = false;
        self.open_note_content.clear();
        self.editor_selected_range = 0..0;
        self.editor_selection_reversed = false;
        self.editor_marked_range = None;
        self.editor_is_selecting = false;
        self.editor_preferred_x = None;
        self.editor_layout = None;
        self.pending_note_save_nonce = 0;
        self.panel_mode = PanelMode::Explorer;
        self.workspace_mode = WorkspaceMode::OpenEditors;
        self.palette_open = false;
        self.palette_mode = PaletteMode::Commands;
        self.palette_query.clear();
        self.palette_selected = 0;
        self.palette_results.clear();
        self.pending_palette_nonce = 0;
        self.search_query.clear();
        self.search_selected = 0;
        self.search_results.clear();
        self.pending_search_nonce = 0;
        self.settings_section = SettingsSection::Appearance;
        self.settings_open = false;
        self.settings_language_menu_open = false;
        self.split_editor = false;
        self.open_note_word_count = 0;
        self.status = SharedString::from("Scanning...");

        cx.spawn(|this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let vault = Vault::open(&vault_path)?;
                        let root_name = vault
                            .root()
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Vault")
                            .to_string();

                        let started_at = Instant::now();
                        let entries = vault.fast_scan_notes()?;
                        let index = build_explorer_index(&vault, &entries)?;
                        let duration_ms = started_at.elapsed().as_millis();

                        Ok::<_, anyhow::Error>((
                            vault,
                            SharedString::from(root_name),
                            index,
                            entries.len(),
                            duration_ms,
                        ))
                    })
                    .await;

                this.update(&mut cx, |this, cx| match result {
                    Ok((vault, root_name, index, note_count, duration_ms)) => {
                        this.vault_state = VaultState::Opened { vault, root_name };
                        this.scan_state = ScanState::Ready {
                            note_count,
                            duration_ms,
                        };
                        this.explorer_rows_filtered.clear();
                        this.explorer_folder_children = index.folder_children;
                        this.folder_notes = index.folder_notes;
                        this.explorer_all_note_paths = Arc::new(index.all_note_paths);
                        this.explorer_all_note_paths_lower = Arc::new(index.all_note_paths_lower);
                        this.explorer_expanded_folders.clear();
                        this.explorer_expanded_folders.insert(String::new());
                        this.rebuild_explorer_rows();
                        this.status = SharedString::from("Ready");
                        this.activate_plugins(PluginActivationEvent::OnVaultOpened);
                        cx.notify();
                    }
                    Err(err) => {
                        this.vault_state = VaultState::Error {
                            message: SharedString::from(err.to_string()),
                        };
                        this.scan_state = ScanState::Error {
                            message: SharedString::from("Scan failed"),
                        };
                        this.explorer_rows.clear();
                        this.explorer_rows_filtered.clear();
                        this.explorer_folder_children.clear();
                        this.explorer_expanded_folders.clear();
                        this.explorer_all_note_paths = Arc::new(Vec::new());
                        this.explorer_all_note_paths_lower = Arc::new(Vec::new());
                        this.folder_notes.clear();
                        this.status = SharedString::from("Scan failed");
                        cx.notify();
                    }
                })
                .ok();
            }
        })
    }

    fn vault(&self) -> Option<Vault> {
        match &self.vault_state {
            VaultState::Opened { vault, .. } => Some(vault.clone()),
            _ => None,
        }
    }

    fn derive_note_title(&self, note_path: &str) -> String {
        if self.open_note_loading || self.open_note_path.as_deref() != Some(note_path) {
            return file_name(note_path);
        }

        for line in self.open_note_content.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("# ") {
                let title = rest.trim();
                if !title.is_empty() {
                    return title.to_string();
                }
            }
        }

        file_name(note_path)
    }

    fn cursor_line_col(&self) -> (usize, usize) {
        if self.open_note_loading || self.open_note_path.is_none() {
            return (1, 1);
        }

        let cursor = self
            .editor_cursor_offset()
            .min(self.open_note_content.len());
        let prefix = &self.open_note_content[..cursor];
        let line = prefix.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1;
        let col = prefix
            .rsplit('\n')
            .next()
            .map(|s| s.chars().count() + 1)
            .unwrap_or(1);

        (line, col)
    }

    fn export_open_note(&mut self, cx: &mut Context<Self>) {
        if self.open_note_loading || self.open_note_path.is_none() {
            return;
        }

        cx.write_to_clipboard(ClipboardItem::new_string(self.open_note_content.clone()));
        self.status = SharedString::from("Copied to clipboard");
        cx.notify();
    }

    fn rescan_vault(&mut self, cx: &mut Context<Self>) {
        let Some(vault) = self.vault() else {
            return;
        };

        self.scan_state = ScanState::Scanning;
        self.status = SharedString::from("Scanning...");
        cx.notify();

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let result = cx
                        .background_executor()
                        .spawn({
                            let vault = vault.clone();
                            async move {
                                let started_at = Instant::now();
                                let entries = vault.fast_scan_notes()?;
                                let index = build_explorer_index(&vault, &entries)?;
                                let duration_ms = started_at.elapsed().as_millis();
                                Ok::<_, anyhow::Error>((index, entries.len(), duration_ms))
                            }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| match result {
                        Ok((index, note_count, duration_ms)) => {
                            this.scan_state = ScanState::Ready {
                                note_count,
                                duration_ms,
                            };
                            this.explorer_folder_children = index.folder_children;
                            this.folder_notes = index.folder_notes;
                            this.explorer_all_note_paths = Arc::new(index.all_note_paths);
                            this.explorer_all_note_paths_lower =
                                Arc::new(index.all_note_paths_lower);
                            this.rebuild_explorer_rows();

                            if this.is_filtering() {
                                this.schedule_apply_filter(Duration::ZERO, cx);
                            }
                            if !this.search_query.trim().is_empty() {
                                this.schedule_apply_search(Duration::ZERO, cx);
                            }
                            if this.palette_open
                                && this.palette_mode == PaletteMode::QuickOpen
                                && !this.palette_query.trim().is_empty()
                            {
                                this.schedule_apply_palette_results(Duration::ZERO, cx);
                            }

                            this.status = SharedString::from("Ready");
                            cx.notify();
                        }
                        Err(err) => {
                            this.scan_state = ScanState::Error {
                                message: SharedString::from("Scan failed"),
                            };
                            this.status = SharedString::from(format!("Scan failed: {err}"));
                            cx.notify();
                        }
                    })
                    .ok();
                }
            },
        )
        .detach();
    }

    fn base_folder_for_new_items(&self) -> String {
        let Some(selected) = self.selected_note.as_deref() else {
            return String::new();
        };
        match selected.rsplit_once('/') {
            Some((folder, _)) => folder.to_string(),
            None => String::new(),
        }
    }

    fn create_new_note(&mut self, cx: &mut Context<Self>) {
        let Some(vault) = self.vault() else {
            return;
        };

        let folder = self.base_folder_for_new_items();
        self.status = SharedString::from("Creating note...");
        cx.notify();

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let result = cx
                        .background_executor()
                        .spawn({
                            let vault = vault.clone();
                            let folder = folder.clone();
                            async move {
                                for i in 0..1000usize {
                                    let file = if i == 0 {
                                        "new-note.md".to_string()
                                    } else {
                                        format!("new-note-{}.md", i + 1)
                                    };
                                    let rel = if folder.is_empty() {
                                        file
                                    } else {
                                        format!("{folder}/{file}")
                                    };
                                    let rel = normalize_vault_rel_path(&rel)?;
                                    let full = join_inside(vault.root(), &rel)?;
                                    if full.exists() {
                                        continue;
                                    }

                                    vault.write_note(&rel, "# New Note\n\n")?;
                                    return Ok::<_, anyhow::Error>(rel);
                                }
                                anyhow::bail!("failed to generate a unique note name");
                            }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| match result {
                        Ok(path) => {
                            this.open_note(path, cx);
                            this.rescan_vault(cx);
                        }
                        Err(err) => {
                            this.status = SharedString::from(format!("Create note failed: {err}"));
                            cx.notify();
                        }
                    })
                    .ok();
                }
            },
        )
        .detach();
    }

    fn create_new_folder(&mut self, cx: &mut Context<Self>) {
        let Some(vault) = self.vault() else {
            return;
        };

        let base_folder = self.base_folder_for_new_items();
        self.status = SharedString::from("Creating folder...");
        cx.notify();

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let result = cx
                        .background_executor()
                        .spawn({
                            let vault = vault.clone();
                            let base_folder = base_folder.clone();
                            async move {
                                for i in 0..1000usize {
                                    let dir = if i == 0 {
                                        "new-folder".to_string()
                                    } else {
                                        format!("new-folder-{}", i + 1)
                                    };
                                    let rel = if base_folder.is_empty() {
                                        dir
                                    } else {
                                        format!("{base_folder}/{dir}")
                                    };

                                    let rel = normalize_folder_rel_path(&rel)?;
                                    let full = join_inside(vault.root(), &rel)?;
                                    if full.exists() {
                                        continue;
                                    }

                                    fs::create_dir_all(&full)?;
                                    return Ok::<_, anyhow::Error>(rel);
                                }
                                anyhow::bail!("failed to generate a unique folder name");
                            }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| match result {
                        Ok(folder) => {
                            this.explorer_expanded_folders.insert(folder.clone());
                            this.rescan_vault(cx);
                            this.status = SharedString::from("Folder created");
                            cx.notify();
                        }
                        Err(err) => {
                            this.status =
                                SharedString::from(format!("Create folder failed: {err}"));
                            cx.notify();
                        }
                    })
                    .ok();
                }
            },
        )
        .detach();
    }

    fn close_editor(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(ix) = self.open_editors.iter().position(|p| p == path) else {
            return;
        };

        let is_current = self.open_note_path.as_deref() == Some(path);
        if is_current && self.open_note_dirty {
            self.status = SharedString::from("Save before closing");
            cx.notify();
            return;
        }

        self.open_editors.remove(ix);
        if self.selected_note.as_deref() == Some(path) {
            self.selected_note = None;
        }

        if is_current {
            if let Some(next) = self
                .open_editors
                .get(ix.saturating_sub(1))
                .or_else(|| self.open_editors.get(ix))
                .cloned()
            {
                self.open_note(next, cx);
            } else {
                self.open_note_path = None;
                self.open_note_loading = false;
                self.open_note_dirty = false;
                self.open_note_content.clear();
                self.open_note_word_count = 0;
                self.editor_selected_range = 0..0;
                self.editor_selection_reversed = false;
                self.editor_marked_range = None;
                self.editor_is_selecting = false;
                self.editor_preferred_x = None;
                self.editor_layout = None;
                self.status = SharedString::from("Ready");
                cx.notify();
            }
        } else {
            cx.notify();
        }
    }

    fn open_palette(&mut self, mode: PaletteMode, cx: &mut Context<Self>) {
        self.palette_open = true;
        self.palette_mode = mode;
        self.palette_query.clear();
        self.palette_selected = 0;
        self.palette_results.clear();
        self.pending_palette_nonce = 0;
        cx.notify();
    }

    fn close_palette(&mut self, cx: &mut Context<Self>) {
        self.palette_open = false;
        self.palette_query.clear();
        self.palette_selected = 0;
        self.palette_results.clear();
        self.pending_palette_nonce = 0;
        cx.notify();
    }

    fn open_vault_prompt(&mut self, cx: &mut Context<Self>) {
        let default_value = match &self.vault_state {
            VaultState::Opened { vault, .. } => vault.root().to_string_lossy().to_string(),
            VaultState::Opening { path } => path.to_string_lossy().to_string(),
            _ => resolve_vault_path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        };

        self.vault_prompt_open = true;
        self.vault_prompt_needs_focus = true;
        self.vault_prompt_value = default_value;
        self.vault_prompt_error = None;
        self.palette_open = false;
        self.settings_open = false;
        self.settings_language_menu_open = false;
        cx.notify();
    }

    fn close_vault_prompt(&mut self, cx: &mut Context<Self>) {
        self.vault_prompt_open = false;
        self.vault_prompt_needs_focus = false;
        self.vault_prompt_error = None;
        cx.notify();
    }

    fn command_spec_by_id(
        &self,
        id: CommandId,
    ) -> Option<&'static xnote_core::command::CommandSpec> {
        command_specs().iter().find(|spec| spec.id == id)
    }

    fn command_label(&self, id: CommandId) -> String {
        self.command_spec_by_id(id)
            .map(|spec| self.i18n.text(spec.label_key))
            .unwrap_or_else(|| id.as_str().to_string())
    }

    fn command_detail(&self, id: CommandId) -> String {
        self.command_spec_by_id(id)
            .map(|spec| self.i18n.text(spec.detail_key))
            .unwrap_or_default()
    }

    fn command_shortcut(&self, id: CommandId) -> String {
        self.keymap
            .shortcut_for(id)
            .map(ToString::to_string)
            .or_else(|| {
                self.command_spec_by_id(id)
                    .map(|spec| spec.default_shortcut.to_string())
            })
            .unwrap_or_default()
    }

    fn command_from_event(&self, ev: &KeyDownEvent) -> Option<CommandId> {
        let mut context = KeyContext::new();
        context.set(
            "in_editor",
            self.open_note_path.is_some() && !self.open_note_loading,
        );
        context.set("palette_open", self.palette_open);
        context.set("settings_open", self.settings_open);
        context.set("search_panel", self.panel_mode == PanelMode::Search);
        context.set("explorer_panel", self.panel_mode == PanelMode::Explorer);

        self.keymap.resolve_event_in_context(
            &ev.keystroke.key,
            ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform,
            ev.keystroke.modifiers.alt,
            ev.keystroke.modifiers.shift,
            false,
            &context,
        )
    }

    fn persist_settings(&mut self) {
        self.app_settings.locale = self.settings_language.as_tag().to_string();
        if let Some(parent) = self.settings_path.parent() {
            if let Err(err) = save_settings(parent, &self.app_settings) {
                self.status = SharedString::from(format!("Save settings failed: {err}"));
            }
        }

        if let Some(project_path) = &self.project_settings_path {
            if let Some(project_root) = project_path.parent().and_then(|p| p.parent()) {
                let _ = save_project_settings(project_root, &self.app_settings);
            }
        }
    }

    fn activate_plugins(&mut self, event: PluginActivationEvent) {
        self.plugin_activation_state = PluginActivationState::Activating;
        let outcomes =
            self.plugin_registry
                .trigger_event_with_mode(event, self.plugin_runtime_mode, None);

        if let Some(failed) = outcomes.iter().find(|outcome| {
            matches!(
                outcome.state,
                PluginLifecycleState::Failed
                    | PluginLifecycleState::Disabled
                    | PluginLifecycleState::Cancelled
            )
        }) {
            let detail = failed
                .error
                .as_deref()
                .unwrap_or("unknown activation error");
            self.plugin_activation_state = PluginActivationState::Error {
                message: SharedString::from(format!(
                    "Plugin {} activation failed: {detail}",
                    failed.plugin_id
                )),
            };
            return;
        }

        self.plugin_activation_state = PluginActivationState::Ready {
            active_count: self.plugin_registry.active_count(),
        };
    }

    fn filtered_palette_command_indices(&self) -> Vec<usize> {
        let query = self.palette_query.trim().to_lowercase();
        if query.is_empty() {
            return (0..PALETTE_COMMANDS.len()).collect();
        }

        PALETTE_COMMANDS
            .iter()
            .enumerate()
            .filter_map(|(ix, cmd)| {
                let label = self.command_label(cmd.id).to_lowercase();
                let detail = self.command_detail(cmd.id).to_lowercase();
                if label.contains(&query) || detail.contains(&query) {
                    Some(ix)
                } else {
                    None
                }
            })
            .collect()
    }

    fn execute_palette_command(&mut self, id: CommandId, cx: &mut Context<Self>) {
        self.activate_plugins(PluginActivationEvent::OnCommand(id));
        match id {
            CommandId::OpenVault => {
                self.close_palette(cx);
                self.open_vault_prompt(cx);
            }
            CommandId::QuickOpen => self.open_palette(PaletteMode::QuickOpen, cx),
            CommandId::CommandPalette => self.open_palette(PaletteMode::Commands, cx),
            CommandId::Settings => {
                self.close_palette(cx);
                self.settings_open = true;
                self.settings_language_menu_open = false;
                cx.notify();
            }
            CommandId::ReloadVault => {
                self.close_palette(cx);
                self.rescan_vault(cx);
            }
            CommandId::NewNote => {
                self.close_palette(cx);
                self.create_new_note(cx);
            }
            CommandId::SaveFile => {
                self.close_palette(cx);
                self.force_save_note(cx);
            }
            CommandId::ToggleSplit => {
                self.close_palette(cx);
                self.split_editor = !self.split_editor;
                cx.notify();
            }
            CommandId::FocusExplorer => {
                self.close_palette(cx);
                self.panel_mode = PanelMode::Explorer;
                cx.notify();
            }
            CommandId::FocusSearch => {
                self.close_palette(cx);
                self.panel_mode = PanelMode::Search;
                cx.notify();
            }
        }
    }

    fn effective_sidebar_layout(&self, window_width: Pixels) -> (SidebarState, SidebarState) {
        let rail_w = px(48.);
        let splitter_w = px(6.);
        let editor_min_w = px(320.);
        let panel_min_w = px(180.);
        let workspace_min_w = px(220.);

        let mut panel_shell_state = if self.panel_shell_collapsed {
            SidebarState::Hidden
        } else {
            SidebarState::Expanded
        };
        let mut workspace_state = if self.workspace_collapsed {
            SidebarState::Hidden
        } else {
            SidebarState::Expanded
        };

        let required_width = |panel: SidebarState, workspace: SidebarState| -> Pixels {
            let mut required = rail_w + editor_min_w;
            let splitters = (panel != SidebarState::Hidden) as usize
                + (workspace != SidebarState::Hidden) as usize;
            required += splitter_w * splitters;
            required += match panel {
                SidebarState::Expanded => panel_min_w,
                SidebarState::Hidden => px(0.),
            };
            required += match workspace {
                SidebarState::Expanded => workspace_min_w,
                SidebarState::Hidden => px(0.),
            };
            required
        };

        let mut required = required_width(panel_shell_state, workspace_state);
        if window_width < required && workspace_state == SidebarState::Expanded {
            workspace_state = SidebarState::Hidden;
            required = required_width(panel_shell_state, workspace_state);
        }
        if window_width < required && panel_shell_state == SidebarState::Expanded {
            panel_shell_state = SidebarState::Hidden;
        }

        (panel_shell_state, workspace_state)
    }

    fn set_panel_shell_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        if collapsed {
            if !self.panel_shell_collapsed {
                self.panel_shell_saved_width = self.panel_shell_width;
                self.panel_shell_collapsed = true;
                self.panel_shell_tab_toggle_exiting = false;
                self.panel_shell_tab_toggle_anim_nonce =
                    self.panel_shell_tab_toggle_anim_nonce.wrapping_add(1);
                cx.notify();
            }
        } else if self.panel_shell_collapsed {
            self.panel_shell_collapsed = false;
            self.panel_shell_width = self.panel_shell_saved_width.max(px(180.));
            self.panel_shell_tab_toggle_exiting = true;
            self.panel_shell_tab_toggle_anim_nonce =
                self.panel_shell_tab_toggle_anim_nonce.wrapping_add(1);
            let nonce = self.panel_shell_tab_toggle_anim_nonce;
            cx.spawn(
                move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                    let mut cx = cx.clone();
                    async move {
                        Timer::after(Duration::from_millis(160)).await;
                        this.update(&mut cx, |this, cx| {
                            if this.panel_shell_tab_toggle_anim_nonce == nonce
                                && !this.panel_shell_collapsed
                                && this.panel_shell_tab_toggle_exiting
                            {
                                this.panel_shell_tab_toggle_exiting = false;
                                cx.notify();
                            }
                        })
                        .ok();
                    }
                },
            )
            .detach();
            cx.notify();
        }
    }

    fn show_panel_shell(&mut self, cx: &mut Context<Self>) {
        self.set_panel_shell_collapsed(false, cx);
    }

    fn set_workspace_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        if collapsed {
            if !self.workspace_collapsed {
                self.workspace_saved_width = self.workspace_width;
                self.workspace_collapsed = true;
                self.workspace_tab_toggle_exiting = false;
                self.workspace_tab_toggle_anim_nonce =
                    self.workspace_tab_toggle_anim_nonce.wrapping_add(1);
                cx.notify();
            }
        } else if self.workspace_collapsed {
            self.workspace_collapsed = false;
            self.workspace_width = self.workspace_saved_width.max(px(220.));
            self.workspace_tab_toggle_exiting = true;
            self.workspace_tab_toggle_anim_nonce =
                self.workspace_tab_toggle_anim_nonce.wrapping_add(1);
            let nonce = self.workspace_tab_toggle_anim_nonce;
            cx.spawn(
                move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                    let mut cx = cx.clone();
                    async move {
                        Timer::after(Duration::from_millis(160)).await;
                        this.update(&mut cx, |this, cx| {
                            if this.workspace_tab_toggle_anim_nonce == nonce
                                && !this.workspace_collapsed
                                && this.workspace_tab_toggle_exiting
                            {
                                this.workspace_tab_toggle_exiting = false;
                                cx.notify();
                            }
                        })
                        .ok();
                    }
                },
            )
            .detach();
            cx.notify();
        }
    }

    fn begin_splitter_drag(
        &mut self,
        kind: SplitterKind,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        let start_width = match kind {
            SplitterKind::PanelShell => self.panel_shell_width,
            SplitterKind::Workspace => self.workspace_width,
        };
        self.splitter_drag = Some(SplitterDrag {
            kind,
            start_x: event.position.x,
            start_width,
        });
        cx.notify();
    }

    fn end_splitter_drag(&mut self, cx: &mut Context<Self>) {
        if self.splitter_drag.take().is_some() {
            cx.notify();
        }
    }

    fn on_splitter_drag_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.splitter_drag else {
            return;
        };

        let rail_w = px(48.);
        let splitter_w = px(6.);
        let editor_min_w = px(320.);
        let panel_min_w = px(180.);
        let workspace_min_w = px(220.);

        let window_w = window.bounds().size.width;
        let (panel_shell_state, workspace_state) = self.effective_sidebar_layout(window_w);
        let panel_shell_present = panel_shell_state != SidebarState::Hidden;
        let workspace_present = workspace_state != SidebarState::Hidden;
        let panel_shell_w = match panel_shell_state {
            SidebarState::Expanded => self.panel_shell_width.max(panel_min_w),
            SidebarState::Hidden => px(0.),
        };
        let workspace_w = match workspace_state {
            SidebarState::Expanded => self.workspace_width.max(workspace_min_w),
            SidebarState::Hidden => px(0.),
        };

        let delta_x = event.position.x - drag.start_x;

        match drag.kind {
            SplitterKind::PanelShell => {
                if !panel_shell_present {
                    return;
                }
                let right = editor_min_w
                    + splitter_w
                    + if workspace_present {
                        workspace_w + splitter_w
                    } else {
                        px(0.)
                    };
                let reserve = rail_w + right;
                let max_w = if window_w > reserve {
                    window_w - reserve
                } else {
                    panel_min_w
                };
                let next = (drag.start_width + delta_x).clamp(panel_min_w, max_w.max(panel_min_w));
                self.panel_shell_width = next;
                self.panel_shell_saved_width = next;
                cx.notify();
            }
            SplitterKind::Workspace => {
                if !workspace_present {
                    return;
                }
                let left = rail_w
                    + if panel_shell_present {
                        panel_shell_w + splitter_w
                    } else {
                        px(0.)
                    };
                let reserve = left + splitter_w + editor_min_w;
                let max_w = if window_w > reserve {
                    window_w - reserve
                } else {
                    workspace_min_w
                };
                let next =
                    (drag.start_width + delta_x).clamp(workspace_min_w, max_w.max(workspace_min_w));
                self.workspace_width = next;
                self.workspace_saved_width = next;
                cx.notify();
            }
        }
    }

    fn on_splitter_drag_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.end_splitter_drag(cx);
    }

    fn on_vault_prompt_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
        let key = ev.keystroke.key.to_lowercase();

        if ctrl {
            match key.as_str() {
                "k" => {
                    self.close_vault_prompt(cx);
                    self.open_palette(PaletteMode::Commands, cx);
                    return;
                }
                "p" => {
                    self.close_vault_prompt(cx);
                    self.open_palette(PaletteMode::QuickOpen, cx);
                    return;
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        let text = text.replace("\r\n", "\n");
                        if let Some(first) = text.lines().next() {
                            let first = first.trim();
                            if !first.is_empty() {
                                self.vault_prompt_value.push_str(first);
                                self.vault_prompt_error = None;
                                cx.notify();
                            }
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        match key.as_str() {
            "escape" => {
                self.close_vault_prompt(cx);
            }
            "backspace" => {
                self.vault_prompt_value.pop();
                self.vault_prompt_error = None;
                cx.notify();
            }
            "enter" | "return" => {
                let value = self.vault_prompt_value.trim().to_string();
                if value.is_empty() {
                    self.vault_prompt_error = Some(SharedString::from(
                        self.i18n.text("prompt.enter_vault_path"),
                    ));
                    cx.notify();
                    return;
                }

                let path = PathBuf::from(value);
                if !path.is_dir() {
                    self.vault_prompt_error = Some(SharedString::from(
                        self.i18n.text("prompt.vault_path_not_folder"),
                    ));
                    cx.notify();
                    return;
                }

                self.close_vault_prompt(cx);
                self.open_vault(path, cx).detach();
            }
            _ => {
                if ctrl {
                    return;
                }
                let Some(text) = ev.keystroke.key_char.as_ref() else {
                    return;
                };
                if text.is_empty() {
                    return;
                }
                self.vault_prompt_value.push_str(text);
                self.vault_prompt_error = None;
                cx.notify();
            }
        }
    }

    fn palette_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, placeholder, group_title) = match self.palette_mode {
            PaletteMode::Commands => (
                self.i18n.text("palette.title.commands"),
                self.i18n.text("palette.placeholder.commands"),
                self.i18n.text("palette.group.navigation"),
            ),
            PaletteMode::QuickOpen => (
                self.i18n.text("palette.title.quick_open"),
                self.i18n.text("palette.placeholder.quick_open"),
                self.i18n.text("palette.group.files"),
            ),
        };

        let query_empty = self.palette_query.trim().is_empty();
        let input_text = if query_empty {
            SharedString::from(placeholder)
        } else {
            SharedString::from(self.palette_query.trim().to_string())
        };
        let input_color = if query_empty { 0x9ca3af } else { 0x111827 };

        let item_count = match self.palette_mode {
            PaletteMode::Commands => self.filtered_palette_command_indices().len(),
            PaletteMode::QuickOpen => self.palette_results.len(),
        };

        let list = uniform_list(
            "palette.items",
            item_count.max(1),
            cx.processor(|this, range: std::ops::Range<usize>, _window, cx| {
                match this.palette_mode {
                    PaletteMode::Commands => {
                        let filtered = this.filtered_palette_command_indices();
                        if filtered.is_empty() {
                            return range
                                .map(|ix| {
                                    div()
                                        .id(ElementId::named_usize("palette.empty", ix))
                                        .h(px(44.))
                                        .px(px(10.))
                                        .font_family("Inter")
                                        .text_size(px(13.))
                                        .font_weight(FontWeight(700.))
                                        .text_color(rgb(0x6b7280))
                                        .child(if ix == 0 {
                                            this.i18n.text("palette.empty_commands")
                                        } else {
                                            String::new()
                                        })
                                })
                                .collect::<Vec<_>>();
                        }

                        range
                            .map(|ix| {
                                let Some(cmd_ix) = filtered.get(ix).copied() else {
                                    return div()
                                        .id(ElementId::named_usize("palette.missing", ix))
                                        .h(px(44.))
                                        .px(px(10.))
                                        .child("");
                                };
                                let Some(cmd) = PALETTE_COMMANDS.get(cmd_ix) else {
                                    return div()
                                        .id(ElementId::named_usize("palette.missing", ix))
                                        .h(px(44.))
                                        .px(px(10.))
                                        .child("");
                                };

                                let selected = ix == this.palette_selected;
                                let icon_color = if selected { 0x0b4b57 } else { 0x6b7280 };
                                let label = this.command_label(cmd.id);
                                let detail = this.command_detail(cmd.id);
                                let shortcut_text = this.command_shortcut(cmd.id);

                                let left = div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(ui_icon(cmd.icon, 16., icon_color))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_0()
                                            .child(
                                                div()
                                                    .font_family("Inter")
                                                    .text_size(px(13.))
                                                    .font_weight(FontWeight(800.))
                                                    .text_color(rgb(0x111827))
                                                    .child(label),
                                            )
                                            .child(
                                                div()
                                                    .font_family("Inter")
                                                    .text_size(px(11.))
                                                    .font_weight(FontWeight(650.))
                                                    .text_color(rgb(0x6b7280))
                                                    .child(detail),
                                            ),
                                    );

                                let shortcut = div()
                                    .h(px(28.))
                                    .bg(rgb(0xf2f4f7))
                                    .border_1()
                                    .border_color(rgb(0xc8cdd5))
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .font_family("IBM Plex Mono")
                                            .text_size(px(12.))
                                            .font_weight(FontWeight(750.))
                                            .text_color(rgb(0x6b7280))
                                            .child(shortcut_text),
                                    );

                                div()
                                    .id(ElementId::Name(SharedString::from(format!(
                                        "palette.cmd:{cmd_ix}"
                                    ))))
                                    .h(px(44.))
                                    .w_full()
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .relative()
                                    .overflow_hidden()
                                    .cursor_pointer()
                                    .bg(if selected {
                                        rgb(0xe6f7fa)
                                    } else {
                                        rgba(0x00000000)
                                    })
                                    .when(selected, |this| {
                                        this.border_1().border_color(rgb(0xb6edf5))
                                    })
                                    .when(selected, |this| {
                                        this.child(
                                            div()
                                                .absolute()
                                                .top(px(-8.))
                                                .right(px(4.))
                                                .child(ui_corner_tag(0x0b4b57)),
                                        )
                                    })
                                    .hover(|this| this.bg(rgb(0xe6f7fa)))
                                    .on_click(cx.listener(
                                        move |this, _ev: &ClickEvent, _window, cx| {
                                            this.palette_selected = ix;
                                            this.execute_palette_command(cmd.id, cx);
                                        },
                                    ))
                                    .child(left)
                                    .child(shortcut)
                            })
                            .collect::<Vec<_>>()
                    }
                    PaletteMode::QuickOpen => {
                        if this.palette_results.is_empty() {
                            let msg = if this.palette_query.trim().is_empty() {
                                "Type to search…"
                            } else {
                                "No matches"
                            };
                            return range
                                .map(|ix| {
                                    div()
                                        .id(ElementId::named_usize("palette.quick_open.empty", ix))
                                        .h(px(44.))
                                        .px_3()
                                        .font_family("Inter")
                                        .text_size(px(13.))
                                        .font_weight(FontWeight(700.))
                                        .text_color(rgb(0x6b7280))
                                        .child(if ix == 0 { msg } else { "" })
                                })
                                .collect::<Vec<_>>();
                        }

                        range
                            .map(|ix| {
                                let Some(note_ix) = this.palette_results.get(ix).copied() else {
                                    return div()
                                        .id(ElementId::named_usize(
                                            "palette.quick_open.missing",
                                            ix,
                                        ))
                                        .h(px(44.))
                                        .px_3()
                                        .child("");
                                };
                                let Some(path) = this.explorer_all_note_paths.get(note_ix).cloned()
                                else {
                                    return div()
                                        .id(ElementId::named_usize(
                                            "palette.quick_open.missing",
                                            ix,
                                        ))
                                        .h(px(44.))
                                        .px_3()
                                        .child("");
                                };

                                let selected = ix == this.palette_selected;
                                let open_path = path.clone();

                                div()
                                    .id(ElementId::Name(SharedString::from(format!(
                                        "palette.note:{path}"
                                    ))))
                                    .h(px(44.))
                                    .w_full()
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .bg(if selected {
                                        rgb(0xe6f7fa)
                                    } else {
                                        rgba(0x00000000)
                                    })
                                    .when(selected, |this| {
                                        this.border_1().border_color(rgb(0xb6edf5))
                                    })
                                    .hover(|this| this.bg(rgb(0xe6f7fa)))
                                    .on_click(cx.listener(
                                        move |this, _ev: &ClickEvent, _window, cx| {
                                            this.palette_selected = ix;
                                            this.close_palette(cx);
                                            this.open_note(open_path.clone(), cx);
                                        },
                                    ))
                                    .child(ui_icon(ICON_FILE_TEXT, 16., 0x6b7280))
                                    .child(
                                        div()
                                            .font_family("IBM Plex Mono")
                                            .text_size(px(11.))
                                            .font_weight(FontWeight(750.))
                                            .text_color(rgb(0x111827))
                                            .child(path),
                                    )
                            })
                            .collect::<Vec<_>>()
                    }
                }
            }),
        )
        .h_full();

        let palette_box = div()
            .w(px(720.))
            .h(px(520.))
            .bg(rgb(0xf2f4f7))
            .border_1()
            .border_color(rgb(0xc8cdd5))
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .bg(rgb(0xf2f4f7))
                    .p_3()
                    .gap(px(10.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(0x6b7280))
                            .child(title),
                    )
                    .child(
                        div()
                            .id("palette.input")
                            .h(px(44.))
                            .w_full()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xc8cdd5))
                            .px_3()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .focusable()
                            .cursor_text()
                            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                                this.on_palette_key(ev, cx);
                            }))
                            .child(ui_icon(ICON_SEARCH, 16., 0x9ca3af))
                            .child(
                                div()
                                    .font_family("Inter")
                                    .text_size(px(13.))
                                    .font_weight(FontWeight(700.))
                                    .text_color(rgb(input_color))
                                    .child(input_text),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .bg(rgb(0xf2f4f7))
                    .p(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(0x6b7280))
                            .child(group_title),
                    )
                    .child(
                        div()
                            .id("palette.list")
                            .flex_1()
                            .overflow_y_scroll()
                            .child(list),
                    ),
            );

        div()
            .id("palette.overlay")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .child(
                div()
                    .id("palette.backdrop")
                    .size_full()
                    .bg(rgba(0x0000003c))
                    .absolute()
                    .top_0()
                    .left_0()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.close_palette(cx);
                    })),
            )
            .child(
                div()
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .pt_8()
                    .child(palette_box),
            )
    }

    fn vault_prompt_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let query_empty = self.vault_prompt_value.trim().is_empty();
        let input_text = if query_empty {
            SharedString::from("Type a vault folder path…")
        } else {
            SharedString::from(self.vault_prompt_value.clone())
        };
        let input_color = if query_empty { 0x9ca3af } else { 0x111827 };

        let error = self.vault_prompt_error.clone();

        let prompt_box = div()
            .w(px(720.))
            .bg(rgb(0xf2f4f7))
            .border_1()
            .border_color(rgb(0xc8cdd5))
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .bg(rgb(0xf2f4f7))
                    .p_3()
                    .gap(px(10.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(0x6b7280))
                            .child("OPEN VAULT"),
                    )
                    .child(
                        div()
                            .id("vault_prompt.input")
                            .h(px(44.))
                            .w_full()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xc8cdd5))
                            .px_3()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .track_focus(&self.vault_prompt_focus_handle)
                            .focusable()
                            .cursor_text()
                            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                                this.on_vault_prompt_key(ev, cx);
                            }))
                            .child(ui_icon(ICON_VAULT, 16., 0x9ca3af))
                            .child(
                                div()
                                    .font_family("Inter")
                                    .text_size(px(13.))
                                    .font_weight(FontWeight(700.))
                                    .text_color(rgb(input_color))
                                    .child(input_text),
                            ),
                    )
                    .children(error.map(|message| {
                        div()
                            .mt_1()
                            .font_family("Inter")
                            .text_size(px(13.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(0xdc2626))
                            .child(message)
                    }))
                    .child(
                        div()
                            .mt_1()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(0x6b7280))
                            .child("Enter to open · Esc to cancel"),
                    ),
            );

        div()
            .id("vault_prompt.overlay")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .child(
                div()
                    .id("vault_prompt.backdrop")
                    .size_full()
                    .bg(rgba(0x0000003c))
                    .absolute()
                    .top_0()
                    .left_0()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.close_vault_prompt(cx);
                    })),
            )
            .child(
                div()
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .pt_8()
                    .child(prompt_box),
            )
    }

    fn settings_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let nav_item =
            |id: &'static str, icon: &'static str, label: String, section: SettingsSection| {
                let active = self.settings_section == section;
                div()
                    .id(id)
                    .h(px(40.))
                    .w_full()
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .relative()
                    .overflow_hidden()
                    .cursor_pointer()
                    .bg(if active {
                        rgb(0xe6f7fa)
                    } else {
                        rgba(0x00000000)
                    })
                    .when(active, |this| this.border_1().border_color(rgb(0xb6edf5)))
                    .when(active, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(-8.))
                                .right(px(12.))
                                .child(ui_corner_tag(0x0b4b57)),
                        )
                    })
                    .hover(|this| this.bg(rgb(0xe6f7fa)))
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        this.settings_section = section;
                        this.settings_language_menu_open = false;
                        cx.notify();
                    }))
                    .child(ui_icon(icon, 16., if active { 0x0b4b57 } else { 0x6b7280 }))
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(13.))
                            .font_weight(FontWeight(if active { 800. } else { 700. }))
                            .text_color(rgb(if active { 0x0b4b57 } else { 0x111827 }))
                            .child(label),
                    )
            };

        let page_title = match self.settings_section {
            SettingsSection::About => self.i18n.text("settings.nav.about"),
            SettingsSection::Appearance => self.i18n.text("settings.nav.appearance"),
            SettingsSection::Editor => self.i18n.text("settings.nav.editor"),
            SettingsSection::FilesLinks => self.i18n.text("settings.nav.files"),
            SettingsSection::Hotkeys => self.i18n.text("settings.nav.hotkeys"),
            SettingsSection::Advanced => self.i18n.text("settings.nav.advanced"),
        };

        let appearance_content = {
            let theme_button = |id: &'static str, label: String, theme: SettingsTheme| {
                let active = self.settings_theme == theme;
                div()
                    .id(id)
                    .h_full()
                    .flex_1()
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(if active {
                        rgb(0xe6f7fa)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|this| this.bg(rgb(0xe6f7fa)))
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        this.settings_theme = theme;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(13.))
                            .font_weight(FontWeight(if active { 800. } else { 700. }))
                            .text_color(rgb(if active { 0x0b4b57 } else { 0x111827 }))
                            .child(label),
                    )
            };

            let segmented = div()
                .h(px(36.))
                .bg(rgb(0xf2f4f7))
                .border_1()
                .border_color(rgb(0xe5e7eb))
                .flex()
                .items_center()
                .child(theme_button(
                    "settings.theme.dark",
                    self.i18n.text("settings.theme.dark"),
                    SettingsTheme::Dark,
                ))
                .child(theme_button(
                    "settings.theme.light",
                    self.i18n.text("settings.theme.light"),
                    SettingsTheme::Light,
                ));

            let swatch_color = match self.settings_accent {
                SettingsAccent::Default => 0x6d5df2,
                SettingsAccent::Blue => 0x2563eb,
            };

            let accent_chip = |id: &'static str, label: String, accent: SettingsAccent| {
                let active = self.settings_accent == accent;
                div()
                    .id(id)
                    .h(px(28.))
                    .px(px(10.))
                    .bg(if active { rgb(0xe6f7fa) } else { rgb(0xf2f4f7) })
                    .border_1()
                    .border_color(rgb(if active { 0xb6edf5 } else { 0xc8cdd5 }))
                    .cursor_pointer()
                    .hover(|this| this.bg(rgb(0xe6f7fa)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        this.settings_accent = accent;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(12.))
                            .font_weight(FontWeight(if active { 800. } else { 700. }))
                            .text_color(rgb(if active { 0x0b4b57 } else { 0x111827 }))
                            .child(label),
                    )
            };

            let accent_row = div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .w(px(40.))
                        .h(px(28.))
                        .bg(rgb(swatch_color))
                        .border_1()
                        .border_color(rgb(0xc8cdd5)),
                )
                .child(accent_chip(
                    "settings.accent.default",
                    self.i18n.text("settings.accent.default"),
                    SettingsAccent::Default,
                ))
                .child(accent_chip(
                    "settings.accent.blue",
                    self.i18n.text("settings.accent.blue"),
                    SettingsAccent::Blue,
                ));

            let language_name = match self.settings_language {
                Locale::EnUs => self.i18n.text("settings.language.english"),
                Locale::ZhCn => self.i18n.text("settings.language.chinese"),
            };

            let language_select = div()
                .id("settings.language.select")
                .w(px(240.))
                .h(px(34.))
                .bg(rgb(0xf2f4f7))
                .border_1()
                .border_color(rgb(0xc8cdd5))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .hover(|this| this.bg(rgb(0xe1e5ea)))
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    this.settings_language_menu_open = !this.settings_language_menu_open;
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(13.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(0x111827))
                        .child(language_name),
                )
                .child(ui_icon(ICON_CHEVRON_DOWN, 16., 0x6b7280));

            let language_menu = div()
                .id("settings.language.menu")
                .w(px(240.))
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgb(0xc8cdd5))
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("settings.language.english")
                        .h(px(34.))
                        .px_3()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .hover(|this| this.bg(rgb(0xe6f7fa)))
                        .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                            this.settings_language = Locale::EnUs;
                            this.i18n.set_locale(this.settings_language);
                            this.status = SharedString::from(this.i18n.text("status.ready"));
                            this.persist_settings();
                            this.settings_language_menu_open = false;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .font_family("Inter")
                                .text_size(px(13.))
                                .font_weight(FontWeight(700.))
                                .text_color(rgb(0x111827))
                                .child(self.i18n.text("settings.language.english")),
                        ),
                )
                .child(
                    div()
                        .id("settings.language.chinese")
                        .h(px(34.))
                        .px_3()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .hover(|this| this.bg(rgb(0xe6f7fa)))
                        .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                            this.settings_language = Locale::ZhCn;
                            this.i18n.set_locale(this.settings_language);
                            this.status = SharedString::from(this.i18n.text("status.ready"));
                            this.persist_settings();
                            this.settings_language_menu_open = false;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .font_family("Inter")
                                .text_size(px(13.))
                                .font_weight(FontWeight(700.))
                                .text_color(rgb(0x111827))
                                .child(self.i18n.text("settings.language.chinese")),
                        ),
                );

            let language_section = div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(11.))
                        .font_weight(FontWeight(800.))
                        .text_color(rgb(0x6b7280))
                        .child(self.i18n.text("settings.section.language")),
                )
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(0x6b7280))
                        .child(self.i18n.text("settings.language.hint")),
                )
                .child(language_select)
                .children(
                    self.settings_language_menu_open
                        .then_some(div().mt_1().child(language_menu)),
                );

            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .font_family("IBM Plex Mono")
                                .text_size(px(11.))
                                .font_weight(FontWeight(800.))
                                .text_color(rgb(0x6b7280))
                                .child(self.i18n.text("settings.section.theme")),
                        )
                        .child(segmented),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .font_family("IBM Plex Mono")
                                .text_size(px(11.))
                                .font_weight(FontWeight(800.))
                                .text_color(rgb(0x6b7280))
                                .child(self.i18n.text("settings.section.colors")),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .font_family("Inter")
                                        .text_size(px(13.))
                                        .font_weight(FontWeight(800.))
                                        .text_color(rgb(0x111827))
                                        .child(self.i18n.text("settings.colors.accent")),
                                )
                                .child(
                                    div()
                                        .font_family("Inter")
                                        .text_size(px(12.))
                                        .font_weight(FontWeight(650.))
                                        .text_color(rgb(0x6b7280))
                                        .child(self.i18n.text("settings.colors.accent.hint")),
                                )
                                .child(accent_row),
                        ),
                )
                .child(language_section)
                .into_any_element()
        };

        let placeholder = |text: String| {
            div()
                .font_family("Inter")
                .text_size(px(13.))
                .font_weight(FontWeight(650.))
                .text_color(rgb(0x6b7280))
                .child(text)
                .into_any_element()
        };

        let about_placeholder = format!(
            "About page is not implemented yet.\nLocale: {}\nPlugins: {}\nPlugin runtime mode: {}\nPlugin runtime: {}",
            self.i18n.locale().as_tag(),
            self.plugin_registry.list().len(),
            self.plugin_runtime_mode.as_tag(),
            match &self.plugin_activation_state {
                PluginActivationState::Idle => "idle".to_string(),
                PluginActivationState::Activating => "activating".to_string(),
                PluginActivationState::Ready { active_count } => {
                    format!("ready ({active_count} active)")
                }
                PluginActivationState::Error { message } => {
                    format!("error ({})", message)
                }
            },
        );

        let page_content = match self.settings_section {
            SettingsSection::Appearance => appearance_content,
            SettingsSection::About => placeholder(about_placeholder),
            SettingsSection::Editor => {
                placeholder("Editor settings are not implemented yet.".to_string())
            }
            SettingsSection::FilesLinks => {
                placeholder("Files & Links settings are not implemented yet.".to_string())
            }
            SettingsSection::Hotkeys => {
                placeholder("Hotkeys settings are not implemented yet.".to_string())
            }
            SettingsSection::Advanced => {
                placeholder("Advanced settings are not implemented yet.".to_string())
            }
        };

        let modal = div()
            .w(px(980.))
            .h(px(640.))
            .bg(rgb(0xf2f4f7))
            .border_1()
            .border_color(rgb(0xc8cdd5))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(44.))
                    .w_full()
                    .bg(rgb(0xf2f4f7))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(13.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(0x111827))
                            .child(self.i18n.text("settings.title")),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("settings.close")
                            .h(px(32.))
                            .w(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|this| this.bg(rgb(0xe1e5ea)))
                            .occlude()
                            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                                this.settings_open = false;
                                this.settings_language_menu_open = false;
                                cx.notify();
                            }))
                            .child(ui_icon(ICON_X, 16., 0x6b7280)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .child(
                        div()
                            .w(px(240.))
                            .h_full()
                            .bg(rgb(0xf2f4f7))
                            .p(px(10.))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(nav_item(
                                "settings.nav.about",
                                ICON_USER,
                                self.i18n.text("settings.nav.about"),
                                SettingsSection::About,
                            ))
                            .child(nav_item(
                                "settings.nav.appearance",
                                ICON_BRUSH,
                                self.i18n.text("settings.nav.appearance"),
                                SettingsSection::Appearance,
                            ))
                            .child(nav_item(
                                "settings.nav.editor",
                                ICON_SLIDERS_HORIZONTAL,
                                self.i18n.text("settings.nav.editor"),
                                SettingsSection::Editor,
                            ))
                            .child(nav_item(
                                "settings.nav.files",
                                ICON_FILE_COG,
                                self.i18n.text("settings.nav.files"),
                                SettingsSection::FilesLinks,
                            ))
                            .child(nav_item(
                                "settings.nav.hotkeys",
                                ICON_KEYBOARD,
                                self.i18n.text("settings.nav.hotkeys"),
                                SettingsSection::Hotkeys,
                            ))
                            .child(nav_item(
                                "settings.nav.advanced",
                                ICON_SLIDERS_HORIZONTAL,
                                self.i18n.text("settings.nav.advanced"),
                                SettingsSection::Advanced,
                            )),
                    )
                    .child(div().w(px(1.)).h_full().bg(rgb(0xe5e7eb)))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .bg(rgb(0xffffff))
                            .px_4()
                            .py(px(14.))
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .font_family("Inter")
                                    .text_size(px(16.))
                                    .font_weight(FontWeight(900.))
                                    .text_color(rgb(0x111827))
                                    .child(page_title),
                            )
                            .child(page_content),
                    ),
            );

        div()
            .id("settings.overlay")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .child(
                div()
                    .id("settings.backdrop")
                    .size_full()
                    .bg(rgba(0x0000003c))
                    .absolute()
                    .top_0()
                    .left_0()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.settings_open = false;
                        this.settings_language_menu_open = false;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(modal),
            )
    }

    fn is_filtering(&self) -> bool {
        !self.explorer_filter.trim().is_empty()
    }

    fn schedule_apply_filter(&mut self, delay: Duration, cx: &mut Context<Self>) {
        self.next_filter_nonce = self.next_filter_nonce.wrapping_add(1);
        let nonce = self.next_filter_nonce;
        self.pending_filter_nonce = nonce;

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    if delay > Duration::ZERO {
                        Timer::after(delay).await;
                    }

                    let Some((query, paths_lower)) = this
                        .update(&mut cx, |this, cx| {
                            if this.pending_filter_nonce != nonce {
                                return None;
                            }

                            let query = this.explorer_filter.trim().to_lowercase();
                            if query.is_empty() {
                                this.explorer_rows_filtered.clear();
                                cx.notify();
                                return None;
                            }

                            Some((query, this.explorer_all_note_paths_lower.clone()))
                        })
                        .ok()
                        .flatten()
                    else {
                        return;
                    };

                    let matched_indices: Vec<usize> = cx
                        .background_executor()
                        .spawn(async move {
                            let mut out = Vec::new();
                            for (ix, path_lower) in paths_lower.iter().enumerate() {
                                if path_lower.contains(&query) {
                                    out.push(ix);
                                }
                            }
                            out
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_filter_nonce != nonce {
                            return;
                        }

                        this.explorer_rows_filtered = matched_indices;
                        cx.notify();
                    })
                    .ok();
                }
            },
        )
        .detach();
    }

    fn schedule_apply_search(&mut self, delay: Duration, cx: &mut Context<Self>) {
        self.next_search_nonce = self.next_search_nonce.wrapping_add(1);
        let nonce = self.next_search_nonce;
        self.pending_search_nonce = nonce;

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    if delay > Duration::ZERO {
                        Timer::after(delay).await;
                    }

                    let Some((query, note_paths, vault)) = this
                        .update(&mut cx, |this, cx| {
                            if this.pending_search_nonce != nonce {
                                return None;
                            }

                            let query = this.search_query.trim().to_string();
                            if query.is_empty() {
                                this.search_selected = 0;
                                this.search_results.clear();
                                cx.notify();
                                return None;
                            }

                            let Some(vault) = this.vault() else {
                                this.search_selected = 0;
                                this.search_results.clear();
                                cx.notify();
                                return None;
                            };

                            Some((query, this.explorer_all_note_paths.clone(), vault))
                        })
                        .ok()
                        .flatten()
                    else {
                        return;
                    };

                    let search_rows: Vec<SearchRow> = cx
                        .background_executor()
                        .spawn(async move {
                            const MAX_FILES_WITH_MATCHES: usize = 30;
                            const MAX_MATCH_ROWS: usize = 200;
                            const MAX_PREVIEW_MATCHES_PER_FILE: usize = 3;
                            const MAX_MATCHES_TO_COUNT_PER_FILE: usize = 50;

                            let query_lower = query.to_ascii_lowercase();
                            let start = Instant::now();
                            let budget = Duration::from_millis(250);

                            let mut out = Vec::new();
                            let mut files_with_matches = 0usize;
                            let mut match_rows = 0usize;

                            for path in note_paths.iter() {
                                if files_with_matches >= MAX_FILES_WITH_MATCHES
                                    || match_rows >= MAX_MATCH_ROWS
                                {
                                    break;
                                }
                                if start.elapsed() >= budget {
                                    break;
                                }

                                let content = match vault.read_note(path) {
                                    Ok(s) => s,
                                    Err(_) => continue,
                                };

                                let mut previews: Vec<(usize, String)> = Vec::new();
                                let mut match_count = 0usize;

                                for (line_ix, line) in content.lines().enumerate() {
                                    if match_count >= MAX_MATCHES_TO_COUNT_PER_FILE
                                        && previews.len() >= MAX_PREVIEW_MATCHES_PER_FILE
                                    {
                                        break;
                                    }

                                    if line.to_ascii_lowercase().contains(&query_lower) {
                                        match_count += 1;
                                        if previews.len() < MAX_PREVIEW_MATCHES_PER_FILE {
                                            let mut preview = line.trim_end().to_string();
                                            if preview.len() > 120 {
                                                preview.truncate(120);
                                                preview.push('…');
                                            }
                                            previews.push((line_ix + 1, preview));
                                        }
                                    }
                                }

                                if match_count == 0 {
                                    continue;
                                }

                                files_with_matches += 1;
                                out.push(SearchRow::File {
                                    path: path.clone(),
                                    match_count,
                                });
                                for (line, preview) in previews {
                                    if match_rows >= MAX_MATCH_ROWS {
                                        break;
                                    }
                                    match_rows += 1;
                                    out.push(SearchRow::Match {
                                        path: path.clone(),
                                        line,
                                        preview,
                                    });
                                }
                            }

                            out
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_search_nonce != nonce {
                            return;
                        }

                        this.search_results = search_rows;
                        if this.search_selected >= this.search_results.len() {
                            this.search_selected = 0;
                        }
                        cx.notify();
                    })
                    .ok();
                }
            },
        )
        .detach();
    }

    fn schedule_apply_palette_results(&mut self, delay: Duration, cx: &mut Context<Self>) {
        self.next_palette_nonce = self.next_palette_nonce.wrapping_add(1);
        let nonce = self.next_palette_nonce;
        self.pending_palette_nonce = nonce;

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    if delay > Duration::ZERO {
                        Timer::after(delay).await;
                    }

                    let Some((query, paths_lower)) = this
                        .update(&mut cx, |this, cx| {
                            if this.pending_palette_nonce != nonce {
                                return None;
                            }

                            let query = this.palette_query.trim().to_lowercase();
                            if query.is_empty() {
                                this.palette_selected = 0;
                                this.palette_results.clear();
                                cx.notify();
                                return None;
                            }

                            Some((query, this.explorer_all_note_paths_lower.clone()))
                        })
                        .ok()
                        .flatten()
                    else {
                        return;
                    };

                    let matched_indices: Vec<usize> = cx
                        .background_executor()
                        .spawn(async move {
                            let mut out = Vec::new();
                            for (ix, path_lower) in paths_lower.iter().enumerate() {
                                if path_lower.contains(&query) {
                                    out.push(ix);
                                    if out.len() >= 200 {
                                        break;
                                    }
                                }
                            }
                            out
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_palette_nonce != nonce {
                            return;
                        }

                        this.palette_results = matched_indices;
                        if this.palette_selected >= this.palette_results.len() {
                            this.palette_selected = 0;
                        }
                        cx.notify();
                    })
                    .ok();
                }
            },
        )
        .detach();
    }

    fn on_filter_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.palette_open {
            self.on_palette_key(ev, cx);
            return;
        }
        if self.settings_open {
            if ev.keystroke.key.eq_ignore_ascii_case("escape") {
                self.settings_open = false;
                self.settings_language_menu_open = false;
                cx.notify();
            }
            return;
        }

        if ev.keystroke.modifiers.alt {
            match ev.keystroke.key.as_str() {
                "1" => {
                    self.panel_mode = PanelMode::Explorer;
                    cx.notify();
                    return;
                }
                "2" => {
                    self.panel_mode = PanelMode::Search;
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }

        if let Some(command) = self.command_from_event(ev) {
            match command {
                CommandId::CommandPalette
                | CommandId::QuickOpen
                | CommandId::OpenVault
                | CommandId::FocusExplorer
                | CommandId::FocusSearch => {
                    self.execute_palette_command(command, cx);
                    return;
                }
                _ => {}
            }
        }

        let ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
        let key = ev.keystroke.key.to_lowercase();
        if ctrl {
            match key.as_str() {
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        let text = text.replace("\r\n", "\n");
                        if let Some(first) = text.lines().next() {
                            let first = first.trim();
                            if !first.is_empty() {
                                self.explorer_filter.push_str(first);
                                self.schedule_apply_filter(Duration::ZERO, cx);
                                cx.notify();
                            }
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        if ev.keystroke.key.eq_ignore_ascii_case("escape") {
            if !self.explorer_filter.is_empty() {
                self.explorer_filter.clear();
                self.explorer_rows_filtered.clear();
                self.pending_filter_nonce = 0;
                self.status = SharedString::from("Ready");
                cx.notify();
            }
            return;
        }
        match key.as_str() {
            "backspace" => {
                if self.explorer_filter.pop().is_some() {
                    self.schedule_apply_filter(Duration::from_millis(60), cx);
                    cx.notify();
                }
            }
            "enter" | "return" => {}
            _ => {
                if ctrl {
                    return;
                }
                let Some(text) = ev.keystroke.key_char.as_ref() else {
                    return;
                };
                if text.is_empty() {
                    return;
                }
                self.explorer_filter.push_str(text);
                self.schedule_apply_filter(Duration::from_millis(60), cx);
                cx.notify();
            }
        }
    }

    fn on_search_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.palette_open {
            self.on_palette_key(ev, cx);
            return;
        }
        if self.settings_open {
            if ev.keystroke.key.eq_ignore_ascii_case("escape") {
                self.settings_open = false;
                self.settings_language_menu_open = false;
                cx.notify();
            }
            return;
        }

        if ev.keystroke.modifiers.alt {
            match ev.keystroke.key.as_str() {
                "1" => {
                    self.panel_mode = PanelMode::Explorer;
                    cx.notify();
                    return;
                }
                "2" => {
                    self.panel_mode = PanelMode::Search;
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }

        if let Some(command) = self.command_from_event(ev) {
            match command {
                CommandId::CommandPalette
                | CommandId::QuickOpen
                | CommandId::OpenVault
                | CommandId::FocusExplorer
                | CommandId::FocusSearch => {
                    self.execute_palette_command(command, cx);
                    return;
                }
                _ => {}
            }
        }

        let ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
        let key = ev.keystroke.key.to_lowercase();

        if ctrl {
            match key.as_str() {
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        let text = text.replace("\r\n", "\n");
                        if let Some(first) = text.lines().next() {
                            let first = first.trim();
                            if !first.is_empty() {
                                self.search_query.push_str(first);
                                self.search_selected = 0;
                                self.schedule_apply_search(Duration::ZERO, cx);
                                cx.notify();
                            }
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        match key.as_str() {
            "escape" => {
                if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.search_selected = 0;
                    self.search_results.clear();
                    self.pending_search_nonce = 0;
                    self.status = SharedString::from("Ready");
                    cx.notify();
                } else {
                    self.panel_mode = PanelMode::Explorer;
                    cx.notify();
                }
            }
            "backspace" => {
                if self.search_query.pop().is_some() {
                    self.schedule_apply_search(Duration::from_millis(60), cx);
                    cx.notify();
                }
            }
            "up" => {
                if self.search_selected > 0 {
                    self.search_selected -= 1;
                    cx.notify();
                }
            }
            "down" => {
                if self.search_selected + 1 < self.search_results.len() {
                    self.search_selected += 1;
                    cx.notify();
                }
            }
            "enter" | "return" => {
                if let Some(row) = self.search_results.get(self.search_selected) {
                    match row {
                        SearchRow::File { path, .. } => {
                            self.open_note(path.clone(), cx);
                        }
                        SearchRow::Match { path, line, .. } => {
                            self.open_note_at_line(path.clone(), *line, cx);
                        }
                    }
                }
            }
            _ => {
                if ctrl {
                    return;
                }

                let Some(text) = ev.keystroke.key_char.as_ref() else {
                    return;
                };
                if text.is_empty() {
                    return;
                }
                self.search_query.push_str(text);
                self.search_selected = 0;
                self.schedule_apply_search(Duration::from_millis(60), cx);
                cx.notify();
            }
        }
    }

    fn on_palette_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
        let key = ev.keystroke.key.to_lowercase();

        if let Some(command) = self.command_from_event(ev) {
            match command {
                CommandId::OpenVault
                | CommandId::ReloadVault
                | CommandId::Settings
                | CommandId::NewNote
                | CommandId::SaveFile
                | CommandId::ToggleSplit
                | CommandId::FocusExplorer
                | CommandId::FocusSearch => {
                    self.execute_palette_command(command, cx);
                    return;
                }
                _ => {}
            }
        }

        if key == "escape" {
            self.close_palette(cx);
            return;
        }

        if ctrl {
            match key.as_str() {
                "k" => {
                    self.palette_mode = PaletteMode::Commands;
                    self.palette_query.clear();
                    self.palette_selected = 0;
                    self.palette_results.clear();
                    self.pending_palette_nonce = 0;
                    cx.notify();
                    return;
                }
                "p" => {
                    self.palette_mode = PaletteMode::QuickOpen;
                    self.palette_query.clear();
                    self.palette_selected = 0;
                    self.palette_results.clear();
                    self.pending_palette_nonce = 0;
                    cx.notify();
                    return;
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        let text = text.replace("\r\n", "\n");
                        if let Some(first) = text.lines().next() {
                            let first = first.trim();
                            if !first.is_empty() {
                                self.palette_query.push_str(first);
                                self.palette_selected = 0;
                                match self.palette_mode {
                                    PaletteMode::Commands => cx.notify(),
                                    PaletteMode::QuickOpen => {
                                        self.schedule_apply_palette_results(Duration::ZERO, cx);
                                        cx.notify();
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        let list_len = match self.palette_mode {
            PaletteMode::Commands => self.filtered_palette_command_indices().len(),
            PaletteMode::QuickOpen => self.palette_results.len(),
        };

        match key.as_str() {
            "up" => {
                if self.palette_selected > 0 {
                    self.palette_selected -= 1;
                    cx.notify();
                }
            }
            "down" => {
                if self.palette_selected + 1 < list_len {
                    self.palette_selected += 1;
                    cx.notify();
                }
            }
            "enter" | "return" => match self.palette_mode {
                PaletteMode::Commands => {
                    let filtered = self.filtered_palette_command_indices();
                    let Some(cmd_ix) = filtered.get(self.palette_selected).copied() else {
                        return;
                    };
                    let cmd = PALETTE_COMMANDS.get(cmd_ix).map(|c| c.id);
                    if let Some(cmd) = cmd {
                        self.execute_palette_command(cmd, cx);
                    }
                }
                PaletteMode::QuickOpen => {
                    let Some(note_ix) = self.palette_results.get(self.palette_selected).copied()
                    else {
                        return;
                    };
                    let Some(path) = self.explorer_all_note_paths.get(note_ix).cloned() else {
                        return;
                    };
                    self.close_palette(cx);
                    self.open_note(path, cx);
                }
            },
            "backspace" => {
                if self.palette_query.pop().is_some() {
                    self.palette_selected = 0;
                    match self.palette_mode {
                        PaletteMode::Commands => cx.notify(),
                        PaletteMode::QuickOpen => {
                            self.schedule_apply_palette_results(Duration::from_millis(60), cx);
                            cx.notify();
                        }
                    }
                }
            }
            _ => {
                if ctrl {
                    return;
                }
                let Some(text) = ev.keystroke.key_char.as_ref() else {
                    return;
                };
                if text.is_empty() {
                    return;
                }

                self.palette_query.push_str(text);
                self.palette_selected = 0;
                match self.palette_mode {
                    PaletteMode::Commands => cx.notify(),
                    PaletteMode::QuickOpen => {
                        self.schedule_apply_palette_results(Duration::from_millis(60), cx);
                        cx.notify();
                    }
                }
            }
        }
    }

    fn rebuild_explorer_rows(&mut self) {
        let root_name = match &self.vault_state {
            VaultState::Opened { root_name, .. } => root_name.to_string(),
            _ => "Vault".to_string(),
        };

        let root_expanded = self.explorer_expanded_folders.contains("");
        let mut rows = Vec::new();
        rows.push(ExplorerRow::Vault {
            root_name,
            expanded: root_expanded,
        });

        match &self.vault_state {
            VaultState::NotConfigured => rows.push(ExplorerRow::Hint {
                text: SharedString::from(self.i18n.text("hint.open_vault")),
            }),
            VaultState::Error { .. } => rows.push(ExplorerRow::Hint {
                text: SharedString::from(self.i18n.text("hint.vault_error")),
            }),
            _ => {}
        }

        if root_expanded {
            self.append_folder_contents("", 1, &mut rows);
        }

        self.explorer_rows = rows;
    }

    fn append_folder_contents(&self, folder: &str, depth: usize, rows: &mut Vec<ExplorerRow>) {
        let children = self
            .explorer_folder_children
            .get(folder)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let notes = self
            .folder_notes
            .get(folder)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let total = children.len() + notes.len();
        for ix in 0..total {
            if ix < children.len() {
                let child = &children[ix];
                let name = child
                    .rsplit('/')
                    .next()
                    .unwrap_or(child.as_str())
                    .to_string();
                let expanded = self.explorer_expanded_folders.contains(child.as_str());
                let has_children = self
                    .explorer_folder_children
                    .get(child)
                    .is_some_and(|v| !v.is_empty())
                    || self.folder_notes.get(child).is_some_and(|v| !v.is_empty());

                rows.push(ExplorerRow::Folder {
                    folder: child.clone(),
                    name,
                    depth,
                    expanded,
                    has_children,
                });

                if expanded {
                    self.append_folder_contents(child, depth + 1, rows);
                }
            } else {
                let path = &notes[ix - children.len()];
                rows.push(ExplorerRow::Note {
                    folder: folder.to_string(),
                    path: path.clone(),
                    file_name: file_name(path),
                    depth,
                });
            }
        }
    }

    fn toggle_folder_expanded(&mut self, folder: &str, cx: &mut Context<Self>) {
        let has_children = self
            .explorer_folder_children
            .get(folder)
            .is_some_and(|v| !v.is_empty())
            || self.folder_notes.get(folder).is_some_and(|v| !v.is_empty());
        if !has_children {
            return;
        }

        if self.explorer_expanded_folders.contains(folder) {
            self.explorer_expanded_folders.remove(folder);
        } else {
            self.explorer_expanded_folders.insert(folder.to_string());
        }

        self.drag_over = None;
        self.rebuild_explorer_rows();
        cx.notify();
    }

    fn expand_note_ancestors(&mut self, note_path: &str) -> bool {
        let mut changed = false;
        if self.explorer_expanded_folders.insert(String::new()) {
            changed = true;
        }

        let mut folder = match note_path.rsplit_once('/') {
            Some((folder, _)) => folder.to_string(),
            None => String::new(),
        };

        while !folder.is_empty() {
            if self.explorer_expanded_folders.insert(folder.clone()) {
                changed = true;
            }
            match folder.rsplit_once('/') {
                Some((parent, _)) => folder = parent.to_string(),
                None => folder.clear(),
            }
        }

        changed
    }

    fn open_note(&mut self, note_path: String, cx: &mut Context<Self>) {
        let Some(vault) = self.vault() else {
            return;
        };

        self.pending_open_note_cursor = self
            .pending_open_note_cursor
            .take()
            .filter(|(path, _line)| path == note_path.as_str());

        self.selected_note = Some(note_path.clone());
        if self.expand_note_ancestors(&note_path) {
            self.rebuild_explorer_rows();
        }
        if !self.open_editors.iter().any(|p| p == &note_path) {
            self.open_editors.push(note_path.clone());
        }
        self.open_note_path = Some(note_path.clone());
        self.open_note_loading = true;
        self.open_note_dirty = false;
        self.open_note_content.clear();
        self.editor_selected_range = 0..0;
        self.editor_selection_reversed = false;
        self.editor_marked_range = None;
        self.editor_is_selecting = false;
        self.editor_preferred_x = None;
        self.editor_layout = None;
        self.pending_note_save_nonce = 0;

        self.next_note_open_nonce = self.next_note_open_nonce.wrapping_add(1);
        let open_nonce = self.next_note_open_nonce;
        self.current_note_open_nonce = open_nonce;

        self.status = SharedString::from(format!("Loading note: {note_path}"));
        cx.notify();

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                let vault = vault.clone();
                let note_path = note_path.clone();
                async move {
                    let read_result: anyhow::Result<String> = cx
                        .background_executor()
                        .spawn({
                            let vault = vault.clone();
                            let note_path = note_path.clone();
                            async move { vault.read_note(&note_path) }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.current_note_open_nonce != open_nonce
                            || this.open_note_path.as_deref() != Some(note_path.as_str())
                        {
                            return;
                        }

                        this.open_note_loading = false;
                        match read_result {
                            Ok(content) => {
                                this.open_note_content = content;
                                this.open_note_word_count = count_words(&this.open_note_content);

                                if let Some((pending_path, pending_line)) =
                                    this.pending_open_note_cursor.take()
                                {
                                    if pending_path == note_path {
                                        let offset = byte_offset_for_line(
                                            &this.open_note_content,
                                            pending_line,
                                        );
                                        this.editor_selected_range = offset..offset;
                                        this.editor_selection_reversed = false;
                                        this.editor_preferred_x = None;
                                    } else {
                                        this.pending_open_note_cursor =
                                            Some((pending_path, pending_line));
                                    }
                                }

                                this.status = SharedString::from("Ready");
                            }
                            Err(err) => {
                                this.open_note_content = format!("Failed to load note: {err}");
                                this.open_note_word_count = 0;
                                this.status = SharedString::from("Failed to load note");
                            }
                        }

                        cx.notify();
                    })
                    .ok();
                }
            },
        )
        .detach();
    }

    fn open_note_at_line(&mut self, note_path: String, line: usize, cx: &mut Context<Self>) {
        self.pending_open_note_cursor = Some((note_path.clone(), line.max(1)));
        self.open_note(note_path, cx);
    }

    fn editor_cursor_offset(&self) -> usize {
        if self.editor_selection_reversed {
            self.editor_selected_range.start
        } else {
            self.editor_selected_range.end
        }
    }

    fn editor_move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.min(self.open_note_content.len());
        self.editor_selected_range = offset..offset;
        self.editor_selection_reversed = false;
        cx.notify();
    }

    fn editor_select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.min(self.open_note_content.len());
        if self.editor_selection_reversed {
            self.editor_selected_range.start = offset
        } else {
            self.editor_selected_range.end = offset
        };
        if self.editor_selected_range.end < self.editor_selected_range.start {
            self.editor_selection_reversed = !self.editor_selection_reversed;
            self.editor_selected_range =
                self.editor_selected_range.end..self.editor_selected_range.start;
        }
        cx.notify();
    }

    fn editor_previous_boundary(&self, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }

        let mut prev = 0usize;
        for (ix, _ch) in self.open_note_content.char_indices() {
            if ix >= offset {
                break;
            }
            prev = ix;
        }
        prev
    }

    fn editor_next_boundary(&self, offset: usize) -> usize {
        for (ix, _ch) in self.open_note_content.char_indices() {
            if ix > offset {
                return ix;
            }
        }
        self.open_note_content.len()
    }

    fn editor_line_start(&self, offset: usize) -> usize {
        let offset = offset.min(self.open_note_content.len());
        let prefix = &self.open_note_content[..offset];
        match prefix.rfind('\n') {
            Some(ix) => ix + 1,
            None => 0,
        }
    }

    fn editor_line_end(&self, offset: usize) -> usize {
        let offset = offset.min(self.open_note_content.len());
        let suffix = &self.open_note_content[offset..];
        match suffix.find('\n') {
            Some(rel) => offset + rel,
            None => self.open_note_content.len(),
        }
    }

    fn editor_index_for_point(&self, position: Point<Pixels>) -> Option<usize> {
        let layout = self.editor_layout.as_ref()?;
        match layout.index_for_position(position) {
            Ok(ix) | Err(ix) => Some(ix.min(self.open_note_content.len())),
        }
    }

    fn editor_select_all(&mut self, cx: &mut Context<Self>) {
        self.editor_selected_range = 0..self.open_note_content.len();
        self.editor_selection_reversed = false;
        self.editor_preferred_x = None;
        cx.notify();
    }

    fn editor_copy(&mut self, cx: &mut Context<Self>) {
        if self.editor_selected_range.is_empty() {
            return;
        }
        let Some(text) = self
            .open_note_content
            .get(self.editor_selected_range.clone())
        else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        self.status = SharedString::from("Copied");
        cx.notify();
    }

    fn editor_cut(&mut self, cx: &mut Context<Self>) {
        if self.editor_selected_range.is_empty() {
            return;
        }
        self.editor_copy(cx);
        self.editor_replace_selection("", cx);
    }

    fn editor_paste(&mut self, cx: &mut Context<Self>) {
        let Some(mut text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        if text.contains("\r\n") {
            text = text.replace("\r\n", "\n");
        }
        self.editor_replace_selection(&text, cx);
    }

    fn editor_replace_selection(&mut self, new_text: &str, cx: &mut Context<Self>) {
        if self.open_note_loading || self.open_note_path.is_none() {
            return;
        }

        let range = self
            .editor_marked_range
            .clone()
            .unwrap_or_else(|| self.editor_selected_range.clone());

        if range.is_empty() && new_text.is_empty() {
            return;
        }

        self.open_note_content = self.open_note_content[0..range.start].to_owned()
            + new_text
            + &self.open_note_content[range.end..];
        let cursor = range.start + new_text.len();
        self.editor_selected_range = cursor..cursor;
        self.editor_selection_reversed = false;
        self.editor_marked_range = None;
        self.editor_preferred_x = None;

        self.open_note_dirty = true;
        self.status = SharedString::from("Editing...");
        self.schedule_save_note(Duration::from_millis(500), cx);
        cx.notify();
    }

    fn editor_move_vertical(&mut self, direction: i32, selecting: bool, cx: &mut Context<Self>) {
        let Some(layout) = self.editor_layout.as_ref() else {
            return;
        };

        let cursor = self.editor_cursor_offset();
        let Some(cursor_pos) = layout.position_for_index(cursor) else {
            return;
        };
        let Some(line_height) = layout.line_height() else {
            return;
        };

        let desired_x = self.editor_preferred_x.unwrap_or(cursor_pos.x);
        let target_y = cursor_pos.y + line_height * direction as f32;
        let target = point(desired_x, target_y);
        let new_cursor = match layout.index_for_position(target) {
            Ok(ix) | Err(ix) => ix,
        };

        self.editor_preferred_x = Some(desired_x);
        if selecting {
            self.editor_select_to(new_cursor, cx);
        } else {
            self.editor_move_to(new_cursor, cx);
        }
    }

    fn on_editor_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.open_note_loading || self.open_note_path.is_none() {
            return;
        }

        window.focus(&self.editor_focus_handle);
        self.editor_is_selecting = true;
        self.editor_preferred_x = None;

        let Some(index) = self.editor_index_for_point(event.position) else {
            return;
        };

        if event.modifiers.shift {
            self.editor_select_to(index, cx);
        } else {
            self.editor_move_to(index, cx);
        }
    }

    fn on_editor_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor_is_selecting {
            self.editor_is_selecting = false;
            cx.notify();
        }
    }

    fn on_editor_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editor_is_selecting {
            return;
        }
        let Some(index) = self.editor_index_for_point(event.position) else {
            return;
        };
        self.editor_select_to(index, cx);
    }

    fn on_editor_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
        let shift = ev.keystroke.modifiers.shift;
        let alt = ev.keystroke.modifiers.alt;

        let key = ev.keystroke.key.to_lowercase();

        if self.palette_open {
            self.on_palette_key(ev, cx);
            return;
        }
        if self.settings_open {
            if key == "escape" {
                self.settings_open = false;
                self.settings_language_menu_open = false;
                cx.notify();
            }
            return;
        }

        if let Some(command) = self.command_from_event(ev) {
            match command {
                CommandId::CommandPalette
                | CommandId::QuickOpen
                | CommandId::OpenVault
                | CommandId::ReloadVault
                | CommandId::NewNote
                | CommandId::Settings
                | CommandId::ToggleSplit
                | CommandId::SaveFile
                | CommandId::FocusExplorer
                | CommandId::FocusSearch => {
                    self.execute_palette_command(command, cx);
                    return;
                }
            }
        }

        if ctrl {
            match key.as_str() {
                "a" => {
                    self.editor_select_all(cx);
                    return;
                }
                "c" => {
                    self.editor_copy(cx);
                    return;
                }
                "x" => {
                    self.editor_cut(cx);
                    return;
                }
                "v" => {
                    self.editor_paste(cx);
                    return;
                }
                _ => {}
            }
        }

        if alt {
            match key.as_str() {
                "1" => {
                    self.panel_mode = PanelMode::Explorer;
                    cx.notify();
                    return;
                }
                "2" => {
                    self.panel_mode = PanelMode::Search;
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }

        if self.open_note_loading || self.open_note_path.is_none() {
            return;
        }

        match key.as_str() {
            "backspace" => {
                self.editor_preferred_x = None;
                if self.editor_selected_range.is_empty() {
                    let cursor = self.editor_cursor_offset();
                    let start = self.editor_previous_boundary(cursor);
                    self.editor_selected_range = start..cursor;
                    self.editor_selection_reversed = false;
                }
                self.editor_replace_selection("", cx);
            }
            "delete" => {
                self.editor_preferred_x = None;
                if self.editor_selected_range.is_empty() {
                    let cursor = self.editor_cursor_offset();
                    let end = self.editor_next_boundary(cursor);
                    self.editor_selected_range = cursor..end;
                    self.editor_selection_reversed = false;
                }
                self.editor_replace_selection("", cx);
            }
            "left" => {
                self.editor_preferred_x = None;
                let cursor = self.editor_cursor_offset();
                let next = if self.editor_selected_range.is_empty() {
                    self.editor_previous_boundary(cursor)
                } else {
                    self.editor_selected_range.start
                };
                if shift {
                    self.editor_select_to(next, cx);
                } else {
                    self.editor_move_to(next, cx);
                }
            }
            "right" => {
                self.editor_preferred_x = None;
                let cursor = self.editor_cursor_offset();
                let next = if self.editor_selected_range.is_empty() {
                    self.editor_next_boundary(cursor)
                } else {
                    self.editor_selected_range.end
                };
                if shift {
                    self.editor_select_to(next, cx);
                } else {
                    self.editor_move_to(next, cx);
                }
            }
            "up" => self.editor_move_vertical(-1, shift, cx),
            "down" => self.editor_move_vertical(1, shift, cx),
            "home" => {
                self.editor_preferred_x = None;
                let cursor = self.editor_cursor_offset();
                let next = self.editor_line_start(cursor);
                if shift {
                    self.editor_select_to(next, cx);
                } else {
                    self.editor_move_to(next, cx);
                }
            }
            "end" => {
                self.editor_preferred_x = None;
                let cursor = self.editor_cursor_offset();
                let next = self.editor_line_end(cursor);
                if shift {
                    self.editor_select_to(next, cx);
                } else {
                    self.editor_move_to(next, cx);
                }
            }
            "enter" | "return" => self.editor_replace_selection("\n", cx),
            "tab" => self.editor_replace_selection("\t", cx),
            _ => {}
        }
    }

    fn force_save_note(&mut self, cx: &mut Context<Self>) {
        if !self.open_note_dirty {
            return;
        }
        self.schedule_save_note(Duration::from_millis(0), cx);
    }

    fn schedule_save_note(&mut self, delay: Duration, cx: &mut Context<Self>) {
        let Some(vault) = self.vault() else {
            return;
        };
        let Some(note_path) = self.open_note_path.clone() else {
            return;
        };
        if self.open_note_loading || !self.open_note_dirty || self.editor_marked_range.is_some() {
            return;
        }

        self.next_note_save_nonce = self.next_note_save_nonce.wrapping_add(1);
        let save_nonce = self.next_note_save_nonce;
        self.pending_note_save_nonce = save_nonce;
        let open_nonce = self.current_note_open_nonce;

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                let vault = vault.clone();
                let note_path = note_path.clone();
                async move {
                    if delay > Duration::ZERO {
                        Timer::after(delay).await;
                    }

                    let content_to_save = this
                        .update(&mut cx, |this, _cx| {
                            if this.current_note_open_nonce != open_nonce
                                || this.open_note_path.as_deref() != Some(note_path.as_str())
                                || this.pending_note_save_nonce != save_nonce
                                || !this.open_note_dirty
                                || this.editor_marked_range.is_some()
                            {
                                return None;
                            }
                            Some(this.open_note_content.clone())
                        })
                        .ok()
                        .flatten();

                    let Some(content_to_save) = content_to_save else {
                        return;
                    };

                    let save_result: anyhow::Result<()> = cx
                        .background_executor()
                        .spawn({
                            let vault = vault.clone();
                            let note_path = note_path.clone();
                            async move { vault.write_note(&note_path, &content_to_save) }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.current_note_open_nonce != open_nonce
                            || this.open_note_path.as_deref() != Some(note_path.as_str())
                            || this.pending_note_save_nonce != save_nonce
                        {
                            return;
                        }

                        match save_result {
                            Ok(()) => {
                                this.open_note_dirty = false;
                                this.status = SharedString::from("Ready");
                            }
                            Err(err) => {
                                this.status = SharedString::from(format!("Save failed: {err}"))
                            }
                        }

                        cx.notify();
                    })
                    .ok();
                }
            },
        )
        .detach();
    }

    fn set_drag_over(
        &mut self,
        folder: String,
        target_path: String,
        insert_after: bool,
        cx: &mut Context<Self>,
    ) {
        self.drag_over = Some(DragOver {
            folder,
            target_path,
            insert_after,
        });
        cx.notify();
    }

    fn clear_drag_over(&mut self, cx: &mut Context<Self>) {
        if self.drag_over.is_some() {
            self.drag_over = None;
            cx.notify();
        }
    }

    fn handle_drop(
        &mut self,
        dragged: &DraggedNote,
        target_folder: &str,
        target_path: &str,
        cx: &mut Context<Self>,
    ) {
        if target_folder.is_empty() {
            return;
        }
        if dragged.folder != target_folder {
            return;
        }

        let insert_after = self
            .drag_over
            .as_ref()
            .filter(|d| d.folder == target_folder && d.target_path == target_path)
            .map(|d| d.insert_after)
            .unwrap_or(false);

        if self.reorder_folder(target_folder, &dragged.path, target_path, insert_after) {
            self.schedule_save_folder_order(target_folder, cx);
        }

        self.clear_drag_over(cx);
    }

    fn reorder_folder(
        &mut self,
        folder: &str,
        dragged_path: &str,
        target_path: &str,
        insert_after: bool,
    ) -> bool {
        let Some(order) = self.folder_notes.get_mut(folder) else {
            return false;
        };

        let Some(from_ix) = order.iter().position(|p| p == dragged_path) else {
            return false;
        };
        let Some(mut to_ix) = order.iter().position(|p| p == target_path) else {
            return false;
        };

        if dragged_path == target_path {
            return false;
        }

        let moved = order.remove(from_ix);
        if from_ix < to_ix {
            to_ix = to_ix.saturating_sub(1);
        }
        if insert_after {
            to_ix = to_ix.saturating_add(1);
        }
        if to_ix > order.len() {
            to_ix = order.len();
        }
        order.insert(to_ix, moved);

        self.rebuild_explorer_rows();
        true
    }

    fn schedule_save_folder_order(&mut self, folder: &str, cx: &mut Context<Self>) {
        let Some(vault) = self.vault() else {
            return;
        };
        if folder.is_empty() {
            return;
        }

        self.next_order_nonce = self.next_order_nonce.wrapping_add(1);
        let nonce = self.next_order_nonce;
        let folder = folder.to_string();
        self.pending_order_nonce_by_folder
            .insert(folder.clone(), nonce);

        self.status = SharedString::from(format!("Saving order: {folder}/"));
        cx.notify();

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                let vault = vault.clone();
                let folder = folder.clone();
                async move {
                    Timer::after(Duration::from_millis(250)).await;

                    let order_to_save = this
                        .update(&mut cx, |this, _cx| {
                            match this.pending_order_nonce_by_folder.get(&folder) {
                                Some(n) if *n == nonce => this.folder_notes.get(&folder).cloned(),
                                _ => None,
                            }
                        })
                        .ok()
                        .flatten();

                    let Some(order_to_save) = order_to_save else {
                        return;
                    };

                    let save_result = cx
                        .background_executor()
                        .spawn({
                            let vault = vault.clone();
                            let folder = folder.clone();
                            async move { vault.save_folder_order(&folder, &order_to_save) }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        match save_result {
                            Ok(()) => {
                                this.status = SharedString::from(format!("Saved order: {folder}/"))
                            }
                            Err(err) => {
                                this.status =
                                    SharedString::from(format!("Failed to save order: {err}"))
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                }
            },
        )
        .detach();
    }
}

fn offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0usize;
    let mut utf16_count = 0usize;

    for ch in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }

    utf8_offset.min(text.len())
}

fn offset_to_utf16(text: &str, offset: usize) -> usize {
    let mut utf16_offset = 0usize;
    let mut utf8_count = 0usize;

    for ch in text.chars() {
        if utf8_count >= offset {
            break;
        }
        utf8_count += ch.len_utf8();
        utf16_offset += ch.len_utf16();
    }

    utf16_offset
}

fn range_to_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    offset_to_utf16(text, range.start)..offset_to_utf16(text, range.end)
}

fn range_from_utf16(text: &str, range_utf16: &Range<usize>) -> Range<usize> {
    offset_from_utf16(text, range_utf16.start)..offset_from_utf16(text, range_utf16.end)
}

#[derive(Default, Clone)]
struct NoteEditorLayout(Rc<RefCell<Option<NoteEditorLayoutInner>>>);

struct NoteEditorLayoutInner {
    lines: Vec<gpui::WrappedLine>,
    line_height: Pixels,
    wrap_width: Option<Pixels>,
    size: Option<Size<Pixels>>,
    bounds: Option<Bounds<Pixels>>,
}

impl NoteEditorLayout {
    fn layout(&self, view: Entity<XnoteWindow>, window: &mut Window, _cx: &mut App) -> LayoutId {
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = text_style
            .line_height
            .to_pixels(font_size.into(), window.rem_size());

        let mut style = Style::default();
        style.size.width = relative(1.).into();

        window.request_measured_layout(style, {
            let element_state = self.clone();
            move |known_dimensions, available_space, window, cx| {
                let wrap_width = known_dimensions.width.or(match available_space.width {
                    AvailableSpace::Definite(x) => Some(x),
                    _ => None,
                });

                if let Some(inner) = element_state.0.borrow().as_ref() {
                    if inner.size.is_some()
                        && (wrap_width.is_none() || wrap_width == inner.wrap_width)
                    {
                        return inner.size.unwrap();
                    }
                }

                let view = view.read(cx);
                let text = SharedString::from(view.open_note_content.clone());
                let len = text.len();
                let selection = view.editor_selected_range.clone();
                let marked = view.editor_marked_range.clone();

                let base = TextRun {
                    len,
                    font: text_style.font(),
                    color: text_style.color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };

                let mut boundaries = vec![0usize, len];
                if !selection.is_empty() {
                    boundaries.push(selection.start.min(len));
                    boundaries.push(selection.end.min(len));
                }
                if let Some(marked) = marked.as_ref() {
                    boundaries.push(marked.start.min(len));
                    boundaries.push(marked.end.min(len));
                }
                boundaries.sort_unstable();
                boundaries.dedup();

                let mut runs = Vec::new();
                for w in boundaries.windows(2) {
                    let start = w[0];
                    let end = w[1];
                    if end <= start {
                        continue;
                    }

                    let in_selection =
                        !selection.is_empty() && start >= selection.start && end <= selection.end;
                    let in_marked = marked
                        .as_ref()
                        .is_some_and(|m| start >= m.start && end <= m.end);

                    let mut run = base.clone();
                    run.len = end - start;
                    if in_selection {
                        run.background_color = Some(rgba(0x332563eb).into());
                    }
                    if in_marked {
                        run.underline = Some(UnderlineStyle {
                            color: Some(run.color),
                            thickness: px(1.0),
                            wavy: false,
                        });
                    }

                    runs.push(run);
                }

                let lines = match window
                    .text_system()
                    .shape_text(text, font_size, &runs, wrap_width, None)
                {
                    Ok(lines) => lines.into_iter().collect::<Vec<_>>(),
                    Err(_) => Vec::new(),
                };

                let mut size: Size<Pixels> = Size::default();
                for line in &lines {
                    let line_size = line.size(line_height);
                    size.height += line_size.height;
                    size.width = size.width.max(line_size.width).ceil();
                }
                if let Some(wrap_width) = wrap_width {
                    size.width = wrap_width;
                }

                element_state.0.borrow_mut().replace(NoteEditorLayoutInner {
                    lines,
                    line_height,
                    wrap_width,
                    size: Some(size),
                    bounds: None,
                });

                size
            }
        })
    }

    fn prepaint(&self, bounds: Bounds<Pixels>) {
        if let Some(inner) = self.0.borrow_mut().as_mut() {
            inner.bounds = Some(bounds);
        }
    }

    fn line_height(&self) -> Option<Pixels> {
        self.0.borrow().as_ref().map(|inner| inner.line_height)
    }

    fn position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        let inner = self.0.borrow();
        let inner = inner.as_ref()?;
        let bounds = inner.bounds?;
        let line_height = inner.line_height;

        let mut line_origin = bounds.origin;
        let mut line_start_ix = 0usize;

        for line in &inner.lines {
            let line_end_ix = line_start_ix + line.len();
            if index < line_start_ix {
                break;
            } else if index > line_end_ix {
                line_origin.y += line.size(line_height).height;
                line_start_ix = line_end_ix + 1;
                continue;
            }

            let ix_within_line = index - line_start_ix;
            let pos = line.position_for_index(ix_within_line, line_height)?;
            return Some(line_origin + pos);
        }

        None
    }

    fn index_for_position(&self, position: Point<Pixels>) -> Result<usize, usize> {
        let inner = self.0.borrow();
        let Some(inner) = inner.as_ref() else {
            return Err(0);
        };
        let Some(bounds) = inner.bounds else {
            return Err(0);
        };

        if position.y < bounds.top() {
            return Err(0);
        }

        let line_height = inner.line_height;
        let mut line_origin = bounds.origin;
        let mut line_start_ix = 0usize;

        for line in &inner.lines {
            let line_bottom = line_origin.y + line.size(line_height).height;
            if position.y > line_bottom {
                line_origin.y = line_bottom;
                line_start_ix += line.len() + 1;
                continue;
            }

            let position_within_line = position - line_origin;
            return match line.index_for_position(position_within_line, line_height) {
                Ok(ix) => Ok(line_start_ix + ix),
                Err(ix) => Err(line_start_ix + ix),
            };
        }

        Err(line_start_ix.saturating_sub(1))
    }

    fn bounds_for_byte_range(&self, range: Range<usize>) -> Option<Bounds<Pixels>> {
        let inner = self.0.borrow();
        let inner = inner.as_ref()?;
        let bounds = inner.bounds?;
        let line_height = inner.line_height;

        let start_pos = self
            .position_for_index(range.start)
            .unwrap_or(bounds.origin);
        let end_pos = self.position_for_index(range.end).unwrap_or(start_pos);
        let left = start_pos.x.min(end_pos.x);
        let right = start_pos.x.max(end_pos.x);
        let top = start_pos.y.min(end_pos.y);
        let bottom = start_pos.y.max(end_pos.y) + line_height;
        Some(Bounds::from_corners(
            point(left, top),
            point(right + px(2.0), bottom),
        ))
    }
}

struct NoteEditorElement {
    view: Entity<XnoteWindow>,
}

impl IntoElement for NoteEditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for NoteEditorElement {
    type RequestLayoutState = NoteEditorLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout = NoteEditorLayout::default();
        let layout_id = layout.layout(self.view.clone(), window, cx);
        (layout_id, layout)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        layout.prepaint(bounds);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.view.read(cx).editor_focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );

        if let Some(inner) = layout.0.borrow().as_ref() {
            let line_height = inner.line_height;
            let text_style = window.text_style();
            let mut line_origin = bounds.origin;
            for line in &inner.lines {
                let _ = line.paint_background(
                    line_origin,
                    line_height,
                    text_style.text_align,
                    Some(bounds),
                    window,
                    cx,
                );
                let _ = line.paint(
                    line_origin,
                    line_height,
                    text_style.text_align,
                    Some(bounds),
                    window,
                    cx,
                );
                line_origin.y += line.size(line_height).height;
            }
        }

        let (selected_range, selection_reversed) = {
            let view = self.view.read(cx);
            (
                view.editor_selected_range.clone(),
                view.editor_selection_reversed,
            )
        };

        if focus_handle.is_focused(window) && selected_range.is_empty() {
            let cursor = if selection_reversed {
                selected_range.start
            } else {
                selected_range.end
            };

            if let Some(cursor_pos) = layout.position_for_index(cursor) {
                let cursor_quad: PaintQuad = fill(
                    Bounds::new(
                        point(cursor_pos.x, cursor_pos.y),
                        size(px(2.0), layout.line_height().unwrap_or(px(16.0))),
                    ),
                    rgb(0x2563eb),
                );
                window.paint_quad(cursor_quad);
            }
        }

        self.view.update(cx, |view, _cx| {
            view.editor_layout = Some(layout.clone());
        });
    }
}

impl EntityInputHandler for XnoteWindow {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        if self.open_note_loading || self.open_note_path.is_none() {
            return None;
        }

        let range = range_from_utf16(&self.open_note_content, &range_utf16);
        let range = range.start.min(self.open_note_content.len())
            ..range.end.min(self.open_note_content.len());
        actual_range.replace(range_to_utf16(&self.open_note_content, &range));
        Some(self.open_note_content.get(range)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if self.open_note_loading || self.open_note_path.is_none() {
            return None;
        }

        Some(UTF16Selection {
            range: range_to_utf16(&self.open_note_content, &self.editor_selected_range),
            reversed: self.editor_selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.editor_marked_range
            .as_ref()
            .map(|range| range_to_utf16(&self.open_note_content, range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.editor_marked_range.take().is_some() {
            self.schedule_save_note(Duration::from_millis(500), cx);
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.open_note_loading || self.open_note_path.is_none() {
            return;
        }

        let mut new_text = new_text;
        let normalized;
        if new_text.contains("\r\n") {
            normalized = new_text.replace("\r\n", "\n");
            new_text = &normalized;
        }

        let range = range_utf16
            .as_ref()
            .map(|range_utf16| range_from_utf16(&self.open_note_content, range_utf16))
            .or(self.editor_marked_range.clone())
            .unwrap_or_else(|| self.editor_selected_range.clone());

        self.open_note_content = self.open_note_content[0..range.start].to_owned()
            + new_text
            + &self.open_note_content[range.end..];
        let cursor = range.start + new_text.len();
        self.editor_selected_range = cursor..cursor;
        self.editor_selection_reversed = false;
        self.editor_marked_range = None;
        self.editor_preferred_x = None;

        self.open_note_dirty = true;
        self.status = SharedString::from("Editing...");
        self.schedule_save_note(Duration::from_millis(500), cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.open_note_loading || self.open_note_path.is_none() {
            return;
        }

        let range = range_utf16
            .as_ref()
            .map(|range_utf16| range_from_utf16(&self.open_note_content, range_utf16))
            .or(self.editor_marked_range.clone())
            .unwrap_or_else(|| self.editor_selected_range.clone());

        self.open_note_content = self.open_note_content[0..range.start].to_owned()
            + new_text
            + &self.open_note_content[range.end..];

        self.editor_marked_range = if new_text.is_empty() {
            None
        } else {
            Some(range.start..range.start + new_text.len())
        };

        let selected = if let Some(sel_utf16) = new_selected_range_utf16.as_ref() {
            let rel = range_from_utf16(new_text, sel_utf16);
            (range.start + rel.start)..(range.start + rel.end)
        } else {
            let cursor = range.start + new_text.len();
            cursor..cursor
        };

        self.editor_selected_range = selected;
        self.editor_selection_reversed = false;
        self.editor_preferred_x = None;

        self.open_note_dirty = true;
        self.status = SharedString::from("Editing...");
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if self.open_note_loading || self.open_note_path.is_none() {
            return None;
        }
        let layout = self.editor_layout.as_ref()?;
        let range = range_from_utf16(&self.open_note_content, &range_utf16);
        layout.bounds_for_byte_range(range)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        if self.open_note_loading || self.open_note_path.is_none() {
            return None;
        }
        let layout = self.editor_layout.as_ref()?;
        let utf8_index = match layout.index_for_position(point) {
            Ok(ix) | Err(ix) => ix,
        };
        Some(offset_to_utf16(&self.open_note_content, utf8_index))
    }
}

struct DragPreview {
    label: SharedString,
}

impl Render for DragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(0x111827))
            .text_color(rgb(0xffffff))
            .child(self.label.clone())
    }
}

impl Render for XnoteWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.vault_prompt_open && self.vault_prompt_needs_focus {
            window.focus(&self.vault_prompt_focus_handle);
            self.vault_prompt_needs_focus = false;
        }

        let (_window_title, _workspace_hint) = match &self.vault_state {
            VaultState::Opened { root_name, .. } => (
                SharedString::from("XNote"),
                SharedString::from(format!("Workspace: {root_name}")),
            ),
            VaultState::Opening { path } => (
                SharedString::from("XNote"),
                SharedString::from(format!("Opening: {}", path.display())),
            ),
            VaultState::Error { message } => (
                SharedString::from("XNote"),
                SharedString::from(format!("Vault error: {message}")),
            ),
            VaultState::NotConfigured => (
                SharedString::from("XNote"),
                SharedString::from("No vault configured (set XNOTE_VAULT or use --vault <path>)"),
            ),
        };

        let _scan_hint = match &self.scan_state {
            ScanState::Idle => SharedString::from(""),
            ScanState::Scanning => SharedString::from("Scanning..."),
            ScanState::Ready {
                note_count,
                duration_ms,
            } => SharedString::from(format!("{note_count} notes ({duration_ms} ms)")),
            ScanState::Error { message } => SharedString::from(message.to_string()),
        };

        let rail_button =
            |id: &'static str,
             icon: &'static str,
             icon_color: u32,
             active: bool,
             on_click: fn(&mut XnoteWindow, &mut Context<XnoteWindow>)| {
                div()
                    .id(id)
                    .h(px(40.))
                    .w_full()
                    .flex()
                    .items_center()
                    .bg(if active {
                        rgb(0xe1e5ea)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|this| this.bg(rgb(0xe1e5ea)))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        on_click(this, cx);
                    }))
                    .child(div().w(px(3.)).h_full().bg(if active {
                        rgb(0xb6edf5)
                    } else {
                        rgba(0x00000000)
                    }))
                    .child(
                        div()
                            .w_full()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(ui_icon(icon, 20., icon_color)),
                    )
            };

        let rail_top = div()
            .flex()
            .flex_col()
            .child(rail_button(
                "rail.explorer",
                ICON_FOLDER,
                0x111827,
                self.panel_mode == PanelMode::Explorer,
                |this, cx| {
                    this.panel_mode = PanelMode::Explorer;
                    this.show_panel_shell(cx);
                    cx.notify();
                },
            ))
            .child(rail_button(
                "rail.search",
                ICON_SEARCH,
                0x374151,
                self.panel_mode == PanelMode::Search,
                |this, cx| {
                    this.panel_mode = PanelMode::Search;
                    this.show_panel_shell(cx);
                    cx.notify();
                },
            ));

        let rail_bottom = div()
            .flex()
            .flex_col()
            .justify_end()
            .h(px(64.))
            .child(rail_button(
                "rail.settings",
                ICON_SETTINGS,
                0x6b7280,
                false,
                |this, cx| {
                    this.settings_open = true;
                    this.settings_language_menu_open = false;
                    cx.notify();
                },
            ));

        let rail = div()
            .w(px(48.))
            .min_w(px(48.))
            .max_w(px(48.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(0xeef0f2))
            .border_r_1()
            .border_color(rgb(0xc8cdd5))
            .flex()
            .flex_col()
            .justify_between()
            .child(rail_top)
            .child(rail_bottom);

        let rail_w = px(48.);
        let splitter_w = px(6.);
        let editor_min_w = px(320.);
        let panel_min_w = px(180.);
        let workspace_min_w = px(220.);

        let window_w = window.bounds().size.width;
        let (panel_shell_state, workspace_state) = self.effective_sidebar_layout(window_w);
        let panel_shell_present = panel_shell_state != SidebarState::Hidden;
        let workspace_present = workspace_state != SidebarState::Hidden;

        let splitter_count = (panel_shell_present as usize) + (workspace_present as usize);
        let reserve = rail_w + editor_min_w + splitter_w * splitter_count;
        let available_for_expanded = if window_w > reserve {
            window_w - reserve
        } else {
            px(0.)
        };

        let mut panel_shell_expanded_w = self.panel_shell_width.max(panel_min_w);
        let mut workspace_expanded_w = self.workspace_width.max(workspace_min_w);

        match (panel_shell_state, workspace_state) {
            (SidebarState::Expanded, SidebarState::Expanded) => {
                let max_panel = (available_for_expanded - workspace_min_w).max(panel_min_w);
                panel_shell_expanded_w = panel_shell_expanded_w.clamp(panel_min_w, max_panel);
                let max_workspace =
                    (available_for_expanded - panel_shell_expanded_w).max(workspace_min_w);
                workspace_expanded_w = workspace_expanded_w.clamp(workspace_min_w, max_workspace);
                let max_panel = (available_for_expanded - workspace_expanded_w).max(panel_min_w);
                panel_shell_expanded_w = panel_shell_expanded_w.clamp(panel_min_w, max_panel);
            }
            (SidebarState::Expanded, _) => {
                let max_panel = available_for_expanded.max(panel_min_w);
                panel_shell_expanded_w = panel_shell_expanded_w.clamp(panel_min_w, max_panel);
            }
            (_, SidebarState::Expanded) => {
                let max_workspace = available_for_expanded.max(workspace_min_w);
                workspace_expanded_w = workspace_expanded_w.clamp(workspace_min_w, max_workspace);
            }
            _ => {}
        }

        let panel_shell_width = match panel_shell_state {
            SidebarState::Expanded => panel_shell_expanded_w,
            SidebarState::Hidden => px(0.),
        };
        let workspace_width = match workspace_state {
            SidebarState::Expanded => workspace_expanded_w,
            SidebarState::Hidden => px(0.),
        };

        if panel_shell_state == SidebarState::Expanded
            && self.panel_shell_width != panel_shell_width
        {
            self.panel_shell_width = panel_shell_width;
            self.panel_shell_saved_width = panel_shell_width;
        }
        if workspace_state == SidebarState::Expanded && self.workspace_width != workspace_width {
            self.workspace_width = workspace_width;
            self.workspace_saved_width = workspace_width;
        }

        let explorer_panel =
            div()
                .w(panel_shell_width)
                .min_w(panel_shell_width)
                .max_w(panel_shell_width)
                .flex_shrink_0()
                .h_full()
                .bg(rgb(0xf2f4f7))
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(28.))
                        .px_3()
                        .flex()
                        .items_center()
                        .bg(rgb(0xeef0f2))
                        .gap(px(10.))
                        .child(
                            div()
                                .font_family("IBM Plex Mono")
                                .text_size(px(10.))
                                .font_weight(FontWeight(900.))
                                .text_color(rgb(0x374151))
                                .child("EXPLORER"),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .id("panel_shell.collapse.explorer")
                                .h(px(24.))
                                .w(px(24.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|this| this.bg(rgb(0xe1e5ea)))
                                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                                    this.set_panel_shell_collapsed(true, cx);
                                }))
                                .child(ui_icon(ICON_PANEL_LEFT_CLOSE, 16., 0x6b7280)),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .id("explorer.new_file")
                                        .h(px(24.))
                                        .w(px(24.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .hover(|this| this.bg(rgb(0xe1e5ea)))
                                        .on_click(cx.listener(
                                            |this, _ev: &ClickEvent, _window, cx| {
                                                this.create_new_note(cx);
                                            },
                                        ))
                                        .child(ui_icon(ICON_FILE_PLUS, 16., 0x6b7280)),
                                )
                                .child(
                                    div()
                                        .id("explorer.new_folder")
                                        .h(px(24.))
                                        .w(px(24.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .hover(|this| this.bg(rgb(0xe1e5ea)))
                                        .on_click(cx.listener(
                                            |this, _ev: &ClickEvent, _window, cx| {
                                                this.create_new_folder(cx);
                                            },
                                        ))
                                        .child(ui_icon(ICON_FOLDER_PLUS, 16., 0x6b7280)),
                                )
                                .child(
                                    div()
                                        .id("explorer.refresh")
                                        .h(px(24.))
                                        .w(px(24.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .hover(|this| this.bg(rgb(0xe1e5ea)))
                                        .on_click(cx.listener(
                                            |this, _ev: &ClickEvent, _window, cx| {
                                                this.rescan_vault(cx);
                                            },
                                        ))
                                        .child(ui_icon(ICON_REFRESH_CW, 16., 0x6b7280)),
                                ),
                        ),
                )
                .child(div().h(px(1.)).w_full().bg(rgb(0xc8cdd5)))
                .child(
                    div()
                        .id("explorer.filter")
                        .h(px(36.))
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .bg(if self.is_filtering() {
                            rgb(0xe6f7fa)
                        } else {
                            rgb(0xf2f4f7)
                        })
                        .focusable()
                        .cursor_pointer()
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                            this.on_filter_key(ev, cx);
                        }))
                        .child(ui_icon(ICON_FUNNEL, 14., 0x6b7280))
                        .child(
                            div()
                                .font_family("IBM Plex Mono")
                                .text_size(px(11.))
                                .font_weight(FontWeight(if self.is_filtering() {
                                    900.
                                } else {
                                    650.
                                }))
                                .text_color(if self.is_filtering() {
                                    rgb(0x0b4b57)
                                } else {
                                    rgb(0x6b7280)
                                })
                                .child(SharedString::from(if self.explorer_filter.is_empty() {
                                    "Filter files…".to_string()
                                } else {
                                    format!("Filter: {}", self.explorer_filter)
                                })),
                        ),
                )
                .child(div().h(px(2.)).w_full().bg(if self.is_filtering() {
                    rgb(0x2563eb)
                } else {
                    rgb(0xb6edf5)
                }))
                .child(
                    div()
                        .id("explorer.list")
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .bg(rgb(0xf2f4f7))
                        .py(px(10.))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _ev, _window, cx| this.clear_drag_over(cx)),
                        )
                        .child(
                            uniform_list(
                                "explorer",
                                if self.is_filtering() {
                                    self.explorer_rows_filtered.len()
                                } else {
                                    self.explorer_rows.len()
                                },
                                cx.processor(|this, range: std::ops::Range<usize>, _window, cx| {
                                    if this.is_filtering() {
                                        let matches = &this.explorer_rows_filtered;
                                        return range
                                            .map(|ix| {
                                                let Some(note_ix) = matches.get(ix) else {
                                                    return div()
                                                        .id(ElementId::named_usize(
                                                            "explorer.filtered.missing",
                                                            ix,
                                                        ))
                                                        .px_3()
                                                        .py_2()
                                                        .child("");
                                                };
                                                let Some(path) =
                                                    this.explorer_all_note_paths.get(*note_ix)
                                                else {
                                                    return div()
                                                        .id(ElementId::named_usize(
                                                            "explorer.filtered.missing",
                                                            ix,
                                                        ))
                                                        .px_3()
                                                        .py_2()
                                                        .child("");
                                                };

                                                let is_selected = this.selected_note.as_deref()
                                                    == Some(path.as_str());
                                                let selected_path = path.clone();
                                                let display_name = path.clone();

                                                div()
                        .id(ElementId::Name(SharedString::from(format!("note:{path}"))))
                        .h(px(22.))
                        .w_full()
                        .px_1()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .overflow_hidden()
                        .cursor_pointer()
                        .when(is_selected, |this| this.bg(rgb(0xe6f7fa)))
                        .when(!is_selected, |this| this.hover(|this| this.bg(rgb(0xe1e5ea))))
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                          this.open_note(selected_path.clone(), cx);
                        }))
                        .child(ui_icon(
                          ICON_FILE_TEXT,
                          14.,
                          if is_selected { 0x0b4b57 } else { 0x6b7280 },
                        ))
                        .child(
                          div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(if is_selected { 900. } else { 750. }))
                            .text_color(rgb(if is_selected { 0x0b4b57 } else { 0x111827 }))
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(display_name),
                        )
                                            })
                                            .collect::<Vec<_>>();
                                    }

                                    let is_filtering = false;
                                    let rows: &[ExplorerRow] = &this.explorer_rows;
                                    range
                                        .map(|ix| match rows.get(ix) {
                                            Some(ExplorerRow::Vault {
                                                root_name,
                                                expanded,
                                            }) => {
                                                if is_filtering {
                                                    return div()
                                                        .id(ElementId::named_usize(
                                                            "explorer.vault.hidden",
                                                            ix,
                                                        ))
                                                        .px_3()
                                                        .py_2()
                                                        .child("");
                                                }

                                                let chevron = if *expanded {
                                                    ICON_CHEVRON_DOWN
                                                } else {
                                                    ICON_CHEVRON_RIGHT
                                                };
                                                let folder = String::new();
                                                div()
                        .id(ElementId::Name(SharedString::from("explorer.vault")))
                        .h(px(22.))
                        .w_full()
                        .px_1()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .overflow_hidden()
                        .cursor_pointer()
                        .hover(|this| this.bg(rgb(0xe1e5ea)))
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                          match this.vault_state {
                            VaultState::Opened { .. } => this.toggle_folder_expanded(&folder, cx),
                            VaultState::Opening { .. } => {}
                            _ => this.open_vault_prompt(cx),
                          }
                        }))
                        .child(ui_icon(chevron, 14., 0x6b7280))
                        .child(ui_icon(ICON_VAULT, 14., 0x6b7280))
                        .child(
                          div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(900.))
                            .text_color(rgb(0x111827))
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(root_name.clone()),
                        )
                                            }
                                            Some(ExplorerRow::Hint { text }) => div()
                                                .id(ElementId::named_usize("explorer.hint", ix))
                                                .h(px(22.))
                                                .w_full()
                                                .px_1()
                                                .flex()
                                                .items_center()
                                                .gap(px(6.))
                                                .overflow_hidden()
                                                .cursor_pointer()
                                                .hover(|this| this.bg(rgb(0xe1e5ea)))
                                                .on_click(cx.listener(
                                                    |this, _ev: &ClickEvent, _window, cx| {
                                                        this.open_vault_prompt(cx);
                                                    },
                                                ))
                                                .child(ui_icon(ICON_FOLDER_OPEN, 14., 0x6b7280))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .overflow_hidden()
                                                        .font_family("IBM Plex Mono")
                                                        .text_size(px(11.))
                                                        .font_weight(FontWeight(650.))
                                                        .text_color(rgb(0x6b7280))
                                                        .whitespace_nowrap()
                                                        .text_ellipsis()
                                                        .child(text.clone()),
                                                ),
                                            Some(ExplorerRow::Folder {
                                                folder,
                                                name,
                                                depth,
                                                expanded,
                                                has_children,
                                                ..
                                            }) => {
                                                if is_filtering {
                                                    return div()
                                                        .id(ElementId::named_usize(
                                                            "explorer.folder.hidden",
                                                            ix,
                                                        ))
                                                        .px_3()
                                                        .py_2()
                                                        .child("");
                                                }

                                                let chevron = if *expanded {
                                                    ICON_CHEVRON_DOWN
                                                } else {
                                                    ICON_CHEVRON_RIGHT
                                                };
                                                let chevron_icon = if *has_children {
                                                    Some(ui_icon(chevron, 14., 0x6b7280))
                                                } else {
                                                    None
                                                };

                                                let line_mask = depth_line_mask(*depth);
                                                let indent_el =
                                                    tree_indent_guides(*depth, line_mask);
                                                let folder_path = folder.clone();
                                                let show_child_stem = *expanded && *has_children;

                                                div()
                        .id(ElementId::Name(SharedString::from(format!("folder:{folder}"))))
                        .h(px(22.))
                        .w_full()
                        .px_1()
                        .flex()
                        .items_center()
                        .gap(px(0.))
                        .overflow_hidden()
                        .cursor_pointer()
                        .hover(|this| this.bg(rgb(0xe1e5ea)))
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                          this.toggle_folder_expanded(&folder_path, cx);
                        }))
                        .child(indent_el)
                        .child(
                          div()
                            .w(px(14.))
                            .h_full()
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(show_child_stem, |this| {
                              this.child(
                                div()
                                  .absolute()
                                  .left(px(7.))
                                  .top(px(11.))
                                  .bottom_0()
                                  .w(px(1.))
                                  .bg(rgb(0xc8cdd5)),
                              )
                            })
                            .children(chevron_icon),
                        )
                        .child(div().w(px(6.)))
                        .child(ui_icon(ICON_FOLDER, 14., 0x6b7280))
                        .child(div().w(px(6.)))
                        .child(
                          div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(if *expanded { 850. } else { 800. }))
                            .text_color(rgb(if *expanded { 0x111827 } else { 0x374151 }))
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(name.clone()),
                        )
                                            }
                                            Some(ExplorerRow::Note {
                                                folder,
                                                path,
                                                file_name,
                                                depth,
                                                ..
                                            }) => {
                                                let is_selected = this.selected_note.as_deref()
                                                    == Some(path.as_str());
                                                let is_drag_target =
                                                    this.drag_over.as_ref().is_some_and(|d| {
                                                        d.folder == *folder
                                                            && d.target_path == *path
                                                    });

                                                let row_id = ElementId::Name(SharedString::from(
                                                    format!("note:{path}"),
                                                ));
                                                let dragged_value = DraggedNote {
                                                    folder: folder.clone(),
                                                    path: path.clone(),
                                                };

                                                let target_folder = folder.clone();
                                                let target_path = path.clone();
                                                let selected_path = path.clone();
                                                let display_name = if is_filtering {
                                                    path.clone()
                                                } else {
                                                    file_name.clone()
                                                };

                                                let line_mask = depth_line_mask(*depth);
                                                let indent_el =
                                                    tree_indent_guides((*depth).max(1), line_mask);

                                                let icon_color =
                                                    if is_selected { 0x0b4b57 } else { 0x6b7280 };
                                                let text_color =
                                                    if is_selected { 0x0b4b57 } else { 0x111827 };
                                                let text_weight =
                                                    if is_selected { 900. } else { 750. };

                                                let mut row = div()
                        .id(row_id)
                        .h(px(22.))
                        .w_full()
                        .px_1()
                        .flex()
                        .items_center()
                        .gap(px(0.))
                        .overflow_hidden()
                        .cursor_pointer()
                        .when(is_selected, |this| this.bg(rgb(0xe6f7fa)))
                        .when(!is_selected && !is_drag_target, |this| {
                          this.hover(|this| this.bg(rgb(0xe1e5ea)))
                        })
                        .when(is_drag_target, |this| {
                          this
                            .bg(rgb(0xe6f7fa))
                            .border_1()
                            .border_color(rgb(0x2563eb))
                        })
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                          this.open_note(selected_path.clone(), cx);
                        }));

                                                if !is_filtering {
                                                    row = row
                          .on_drag(dragged_value, |dragged, _offset, _window, cx| {
                            cx.new(|_| DragPreview {
                              label: SharedString::from(dragged.path.clone()),
                            })
                          })
                          .can_drop({
                            let target_folder = target_folder.clone();
                            let target_path = target_path.clone();
                            move |dragged, _window, _cx| {
                              dragged.downcast_ref::<DraggedNote>().is_some_and(|d| {
                                !target_folder.is_empty()
                                  && d.folder == target_folder
                                  && d.path != target_path
                              })
                            }
                          })
                          .on_drag_move::<DraggedNote>(cx.listener({
                            let target_folder = target_folder.clone();
                            let target_path = target_path.clone();
                            move |this, ev: &DragMoveEvent<DraggedNote>, _window, cx| {
                              let Some(dragged) = ev.dragged_item().downcast_ref::<DraggedNote>()
                              else {
                                return;
                              };
                              if dragged.folder != target_folder || target_folder.is_empty() {
                                return;
                              }
                              let mid_y = ev.bounds.origin.y + ev.bounds.size.height * 0.5;
                              let insert_after = ev.event.position.y >= mid_y;
                              this.set_drag_over(
                                target_folder.clone(),
                                target_path.clone(),
                                insert_after,
                                cx,
                              );
                            }
                          }))
                          .on_drop::<DraggedNote>(cx.listener({
                            let target_folder = target_folder.clone();
                            let target_path = target_path.clone();
                            move |this, dragged: &DraggedNote, _window, cx| {
                              this.handle_drop(dragged, &target_folder, &target_path, cx);
                            }
                          }));
                                                }

                                                row.child(indent_el)
                                                    .child(div().w(px(6.)))
                                                    .child(ui_icon(ICON_FILE_TEXT, 14., icon_color))
                                                    .child(div().w(px(6.)))
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .overflow_hidden()
                                                            .font_family("IBM Plex Mono")
                                                            .text_size(px(11.))
                                                            .font_weight(FontWeight(text_weight))
                                                            .text_color(rgb(text_color))
                                                            .whitespace_nowrap()
                                                            .text_ellipsis()
                                                            .child(display_name),
                                                    )
                                            }
                                            None => div()
                                                .id(ElementId::named_usize("explorer.missing", ix))
                                                .px_3()
                                                .py_2()
                                                .child(""),
                                        })
                                        .collect::<Vec<_>>()
                                }),
                            )
                            .h_full(),
                        ),
                );

        let search_vault_label = match &self.vault_state {
            VaultState::Opened { root_name, .. } => root_name.clone(),
            _ => SharedString::from("None"),
        };

        let search_panel = div()
      .w(panel_shell_width)
      .min_w(panel_shell_width)
      .max_w(panel_shell_width)
      .flex_shrink_0()
      .h_full()
      .bg(rgb(0xf2f4f7))
      .flex()
      .flex_col()
      .child(
        div()
          .h(px(28.))
          .px_3()
          .flex()
          .items_center()
          .bg(rgb(0xeef0f2))
          .gap(px(10.))
          .child(
            div()
              .font_family("IBM Plex Mono")
              .text_size(px(10.))
              .font_weight(FontWeight(900.))
              .text_color(rgb(0x374151))
              .child("SEARCH"),
          )
          .child(div().flex_1())
          .child(
            div()
              .id("panel_shell.collapse.search")
              .h(px(24.))
              .w(px(24.))
              .flex()
              .items_center()
              .justify_center()
              .cursor_pointer()
              .hover(|this| this.bg(rgb(0xe1e5ea)))
              .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                this.set_panel_shell_collapsed(true, cx);
              }))
              .child(ui_icon(ICON_PANEL_LEFT_CLOSE, 16., 0x6b7280)),
          )
      )
      .child(div().h(px(1.)).w_full().bg(rgb(0xc8cdd5)))
      .child(
        div()
          .id("search.input")
          .h(px(36.))
          .px_3()
          .flex()
          .items_center()
          .gap_2()
          .bg(if self.search_query.trim().is_empty() {
            rgb(0xf2f4f7)
          } else {
            rgb(0xe6f7fa)
          })
          .focusable()
          .cursor_pointer()
          .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
            this.on_search_key(ev, cx);
          }))
          .child(ui_icon(ICON_SEARCH, 14., 0x6b7280))
          .child(
            div()
              .font_family("IBM Plex Mono")
              .text_size(px(11.))
              .font_weight(FontWeight(if self.search_query.trim().is_empty() {
                650.
              } else {
                900.
              }))
              .text_color(if self.search_query.trim().is_empty() {
                rgb(0x6b7280)
              } else {
                rgb(0x0b4b57)
              })
              .child(SharedString::from(if self.search_query.trim().is_empty() {
                "Search…".to_string()
              } else {
                format!("Search: {}", self.search_query.trim())
              })),
          ),
      )
      .child(div().h(px(2.)).w_full().bg(if self.search_query.trim().is_empty() {
        rgb(0xb6edf5)
      } else {
        rgb(0x2563eb)
      }))
      .child(
        div()
          .id("search.results")
          .flex_1()
          .min_h_0()
          .w_full()
          .bg(rgb(0xf2f4f7))
          .flex()
          .flex_col()
          .child(
            div()
              .px_3()
              .pt(px(10.))
              .flex()
              .flex_col()
              .gap(px(6.))
              .child(
                div()
                  .font_family("IBM Plex Mono")
                  .text_size(px(11.))
                  .font_weight(FontWeight(800.))
                  .text_color(rgb(0x374151))
                  .child(search_vault_label.clone()),
              )
              .child(
                div()
                  .font_family("IBM Plex Mono")
                  .text_size(px(10.))
                  .font_weight(FontWeight(900.))
                  .text_color(rgb(0x6b7280))
                  .child("RESULTS"),
              ),
          )
          .child(
            div()
              .id("search.results.scroll")
              .flex_1()
              .overflow_y_scroll()
              .p(px(10.))
              .child(
                uniform_list(
                  "search.results",
                  self.search_results.len().max(1),
                  cx.processor(|this, range: std::ops::Range<usize>, _window, list_cx| {
                    if this.search_query.trim().is_empty() {
                      return range
                        .map(|ix| {
                          div()
                            .id(ElementId::named_usize("search.placeholder", ix))
                            .h(px(22.))
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(0x6b7280))
                            .child(if ix == 0 { "Type to search…" } else { "" })
                        })
                        .collect::<Vec<_>>();
                    }

                    if this.search_results.is_empty() {
                      return range
                        .map(|ix| {
                          div()
                            .id(ElementId::named_usize("search.empty", ix))
                            .h(px(22.))
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(0x6b7280))
                            .child(if ix == 0 { "No matches" } else { "" })
                        })
                        .collect::<Vec<_>>();
                    }

                    range
                      .map(|ix| {
                        let Some(row) = this.search_results.get(ix) else {
                          return div()
                            .id(ElementId::named_usize("search.missing", ix))
                            .h(px(22.))
                            .child("");
                        };

                        let selected = ix == this.search_selected;

                        match row {
                          SearchRow::File { path, match_count } => {
                            let path_for_click = path.clone();
                            let label = SharedString::from(format!("{path} ({match_count})"));

                            div()
                              .id(ElementId::Name(SharedString::from(format!(
                                "search.file:{path}"
                              ))))
                              .h(px(22.))
                              .w_full()
                              .flex()
                              .items_center()
                              .cursor_pointer()
                              .bg(if selected { rgb(0xe6f7fa) } else { rgba(0x00000000) })
                              .when(!selected, |this| this.hover(|this| this.bg(rgb(0xe1e5ea))))
                              .on_click(list_cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                                this.search_selected = ix;
                                this.open_note(path_for_click.clone(), cx);
                              }))
                              .child(
                                div()
                                  .min_w_0()
                                  .flex_1()
                                  .overflow_hidden()
                                  .font_family("IBM Plex Mono")
                                  .text_size(px(11.))
                                  .font_weight(FontWeight(900.))
                                  .text_color(rgb(0x111827))
                                  .whitespace_nowrap()
                                  .text_ellipsis()
                                  .child(label),
                              )
                          }
                          SearchRow::Match { path, line, preview } => {
                            let path_for_click = path.clone();
                            let line_for_click = *line;
                            let label = SharedString::from(format!("{line}: {preview}"));

                            div()
                              .id(ElementId::Name(SharedString::from(format!(
                                "search.match:{path}:{line}"
                              ))))
                              .h(px(22.))
                              .w_full()
                              .flex()
                              .items_center()
                              .cursor_pointer()
                              .bg(if selected { rgb(0xe6f7fa) } else { rgba(0x00000000) })
                              .when(!selected, |this| this.hover(|this| this.bg(rgb(0xe1e5ea))))
                              .on_click(list_cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                                this.search_selected = ix;
                                this.open_note_at_line(path_for_click.clone(), line_for_click, cx);
                              }))
                              .child(
                                div()
                                  .min_w_0()
                                  .flex_1()
                                  .overflow_hidden()
                                  .font_family("IBM Plex Mono")
                                  .text_size(px(10.))
                                  .font_weight(FontWeight(650.))
                                  .text_color(rgb(0x374151))
                                  .whitespace_nowrap()
                                  .text_ellipsis()
                                  .child(label),
                              )
                          }
                        }
                      })
                      .collect::<Vec<_>>()
                  }),
                )
                .h_full(),
              ),
          ),
      );

        let panel_shell = match self.panel_mode {
            PanelMode::Explorer => explorer_panel.into_any_element(),
            PanelMode::Search => search_panel.into_any_element(),
        };

        let workspace_mode_button = |id: &'static str, icon: &'static str, mode: WorkspaceMode| {
            let active = self.workspace_mode == mode;
            div()
                .id(id)
                .h(px(24.))
                .w(px(24.))
                .border_1()
                .border_color(rgb(0xc8cdd5))
                .bg(if active { rgb(0xe1e5ea) } else { rgb(0xeef0f2) })
                .cursor_pointer()
                .hover(|this| this.bg(rgb(0xe1e5ea)))
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                    this.workspace_mode = mode;
                    cx.notify();
                }))
                .child(ui_icon(icon, 16., if active { 0x0b4b57 } else { 0x6b7280 }))
        };

        let mode_bar = div()
            .h(px(28.))
            .w_full()
            .bg(rgb(0xeef0f2))
            .px_2()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(workspace_mode_button(
                "workspace.mode.open",
                ICON_FILE_TEXT,
                WorkspaceMode::OpenEditors,
            ))
            .child(workspace_mode_button(
                "workspace.mode.refs",
                ICON_LINK_2,
                WorkspaceMode::References,
            ))
            .child(workspace_mode_button(
                "workspace.mode.bookmarks",
                ICON_BOOKMARK,
                WorkspaceMode::Bookmarks,
            ))
            .child(div().flex_1())
            .child(
                div()
                    .id("workspace.collapse")
                    .h(px(24.))
                    .w(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|this| this.bg(rgb(0xe1e5ea)))
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.set_workspace_collapsed(true, cx);
                    }))
                    .child(ui_icon(ICON_PANEL_RIGHT_CLOSE, 16., 0x6b7280)),
            );

        let open_editors_view = {
            let mut open_list = div().flex().flex_col().gap(px(2.)).py(px(6.));
            if self.open_editors.is_empty() {
                open_list = open_list.child(
                    div()
                        .px_3()
                        .py_2()
                        .font_family("IBM Plex Mono")
                        .text_size(px(11.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(0x6b7280))
                        .child("No open editors"),
                );
            } else {
                for path in &self.open_editors {
                    let is_active = self.open_note_path.as_deref() == Some(path.as_str());
                    let icon_color = if is_active { 0x0b4b57 } else { 0x6b7280 };
                    let text_color = if is_active { 0x111827 } else { 0x374151 };
                    let close_color = if is_active { 0x6b7280 } else { 0x9ca3af };
                    let text_weight = if is_active { 850. } else { 750. };

                    let path = path.clone();
                    let label = file_name(&path);
                    let close_path = path.clone();

                    let close_button = div()
                        .id(ElementId::Name(SharedString::from(format!(
                            "open.close:{close_path}"
                        ))))
                        .h(px(28.))
                        .w(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|this| this.bg(rgb(0xe1e5ea)))
                        .occlude()
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                            this.close_editor(&close_path, cx);
                        }))
                        .child(ui_icon(ICON_X, 14., close_color));

                    open_list = open_list.child(
                        div()
                            .id(ElementId::Name(SharedString::from(format!("open:{path}"))))
                            .h(px(28.))
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_2()
                            .bg(if is_active {
                                rgb(0xe1e5ea)
                            } else {
                                rgba(0x00000000)
                            })
                            .cursor_pointer()
                            .when(!is_active, |this| this.hover(|this| this.bg(rgb(0xe1e5ea))))
                            .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                                this.open_note(path.clone(), cx);
                            }))
                            .child(div().w(px(3.)).h_full().bg(if is_active {
                                rgb(0xb6edf5)
                            } else {
                                rgba(0x00000000)
                            }))
                            .child(ui_icon(ICON_FILE_TEXT, 14., icon_color))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .font_family("IBM Plex Mono")
                                    .text_size(px(11.))
                                    .font_weight(FontWeight(text_weight))
                                    .text_color(rgb(text_color))
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(label),
                            )
                            .child(close_button)
                            .child(div().w(px(2.))),
                    );
                }
            }

            div()
                .flex()
                .flex_col()
                .w_full()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .h(px(28.))
                        .w_full()
                        .bg(rgb(0xeef0f2))
                        .px_3()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .font_family("IBM Plex Mono")
                                .text_size(px(10.))
                                .font_weight(FontWeight(900.))
                                .text_color(rgb(0x374151))
                                .child("OPEN EDITORS"),
                        ),
                )
                .child(
                    div()
                        .id("workspace.open_editors.scroll")
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .overflow_y_scroll()
                        .bg(rgb(0xf2f4f7))
                        .child(open_list),
                )
        };

        let references_view = div()
            .id("workspace.refs")
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_3()
            .bg(rgb(0xeef0f2))
            .child(
                div()
                    .font_family("IBM Plex Mono")
                    .text_size(px(10.))
                    .font_weight(FontWeight(900.))
                    .text_color(rgb(0x6b7280))
                    .child("REFERENCES"),
            )
            .child(
                div()
                    .font_family("IBM Plex Mono")
                    .text_size(px(10.))
                    .font_weight(FontWeight(650.))
                    .text_color(rgb(0x6b7280))
                    .child("(links / backlinks appear here)"),
            );

        let bookmarks_view = div()
            .id("workspace.bookmarks")
            .flex()
            .flex_col()
            .w_full()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_3()
            .bg(rgb(0xeef0f2))
            .child(
                div()
                    .font_family("IBM Plex Mono")
                    .text_size(px(10.))
                    .font_weight(FontWeight(900.))
                    .text_color(rgb(0x6b7280))
                    .child("BOOKMARKS"),
            )
            .child(
                div()
                    .font_family("IBM Plex Mono")
                    .text_size(px(10.))
                    .font_weight(FontWeight(650.))
                    .text_color(rgb(0x6b7280))
                    .child("(pinned notes and saved searches)"),
            );

        let workspace_view = match self.workspace_mode {
            WorkspaceMode::OpenEditors => open_editors_view.into_any_element(),
            WorkspaceMode::References => references_view.into_any_element(),
            WorkspaceMode::Bookmarks => bookmarks_view.into_any_element(),
        };

        let workspace_panel = div()
            .w(workspace_width)
            .min_w(workspace_width)
            .max_w(workspace_width)
            .flex_shrink_0()
            .h_full()
            .bg(rgb(0xeef0f2))
            .flex()
            .flex_col()
            .child(mode_bar)
            .child(div().h(px(1.)).w_full().bg(rgb(0xc8cdd5)))
            .child(workspace_view);

        let note_path = self.open_note_path.as_deref();
        let note_title = note_path
            .map(|p| self.derive_note_title(p))
            .unwrap_or_else(|| "No note selected".to_string());

        let tab_action =
            |id: &'static str,
             icon: &'static str,
             on_click: fn(&mut XnoteWindow, &mut Context<XnoteWindow>)| {
                div()
                    .id(id)
                    .h(px(28.))
                    .w(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|this| this.bg(rgb(0xe1e5ea)))
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        on_click(this, cx);
                    }))
                    .child(ui_icon(icon, 16., 0x6b7280))
            };

        let sidebar_toggle_anim =
            Animation::new(Duration::from_millis(160)).with_easing(ease_in_out);
        let tab_action_animated =
            |id: &'static str,
             icon: &'static str,
             animation_id: ElementId,
             exiting: bool,
             on_click: fn(&mut XnoteWindow, &mut Context<XnoteWindow>)| {
                tab_action(id, icon, on_click).relative().with_animation(
                    animation_id,
                    sidebar_toggle_anim.clone(),
                    move |this, delta| {
                        let t = if exiting { 1. - delta } else { delta };
                        let x = px(-10. * (1. - t));
                        this.left(x).opacity(t)
                    },
                )
            };

        let mut tabs_bar = div()
            .id("editor.tabs")
            .h(px(28.))
            .w_full()
            .bg(rgb(0xeef0f2))
            .flex()
            .items_center()
            .gap(px(2.))
            .min_w_0()
            .overflow_x_hidden();

        let show_panel_shell_toggle =
            self.panel_shell_collapsed || self.panel_shell_tab_toggle_exiting;
        let show_workspace_toggle = self.workspace_collapsed || self.workspace_tab_toggle_exiting;
        if show_panel_shell_toggle || show_workspace_toggle {
            let mut leading = div()
                .id("editor.tabs.leading")
                .h_full()
                .flex()
                .items_center()
                .gap(px(2.))
                .px(px(2.))
                .flex_shrink_0()
                .overflow_hidden();

            if show_panel_shell_toggle {
                let exiting = self.panel_shell_tab_toggle_exiting && !self.panel_shell_collapsed;
                let anim_id = ElementId::Name(SharedString::from(format!(
                    "editor.toggle.panel_shell.anim:{}",
                    self.panel_shell_tab_toggle_anim_nonce
                )));
                leading = leading.child(tab_action_animated(
                    "editor.toggle.panel_shell",
                    ICON_PANEL_LEFT_OPEN,
                    anim_id,
                    exiting,
                    |this, cx| {
                        this.set_panel_shell_collapsed(false, cx);
                    },
                ));
            }
            if show_workspace_toggle {
                let exiting = self.workspace_tab_toggle_exiting && !self.workspace_collapsed;
                let anim_id = ElementId::Name(SharedString::from(format!(
                    "editor.toggle.workspace.anim:{}",
                    self.workspace_tab_toggle_anim_nonce
                )));
                leading = leading.child(tab_action_animated(
                    "editor.toggle.workspace",
                    ICON_PANEL_RIGHT_OPEN,
                    anim_id,
                    exiting,
                    |this, cx| {
                        this.set_workspace_collapsed(false, cx);
                    },
                ));
            }

            tabs_bar = tabs_bar
                .child(leading)
                .child(div().w(px(1.)).h(px(16.)).bg(rgb(0xc8cdd5)));
        }

        for path in &self.open_editors {
            let is_active = self.open_note_path.as_deref() == Some(path.as_str());
            let icon_color = if is_active { 0x0b4b57 } else { 0x6b7280 };
            let text_color = if is_active { 0x111827 } else { 0x374151 };
            let close_color = if is_active { 0x6b7280 } else { 0x9ca3af };
            let text_weight = if is_active { 850. } else { 750. };

            let path = path.clone();
            let label = file_name(&path);
            let close_path = path.clone();

            let close_button = div()
                .id(ElementId::Name(SharedString::from(format!(
                    "tab.close:{close_path}"
                ))))
                .h(px(28.))
                .w(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|this| this.bg(rgb(0xe1e5ea)))
                .occlude()
                .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                    this.close_editor(&close_path, cx);
                }))
                .child(ui_icon(ICON_X, 14., close_color));

            tabs_bar = tabs_bar.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!("tab:{path}"))))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px(px(10.))
                    .max_w(px(220.))
                    .overflow_hidden()
                    .bg(if is_active {
                        rgb(0xffffff)
                    } else {
                        rgb(0xeef0f2)
                    })
                    .cursor_pointer()
                    .when(!is_active, |this| this.hover(|this| this.bg(rgb(0xe1e5ea))))
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        this.open_note(path.clone(), cx);
                    }))
                    .child(ui_icon(ICON_FILE_TEXT, 14., icon_color))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(text_weight))
                            .text_color(rgb(text_color))
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(label),
                    )
                    .child(close_button),
            );
        }

        tabs_bar = tabs_bar
            .child(div().flex_1())
            .child(tab_action("editor.new", ICON_PLUS, |this, cx| {
                this.create_new_note(cx);
            }))
            .child(tab_action("editor.export", ICON_DOWNLOAD, |this, cx| {
                this.export_open_note(cx);
            }))
            .child(tab_action("editor.split", ICON_COLUMNS_2, |this, cx| {
                this.split_editor = !this.split_editor;
                cx.notify();
            }))
            .child(div().w(px(4.)));

        let editor_body_placeholder = if self.open_note_path.is_none() {
            match &self.vault_state {
                VaultState::Opened { .. } => "Select a note in Explorer to open.".to_string(),
                VaultState::Opening { .. } => "Opening vault...".to_string(),
                VaultState::Error { message } => format!("Vault error: {message}"),
                VaultState::NotConfigured => {
                    "No vault configured. Press Ctrl+O to open a vault folder.".to_string()
                }
            }
        } else {
            "Loading...".to_string()
        };

        let sync_status = if self.open_note_loading {
            "Loading"
        } else if self.open_note_dirty {
            "Unsaved"
        } else {
            "Synced"
        };

        let meta_line = note_path.map(|p| {
            SharedString::from(format!(
                "{p} · {} words · {sync_status}",
                self.open_note_word_count
            ))
        });

        let breadcrumbs = {
            let (folder, file) = note_path
                .and_then(|p| p.rsplit_once('/'))
                .map(|(f, n)| (Some(f.to_string()), n.to_string()))
                .unwrap_or_else(|| (None, note_path.unwrap_or("").to_string()));

            let mut segments = div().flex().items_center().gap(px(6.));
            if let Some(folder) = folder {
                segments = segments
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(750.))
                            .text_color(rgb(0x6b7280))
                            .child(folder),
                    )
                    .child(ui_icon(ICON_CHEVRON_RIGHT, 12., 0x9ca3af));
            }
            segments = segments.child(
                div()
                    .font_family("IBM Plex Mono")
                    .text_size(px(11.))
                    .font_weight(FontWeight(850.))
                    .text_color(rgb(0x111827))
                    .child(file),
            );

            div()
                .h(px(24.))
                .w_full()
                .bg(rgb(0xffffff))
                .px(px(18.))
                .flex()
                .items_center()
                .gap_2()
                .child(ui_icon(ICON_FILE_TEXT, 14., 0x9ca3af))
                .child(segments)
        };

        let editor_header = div()
            .h(px(88.))
            .w_full()
            .bg(rgb(0xffffff))
            .px(px(18.))
            .py(px(14.))
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(6.))
            .child(
                div()
                    .font_family("Inter")
                    .text_size(px(20.))
                    .font_weight(FontWeight(900.))
                    .text_color(rgb(0x111827))
                    .child(note_title),
            )
            .children(meta_line.map(|meta| {
                div()
                    .font_family("IBM Plex Mono")
                    .text_size(px(11.))
                    .font_weight(FontWeight(800.))
                    .text_color(rgb(0x6b7280))
                    .child(meta)
            }));

        let editor_pane = |id: &'static str, interactive: bool| {
            let mut pane = div()
                .id(id)
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_y_scroll()
                .px(px(18.))
                .py(px(10.))
                .font_family("IBM Plex Mono")
                .text_size(px(13.))
                .text_color(rgb(0x111827))
                .bg(rgb(0xffffff));

            if self.open_note_path.is_some() && !self.open_note_loading {
                if interactive {
                    pane = pane
                        .track_focus(&self.editor_focus_handle)
                        .cursor(CursorStyle::IBeam)
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                            this.on_editor_key(ev, cx);
                        }))
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::on_editor_mouse_down))
                        .on_mouse_up(MouseButton::Left, cx.listener(Self::on_editor_mouse_up))
                        .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_editor_mouse_up))
                        .on_mouse_move(cx.listener(Self::on_editor_mouse_move));
                }
                pane.child(NoteEditorElement { view: cx.entity() })
                    .into_any_element()
            } else {
                pane.font_family("IBM Plex Mono")
                    .text_size(px(11.))
                    .font_weight(FontWeight(650.))
                    .text_color(rgb(0x6b7280))
                    .child(editor_body_placeholder.clone())
                    .into_any_element()
            }
        };

        let editor_body = if self.split_editor {
            div()
                .id("editor.split")
                .flex_1()
                .min_h_0()
                .flex()
                .flex_row()
                .child(editor_pane("editor.pane.left", true))
                .child(div().w(px(1.)).h_full().bg(rgb(0xe5e7eb)))
                .child(editor_pane("editor.pane.right", false))
                .into_any_element()
        } else {
            editor_pane("editor.pane.single", true)
        };

        let editor = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(rgb(0xffffff))
            .flex()
            .flex_col()
            .child(tabs_bar)
            .child(div().h(px(1.)).w_full().bg(rgb(0xe5e7eb)))
            .child(breadcrumbs)
            .child(div().h(px(1.)).w_full().bg(rgb(0xe5e7eb)))
            .child(editor_header)
            .child(div().h(px(1.)).w_full().bg(rgb(0xe5e7eb)))
            .child(editor_body);

        let titlebar_command = div()
            .id("titlebar.command")
            .w(px(520.))
            .h(px(24.))
            .bg(rgb(0xf5f6f8))
            .border_1()
            .border_color(rgb(0xc8cdd5))
            .px(px(10.))
            .cursor_pointer()
            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                this.open_palette(PaletteMode::Commands, cx);
            }))
            .flex()
            .flex_col()
            .justify_center()
            .child(
                div()
                    .h(px(20.))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(ui_icon(ICON_SEARCH, 14., 0x6b7280))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(0x6b7280))
                            .child("Search / Commands (Ctrl+K)"),
                    ),
            )
            .child(div().h(px(2.)).w_full().bg(rgb(0xb6edf5)));

        let titlebar_left = div()
            .id("titlebar.left")
            .flex_1()
            .h_full()
            .px(px(6.))
            .window_control_area(WindowControlArea::Drag);

        let titlebar_right_spacer = div()
            .id("titlebar.right_spacer")
            .flex_1()
            .h_full()
            .window_control_area(WindowControlArea::Drag);

        let titlebar_window_button =
            |id: &'static str, icon: &'static str, on_click: fn(&mut Window)| {
                div()
                    .id(id)
                    .w(px(36.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|this| this.bg(rgb(0xe1e5ea)))
                    .on_click(
                        move |_ev: &ClickEvent, window: &mut Window, _cx: &mut App| {
                            on_click(window);
                        },
                    )
                    .child(ui_icon(icon, 16., 0x6b7280))
            };

        let titlebar_window_controls = div()
            .id("titlebar.controls")
            .h_full()
            .flex()
            .items_center()
            .justify_end()
            .child(titlebar_window_button(
                "titlebar.min",
                ICON_MINUS,
                |window| {
                    window.minimize_window();
                },
            ))
            .child(titlebar_window_button(
                "titlebar.max",
                ICON_SQUARE,
                |window| {
                    window.zoom_window();
                },
            ))
            .child(titlebar_window_button("titlebar.close", ICON_X, |window| {
                window.remove_window();
            }));

        let menu_bar = div()
            .h(px(32.))
            .w_full()
            .bg(rgb(0xeceff3))
            .flex()
            .items_center()
            .child(titlebar_left)
            .child(
                div()
                    .w(px(520.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(titlebar_command),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .child(titlebar_right_spacer)
                    .child(titlebar_window_controls),
            );

        let top = div()
            .h(px(33.))
            .w_full()
            .bg(rgb(0xeef0f2))
            .flex()
            .flex_col()
            .child(menu_bar)
            .child(div().h(px(1.)).w_full().bg(rgb(0xc8cdd5)));

        let workspace_name = match &self.vault_state {
            VaultState::Opened { root_name, .. } => root_name.to_string(),
            _ => "None".to_string(),
        };

        let (cursor_line, cursor_col) = self.cursor_line_col();

        let sync_status = if self.open_note_loading {
            SharedString::from("Loading")
        } else if self.open_note_dirty {
            SharedString::from("Unsaved")
        } else {
            SharedString::from("Synced")
        };

        let status_bar = div()
            .h(px(28.))
            .w_full()
            .bg(rgb(0xeceff3))
            .px_3()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("status.module")
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .cursor_pointer()
                            .hover(|this| this.bg(rgb(0xe1e5ea)))
                            .px_2()
                            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                                this.open_palette(PaletteMode::Commands, cx);
                            }))
                            .child(ui_icon(ICON_GRID_2X2, 14., 0x6b7280))
                            .child(
                                div()
                                    .font_family("IBM Plex Mono")
                                    .text_size(px(10.))
                                    .font_weight(FontWeight(800.))
                                    .text_color(rgb(0x374151))
                                    .child("Knowledge"),
                            )
                            .child(ui_icon(ICON_CHEVRON_DOWN, 14., 0x6b7280)),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(0xc8cdd5)))
                    .child(
                        div()
                            .id("status.workspace")
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .cursor_pointer()
                            .hover(|this| this.bg(rgb(0xe1e5ea)))
                            .px_2()
                            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                                this.open_vault_prompt(cx);
                            }))
                            .child(
                                div()
                                    .font_family("IBM Plex Mono")
                                    .text_size(px(11.))
                                    .font_weight(FontWeight(750.))
                                    .text_color(rgb(0x374151))
                                    .child(SharedString::from(format!(
                                        "Workspace: {workspace_name}"
                                    ))),
                            )
                            .child(ui_icon(ICON_CHEVRON_DOWN, 16., 0x6b7280)),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(0xc8cdd5)))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(0x374151))
                            .child("main"),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(750.))
                            .text_color(rgb(0x6b7280))
                            .child(SharedString::from(format!(
                                "Ln {cursor_line}, Col {cursor_col}"
                            ))),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(0xc8cdd5)))
                    .child(ui_icon(ICON_REFRESH_CW, 14., 0x6b7280))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(0x374151))
                            .child(sync_status),
                    ),
            );

        let splitter_handle = |id: &'static str, kind: SplitterKind| {
            let active = self.splitter_drag.is_some_and(|d| d.kind == kind);
            let line = rgb(0xc8cdd5);
            div()
                .id(id)
                .w(splitter_w)
                .min_w(splitter_w)
                .max_w(splitter_w)
                .flex_shrink_0()
                .h_full()
                .relative()
                .bg(if active { rgb(0xe1e5ea) } else { rgb(0xf5f6f8) })
                .cursor_col_resize()
                .hover(|this| this.bg(rgb(0xeef0f2)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                        this.begin_splitter_drag(kind, ev, cx);
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(2.5))
                        .top_0()
                        .bottom_0()
                        .w(px(1.))
                        .bg(line),
                )
        };

        let mut main_row = div().flex().flex_1().min_h_0().w_full().child(rail);
        if panel_shell_present {
            main_row = main_row
                .child(panel_shell)
                .child(splitter_handle("splitter.panel", SplitterKind::PanelShell));
        }
        if workspace_present {
            main_row = main_row.child(workspace_panel).child(splitter_handle(
                "splitter.workspace",
                SplitterKind::Workspace,
            ));
        }
        main_row = main_row.child(editor);

        let mut root = div()
            .size_full()
            .relative()
            .bg(rgb(0xf5f6f8))
            .flex()
            .flex_col()
            .child(top)
            .child(main_row)
            .child(status_bar);

        if self.palette_open {
            root = root.child(self.palette_overlay(cx));
        }
        if self.settings_open {
            root = root.child(self.settings_overlay(cx));
        }
        if self.vault_prompt_open {
            root = root.child(self.vault_prompt_overlay(cx));
        }
        if self.splitter_drag.is_some() {
            root = root.child(
                div()
                    .id("splitter.drag_overlay")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .right(px(0.))
                    .bg(rgba(0x00000000))
                    .cursor_col_resize()
                    .on_mouse_move(cx.listener(Self::on_splitter_drag_mouse_move))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(Self::on_splitter_drag_mouse_up),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(Self::on_splitter_drag_mouse_up),
                    ),
            );
        }

        root
    }
}

fn resolve_vault_path() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--vault" {
            let p = args.next()?;
            if p.trim().is_empty() {
                continue;
            }
            return Some(PathBuf::from(p));
        }
    }

    match std::env::var("XNOTE_VAULT") {
        Ok(s) if !s.trim().is_empty() => return Some(PathBuf::from(s.trim())),
        _ => {}
    }

    let default = PathBuf::from("Knowledge.vault");
    if default.is_dir() {
        return Some(default);
    }

    None
}

fn app_config_dir() -> PathBuf {
    if let Ok(path) = std::env::var("XNOTE_CONFIG") {
        let path = path.trim();
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }

    if cfg!(target_os = "windows") {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let appdata = appdata.trim();
            if !appdata.is_empty() {
                return Path::new(appdata).join("XNote");
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let home = home.trim();
        if !home.is_empty() {
            return Path::new(home).join(".config").join("xnote");
        }
    }

    PathBuf::from(".xnote")
}

fn load_boot_context() -> BootContext {
    let config_dir = app_config_dir();
    let user_settings_path = settings_path(&config_dir);

    let project_root = resolve_vault_path().and_then(|path| {
        if path.is_dir() {
            Some(path)
        } else {
            path.parent().map(|p| p.to_path_buf())
        }
    });

    let project_settings_path = project_root.as_deref().map(project_settings_path);
    let mut app_settings =
        load_effective_settings(&config_dir, project_root.as_deref()).unwrap_or_default();

    let locale = Locale::from_tag(&app_settings.locale).unwrap_or(Locale::EnUs);
    app_settings.locale = locale.as_tag().to_string();

    let keymap = app_settings
        .build_keymap()
        .unwrap_or_else(|_err| Keymap::default_keymap());
    let plugin_runtime_mode =
        PluginRuntimeMode::from_tag(app_settings.plugin_policy.runtime_mode.as_str());

    BootContext {
        app_settings,
        settings_path: user_settings_path,
        project_settings_path,
        locale,
        keymap,
        plugin_runtime_mode,
    }
}

struct ExplorerIndex {
    folder_children: HashMap<String, Vec<String>>,
    folder_notes: HashMap<String, Vec<String>>,
    all_note_paths: Vec<String>,
    all_note_paths_lower: Vec<String>,
}

fn build_explorer_index(vault: &Vault, entries: &[NoteEntry]) -> anyhow::Result<ExplorerIndex> {
    let mut all_note_paths = Vec::with_capacity(entries.len());
    let mut all_note_paths_lower = Vec::with_capacity(entries.len());
    for e in entries {
        all_note_paths.push(e.path.clone());
        all_note_paths_lower.push(e.path.to_lowercase());
    }

    let mut by_folder: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in entries {
        let folder = match e.path.rsplit_once('/') {
            Some((folder, _)) => folder.to_string(),
            None => String::new(),
        };
        by_folder.entry(folder).or_default().push(e.path.clone());
    }

    let mut child_sets: HashMap<String, BTreeSet<String>> = HashMap::new();
    child_sets.entry(String::new()).or_default();

    let folder_keys: Vec<String> = by_folder.keys().cloned().collect();
    for folder in folder_keys {
        if folder.is_empty() {
            continue;
        }

        let mut full = folder;
        loop {
            let parent = match full.rsplit_once('/') {
                Some((p, _)) => p.to_string(),
                None => String::new(),
            };
            child_sets
                .entry(parent.clone())
                .or_default()
                .insert(full.clone());
            if parent.is_empty() {
                break;
            }
            full = parent;
        }
    }

    let mut folder_notes = HashMap::with_capacity(by_folder.len());
    for (folder, mut default_paths) in by_folder {
        default_paths.sort();

        let ordered_paths = if folder.is_empty() {
            default_paths
        } else {
            let order = vault.load_folder_order(&folder)?;
            apply_folder_order(&default_paths, &order)
        };

        folder_notes.insert(folder, ordered_paths);
    }

    folder_notes.entry(String::new()).or_default();
    for set in child_sets.values() {
        for folder in set {
            folder_notes.entry(folder.clone()).or_default();
        }
    }

    let folder_children = child_sets
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect::<Vec<_>>()))
        .collect::<HashMap<_, _>>();

    Ok(ExplorerIndex {
        folder_children,
        folder_notes,
        all_note_paths,
        all_note_paths_lower,
    })
}

fn apply_folder_order(default_paths: &[String], order: &[String]) -> Vec<String> {
    let existing: HashSet<&str> = default_paths.iter().map(|s| s.as_str()).collect();
    let mut out = Vec::with_capacity(default_paths.len());
    let mut seen: HashSet<&str> = HashSet::with_capacity(default_paths.len());

    for p in order {
        let p = p.as_str();
        if existing.contains(p) && seen.insert(p) {
            out.push(p.to_string());
        }
    }
    for p in default_paths {
        let p = p.as_str();
        if seen.insert(p) {
            out.push(p.to_string());
        }
    }

    out
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn tree_indent_guides(depth: usize, line_mask: u64) -> gpui::Div {
    let line = rgb(0xc8cdd5);
    let mut out = div().h_full().flex().items_center().flex_shrink_0();
    if depth == 0 {
        return out;
    }

    for level in 0..depth {
        let mut col = div().w(px(14.)).h_full().relative();

        if ((line_mask >> level) & 1) == 1 {
            col = col.child(
                div()
                    .absolute()
                    .left(px(7.))
                    .top_0()
                    .bottom_0()
                    .w(px(1.))
                    .bg(line),
            );
        }

        out = out.child(col);
    }

    out
}

fn depth_line_mask(depth: usize) -> u64 {
    if depth >= 64 {
        u64::MAX
    } else {
        (1u64 << depth).saturating_sub(1)
    }
}

fn byte_offset_for_line(s: &str, line: usize) -> usize {
    if line <= 1 {
        return 0;
    }

    let mut current = 1usize;
    for (ix, b) in s.as_bytes().iter().enumerate() {
        if *b == b'\n' {
            current += 1;
            if current == line {
                return ix + 1;
            }
        }
    }

    s.len()
}

fn count_words(s: &str) -> usize {
    s.split_whitespace().filter(|w| !w.is_empty()).count()
}

fn ui_icon(path: &'static str, size_px: f32, color: u32) -> gpui::Svg {
    svg()
        .path(path)
        .w(px(size_px))
        .h(px(size_px))
        .text_color(rgb(color))
}

fn ui_corner_tag(color: u32) -> gpui::Svg {
    svg()
        .path(ICON_CORNER_TAG)
        .w(px(16.))
        .h(px(16.))
        .text_color(rgb(color))
        .with_transformation(Transformation::rotate(radians(std::f32::consts::FRAC_PI_4)))
}

fn main() {
    Application::new()
        .with_assets(UiAssets {
            base: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        })
        .run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1200.0), px(760.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(820.0), px(540.0))),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some(SharedString::from("XNote")),
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(|cx| XnoteWindow::new(cx)),
            )
            .unwrap();
            cx.activate(true);
        });
}
