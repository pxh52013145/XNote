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
use serde_json::json;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use xnote_core::command::{command_specs, CommandId};
use xnote_core::editor::{EditTransaction, EditorBuffer};
use xnote_core::keybind::KeyContext;
use xnote_core::keybind::Keymap;
use xnote_core::knowledge::{KnowledgeIndex, SearchOptions};
use xnote_core::markdown::{
    lint_markdown,
    parse_markdown,
    MarkdownDiagnostic,
    MarkdownDiagnosticSeverity,
    MarkdownInvalidationWindow,
    MarkdownParseResult,
};
use xnote_core::paths::{join_inside, normalize_folder_rel_path, normalize_vault_rel_path};
use xnote_core::plugin::{
    PluginActivationEvent, PluginCapability, PluginLifecycleState, PluginManifest, PluginRegistry,
    PluginRuntimeMode,
};
use xnote_core::settings::{
    load_effective_settings, project_settings_path, save_project_settings, save_settings,
    settings_path, AppSettings,
};
use xnote_core::vault::{NoteEntry, Vault, VaultScan};
use xnote_core::watch::{VaultWatchChange, VaultWatcher};

const ICON_BOOKMARK: &str = "icons/bookmark.svg";
const ICON_BRUSH: &str = "icons/brush.svg";
const ICON_CHEVRON_DOWN: &str = "icons/chevron-down.svg";
const ICON_CHEVRON_RIGHT: &str = "icons/chevron-right.svg";
const ICON_COLUMNS_2: &str = "icons/columns-2.svg";
const ICON_COMMAND: &str = "icons/command.svg";
const ICON_CORNER_TAG: &str = "icons/corner-tag.svg";
const ICON_DOWNLOAD: &str = "icons/download.svg";
const ICON_EYE: &str = "icons/eye.svg";
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

const WATCH_EVENT_DEBOUNCE: Duration = Duration::from_millis(180);
const WATCH_EVENT_DRAIN_INTERVAL: Duration = Duration::from_millis(120);
const WATCH_EVENT_BATCH_MAX: usize = 512;
const SEARCH_QUERY_CACHE_CAPACITY: usize = 64;
const QUICK_OPEN_CACHE_CAPACITY: usize = 64;
const CACHE_DIAGNOSTICS_FLUSH_INTERVAL: Duration = Duration::from_secs(20);
const CACHE_DIAGNOSTICS_PATH: &str = "perf/cache-diagnostics.json";
const CACHE_DIAGNOSTICS_SHORT_WINDOW_SECS: u64 = 5 * 60;
const CACHE_DIAGNOSTICS_LONG_WINDOW_SECS: u64 = 60 * 60;
const CACHE_DIAGNOSTICS_MAX_SNAPSHOTS: usize = 512;
const STATUS_SYNC_SLOT_WIDTH: f32 = 54.0;
const OVERLAY_BACKDROP_ARM_DELAY: Duration = Duration::from_millis(120);
const MARKDOWN_PARSE_DEBOUNCE: Duration = Duration::from_millis(80);
const MARKDOWN_INVALIDATION_CONTEXT_BYTES: usize = 256;
const EDIT_LATENCY_SAMPLES_CAPACITY: usize = 256;
const MAX_EDITOR_HIGHLIGHT_BYTES: usize = 256 * 1024;
const CREATE_RECONCILE_DELAY: Duration = Duration::from_millis(900);
const EDITOR_GUTTER_BASE_WIDTH: f32 = 14.0;
const EDITOR_GUTTER_DIGIT_WIDTH: f32 = 7.0;
const EDITOR_GUTTER_LINE_HEIGHT: f32 = 20.0;
const EDITOR_TEXT_LEFT_PADDING: f32 = 8.0;
const EDITOR_TEXT_MIN_WRAP_WIDTH: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorViewMode {
    Edit,
    Preview,
    Split,
}

#[derive(Clone, Debug)]
struct MarkdownPreviewModel {
    headings: Vec<(u8, String)>,
    blocks: Vec<MarkdownPreviewBlock>,
}

#[derive(Clone, Debug)]
struct MarkdownPreviewBlock {
    kind: MarkdownPreviewBlockKind,
    text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MarkdownPreviewBlockKind {
    Heading(u8),
    Paragraph,
    CodeFence,
    Quote,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorHighlightKind {
    HeadingMarker,
    HeadingText,
    CodeFence,
    CodeText,
    QuoteMarker,
    ListMarker,
    LinkText,
    LinkUrl,
}

#[derive(Clone, Debug)]
struct EditorHighlightSpan {
    range: Range<usize>,
    kind: EditorHighlightKind,
}

#[derive(Clone, Copy, Debug)]
enum EditorMutationSource {
    Keyboard,
    Ime,
    UndoRedo,
}

#[derive(Clone, Debug, Default)]
struct EditLatencyStats {
    samples_ms: VecDeque<u128>,
}

impl EditLatencyStats {
    fn record(&mut self, elapsed_ms: u128) {
        self.samples_ms.push_back(elapsed_ms);
        while self.samples_ms.len() > EDIT_LATENCY_SAMPLES_CAPACITY {
            self.samples_ms.pop_front();
        }
    }

    fn sample_count(&self) -> usize {
        self.samples_ms.len()
    }

    fn p50_ms(&self) -> u128 {
        let mut samples: Vec<u128> = self.samples_ms.iter().copied().collect();
        percentile_u128(&mut samples, 50.0)
    }

    fn p95_ms(&self) -> u128 {
        let mut samples: Vec<u128> = self.samples_ms.iter().copied().collect();
        percentile_u128(&mut samples, 95.0)
    }
}

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
enum IndexState {
    Idle,
    Building,
    Ready {
        note_count: usize,
        duration_ms: u128,
    },
    Error {
        message: SharedString,
    },
}

#[derive(Clone, Debug)]
struct WatcherStatus {
    revision: u64,
    last_error: Option<SharedString>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct QueryCacheStats {
    search_hits: u64,
    search_misses: u64,
    quick_open_hits: u64,
    quick_open_misses: u64,
}

#[derive(Clone, Debug)]
struct CacheStatsSnapshot {
    epoch_secs: u64,
    stats: QueryCacheStats,
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

#[derive(Clone, Debug)]
struct OpenPathMatch {
    path: String,
}

#[derive(Clone, Debug)]
enum WatchInboxMessage {
    Changes(Vec<VaultWatchChange>),
    Error(String),
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
        id: CommandId::Undo,
        icon: ICON_REFRESH_CW,
    },
    PaletteCommandSpec {
        id: CommandId::Redo,
        icon: ICON_REFRESH_CW,
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

impl SettingsTheme {
    fn from_tag(input: &str) -> Self {
        if input.trim().eq_ignore_ascii_case("dark") {
            Self::Dark
        } else {
            Self::Light
        }
    }

    const fn as_tag(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsAccent {
    Default,
    Blue,
}

impl SettingsAccent {
    fn from_tag(input: &str) -> Self {
        if input.trim().eq_ignore_ascii_case("blue") {
            Self::Blue
        } else {
            Self::Default
        }
    }

    const fn as_tag(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Blue => "blue",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct UiTheme {
    app_bg: u32,
    panel_bg: u32,
    surface_bg: u32,
    surface_alt_bg: u32,
    titlebar_bg: u32,
    border: u32,
    text_primary: u32,
    text_secondary: u32,
    text_muted: u32,
    text_subtle: u32,
    interactive_hover: u32,
    accent: u32,
    accent_soft: u32,
    status_loading: u32,
    status_dirty: u32,
    status_synced: u32,
    editor_gutter_bg: u32,
    editor_gutter_text: u32,
    diagnostic_info: u32,
    diagnostic_warning: u32,
    diagnostic_error: u32,
    syntax_heading_marker: u32,
    syntax_heading_text: u32,
    syntax_code_fence: u32,
    syntax_code_text: u32,
    syntax_quote_marker: u32,
    syntax_list_marker: u32,
    syntax_link_text: u32,
    syntax_link_url: u32,
}

impl UiTheme {
    fn from_settings(theme: SettingsTheme, accent: SettingsAccent) -> Self {
        let accent_color = match accent {
            SettingsAccent::Default => 0x6d5df2,
            SettingsAccent::Blue => 0x2563eb,
        };

        match theme {
            SettingsTheme::Light => Self {
                app_bg: 0xf5f6f8,
                panel_bg: 0xeef0f2,
                surface_bg: 0xffffff,
                surface_alt_bg: 0xf2f4f7,
                titlebar_bg: 0xeceff3,
                border: 0xc8cdd5,
                text_primary: 0x111827,
                text_secondary: 0x374151,
                text_muted: 0x6b7280,
                text_subtle: 0x9ca3af,
                interactive_hover: 0xe1e5ea,
                accent: accent_color,
                accent_soft: 0xb6edf5,
                status_loading: 0xf59e0b,
                status_dirty: 0xef4444,
                status_synced: 0x22c55e,
                editor_gutter_bg: 0xe2e8f0,
                editor_gutter_text: 0x64748b,
                diagnostic_info: 0x0284c7,
                diagnostic_warning: 0xd97706,
                diagnostic_error: 0xdc2626,
                syntax_heading_marker: 0x64748b,
                syntax_heading_text: 0x1d4ed8,
                syntax_code_fence: 0x6d28d9,
                syntax_code_text: 0x7c3aed,
                syntax_quote_marker: 0x047857,
                syntax_list_marker: 0xb45309,
                syntax_link_text: 0x0891b2,
                syntax_link_url: 0x0369a1,
            },
            SettingsTheme::Dark => Self {
                app_bg: 0x111827,
                panel_bg: 0x1f2937,
                surface_bg: 0x0f172a,
                surface_alt_bg: 0x1e293b,
                titlebar_bg: 0x111827,
                border: 0x334155,
                text_primary: 0xe5e7eb,
                text_secondary: 0xcbd5e1,
                text_muted: 0x94a3b8,
                text_subtle: 0x64748b,
                interactive_hover: 0x334155,
                accent: accent_color,
                accent_soft: 0x1d4ed8,
                status_loading: 0xfbbf24,
                status_dirty: 0xf87171,
                status_synced: 0x4ade80,
                editor_gutter_bg: 0x0b1220,
                editor_gutter_text: 0x64748b,
                diagnostic_info: 0x38bdf8,
                diagnostic_warning: 0xf59e0b,
                diagnostic_error: 0xef4444,
                syntax_heading_marker: 0x64748b,
                syntax_heading_text: 0x60a5fa,
                syntax_code_fence: 0xa78bfa,
                syntax_code_text: 0xc4b5fd,
                syntax_quote_marker: 0x34d399,
                syntax_list_marker: 0xfbbf24,
                syntax_link_text: 0x22d3ee,
                syntax_link_url: 0x38bdf8,
            },
        }
    }
}

struct XnoteWindow {
    vault_state: VaultState,
    scan_state: ScanState,
    index_state: IndexState,
    explorer_rows: Vec<ExplorerRow>,
    explorer_filter: String,
    explorer_rows_filtered: Vec<usize>,
    next_filter_nonce: u64,
    pending_filter_nonce: u64,
    next_create_reconcile_nonce: u64,
    pending_create_reconcile_nonce: u64,
    pending_created_note_reconcile: HashSet<String>,
    pending_created_folder_reconcile: HashSet<String>,
    pending_watch_changes_until_index_ready: Vec<VaultWatchChange>,
    explorer_folder_children: HashMap<String, Vec<String>>,
    explorer_expanded_folders: HashSet<String>,
    explorer_all_note_paths: Arc<Vec<String>>,
    explorer_all_note_paths_lower: Arc<Vec<String>>,
    folder_notes: HashMap<String, Vec<String>>,
    selected_note: Option<String>,
    selected_explorer_folder: Option<String>,
    drag_over: Option<DragOver>,
    next_order_nonce: u64,
    pending_order_nonce_by_folder: HashMap<String, u64>,
    open_editors: Vec<String>,
    open_note_path: Option<String>,
    open_note_loading: bool,
    open_note_dirty: bool,
    open_note_content: String,
    editor_buffer: Option<EditorBuffer>,
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
    palette_results: Vec<OpenPathMatch>,
    next_palette_nonce: u64,
    pending_palette_nonce: u64,
    palette_backdrop_armed_until: Option<Instant>,
    vault_prompt_open: bool,
    vault_prompt_needs_focus: bool,
    vault_prompt_value: String,
    vault_prompt_error: Option<SharedString>,
    vault_prompt_backdrop_armed_until: Option<Instant>,
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
    settings_backdrop_armed_until: Option<Instant>,
    editor_view_mode: EditorViewMode,
    open_note_word_count: usize,
    open_note_heading_count: usize,
    open_note_link_count: usize,
    open_note_code_fence_count: usize,
    markdown_preview: MarkdownPreviewModel,
    markdown_diagnostics: Vec<MarkdownDiagnostic>,
    editor_highlight_spans: Vec<EditorHighlightSpan>,
    pending_markdown_invalidation: Option<MarkdownInvalidationWindow>,
    next_markdown_parse_nonce: u64,
    pending_markdown_parse_nonce: u64,
    edit_latency_stats: EditLatencyStats,
    pending_open_note_cursor: Option<(String, usize)>,
    knowledge_index: Option<Arc<KnowledgeIndex>>,
    search_options: SearchOptions,
    watcher_status: WatcherStatus,
    watch_scan_fingerprint: u64,
    watch_scan_entries: usize,
    watch_inbox: Option<Receiver<WatchInboxMessage>>,
    index_generation: u64,
    search_query_cache: HashMap<String, Vec<SearchRow>>,
    search_query_cache_order: VecDeque<String>,
    quick_open_query_cache: HashMap<String, Vec<OpenPathMatch>>,
    quick_open_query_cache_order: VecDeque<String>,
    cache_stats: QueryCacheStats,
    cache_stats_last_flushed: QueryCacheStats,
    cache_stats_snapshots: VecDeque<CacheStatsSnapshot>,
    editor_autosave_delay_input: String,
    hotkey_editing_command: Option<CommandId>,
    hotkey_editing_value: String,
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
        let settings_theme = SettingsTheme::from_tag(&boot.app_settings.appearance.theme);
        let settings_accent = SettingsAccent::from_tag(&boot.app_settings.appearance.accent);
        let editor_autosave_delay_input = boot.app_settings.editor.autosave_delay_ms.to_string();
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
                PluginActivationEvent::OnCommand(CommandId::Undo),
                PluginActivationEvent::OnCommand(CommandId::Redo),
                PluginActivationEvent::OnCommand(CommandId::ToggleSplit),
                PluginActivationEvent::OnCommand(CommandId::FocusExplorer),
                PluginActivationEvent::OnCommand(CommandId::FocusSearch),
                PluginActivationEvent::OnCommand(CommandId::Settings),
            ],
        });

        let mut this = Self {
            vault_state: VaultState::NotConfigured,
            scan_state: ScanState::Idle,
            index_state: IndexState::Idle,
            explorer_rows: Vec::new(),
            explorer_filter: String::new(),
            explorer_rows_filtered: Vec::new(),
            next_filter_nonce: 0,
            pending_filter_nonce: 0,
            next_create_reconcile_nonce: 0,
            pending_create_reconcile_nonce: 0,
            pending_created_note_reconcile: HashSet::new(),
            pending_created_folder_reconcile: HashSet::new(),
            pending_watch_changes_until_index_ready: Vec::new(),
            explorer_folder_children: HashMap::new(),
            explorer_expanded_folders: HashSet::new(),
            explorer_all_note_paths: Arc::new(Vec::new()),
            explorer_all_note_paths_lower: Arc::new(Vec::new()),
            folder_notes: HashMap::new(),
            selected_note: None,
            selected_explorer_folder: None,
            drag_over: None,
            next_order_nonce: 0,
            pending_order_nonce_by_folder: HashMap::new(),
            open_editors: Vec::new(),
            open_note_path: None,
            open_note_loading: false,
            open_note_dirty: false,
            open_note_content: String::new(),
            editor_buffer: None,
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
            palette_backdrop_armed_until: None,
            vault_prompt_open: false,
            vault_prompt_needs_focus: false,
            vault_prompt_value: String::new(),
            vault_prompt_error: None,
            vault_prompt_backdrop_armed_until: None,
            search_query: String::new(),
            search_selected: 0,
            search_results: Vec::new(),
            next_search_nonce: 0,
            pending_search_nonce: 0,
            settings_section: SettingsSection::Appearance,
            settings_open: false,
            settings_theme,
            settings_accent,
            settings_language: boot.locale,
            settings_language_menu_open: false,
            settings_backdrop_armed_until: None,
            editor_view_mode: EditorViewMode::Edit,
            open_note_word_count: 0,
            open_note_heading_count: 0,
            open_note_link_count: 0,
            open_note_code_fence_count: 0,
            markdown_preview: MarkdownPreviewModel {
                headings: Vec::new(),
                blocks: Vec::new(),
            },
            markdown_diagnostics: Vec::new(),
            editor_highlight_spans: Vec::new(),
            pending_markdown_invalidation: None,
            next_markdown_parse_nonce: 0,
            pending_markdown_parse_nonce: 0,
            edit_latency_stats: EditLatencyStats::default(),
            pending_open_note_cursor: None,
            knowledge_index: None,
            search_options: SearchOptions::default(),
            watcher_status: WatcherStatus {
                revision: 0,
                last_error: None,
            },
            watch_scan_fingerprint: 0,
            watch_scan_entries: 0,
            watch_inbox: None,
            index_generation: 0,
            search_query_cache: HashMap::new(),
            search_query_cache_order: VecDeque::new(),
            quick_open_query_cache: HashMap::new(),
            quick_open_query_cache_order: VecDeque::new(),
            cache_stats: QueryCacheStats::default(),
            cache_stats_last_flushed: QueryCacheStats::default(),
            cache_stats_snapshots: VecDeque::new(),
            editor_autosave_delay_input,
            hotkey_editing_command: None,
            hotkey_editing_value: String::new(),
        };

        this.status = SharedString::from(this.i18n.text("status.ready"));

        this.activate_plugins(PluginActivationEvent::OnStartupFinished);

        if let Some(vault_path) = resolve_vault_path() {
            this.open_vault(vault_path, cx).detach();
        } else {
            this.open_vault_prompt(cx);
        }

        this.schedule_watch_event_drain(cx);
        this.schedule_cache_diagnostics_flush(cx);

        this
    }

    fn open_vault(&mut self, vault_path: PathBuf, cx: &mut Context<Self>) -> Task<()> {
        self.vault_state = VaultState::Opening {
            path: vault_path.clone(),
        };
        self.scan_state = ScanState::Scanning;
        self.index_state = IndexState::Building;
        self.explorer_rows.clear();
        self.explorer_filter.clear();
        self.explorer_rows_filtered.clear();
        self.pending_filter_nonce = 0;
        self.pending_create_reconcile_nonce = 0;
        self.pending_created_note_reconcile.clear();
        self.pending_created_folder_reconcile.clear();
        self.pending_watch_changes_until_index_ready.clear();
        self.explorer_folder_children.clear();
        self.explorer_expanded_folders.clear();
        self.explorer_all_note_paths = Arc::new(Vec::new());
        self.explorer_all_note_paths_lower = Arc::new(Vec::new());
        self.folder_notes.clear();
        self.selected_note = None;
        self.selected_explorer_folder = None;
        self.drag_over = None;
        self.pending_order_nonce_by_folder.clear();
        self.open_editors.clear();
        self.open_note_path = None;
        self.open_note_loading = false;
        self.open_note_dirty = false;
        self.open_note_content.clear();
        self.editor_buffer = None;
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
        self.palette_backdrop_armed_until = None;
        self.search_query.clear();
        self.search_selected = 0;
        self.search_results.clear();
        self.pending_search_nonce = 0;
        self.knowledge_index = None;
        self.watcher_status = WatcherStatus {
            revision: 0,
            last_error: None,
        };
        self.watch_scan_fingerprint = 0;
        self.watch_scan_entries = 0;
        self.watch_inbox = None;
        self.bump_index_generation();
        self.settings_section = SettingsSection::Appearance;
        self.settings_open = false;
        self.settings_language_menu_open = false;
        self.settings_backdrop_armed_until = None;
        self.editor_view_mode = EditorViewMode::Edit;
        self.open_note_word_count = 0;
        self.open_note_heading_count = 0;
        self.open_note_link_count = 0;
        self.open_note_code_fence_count = 0;
        self.markdown_preview.headings.clear();
        self.markdown_preview.blocks.clear();
        self.markdown_diagnostics.clear();
        self.editor_highlight_spans.clear();
        self.pending_markdown_invalidation = None;
        self.pending_markdown_parse_nonce = 0;
        self.edit_latency_stats = EditLatencyStats::default();
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
                        let scan = vault.fast_scan_notes_and_folders()?;
                        let index = build_explorer_index(&vault, &scan)?;
                        let duration_ms = started_at.elapsed().as_millis();
                        let note_count = scan.notes.len();

                        Ok::<_, anyhow::Error>((
                            vault,
                            SharedString::from(root_name),
                            index,
                            scan.notes,
                            note_count,
                            duration_ms,
                        ))
                    })
                    .await;

                this.update(&mut cx, |this, cx| match result {
                    Ok((vault, root_name, index, scan_entries, note_count, duration_ms)) => {
                        let watch_fingerprint = compute_entries_fingerprint(&index.all_note_paths);
                        this.vault_state = VaultState::Opened { vault, root_name };
                        this.scan_state = ScanState::Ready {
                            note_count,
                            duration_ms,
                        };
                        this.index_state = IndexState::Building;
                        this.explorer_rows_filtered.clear();
                        this.explorer_folder_children = index.folder_children;
                        this.folder_notes = index.folder_notes;
                        this.explorer_all_note_paths = Arc::new(index.all_note_paths);
                        this.explorer_all_note_paths_lower = Arc::new(index.all_note_paths_lower);
                        this.knowledge_index = None;
                        this.watch_scan_fingerprint = watch_fingerprint;
                        this.watch_scan_entries = note_count;
                        this.watcher_status.last_error = None;
                        this.bump_index_generation();
                        this.start_event_watcher();
                        this.explorer_expanded_folders.clear();
                        this.explorer_expanded_folders.insert(String::new());
                        this.rebuild_explorer_rows();
                        this.status = SharedString::from("Explorer ready, building index...");
                        this.activate_plugins(PluginActivationEvent::OnVaultOpened);
                        this.rebuild_knowledge_index_async(scan_entries, cx);
                        cx.notify();
                    }
                    Err(err) => {
                        this.vault_state = VaultState::Error {
                            message: SharedString::from(err.to_string()),
                        };
                        this.scan_state = ScanState::Error {
                            message: SharedString::from("Scan failed"),
                        };
                        this.index_state = IndexState::Error {
                            message: SharedString::from("Index build failed"),
                        };
                        this.explorer_rows.clear();
                        this.explorer_rows_filtered.clear();
                        this.explorer_folder_children.clear();
                        this.explorer_expanded_folders.clear();
                        this.explorer_all_note_paths = Arc::new(Vec::new());
                        this.explorer_all_note_paths_lower = Arc::new(Vec::new());
                        this.knowledge_index = None;
                        this.watch_scan_fingerprint = 0;
                        this.watch_scan_entries = 0;
                        this.watch_inbox = None;
                        this.bump_index_generation();
                        this.folder_notes.clear();
                        this.status = SharedString::from("Scan failed");
                        this.watcher_status.last_error = Some(SharedString::from(err.to_string()));
                        cx.notify();
                    }
                })
                .ok();
            }
        })
    }

    fn rebuild_knowledge_index_async(&mut self, entries: Vec<NoteEntry>, cx: &mut Context<Self>) {
        let Some(vault) = self.vault() else {
            return;
        };

        self.index_state = IndexState::Building;
        self.bump_index_generation();
        let generation = self.index_generation;

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
                                let knowledge_index =
                                    KnowledgeIndex::build_from_entries(&vault, &entries)?;
                                let duration_ms = started_at.elapsed().as_millis();
                                Ok::<_, anyhow::Error>((knowledge_index, duration_ms))
                            }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| match result {
                    Ok((knowledge_index, duration_ms)) => {
                        if this.index_generation != generation {
                            return;
                        }

                            this.index_state = IndexState::Ready {
                                note_count: knowledge_index.note_count(),
                                duration_ms,
                            };
                        this.knowledge_index = Some(Arc::new(knowledge_index));
                        this.status = SharedString::from("Ready");

                        if !this.pending_watch_changes_until_index_ready.is_empty() {
                            let pending = std::mem::take(
                                &mut this.pending_watch_changes_until_index_ready,
                            );
                            this.apply_watch_changes(pending, cx);
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

                            cx.notify();
                        }
                        Err(err) => {
                            if this.index_generation != generation {
                                return;
                            }

                            this.index_state = IndexState::Error {
                                message: SharedString::from("Index build failed"),
                            };
                            this.knowledge_index = None;
                            this.watcher_status.last_error =
                                Some(SharedString::from(err.to_string()));
                            this.status = SharedString::from(format!("Index build failed: {err}"));
                            cx.notify();
                        }
                    })
                    .ok();
                }
            },
        )
        .detach();
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
        self.index_state = IndexState::Building;
        self.status = SharedString::from("Scanning...");
        self.knowledge_index = None;
        self.bump_index_generation();
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
                                let scan = vault.fast_scan_notes_and_folders()?;
                                let index = build_explorer_index(&vault, &scan)?;
                                let duration_ms = started_at.elapsed().as_millis();
                                let note_count = scan.notes.len();
                                Ok::<_, anyhow::Error>((
                                    index,
                                    scan.notes,
                                    note_count,
                                    duration_ms,
                                ))
                            }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| match result {
                        Ok((index, scan_entries, note_count, duration_ms)) => {
                            let watch_fingerprint =
                                compute_entries_fingerprint(&index.all_note_paths);
                            this.scan_state = ScanState::Ready {
                                note_count,
                                duration_ms,
                            };
                            this.index_state = IndexState::Building;
                            this.explorer_folder_children = index.folder_children;
                            this.folder_notes = index.folder_notes;
                            this.explorer_all_note_paths = Arc::new(index.all_note_paths);
                            this.explorer_all_note_paths_lower =
                                Arc::new(index.all_note_paths_lower);
                            this.watch_scan_fingerprint = watch_fingerprint;
                            this.watch_scan_entries = note_count;
                            this.watcher_status.revision =
                                this.watcher_status.revision.wrapping_add(1);
                            this.watcher_status.last_error = None;
                            this.start_event_watcher();
                            this.rebuild_explorer_rows();
                            if this.is_filtering() {
                                this.schedule_apply_filter(Duration::ZERO, cx);
                            }
                            this.status = SharedString::from("Explorer ready, building index...");
                            this.rebuild_knowledge_index_async(scan_entries, cx);
                            cx.notify();
                        }
                        Err(err) => {
                            this.scan_state = ScanState::Error {
                                message: SharedString::from("Scan failed"),
                            };
                            this.index_state = IndexState::Error {
                                message: SharedString::from("Index build failed"),
                            };
                            this.watcher_status.last_error =
                                Some(SharedString::from(err.to_string()));
                            this.watch_inbox = None;
                            this.bump_index_generation();
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
        resolve_base_folder_for_new_items(
            self.selected_explorer_folder.as_deref(),
            self.selected_note.as_deref(),
            &self.folder_notes,
            &self.explorer_folder_children,
        )
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
                            this.add_note_optimistically(&path, cx);
                            this.open_note(path, cx);
                            this.schedule_reconcile_after_create(cx);
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
                            this.add_folder_optimistically(&folder, cx);
                            this.schedule_reconcile_after_create(cx);
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
                self.editor_buffer = None;
                self.editor_view_mode = EditorViewMode::Edit;
                self.open_note_word_count = 0;
                self.open_note_heading_count = 0;
                self.open_note_link_count = 0;
                self.open_note_code_fence_count = 0;
                self.markdown_preview.headings.clear();
                self.markdown_preview.blocks.clear();
                self.markdown_diagnostics.clear();
                self.editor_highlight_spans.clear();
                self.pending_markdown_invalidation = None;
                self.pending_markdown_parse_nonce = 0;
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
        self.palette_backdrop_armed_until = Some(Instant::now() + OVERLAY_BACKDROP_ARM_DELAY);
        cx.notify();
    }

    fn close_palette(&mut self, cx: &mut Context<Self>) {
        self.palette_open = false;
        self.palette_query.clear();
        self.palette_selected = 0;
        self.palette_results.clear();
        self.pending_palette_nonce = 0;
        self.palette_backdrop_armed_until = None;
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
        self.vault_prompt_backdrop_armed_until = Some(Instant::now() + OVERLAY_BACKDROP_ARM_DELAY);
        self.palette_open = false;
        self.palette_backdrop_armed_until = None;
        self.settings_open = false;
        self.settings_language_menu_open = false;
        self.settings_backdrop_armed_until = None;
        cx.notify();
    }

    fn close_vault_prompt(&mut self, cx: &mut Context<Self>) {
        self.vault_prompt_open = false;
        self.vault_prompt_needs_focus = false;
        self.vault_prompt_error = None;
        self.vault_prompt_backdrop_armed_until = None;
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

    fn rebuild_keymap_from_settings(&mut self) -> Result<(), String> {
        let keymap = self
            .app_settings
            .build_keymap()
            .map_err(|err| err.to_string())?;
        self.keymap = keymap;
        Ok(())
    }

    fn reset_hotkey_override(&mut self, command: CommandId) -> bool {
        self.app_settings
            .keymap_overrides
            .remove(command.as_str())
            .is_some()
    }

    fn reset_all_hotkey_overrides(&mut self) -> bool {
        let changed = !self.app_settings.keymap_overrides.is_empty();
        self.app_settings.keymap_overrides.clear();
        changed
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
        self.app_settings.appearance.theme = self.settings_theme.as_tag().to_string();
        self.app_settings.appearance.accent = self.settings_accent.as_tag().to_string();
        self.app_settings.plugin_policy.runtime_mode =
            self.plugin_runtime_mode.as_tag().to_string();
        self.app_settings.bookmarked_notes = self.bookmarked_notes_snapshot();

        if self.app_settings.editor.autosave_delay_ms < 100 {
            self.app_settings.editor.autosave_delay_ms = 100;
        }

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

    fn autosave_delay(&self) -> Duration {
        Duration::from_millis(self.app_settings.editor.autosave_delay_ms.max(100))
    }

    fn bookmarked_notes_snapshot(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for path in &self.app_settings.bookmarked_notes {
            let Ok(path) = normalize_vault_rel_path(path) else {
                continue;
            };
            if seen.insert(path.clone()) {
                out.push(path);
            }
        }
        out
    }

    fn note_exists(&self, path: &str) -> bool {
        self.explorer_all_note_paths
            .iter()
            .any(|existing| existing == path)
    }

    fn note_title_for_path(&self, path: &str) -> String {
        if self.open_note_path.as_deref() == Some(path) {
            return self.derive_note_title(path);
        }

        self.knowledge_index
            .as_ref()
            .and_then(|index| index.note_summary(path))
            .map(|summary| {
                if summary.title.trim().is_empty() {
                    file_name(path)
                } else {
                    summary.title
                }
            })
            .unwrap_or_else(|| file_name(path))
    }

    fn toggle_current_note_bookmark(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.open_note_path.clone() else {
            return;
        };
        let Ok(path) = normalize_vault_rel_path(&path) else {
            return;
        };

        if let Some(ix) = self
            .app_settings
            .bookmarked_notes
            .iter()
            .position(|existing| existing == &path)
        {
            self.app_settings.bookmarked_notes.remove(ix);
            self.status = SharedString::from("Bookmark removed");
        } else {
            self.app_settings.bookmarked_notes.push(path.clone());
            self.status = SharedString::from("Bookmark added");
        }

        self.persist_settings();
        cx.notify();
    }

    fn refresh_runtime_mode_from_settings(&mut self) {
        let runtime_mode =
            PluginRuntimeMode::from_tag(self.app_settings.plugin_policy.runtime_mode.as_str());
        if self.plugin_runtime_mode != runtime_mode {
            self.plugin_runtime_mode = runtime_mode;
        }
        self.plugin_registry
            .set_policy(self.app_settings.to_plugin_policy());
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
                self.settings_backdrop_armed_until =
                    Some(Instant::now() + OVERLAY_BACKDROP_ARM_DELAY);
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
            CommandId::Undo => {
                self.close_palette(cx);
                self.apply_editor_history(true, cx);
            }
            CommandId::Redo => {
                self.close_palette(cx);
                self.apply_editor_history(false, cx);
            }
            CommandId::ToggleSplit => {
                self.close_palette(cx);
                self.toggle_editor_split_mode(cx);
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

    fn editor_mode_label(&self) -> &'static str {
        match self.editor_view_mode {
            EditorViewMode::Edit => "EDIT",
            EditorViewMode::Preview => "PREVIEW",
            EditorViewMode::Split => "SPLIT",
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
        let ui_theme = UiTheme::from_settings(self.settings_theme, self.settings_accent);
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
        let input_color = if query_empty {
            ui_theme.text_subtle
        } else {
            ui_theme.text_primary
        };

        let item_count = match self.palette_mode {
            PaletteMode::Commands => self.filtered_palette_command_indices().len(),
            PaletteMode::QuickOpen => self.palette_results.len(),
        };

        let list = uniform_list(
            "palette.items",
            item_count.max(1),
            cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
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
                                        .text_color(rgb(ui_theme.text_muted))
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
                                let icon_color = if selected {
                                    ui_theme.accent
                                } else {
                                    ui_theme.text_muted
                                };
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
                                                    .text_color(rgb(ui_theme.text_primary))
                                                    .child(label),
                                            )
                                            .child(
                                                div()
                                                    .font_family("Inter")
                                                    .text_size(px(11.))
                                                    .font_weight(FontWeight(650.))
                                                    .text_color(rgb(ui_theme.text_muted))
                                                    .child(detail),
                                            ),
                                    );

                                let shortcut = div()
                                    .h(px(28.))
                                    .bg(rgb(ui_theme.surface_alt_bg))
                                    .border_1()
                                    .border_color(rgb(ui_theme.border))
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .font_family("IBM Plex Mono")
                                            .text_size(px(12.))
                                            .font_weight(FontWeight(750.))
                                            .text_color(rgb(ui_theme.text_muted))
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
                                        rgb(ui_theme.accent_soft)
                                    } else {
                                        rgba(0x00000000)
                                    })
                                    .when(selected, |this| {
                                        this.border_1().border_color(rgb(ui_theme.accent_soft))
                                    })
                                    .when(selected, |this| {
                                        this.child(
                                            div()
                                                .absolute()
                                                .top(px(-8.))
                                                .right(px(4.))
                                                .child(ui_corner_tag(ui_theme.accent)),
                                        )
                                    })
                                    .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
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
                                        .text_color(rgb(ui_theme.text_muted))
                                        .child(if ix == 0 { msg } else { "" })
                                })
                                .collect::<Vec<_>>();
                        }

                        range
                            .map(|ix| {
                                let Some(open_match) = this.palette_results.get(ix) else {
                                    return div()
                                        .id(ElementId::named_usize(
                                            "palette.quick_open.missing",
                                            ix,
                                        ))
                                        .h(px(44.))
                                        .px_3()
                                        .child("");
                                };
                                let path = open_match.path.clone();

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
                                        rgb(ui_theme.accent_soft)
                                    } else {
                                        rgba(0x00000000)
                                    })
                                    .when(selected, |this| {
                                        this.border_1().border_color(rgb(ui_theme.accent_soft))
                                    })
                                    .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                                    .on_click(cx.listener(
                                        move |this, _ev: &ClickEvent, _window, cx| {
                                            this.palette_selected = ix;
                                            this.close_palette(cx);
                                            this.open_note(open_path.clone(), cx);
                                        },
                                    ))
                                    .child(ui_icon(ICON_FILE_TEXT, 16., ui_theme.text_muted))
                                    .child(
                                        div()
                                            .font_family("IBM Plex Mono")
                                            .text_size(px(11.))
                                            .font_weight(FontWeight(750.))
                                            .text_color(rgb(ui_theme.text_primary))
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
            .bg(rgb(ui_theme.surface_alt_bg))
            .border_1()
            .border_color(rgb(ui_theme.border))
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .bg(rgb(ui_theme.surface_alt_bg))
                    .p_3()
                    .gap(px(10.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(title),
                    )
                    .child(
                        div()
                            .id("palette.input")
                            .h(px(44.))
                            .w_full()
                            .bg(rgb(ui_theme.surface_bg))
                            .border_1()
                            .border_color(rgb(ui_theme.border))
                            .px_3()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .focusable()
                            .cursor_text()
                            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                                this.on_palette_key(ev, cx);
                            }))
                            .child(ui_icon(ICON_SEARCH, 16., ui_theme.text_subtle))
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
                    .bg(rgb(ui_theme.surface_alt_bg))
                    .p(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_muted))
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
            .occlude()
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
                        let armed = this
                            .palette_backdrop_armed_until
                            .is_none_or(|deadline| Instant::now() >= deadline);
                        if armed {
                            this.close_palette(cx);
                        }
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
                    .occlude()
                    .child(palette_box),
            )
    }

    fn vault_prompt_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_theme = UiTheme::from_settings(self.settings_theme, self.settings_accent);
        let query_empty = self.vault_prompt_value.trim().is_empty();
        let input_text = if query_empty {
            SharedString::from("Type a vault folder path…")
        } else {
            SharedString::from(self.vault_prompt_value.clone())
        };
        let input_color = if query_empty {
            ui_theme.text_subtle
        } else {
            ui_theme.text_primary
        };

        let error = self.vault_prompt_error.clone();

        let prompt_box = div()
            .w(px(720.))
            .bg(rgb(ui_theme.surface_alt_bg))
            .border_1()
            .border_color(rgb(ui_theme.border))
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .bg(rgb(ui_theme.surface_alt_bg))
                    .p_3()
                    .gap(px(10.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child("OPEN VAULT"),
                    )
                    .child(
                        div()
                            .id("vault_prompt.input")
                            .h(px(44.))
                            .w_full()
                            .bg(rgb(ui_theme.surface_bg))
                            .border_1()
                            .border_color(rgb(ui_theme.border))
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
                            .child(ui_icon(ICON_VAULT, 16., ui_theme.text_subtle))
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
                            .text_color(rgb(ui_theme.text_muted))
                            .child("Enter to open 路 Esc to cancel"),
                    ),
            );

        div()
            .id("vault_prompt.overlay")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .occlude()
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
                        let armed = this
                            .vault_prompt_backdrop_armed_until
                            .is_none_or(|deadline| Instant::now() >= deadline);
                        if armed {
                            this.close_vault_prompt(cx);
                        }
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
                    .occlude()
                    .child(prompt_box),
            )
    }

    fn settings_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_theme = UiTheme::from_settings(self.settings_theme, self.settings_accent);
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
                        rgb(ui_theme.accent_soft)
                    } else {
                        rgba(0x00000000)
                    })
                    .when(active, |this| {
                        this.border_1().border_color(rgb(ui_theme.accent_soft))
                    })
                    .when(active, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(-8.))
                                .right(px(12.))
                                .child(ui_corner_tag(ui_theme.accent)),
                        )
                    })
                    .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        this.settings_section = section;
                        this.settings_language_menu_open = false;
                        cx.notify();
                    }))
                    .child(ui_icon(
                        icon,
                        16.,
                        if active {
                            ui_theme.accent
                        } else {
                            ui_theme.text_muted
                        },
                    ))
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(13.))
                            .font_weight(FontWeight(if active { 800. } else { 700. }))
                            .text_color(rgb(if active {
                                ui_theme.accent
                            } else {
                                ui_theme.text_primary
                            }))
                            .child(label),
                    )
            };

        let card = |title: String, desc: String, body: gpui::AnyElement| {
            div()
                .w_full()
                .bg(rgb(ui_theme.surface_alt_bg))
                .border_1()
                .border_color(rgb(ui_theme.border))
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(13.))
                        .font_weight(FontWeight(800.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(title),
                )
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child(desc),
                )
                .child(body)
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
                        rgb(ui_theme.accent_soft)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        this.settings_theme = theme;
                        this.persist_settings();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(13.))
                            .font_weight(FontWeight(if active { 800. } else { 700. }))
                            .text_color(rgb(if active {
                                ui_theme.accent
                            } else {
                                ui_theme.text_primary
                            }))
                            .child(label),
                    )
            };

            let segmented = div()
                .h(px(36.))
                .bg(rgb(ui_theme.surface_alt_bg))
                .border_1()
                .border_color(rgb(ui_theme.border))
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
                    .bg(if active {
                        rgb(ui_theme.accent_soft)
                    } else {
                        rgb(ui_theme.surface_alt_bg)
                    })
                    .border_1()
                    .border_color(rgb(if active {
                        ui_theme.accent_soft
                    } else {
                        ui_theme.border
                    }))
                    .cursor_pointer()
                    .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        this.settings_accent = accent;
                        this.persist_settings();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(12.))
                            .font_weight(FontWeight(if active { 800. } else { 700. }))
                            .text_color(rgb(if active {
                                ui_theme.accent
                            } else {
                                ui_theme.text_primary
                            }))
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
                        .border_color(rgb(ui_theme.border)),
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
                .bg(rgb(ui_theme.surface_alt_bg))
                .border_1()
                .border_color(rgb(ui_theme.border))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    this.settings_language_menu_open = !this.settings_language_menu_open;
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(13.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(language_name),
                )
                .child(ui_icon(ICON_CHEVRON_DOWN, 16., ui_theme.text_muted));

            let language_menu = div()
                .id("settings.language.menu")
                .w(px(240.))
                .bg(rgb(ui_theme.surface_bg))
                .border_1()
                .border_color(rgb(ui_theme.border))
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
                        .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
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
                                .text_color(rgb(ui_theme.text_primary))
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
                        .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
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
                                .text_color(rgb(ui_theme.text_primary))
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
                        .text_color(rgb(ui_theme.text_muted))
                        .child(self.i18n.text("settings.section.language")),
                )
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_muted))
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
                                .text_color(rgb(ui_theme.text_muted))
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
                                .text_color(rgb(ui_theme.text_muted))
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
                                        .text_color(rgb(ui_theme.text_primary))
                                        .child(self.i18n.text("settings.colors.accent")),
                                )
                                .child(
                                    div()
                                        .font_family("Inter")
                                        .text_size(px(12.))
                                        .font_weight(FontWeight(650.))
                                        .text_color(rgb(ui_theme.text_muted))
                                        .child(self.i18n.text("settings.colors.accent.hint")),
                                )
                                .child(accent_row),
                        ),
                )
                .child(language_section)
                .into_any_element()
        };

        let about_content = {
            let open_note = self
                .open_note_path
                .clone()
                .unwrap_or_else(|| "(none)".to_string());
            let note_count = self.explorer_all_note_paths.len();
            let bookmark_count = self.app_settings.bookmarked_notes.len();
            let runtime = match &self.plugin_activation_state {
                PluginActivationState::Idle => "idle".to_string(),
                PluginActivationState::Activating => "activating".to_string(),
                PluginActivationState::Ready { active_count } => format!("ready ({active_count})"),
                PluginActivationState::Error { message } => format!("error ({message})"),
            };

            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(card(
                    "Workspace".to_string(),
                    "Current Knowledge workspace and content stats.".to_string(),
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(12.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_secondary))
                        .child(format!(
                            "Vault notes: {note_count}\nOpen note: {open_note}\nBookmarks: {bookmark_count}"
                        ))
                        .into_any_element(),
                ))
                .child(card(
                    "Runtime".to_string(),
                    "Plugin runtime activation and mode summary.".to_string(),
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(12.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_secondary))
                        .child(format!(
                            "Plugins: {}\nRuntime mode: {}\nRuntime status: {}",
                            self.plugin_registry.list().len(),
                            self.plugin_runtime_mode.as_tag(),
                            runtime,
                        ))
                        .into_any_element(),
                ))
                .into_any_element()
        };

        let editor_content = {
            let autosave_select = div()
                .id("settings.editor.autosave")
                .w(px(260.))
                .h(px(34.))
                .bg(rgb(ui_theme.surface_alt_bg))
                .border_1()
                .border_color(rgb(ui_theme.border))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    let current = this.app_settings.editor.autosave_delay_ms;
                    let next = if current <= 500 {
                        1000
                    } else if current <= 1000 {
                        2000
                    } else {
                        500
                    };
                    this.app_settings.editor.autosave_delay_ms = next;
                    this.editor_autosave_delay_input = next.to_string();
                    this.persist_settings();
                    this.status = SharedString::from(format!("Autosave delay set to {next} ms"));
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(13.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(format!("{} ms", self.app_settings.editor.autosave_delay_ms)),
                )
                .child(ui_icon(ICON_CHEVRON_DOWN, 16., ui_theme.text_muted));

            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(card(
                    "Autosave".to_string(),
                    "Controls delayed writeback while editing markdown notes.".to_string(),
                    autosave_select.into_any_element(),
                ))
                .child(card(
                    "Editor behavior".to_string(),
                    "Core editing features are active: selection, IME, clipboard and save."
                        .to_string(),
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(12.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_secondary))
                        .child("Use Ctrl+S to force-save. Autosave applies after idle delay.")
                        .into_any_element(),
                ))
                .into_any_element()
        };

        let files_links_content = {
            let external_sync_toggle = div()
                .id("settings.files.external_sync")
                .h(px(28.))
                .px(px(10.))
                .bg(if self.app_settings.files_links.external_sync {
                    rgb(ui_theme.accent_soft)
                } else {
                    rgb(ui_theme.surface_alt_bg)
                })
                .border_1()
                .border_color(rgb(ui_theme.border))
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    this.app_settings.files_links.external_sync =
                        !this.app_settings.files_links.external_sync;
                    this.persist_settings();
                    this.status =
                        SharedString::from(if this.app_settings.files_links.external_sync {
                            "External sync enabled"
                        } else {
                            "External sync disabled"
                        });
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(if self.app_settings.files_links.external_sync {
                            "Enabled"
                        } else {
                            "Disabled"
                        }),
                );

            let wiki_pref_toggle = div()
                .id("settings.files.prefer_wiki")
                .h(px(28.))
                .px(px(10.))
                .bg(if self.app_settings.files_links.prefer_wikilink_titles {
                    rgb(ui_theme.accent_soft)
                } else {
                    rgb(ui_theme.surface_alt_bg)
                })
                .border_1()
                .border_color(rgb(ui_theme.border))
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    this.app_settings.files_links.prefer_wikilink_titles =
                        !this.app_settings.files_links.prefer_wikilink_titles;
                    this.persist_settings();
                    this.status = SharedString::from(
                        if this.app_settings.files_links.prefer_wikilink_titles {
                            "Prefer wikilink titles enabled"
                        } else {
                            "Prefer wikilink titles disabled"
                        },
                    );
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(if self.app_settings.files_links.prefer_wikilink_titles {
                            "Enabled"
                        } else {
                            "Disabled"
                        }),
                );

            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(card(
                    "External changes sync".to_string(),
                    "Whether to sync external editor updates via watcher.".to_string(),
                    external_sync_toggle.into_any_element(),
                ))
                .child(card(
                    "Wiki link preference".to_string(),
                    "Prefer wiki title resolution over strict path matching.".to_string(),
                    wiki_pref_toggle.into_any_element(),
                ))
                .into_any_element()
        };

        let hotkeys_content = {
            let mut list = div().w_full().flex().flex_col().gap(px(6.));

            for spec in command_specs() {
                let command_id = spec.id;
                let shortcut = self.command_shortcut(spec.id);
                let is_editing = self.hotkey_editing_command == Some(spec.id);
                let row = div()
                    .w_full()
                    .h(px(36.))
                    .px_3()
                    .bg(rgb(ui_theme.surface_alt_bg))
                    .border_1()
                    .border_color(rgb(ui_theme.border))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_family("Inter")
                            .text_size(px(12.))
                            .font_weight(FontWeight(700.))
                            .text_color(rgb(ui_theme.text_primary))
                            .child(self.command_label(spec.id)),
                    )
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(750.))
                            .text_color(rgb(ui_theme.text_secondary))
                            .child(if is_editing {
                                if self.hotkey_editing_value.trim().is_empty() {
                                    "Press keys...".to_string()
                                } else {
                                    self.hotkey_editing_value.clone()
                                }
                            } else {
                                shortcut
                            }),
                    )
                    .child(
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "settings.hotkeys.edit:{}",
                                command_id.as_str()
                            ))))
                            .h(px(24.))
                            .px(px(8.))
                            .bg(if is_editing {
                                rgb(ui_theme.accent_soft)
                            } else {
                                rgb(ui_theme.surface_alt_bg)
                            })
                            .border_1()
                            .border_color(rgb(ui_theme.border))
                            .cursor_pointer()
                            .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                                if this.hotkey_editing_command == Some(command_id) {
                                    this.hotkey_editing_command = None;
                                    this.hotkey_editing_value.clear();
                                } else {
                                    this.hotkey_editing_command = Some(command_id);
                                    this.hotkey_editing_value.clear();
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .font_family("Inter")
                                    .text_size(px(11.))
                                    .font_weight(FontWeight(750.))
                                    .text_color(rgb(ui_theme.text_primary))
                                    .child(if is_editing { "Cancel" } else { "Edit" }),
                            ),
                    )
                    .child(
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "settings.hotkeys.reset:{}",
                                command_id.as_str()
                            ))))
                            .h(px(24.))
                            .px(px(8.))
                            .bg(rgb(ui_theme.surface_alt_bg))
                            .border_1()
                            .border_color(rgb(ui_theme.border))
                            .cursor_pointer()
                            .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                                let changed = this.reset_hotkey_override(command_id);
                                if this.hotkey_editing_command == Some(command_id) {
                                    this.hotkey_editing_command = None;
                                    this.hotkey_editing_value.clear();
                                }

                                if changed {
                                    match this.rebuild_keymap_from_settings() {
                                        Ok(()) => {
                                            this.persist_settings();
                                            this.status = SharedString::from(format!(
                                                "Reset shortcut: {}",
                                                command_id.as_str()
                                            ));
                                        }
                                        Err(err) => {
                                            this.status = SharedString::from(format!(
                                                "Reset shortcut failed ({}): {err}",
                                                command_id.as_str()
                                            ));
                                        }
                                    }
                                } else {
                                    this.status = SharedString::from(format!(
                                        "Shortcut already default: {}",
                                        command_id.as_str()
                                    ));
                                }

                                cx.notify();
                            }))
                            .child(
                                div()
                                    .font_family("Inter")
                                    .text_size(px(11.))
                                    .font_weight(FontWeight(750.))
                                    .text_color(rgb(ui_theme.text_primary))
                                    .child("Reset"),
                            ),
                    );

                list = list.child(row);
            }

            let reset_all = div()
                .id("settings.hotkeys.reset_all")
                .h(px(28.))
                .px(px(10.))
                .bg(rgb(ui_theme.surface_alt_bg))
                .border_1()
                .border_color(rgb(ui_theme.border))
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    let changed = this.reset_all_hotkey_overrides();
                    this.hotkey_editing_command = None;
                    this.hotkey_editing_value.clear();

                    if changed {
                        match this.rebuild_keymap_from_settings() {
                            Ok(()) => {
                                this.persist_settings();
                                this.status = SharedString::from("Reset all shortcuts to defaults");
                            }
                            Err(err) => {
                                this.status = SharedString::from(format!(
                                    "Reset all shortcuts failed: {err}"
                                ));
                            }
                        }
                    } else {
                        this.status = SharedString::from("All shortcuts already default");
                    }

                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.))
                        .font_weight(FontWeight(800.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child("Reset all to defaults"),
                );

            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(card(
                    "Command shortcuts".to_string(),
                    "Click Edit then press a new key chord in this window to override.".to_string(),
                    list.into_any_element(),
                ))
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(11.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child("Tip: Esc exits editing mode. Overrides are persisted to settings."),
                )
                .child(reset_all)
                .into_any_element()
        };

        let advanced_content = {
            let runtime_mode_label = if self.plugin_runtime_mode == PluginRuntimeMode::Process {
                "process"
            } else {
                "in_process"
            };

            let runtime_mode_toggle = div()
                .id("settings.advanced.runtime_mode")
                .h(px(28.))
                .px(px(10.))
                .bg(rgb(ui_theme.surface_alt_bg))
                .border_1()
                .border_color(rgb(ui_theme.border))
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    this.plugin_runtime_mode =
                        if this.plugin_runtime_mode == PluginRuntimeMode::InProcess {
                            PluginRuntimeMode::Process
                        } else {
                            PluginRuntimeMode::InProcess
                        };
                    this.app_settings.plugin_policy.runtime_mode =
                        this.plugin_runtime_mode.as_tag().to_string();
                    this.persist_settings();
                    this.refresh_runtime_mode_from_settings();
                    this.status = SharedString::from(format!(
                        "Plugin runtime mode set to {}",
                        this.plugin_runtime_mode.as_tag()
                    ));
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(11.))
                        .font_weight(FontWeight(750.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(runtime_mode_label),
                );

            let watcher_toggle = div()
                .id("settings.advanced.watcher")
                .h(px(28.))
                .px(px(10.))
                .bg(if self.app_settings.files_links.external_sync {
                    rgb(ui_theme.accent_soft)
                } else {
                    rgb(ui_theme.surface_alt_bg)
                })
                .border_1()
                .border_color(rgb(ui_theme.border))
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    this.app_settings.files_links.external_sync =
                        !this.app_settings.files_links.external_sync;
                    if this.app_settings.files_links.external_sync {
                        this.start_event_watcher();
                    } else {
                        this.watch_inbox = None;
                    }
                    this.persist_settings();
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(if self.app_settings.files_links.external_sync {
                            "Watcher Enabled"
                        } else {
                            "Watcher Disabled"
                        }),
                );

            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(card(
                    "Plugin runtime mode".to_string(),
                    "Switch between in-process and process-isolated runtime.".to_string(),
                    runtime_mode_toggle.into_any_element(),
                ))
                .child(card(
                    "File watcher".to_string(),
                    "Enable or disable external file synchronization watcher.".to_string(),
                    watcher_toggle.into_any_element(),
                ))
                .into_any_element()
        };

        let page_content = match self.settings_section {
            SettingsSection::Appearance => appearance_content,
            SettingsSection::About => about_content,
            SettingsSection::Editor => editor_content,
            SettingsSection::FilesLinks => files_links_content,
            SettingsSection::Hotkeys => hotkeys_content,
            SettingsSection::Advanced => advanced_content,
        };

        let modal = div()
            .w(px(980.))
            .h(px(640.))
            .bg(rgb(ui_theme.surface_alt_bg))
            .border_1()
            .border_color(rgb(ui_theme.border))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(44.))
                    .w_full()
                    .bg(rgb(ui_theme.surface_alt_bg))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(13.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_primary))
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
                            .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                            .occlude()
                            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                                this.settings_open = false;
                                this.settings_language_menu_open = false;
                                this.settings_backdrop_armed_until = None;
                                cx.notify();
                            }))
                            .child(ui_icon(ICON_X, 16., ui_theme.text_muted)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(
                        div()
                            .id("settings.nav.scroll")
                            .w(px(240.))
                            .h_full()
                            .overflow_y_scroll()
                            .bg(rgb(ui_theme.surface_alt_bg))
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
                    .child(div().w(px(1.)).h_full().bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .h_full()
                            .bg(rgb(ui_theme.surface_bg))
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
                                    .text_color(rgb(ui_theme.text_primary))
                                    .child(page_title),
                            )
                            .child(
                                div()
                                    .id("settings.content.scroll")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .pr(px(4.))
                                    .child(page_content),
                            ),
                    ),
            );

        div()
            .id("settings.overlay")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .occlude()
            .focusable()
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                if this.on_settings_key(ev, cx) {
                    return;
                }
                if ev.keystroke.key.eq_ignore_ascii_case("escape") {
                    this.settings_open = false;
                    this.settings_language_menu_open = false;
                    this.settings_backdrop_armed_until = None;
                    cx.notify();
                }
            }))
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
                        let armed = this
                            .settings_backdrop_armed_until
                            .is_none_or(|deadline| Instant::now() >= deadline);
                        if armed {
                            this.settings_open = false;
                            this.settings_language_menu_open = false;
                            this.settings_backdrop_armed_until = None;
                            cx.notify();
                        }
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
                    .occlude()
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

                    let Some((query, vault, knowledge_index, search_options, generation)) = this
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

                            if let Some(cached) = this.search_query_cache.get(&query).cloned() {
                                this.cache_stats.search_hits =
                                    this.cache_stats.search_hits.wrapping_add(1);
                                this.search_selected = 0;
                                this.search_results = cached;
                                let _ = touch_cache_order(
                                    &query,
                                    &mut this.search_query_cache_order,
                                    SEARCH_QUERY_CACHE_CAPACITY,
                                );
                                cx.notify();
                                return None;
                            }
                            this.cache_stats.search_misses =
                                this.cache_stats.search_misses.wrapping_add(1);

                            let Some(vault) = this.vault() else {
                                this.search_selected = 0;
                                this.search_results.clear();
                                cx.notify();
                                return None;
                            };

                            let Some(knowledge_index) = this.knowledge_index.clone() else {
                                this.search_selected = 0;
                                this.search_results.clear();
                                cx.notify();
                                return None;
                            };

                            Some((
                                query,
                                vault,
                                knowledge_index,
                                this.search_options.clone(),
                                this.index_generation,
                            ))
                        })
                        .ok()
                        .flatten()
                    else {
                        return;
                    };

                    let query_for_task = query.clone();

                    let search_rows: Vec<SearchRow> = cx
                        .background_executor()
                        .spawn(async move {
                            let mut out = Vec::new();
                            let outcome = knowledge_index.search(
                                &vault,
                                &query_for_task,
                                search_options.clone(),
                            );
                            let max_rows = search_options.max_match_rows;
                            let mut rows = 0usize;

                            for hit in outcome.hits {
                                if rows >= max_rows {
                                    break;
                                }

                                out.push(SearchRow::File {
                                    path: hit.path.clone(),
                                    match_count: hit.match_count,
                                });

                                for preview in hit.previews {
                                    if rows >= max_rows {
                                        break;
                                    }
                                    out.push(SearchRow::Match {
                                        path: hit.path.clone(),
                                        line: preview.line,
                                        preview: preview.preview,
                                    });
                                    rows += 1;
                                }
                            }

                            out
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_search_nonce != nonce {
                            return;
                        }
                        if this.index_generation != generation {
                            return;
                        }

                        this.search_query_cache
                            .insert(query.clone(), search_rows.clone());
                        if let Some(evicted) = touch_cache_order(
                            &query,
                            &mut this.search_query_cache_order,
                            SEARCH_QUERY_CACHE_CAPACITY,
                        ) {
                            this.search_query_cache.remove(&evicted);
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

                    let Some((query, knowledge_index, generation)) = this
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

                            if let Some(cached) = this.quick_open_query_cache.get(&query).cloned() {
                                this.cache_stats.quick_open_hits =
                                    this.cache_stats.quick_open_hits.wrapping_add(1);
                                this.palette_selected = 0;
                                this.palette_results = cached;
                                let _ = touch_cache_order(
                                    &query,
                                    &mut this.quick_open_query_cache_order,
                                    QUICK_OPEN_CACHE_CAPACITY,
                                );
                                cx.notify();
                                return None;
                            }
                            this.cache_stats.quick_open_misses =
                                this.cache_stats.quick_open_misses.wrapping_add(1);

                            let Some(knowledge_index) = this.knowledge_index.clone() else {
                                this.palette_selected = 0;
                                this.palette_results.clear();
                                cx.notify();
                                return None;
                            };

                            Some((query, knowledge_index, this.index_generation))
                        })
                        .ok()
                        .flatten()
                    else {
                        return;
                    };

                    let query_for_task = query.clone();

                    let matched_paths: Vec<OpenPathMatch> = cx
                        .background_executor()
                        .spawn(async move {
                            knowledge_index
                                .quick_open_paths(&query_for_task, 200)
                                .into_iter()
                                .map(|path| OpenPathMatch { path })
                                .collect()
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_palette_nonce != nonce {
                            return;
                        }
                        if this.index_generation != generation {
                            return;
                        }

                        this.quick_open_query_cache
                            .insert(query.clone(), matched_paths.clone());
                        if let Some(evicted) = touch_cache_order(
                            &query,
                            &mut this.quick_open_query_cache_order,
                            QUICK_OPEN_CACHE_CAPACITY,
                        ) {
                            this.quick_open_query_cache.remove(&evicted);
                        }

                        this.palette_results = matched_paths;
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

    fn start_event_watcher(&mut self) {
        let Some(vault) = self.vault() else {
            self.watch_inbox = None;
            return;
        };

        let root = vault.root().to_path_buf();
        let (tx, rx) = mpsc::channel::<WatchInboxMessage>();

        std::thread::spawn(move || {
            let watcher = match VaultWatcher::new(&root) {
                Ok(watcher) => watcher,
                Err(err) => {
                    let _ = tx.send(WatchInboxMessage::Error(err.to_string()));
                    return;
                }
            };

            loop {
                match watcher.recv_batch(WATCH_EVENT_DEBOUNCE, WATCH_EVENT_BATCH_MAX) {
                    Ok(changes) => {
                        if changes.is_empty() {
                            continue;
                        }
                        if tx.send(WatchInboxMessage::Changes(changes)).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = tx.send(WatchInboxMessage::Error(err.to_string()));
                        break;
                    }
                }
            }
        });

        self.watch_inbox = Some(rx);
    }

    fn schedule_watch_event_drain(&mut self, cx: &mut Context<Self>) {
        cx.spawn(|this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
            let mut cx = cx.clone();
            async move {
                loop {
                    Timer::after(WATCH_EVENT_DRAIN_INTERVAL).await;

                    this.update(&mut cx, |this, cx| {
                        let mut messages = Vec::new();
                        let mut disconnected = false;

                        if let Some(rx) = this.watch_inbox.as_ref() {
                            loop {
                                match rx.try_recv() {
                                    Ok(msg) => messages.push(msg),
                                    Err(TryRecvError::Empty) => break,
                                    Err(TryRecvError::Disconnected) => {
                                        disconnected = true;
                                        break;
                                    }
                                }
                            }
                        }

                        if disconnected {
                            this.watch_inbox = None;
                            this.watcher_status.last_error =
                                Some(SharedString::from("watcher disconnected"));
                            this.status = SharedString::from("Watcher disconnected");
                        }

                        let had_messages = !messages.is_empty();

                        for message in messages {
                            match message {
                                WatchInboxMessage::Changes(changes) => {
                                    this.apply_watch_changes(changes, cx)
                                }
                                WatchInboxMessage::Error(err) => {
                                    this.watch_inbox = None;
                                    this.watcher_status.last_error =
                                        Some(SharedString::from(err.clone()));
                                    this.status =
                                        SharedString::from(format!("Watcher error: {err}"));
                                }
                            }
                        }

                        if disconnected || had_messages {
                            cx.notify();
                        }
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn apply_watch_changes(&mut self, changes: Vec<VaultWatchChange>, cx: &mut Context<Self>) {
        if changes.is_empty() {
            return;
        }

        if self.knowledge_index.is_none() {
            self.pending_watch_changes_until_index_ready.extend(changes);
            return;
        }

        if matches!(self.scan_state, ScanState::Scanning)
            || matches!(self.index_state, IndexState::Building)
        {
            return;
        }

        let Some(vault) = self.vault() else {
            return;
        };

        if changes
            .iter()
            .any(|change| matches!(change, VaultWatchChange::RescanRequired))
        {
            self.watcher_status.revision = self.watcher_status.revision.wrapping_add(1);
            self.watcher_status.last_error = None;
            self.status = SharedString::from("External changes detected");
            self.rescan_vault(cx);
            return;
        }

        let existing_paths_vec = self.explorer_all_note_paths.as_ref().clone();
        let existing_paths = existing_paths_vec
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();

        let mut upsert_paths = Vec::new();
        let mut new_note_paths = Vec::new();
        let mut removed_note_paths = Vec::new();
        let mut moved_note_pairs = Vec::new();
        let mut folder_removed_note_paths = Vec::new();
        let mut folder_moved_note_pairs = Vec::new();
        let mut needs_rescan = false;
        let mut bookmarks_changed = false;

        for change in changes {
            match change {
                VaultWatchChange::NoteChanged { path } => {
                    if existing_paths.contains(path.as_str()) {
                        upsert_paths.push(path);
                    } else {
                        new_note_paths.push(path);
                    }
                }
                VaultWatchChange::NoteRemoved { path } => {
                    if existing_paths.contains(path.as_str()) {
                        removed_note_paths.push(path);
                    }
                }
                VaultWatchChange::NoteMoved { from, to } => {
                    if from == to {
                        continue;
                    }
                    if existing_paths.contains(from.as_str()) {
                        moved_note_pairs.push((from, to));
                    } else if existing_paths.contains(to.as_str()) {
                        upsert_paths.push(to);
                    } else {
                        new_note_paths.push(to);
                    }
                }
                VaultWatchChange::FolderCreated { path } => {
                    ensure_folder_branch(
                        &path,
                        &mut self.explorer_folder_children,
                        &mut self.folder_notes,
                    );
                    self.expand_folder_ancestors(&path);
                }
                VaultWatchChange::FolderRemoved { path } => {
                    let (removed, folder_bookmarks_changed) = self.apply_folder_removed_change(&path);
                    folder_removed_note_paths.extend(removed);
                    bookmarks_changed |= folder_bookmarks_changed;
                }
                VaultWatchChange::FolderMoved { from, to } => {
                    let (moved, folder_bookmarks_changed) =
                        self.apply_folder_moved_change(&from, &to);
                    folder_moved_note_pairs.extend(moved);
                    bookmarks_changed |= folder_bookmarks_changed;
                }
                VaultWatchChange::RescanRequired => needs_rescan = true,
            }
        }

        removed_note_paths.extend(folder_removed_note_paths);
        moved_note_pairs.extend(folder_moved_note_pairs);
        removed_note_paths.sort();
        removed_note_paths.dedup();
        moved_note_pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        moved_note_pairs.dedup();

        if !moved_note_pairs.is_empty()
            && new_note_paths.is_empty()
            && removed_note_paths.is_empty()
        {
            match derive_prefix_moves(&moved_note_pairs) {
                Some(prefix_moves) => {
                    if !prefix_moves.is_empty() {
                        let mut move_map = moved_note_pairs
                            .into_iter()
                            .collect::<HashMap<String, String>>();

                        for from in &existing_paths_vec {
                            if move_map.contains_key(from) {
                                continue;
                            }

                            for (old_prefix, new_prefix) in &prefix_moves {
                                let Some(to) =
                                    rewrite_path_with_prefix(from, old_prefix, new_prefix)
                                else {
                                    continue;
                                };
                                if from != &to {
                                    move_map.insert(from.clone(), to);
                                }
                                break;
                            }
                        }

                        moved_note_pairs = move_map.into_iter().collect();
                        moved_note_pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
                    }
                }
                None => {
                    self.watcher_status.last_error = Some(SharedString::from(
                        "watch prefix-move conflict detected; fallback to rescan",
                    ));
                    self.status = SharedString::from("External move conflict detected");
                    self.rescan_vault(cx);
                    return;
                }
            }
        }

        if needs_rescan {
            self.watcher_status.revision = self.watcher_status.revision.wrapping_add(1);
            self.watcher_status.last_error = None;
            self.status = SharedString::from("External structure change detected");
            self.rescan_vault(cx);
            return;
        }

        if upsert_paths.is_empty()
            && new_note_paths.is_empty()
            && removed_note_paths.is_empty()
            && moved_note_pairs.is_empty()
        {
            return;
        }

        let Some(index) = self.knowledge_index.as_ref() else {
            self.rescan_vault(cx);
            return;
        };

        let mut next_index = (**index).clone();

        if !moved_note_pairs.is_empty() {
            for (from, to) in &moved_note_pairs {
                if self.open_note_path.as_deref() == Some(from.as_str()) {
                    self.open_note_path = Some(to.clone());
                }

                for path in &mut self.open_editors {
                    if path == from {
                        *path = to.clone();
                    }
                }

                if self.selected_note.as_deref() == Some(from.as_str()) {
                    self.selected_note = Some(to.clone());
                }

                for bookmarked in &mut self.app_settings.bookmarked_notes {
                    if bookmarked == from {
                        *bookmarked = to.clone();
                        bookmarks_changed = true;
                    }
                }

                rename_note_in_tree_structures(
                    from,
                    to,
                    &mut self.explorer_folder_children,
                    &mut self.folder_notes,
                );

                next_index.remove_note(from);
                if let Err(err) = next_index.upsert_note(&vault, to) {
                    self.watcher_status.last_error = Some(SharedString::from(format!(
                        "watch move upsert failed: {err}"
                    )));
                    self.rescan_vault(cx);
                    return;
                }
            }
        }

        if !removed_note_paths.is_empty() {
            for path in &removed_note_paths {
                if self.open_note_path.as_deref() == Some(path.as_str()) {
                    self.open_note_path = None;
                    self.open_note_content.clear();
                    self.editor_buffer = None;
                    self.editor_view_mode = EditorViewMode::Edit;
                    self.open_note_dirty = false;
                    self.open_note_word_count = 0;
                    self.open_note_heading_count = 0;
                    self.open_note_link_count = 0;
                    self.open_note_code_fence_count = 0;
                    self.markdown_preview.headings.clear();
                    self.markdown_preview.blocks.clear();
                    self.markdown_diagnostics.clear();
                    self.editor_highlight_spans.clear();
                    self.pending_markdown_invalidation = None;
                    self.pending_markdown_parse_nonce = 0;
                }
                self.open_editors.retain(|editor_path| editor_path != path);
                self.folder_notes
                    .entry(folder_of_note_path(path))
                    .or_default()
                    .retain(|existing| existing != path);
                self.selected_note = self
                    .selected_note
                    .as_ref()
                    .filter(|selected| selected.as_str() != path)
                    .cloned();
                self.app_settings
                    .bookmarked_notes
                    .retain(|bookmarked| bookmarked != path);
                bookmarks_changed = true;
            }

            for path in &removed_note_paths {
                remove_note_from_tree_structures(
                    path,
                    &mut self.explorer_folder_children,
                    &mut self.folder_notes,
                );
                next_index.remove_note(path);
            }
        }

        for path in upsert_paths {
            if let Err(err) = next_index.upsert_note(&vault, &path) {
                self.watcher_status.last_error =
                    Some(SharedString::from(format!("watch upsert failed: {err}")));
                self.rescan_vault(cx);
                return;
            }
        }

        for path in &new_note_paths {
            if let Err(err) = next_index.upsert_note(&vault, path) {
                self.watcher_status.last_error = Some(SharedString::from(format!(
                    "watch new-note upsert failed: {err}"
                )));
                self.rescan_vault(cx);
                return;
            }
            add_note_to_tree_structures(
                path,
                &mut self.explorer_folder_children,
                &mut self.folder_notes,
            );
        }

        let mut fingerprint_paths = self.explorer_all_note_paths.as_ref().clone();
        for (from, to) in &moved_note_pairs {
            for existing in &mut fingerprint_paths {
                if existing == from {
                    *existing = to.clone();
                }
            }
        }
        for removed in &removed_note_paths {
            fingerprint_paths.retain(|existing| existing != removed);
        }
        for added in &new_note_paths {
            if !fingerprint_paths.iter().any(|existing| existing == added) {
                fingerprint_paths.push(added.clone());
            }
        }
        fingerprint_paths.sort();

        self.explorer_all_note_paths = Arc::new(fingerprint_paths.clone());
        self.explorer_all_note_paths_lower = Arc::new(
            fingerprint_paths
                .iter()
                .map(|path| path.to_lowercase())
                .collect(),
        );
        self.knowledge_index = Some(Arc::new(next_index));
        self.watch_scan_fingerprint = compute_entries_fingerprint(&fingerprint_paths);
        self.watch_scan_entries = fingerprint_paths.len();

        if bookmarks_changed {
            self.app_settings.bookmarked_notes = self.bookmarked_notes_snapshot();
            self.persist_settings();
        }

        self.watcher_status.revision = self.watcher_status.revision.wrapping_add(1);
        self.watcher_status.last_error = None;
        self.bump_index_generation();
        self.rebuild_explorer_rows();
        self.status = SharedString::from("External note content updated");

        if !self.search_query.trim().is_empty() {
            self.schedule_apply_search(Duration::ZERO, cx);
        }
        if self.palette_open
            && self.palette_mode == PaletteMode::QuickOpen
            && !self.palette_query.trim().is_empty()
        {
            self.schedule_apply_palette_results(Duration::ZERO, cx);
        }
    }

    fn bump_index_generation(&mut self) {
        self.index_generation = self.index_generation.wrapping_add(1);
        self.search_query_cache.clear();
        self.search_query_cache_order.clear();
        self.quick_open_query_cache.clear();
        self.quick_open_query_cache_order.clear();
    }

    fn record_cache_stats_snapshot(&mut self, epoch_secs: u64) {
        push_cache_stats_snapshot(
            &mut self.cache_stats_snapshots,
            &self.cache_stats,
            epoch_secs,
            CACHE_DIAGNOSTICS_MAX_SNAPSHOTS,
        );
    }

    fn schedule_cache_diagnostics_flush(&mut self, cx: &mut Context<Self>) {
        cx.spawn(|this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
            let mut cx = cx.clone();
            async move {
                loop {
                    Timer::after(CACHE_DIAGNOSTICS_FLUSH_INTERVAL).await;

                    let Some((stats, last_flushed, snapshots, generated_at_utc)) = this
                        .update(&mut cx, |this, _cx| {
                            let generated_at_utc = current_epoch_secs();
                            this.record_cache_stats_snapshot(generated_at_utc);
                            (
                                this.cache_stats.clone(),
                                this.cache_stats_last_flushed.clone(),
                                this.cache_stats_snapshots
                                    .iter()
                                    .cloned()
                                    .collect::<Vec<_>>(),
                                generated_at_utc,
                            )
                        })
                        .ok()
                    else {
                        continue;
                    };

                    if stats == last_flushed {
                        continue;
                    }

                    let diagnostics_path = PathBuf::from(CACHE_DIAGNOSTICS_PATH);
                    let payload =
                        build_cache_diagnostics_payload(&stats, &snapshots, generated_at_utc);

                    let write_result = cx
                        .background_executor()
                        .spawn(async move {
                            if let Some(parent) = diagnostics_path.parent() {
                                fs::create_dir_all(parent)?;
                            }
                            fs::write(&diagnostics_path, payload).map_err(anyhow::Error::from)
                        })
                        .await;

                    this.update(&mut cx, |this, _cx| match write_result {
                        Ok(()) => {
                            this.cache_stats_last_flushed = this.cache_stats.clone();
                        }
                        Err(err) => {
                            this.watcher_status.last_error = Some(SharedString::from(format!(
                                "cache diagnostics flush failed: {err}"
                            )));
                        }
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn on_filter_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.palette_open {
            self.on_palette_key(ev, cx);
            return;
        }
        if self.settings_open {
            if self.on_settings_key(ev, cx) {
                return;
            }
            if ev.keystroke.key.eq_ignore_ascii_case("escape") {
                self.settings_open = false;
                self.settings_language_menu_open = false;
                self.settings_backdrop_armed_until = None;
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
        if ctrl && key.as_str() == "v" {
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

    fn on_settings_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let key = ev.keystroke.key.to_lowercase();
        let ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
        let alt = ev.keystroke.modifiers.alt;
        let shift = ev.keystroke.modifiers.shift;

        if key == "escape" {
            if self.hotkey_editing_command.is_some() {
                self.hotkey_editing_command = None;
                self.hotkey_editing_value.clear();
                cx.notify();
                return true;
            }
            return false;
        }

        if let Some(command) = self.hotkey_editing_command {
            let mut parts: Vec<&str> = Vec::new();
            if ctrl {
                parts.push("Ctrl");
            }
            if alt {
                parts.push("Alt");
            }
            if shift {
                parts.push("Shift");
            }

            let key_norm = if key == " " {
                "space".to_string()
            } else {
                key.clone()
            };

            if key_norm == "control"
                || key_norm == "shift"
                || key_norm == "alt"
                || key_norm == "meta"
            {
                return true;
            }

            parts.push(&key_norm);
            let chord = parts.join("+");
            self.hotkey_editing_value = chord.clone();

            match xnote_core::keybind::KeyChord::normalize_string(&chord) {
                Some(normalized) => {
                    self.app_settings
                        .keymap_overrides
                        .insert(command.as_str().to_string(), normalized.clone());
                    match self.app_settings.build_keymap() {
                        Ok(keymap) => {
                            self.keymap = keymap;
                            self.persist_settings();
                            self.hotkey_editing_command = None;
                            self.hotkey_editing_value.clear();
                            self.status = SharedString::from(format!(
                                "Updated shortcut: {}",
                                command.as_str()
                            ));
                        }
                        Err(err) => {
                            self.status = SharedString::from(format!(
                                "Invalid shortcut for {}: {err}",
                                command.as_str()
                            ));
                        }
                    }
                }
                None => {
                    self.status = SharedString::from(format!("Invalid shortcut format: {chord}"));
                }
            }

            cx.notify();
            return true;
        }

        false
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
                self.settings_backdrop_armed_until = None;
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

        if ctrl && key.as_str() == "v" {
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
                | CommandId::Undo
                | CommandId::Redo
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
                    let Some(open_match) = self.palette_results.get(self.palette_selected) else {
                        return;
                    };
                    let path = open_match.path.clone();
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

    fn expand_folder_ancestors(&mut self, folder: &str) -> bool {
        let mut changed = false;
        if self.explorer_expanded_folders.insert(String::new()) {
            changed = true;
        }

        let mut current = folder.trim_end_matches('/').to_string();
        while !current.is_empty() {
            if self.explorer_expanded_folders.insert(current.clone()) {
                changed = true;
            }
            match current.rsplit_once('/') {
                Some((parent, _)) => current = parent.to_string(),
                None => current.clear(),
            }
        }

        changed
    }

    fn add_note_optimistically(&mut self, note_path: &str, cx: &mut Context<Self>) {
        add_note_to_tree_structures(
            note_path,
            &mut self.explorer_folder_children,
            &mut self.folder_notes,
        );

        let mut paths = self.explorer_all_note_paths.as_ref().clone();
        if !paths.iter().any(|existing| existing == note_path) {
            paths.push(note_path.to_string());
            paths.sort();
        }
        self.explorer_all_note_paths = Arc::new(paths.clone());
        self.explorer_all_note_paths_lower =
            Arc::new(paths.iter().map(|path| path.to_lowercase()).collect());

        let folder = folder_of_note_path(note_path);
        self.selected_explorer_folder = Some(folder.clone());
        self.expand_folder_ancestors(&folder);
        self.watch_scan_entries = self.explorer_all_note_paths.len();
        self.watch_scan_fingerprint = compute_entries_fingerprint(self.explorer_all_note_paths.as_ref());

        self.rebuild_explorer_rows();
        if self.is_filtering() {
            self.schedule_apply_filter(Duration::ZERO, cx);
        }

        self.pending_created_note_reconcile
            .insert(note_path.to_string());
    }

    fn add_folder_optimistically(&mut self, folder: &str, cx: &mut Context<Self>) {
        ensure_folder_branch(
            folder,
            &mut self.explorer_folder_children,
            &mut self.folder_notes,
        );

        self.expand_folder_ancestors(folder);
        self.selected_explorer_folder = Some(folder.to_string());

        if self.is_filtering() {
            let query = self.explorer_filter.trim().to_lowercase();
            if !folder.to_lowercase().contains(&query) {
                self.explorer_filter.clear();
                self.explorer_rows_filtered.clear();
            }
        }

        self.rebuild_explorer_rows();
        if self.is_filtering() {
            self.schedule_apply_filter(Duration::ZERO, cx);
        }

        self.pending_created_folder_reconcile
            .insert(folder.to_string());
    }

    fn schedule_reconcile_after_create(&mut self, cx: &mut Context<Self>) {
        self.next_create_reconcile_nonce = self.next_create_reconcile_nonce.wrapping_add(1);
        let nonce = self.next_create_reconcile_nonce;
        self.pending_create_reconcile_nonce = nonce;

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    Timer::after(CREATE_RECONCILE_DELAY).await;

                    let should_run = this
                        .update(&mut cx, |this, _cx| {
                            if this.pending_create_reconcile_nonce != nonce {
                                return false;
                            }

                            !matches!(this.scan_state, ScanState::Scanning)
                                && !matches!(this.index_state, IndexState::Building)
                        })
                        .ok()
                        .unwrap_or(false);

                    if should_run {
                        this.update(&mut cx, |this, cx| {
                            if this.pending_create_reconcile_nonce != nonce {
                                return;
                            }
                            this.pending_create_reconcile_nonce = 0;
                            this.reconcile_pending_creates(cx);
                        })
                        .ok();
                        return;
                    }

                    this.update(&mut cx, |this, cx| {
                        if this.pending_create_reconcile_nonce != nonce {
                            return;
                        }
                        this.schedule_reconcile_after_create(cx);
                    })
                    .ok();
                }
            },
        )
        .detach();
    }

    fn reconcile_pending_creates(&mut self, cx: &mut Context<Self>) {
        let Some(vault) = self.vault() else {
            return;
        };

        if self.pending_created_note_reconcile.is_empty()
            && self.pending_created_folder_reconcile.is_empty()
        {
            return;
        }

        let notes = self
            .pending_created_note_reconcile
            .drain()
            .collect::<Vec<_>>();
        let folders = self
            .pending_created_folder_reconcile
            .drain()
            .collect::<Vec<_>>();

        let mut reconciled_any = false;

        for note_path in notes {
            match normalize_vault_rel_path(&note_path)
                .and_then(|rel| join_inside(vault.root(), &rel).map(|full| (rel, full)))
            {
                Ok((rel, full)) if full.exists() => {
                    add_note_to_tree_structures(
                        &rel,
                        &mut self.explorer_folder_children,
                        &mut self.folder_notes,
                    );

                    let mut paths = self.explorer_all_note_paths.as_ref().clone();
                    if !paths.iter().any(|existing| existing == &rel) {
                        paths.push(rel);
                        paths.sort();
                        self.explorer_all_note_paths = Arc::new(paths.clone());
                        self.explorer_all_note_paths_lower =
                            Arc::new(paths.iter().map(|path| path.to_lowercase()).collect());
                    }

                    if let Some(index) = self.knowledge_index.as_mut() {
                        if let Some(inner) = Arc::get_mut(index) {
                            let _ = inner.upsert_note(&vault, &note_path);
                        }
                    }
                    reconciled_any = true;
                }
                _ => {
                    remove_note_from_tree_structures(
                        &note_path,
                        &mut self.explorer_folder_children,
                        &mut self.folder_notes,
                    );
                    if let Some(index) = self.knowledge_index.as_mut() {
                        if let Some(inner) = Arc::get_mut(index) {
                            inner.remove_note(&note_path);
                        }
                    }

                    let mut paths = self.explorer_all_note_paths.as_ref().clone();
                    paths.retain(|existing| existing != &note_path);
                    self.explorer_all_note_paths = Arc::new(paths.clone());
                    self.explorer_all_note_paths_lower =
                        Arc::new(paths.iter().map(|path| path.to_lowercase()).collect());
                    reconciled_any = true;
                }
            }
        }

        for folder in folders {
            match normalize_folder_rel_path(&folder)
                .and_then(|rel| join_inside(vault.root(), &rel).map(|full| (rel, full)))
            {
                Ok((rel, full)) if full.exists() && full.is_dir() => {
                    ensure_folder_branch(
                        &rel,
                        &mut self.explorer_folder_children,
                        &mut self.folder_notes,
                    );
                    reconciled_any = true;
                }
                _ => {
                    self.explorer_folder_children.remove(&folder);
                    self.folder_notes.remove(&folder);

                    let mut current = folder.clone();
                    loop {
                        let parent = current
                            .rsplit_once('/')
                            .map(|(p, _)| p.to_string())
                            .unwrap_or_default();

                        if let Some(children) = self.explorer_folder_children.get_mut(&parent) {
                            children.retain(|child| child != &current);
                        }

                        if parent.is_empty() {
                            break;
                        }

                        let parent_has_notes = self
                            .folder_notes
                            .get(&parent)
                            .is_some_and(|notes| !notes.is_empty());
                        let parent_has_children = self
                            .explorer_folder_children
                            .get(&parent)
                            .is_some_and(|children| !children.is_empty());
                        if parent_has_notes || parent_has_children {
                            break;
                        }

                        self.explorer_folder_children.remove(&parent);
                        self.folder_notes.remove(&parent);
                        current = parent;
                    }

                    self.explorer_expanded_folders.remove(&folder);
                    if self.selected_explorer_folder.as_deref() == Some(folder.as_str()) {
                        self.selected_explorer_folder = Some(String::new());
                    }
                    reconciled_any = true;
                }
            }
        }

        if reconciled_any {
            self.watch_scan_entries = self.explorer_all_note_paths.len();
            self.watch_scan_fingerprint =
                compute_entries_fingerprint(self.explorer_all_note_paths.as_ref());
            self.bump_index_generation();
            self.rebuild_explorer_rows();

            if self.is_filtering() {
                self.schedule_apply_filter(Duration::ZERO, cx);
            }
            if !self.search_query.trim().is_empty() {
                self.schedule_apply_search(Duration::ZERO, cx);
            }
            if self.palette_open
                && self.palette_mode == PaletteMode::QuickOpen
                && !self.palette_query.trim().is_empty()
            {
                self.schedule_apply_palette_results(Duration::ZERO, cx);
            }

            self.status = SharedString::from("Create sync reconciled");
            cx.notify();
        }
    }

    fn apply_folder_removed_change(&mut self, folder: &str) -> (Vec<String>, bool) {
        let mut bookmarks_changed = false;
        let notes_to_remove = self
            .explorer_all_note_paths
            .iter()
            .filter(|path| {
                path == &folder || path.starts_with(&format!("{folder}/"))
            })
            .cloned()
            .collect::<Vec<_>>();

        for note_path in &notes_to_remove {
            remove_note_from_tree_structures(
                &note_path,
                &mut self.explorer_folder_children,
                &mut self.folder_notes,
            );
            self.open_editors.retain(|editor_path| editor_path != note_path);
            if self.selected_note.as_deref() == Some(note_path.as_str()) {
                self.selected_note = None;
            }
            if self.open_note_path.as_deref() == Some(note_path.as_str()) {
                self.open_note_path = None;
                self.open_note_content.clear();
                self.editor_buffer = None;
                self.open_note_loading = false;
                self.open_note_dirty = false;
            }
            self.app_settings
                .bookmarked_notes
                .retain(|bookmarked| {
                    let keep = bookmarked != note_path;
                    if !keep {
                        bookmarks_changed = true;
                    }
                    keep
                });
        }

        self.explorer_folder_children.remove(folder);
        self.folder_notes.remove(folder);
        self.explorer_expanded_folders.remove(folder);
        self.pending_created_folder_reconcile.remove(folder);

        if self.selected_explorer_folder.as_deref() == Some(folder) {
            self.selected_explorer_folder = Some(String::new());
        }

        self.explorer_all_note_paths = Arc::new(
            self.explorer_all_note_paths
                .iter()
                .filter(|path| {
                    !(path == &folder || path.starts_with(&format!("{folder}/")))
                })
                .cloned()
                .collect(),
        );
        self.explorer_all_note_paths_lower = Arc::new(
            self.explorer_all_note_paths
                .iter()
                .map(|path| path.to_lowercase())
                .collect(),
        );

        (notes_to_remove, bookmarks_changed)
    }

    fn apply_folder_moved_change(&mut self, from: &str, to: &str) -> (Vec<(String, String)>, bool) {
        if from == to {
            return (Vec::new(), false);
        }

        let mut bookmarks_changed = false;

        let prefix = format!("{from}/");
        let moved_notes = self
            .explorer_all_note_paths
            .iter()
            .filter_map(|path| {
                if path == from {
                    Some((path.clone(), to.to_string()))
                } else if path.starts_with(&prefix) {
                    Some((path.clone(), format!("{to}/{}", &path[prefix.len()..])))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for (old_path, new_path) in &moved_notes {
            rename_note_in_tree_structures(
                old_path,
                new_path,
                &mut self.explorer_folder_children,
                &mut self.folder_notes,
            );

            if self.open_note_path.as_deref() == Some(old_path.as_str()) {
                self.open_note_path = Some(new_path.clone());
            }
            if self.selected_note.as_deref() == Some(old_path.as_str()) {
                self.selected_note = Some(new_path.clone());
            }
            for open in &mut self.open_editors {
                if open == old_path {
                    *open = new_path.clone();
                }
            }
            for bookmarked in &mut self.app_settings.bookmarked_notes {
                if bookmarked == old_path {
                    *bookmarked = new_path.clone();
                    bookmarks_changed = true;
                }
            }
        }

        let mut updated_paths = self.explorer_all_note_paths.as_ref().clone();
        for (old_path, new_path) in &moved_notes {
            for existing in &mut updated_paths {
                if *existing == *old_path {
                    *existing = new_path.clone();
                }
            }
        }
        updated_paths.sort();
        self.explorer_all_note_paths = Arc::new(updated_paths.clone());
        self.explorer_all_note_paths_lower = Arc::new(
            updated_paths
                .iter()
                .map(|path| path.to_lowercase())
                .collect(),
        );

        let rename_prefix = format!("{from}/");
        if let Some(selected) = self.selected_explorer_folder.clone() {
            if selected == from {
                self.selected_explorer_folder = Some(to.to_string());
            } else if selected.starts_with(&rename_prefix) {
                self.selected_explorer_folder =
                    Some(format!("{to}/{}", &selected[rename_prefix.len()..]));
            }
        }
        if self.explorer_expanded_folders.contains(from) {
            self.explorer_expanded_folders.remove(from);
            self.explorer_expanded_folders.insert(to.to_string());
        }

        (moved_notes, bookmarks_changed)
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
        self.selected_explorer_folder = Some(folder_of_note_path(&note_path));
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
        self.editor_buffer = None;
        self.editor_view_mode = EditorViewMode::Edit;
        self.edit_latency_stats = EditLatencyStats::default();
        self.open_note_heading_count = 0;
        self.open_note_link_count = 0;
        self.open_note_code_fence_count = 0;
        self.markdown_preview.headings.clear();
        self.markdown_preview.blocks.clear();
        self.markdown_diagnostics.clear();
        self.editor_highlight_spans.clear();
        self.pending_markdown_invalidation = None;
        self.pending_markdown_parse_nonce = 0;
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
                                this.editor_buffer = Some(EditorBuffer::new(&this.open_note_content));
                                this.open_note_word_count = count_words(&this.open_note_content);
                                this.pending_markdown_invalidation = Some(
                                    MarkdownInvalidationWindow::new(0, this.open_note_content.len()),
                                );
                                this.schedule_markdown_parse(Duration::ZERO, cx);

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
                                this.editor_buffer = Some(EditorBuffer::new(&this.open_note_content));
                                this.open_note_word_count = 0;
                                this.open_note_heading_count = 0;
                                this.open_note_link_count = 0;
                                this.open_note_code_fence_count = 0;
                                this.markdown_preview.headings.clear();
                                this.markdown_preview.blocks.clear();
                                this.markdown_diagnostics.clear();
                                this.editor_highlight_spans.clear();
                                this.pending_markdown_invalidation = None;
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

        self.apply_editor_transaction(
            range,
            new_text,
            EditorMutationSource::Keyboard,
            false,
            cx,
        );
    }

    fn apply_editor_transaction(
        &mut self,
        range: Range<usize>,
        new_text: &str,
        source: EditorMutationSource,
        keep_marked_range: bool,
        cx: &mut Context<Self>,
    ) {
        if self.editor_buffer.is_none() {
            self.editor_buffer = Some(EditorBuffer::new(&self.open_note_content));
        }

        let apply_started_at = Instant::now();

        let Some(buffer) = self.editor_buffer.as_mut() else {
            return;
        };
        let tx = EditTransaction::replace(range.clone(), new_text.to_string());
        if buffer.apply(tx).is_err() {
            self.status = SharedString::from("Edit transaction rejected");
            cx.notify();
            return;
        }

        self.open_note_content = buffer.to_string();

        let cursor = range.start + new_text.len();
        self.editor_selected_range = cursor..cursor;
        self.editor_selection_reversed = false;
        if !keep_marked_range {
            self.editor_marked_range = None;
        }
        self.editor_preferred_x = None;

        self.open_note_word_count = count_words(&self.open_note_content);
        self.open_note_dirty = true;
        self.status = SharedString::from("Editing...");
        self.markdown_invalidation_for_edit(range, new_text.len());
        self.schedule_markdown_parse(MARKDOWN_PARSE_DEBOUNCE, cx);
        self.schedule_save_note(self.autosave_delay(), cx);
        self.record_edit_latency(apply_started_at.elapsed(), source);
        cx.notify();
    }

    fn apply_editor_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        let apply_started_at = Instant::now();

        let Some(buffer) = self.editor_buffer.as_mut() else {
            return;
        };

        let record = if undo {
            buffer.undo().ok().flatten()
        } else {
            buffer.redo().ok().flatten()
        };

        let Some(record) = record else {
            return;
        };

        self.open_note_content = buffer.to_string();
        self.open_note_word_count = count_words(&self.open_note_content);
        let cursor = if undo {
            record.before.range.start + record.before.replacement.len()
        } else {
            record.after.range.start + record.after.replacement.len()
        }
        .min(self.open_note_content.len());
        self.editor_selected_range = cursor..cursor;
        self.editor_selection_reversed = false;
        self.editor_marked_range = None;
        self.editor_preferred_x = None;
        self.open_note_dirty = true;
        self.status = SharedString::from("Editing...");

        self.pending_markdown_invalidation = Some(MarkdownInvalidationWindow::new(
            0,
            self.open_note_content.len(),
        ));
        self.schedule_markdown_parse(MARKDOWN_PARSE_DEBOUNCE, cx);
        self.schedule_save_note(self.autosave_delay(), cx);
        self.record_edit_latency(apply_started_at.elapsed(), EditorMutationSource::UndoRedo);
        cx.notify();
    }

    fn markdown_invalidation_for_edit(&mut self, edit_range: Range<usize>, inserted_len: usize) {
        let window = MarkdownInvalidationWindow::from_edit(
            edit_range,
            inserted_len,
            self.open_note_content.len(),
            MARKDOWN_INVALIDATION_CONTEXT_BYTES,
        );

        if let Some(pending) = self.pending_markdown_invalidation.as_mut() {
            pending.merge(&window);
        } else {
            self.pending_markdown_invalidation = Some(window);
        }
    }

    fn refresh_markdown_preview_model(&mut self, parsed: &MarkdownParseResult) {
        self.markdown_preview.headings = parsed.summary.headings.clone();
        self.markdown_preview.blocks = parsed
            .blocks
            .iter()
            .map(|block| MarkdownPreviewBlock {
                kind: match block.kind {
                    xnote_core::markdown::MarkdownBlockKind::Heading(level) => {
                        MarkdownPreviewBlockKind::Heading(level)
                    }
                    xnote_core::markdown::MarkdownBlockKind::Paragraph => {
                        MarkdownPreviewBlockKind::Paragraph
                    }
                    xnote_core::markdown::MarkdownBlockKind::CodeFence => {
                        MarkdownPreviewBlockKind::CodeFence
                    }
                    xnote_core::markdown::MarkdownBlockKind::Quote => MarkdownPreviewBlockKind::Quote,
                    xnote_core::markdown::MarkdownBlockKind::List => MarkdownPreviewBlockKind::List,
                },
                text: block.text.clone(),
            })
            .collect();
    }

    fn refresh_editor_highlight_spans(&mut self) {
        self.editor_highlight_spans = build_editor_highlight_spans(&self.open_note_content);
    }

    fn record_edit_latency(&mut self, elapsed: Duration, _source: EditorMutationSource) {
        self.edit_latency_stats.record(elapsed.as_millis());
    }

    fn set_editor_view_mode(&mut self, mode: EditorViewMode, cx: &mut Context<Self>) {
        self.editor_view_mode = mode;
        if mode != EditorViewMode::Edit {
            self.pending_markdown_invalidation = Some(MarkdownInvalidationWindow::new(
                0,
                self.open_note_content.len(),
            ));
            self.schedule_markdown_parse(Duration::ZERO, cx);
        }
        cx.notify();
    }

    fn toggle_editor_split_mode(&mut self, cx: &mut Context<Self>) {
        let next = if self.editor_view_mode == EditorViewMode::Split {
            EditorViewMode::Edit
        } else {
            EditorViewMode::Split
        };
        self.set_editor_view_mode(next, cx);
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
                self.settings_backdrop_armed_until = None;
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
                | CommandId::Undo
                | CommandId::Redo
                | CommandId::FocusExplorer
                | CommandId::FocusSearch => {
                    self.execute_palette_command(command, cx);
                    return;
                }
            }
        }

        if ctrl && !alt {
            match key.as_str() {
                "z" => {
                    self.apply_editor_history(!shift, cx);
                    return;
                }
                "y" => {
                    self.apply_editor_history(false, cx);
                    return;
                }
                _ => {}
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

    fn schedule_markdown_parse(&mut self, delay: Duration, cx: &mut Context<Self>) {
        if self.open_note_loading || self.open_note_path.is_none() {
            return;
        }

        self.next_markdown_parse_nonce = self.next_markdown_parse_nonce.wrapping_add(1);
        let nonce = self.next_markdown_parse_nonce;
        self.pending_markdown_parse_nonce = nonce;

        self.pending_markdown_invalidation = None;

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    if delay > Duration::ZERO {
                        Timer::after(delay).await;
                    }

                    let Some(content) = this
                        .update(&mut cx, |this, _cx| {
                            if this.pending_markdown_parse_nonce != nonce
                                || this.open_note_loading
                                || this.open_note_path.is_none()
                            {
                                return None;
                            }
                            Some(this.open_note_content.clone())
                        })
                        .ok()
                        .flatten()
                    else {
                        return;
                    };

                    let (parsed, diagnostics) = cx
                        .background_executor()
                        .spawn(async move {
                            let parsed = parse_markdown(&content);
                            let diagnostics = lint_markdown(&content);
                            (parsed, diagnostics)
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_markdown_parse_nonce != nonce {
                            return;
                        }
                        this.open_note_heading_count = parsed.summary.headings.len();
                        this.open_note_link_count = parsed.summary.links.len();
                        this.open_note_code_fence_count = parsed.summary.code_fence_count;
                        this.refresh_markdown_preview_model(&parsed);
                        this.markdown_diagnostics = diagnostics;
                        this.refresh_editor_highlight_spans();
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

#[derive(Clone)]
struct NoteEditorLayout {
    view: Entity<XnoteWindow>,
    state: Rc<RefCell<Option<NoteEditorLayoutInner>>>,
}

impl NoteEditorLayout {
    fn new(view: Entity<XnoteWindow>) -> Self {
        Self {
            view,
            state: Rc::new(RefCell::new(None)),
        }
    }
}

impl Default for NoteEditorLayout {
    fn default() -> Self {
        panic!("NoteEditorLayout::default must not be used")
    }
}

struct NoteEditorLayoutInner {
    lines: Vec<gpui::WrappedLine>,
    logical_line_numbers: Vec<usize>,
    gutter_width: Pixels,
    gutter_digits: usize,
    ui_theme: UiTheme,
    line_height: Pixels,
    wrap_width: Option<Pixels>,
    size: Option<Size<Pixels>>,
    bounds: Option<Bounds<Pixels>>,
}

impl NoteEditorLayout {
    fn layout(&self, window: &mut Window, _cx: &mut App) -> LayoutId {
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

                let max_line_number = element_state
                    .view
                    .read(cx)
                    .open_note_content
                    .lines()
                    .count()
                    .max(1);
                let gutter_digits = line_number_digits(max_line_number);
                let gutter_width = px(editor_gutter_width_for_digits(gutter_digits));
                let text_x_offset = editor_text_x_offset(gutter_width);
                let text_wrap_width =
                    wrap_width.map(|w| (w - text_x_offset).max(px(EDITOR_TEXT_MIN_WRAP_WIDTH)));

                if let Some(inner) = element_state.state.borrow().as_ref() {
                    if inner.size.is_some()
                        && (wrap_width.is_none() || wrap_width == inner.wrap_width)
                    {
                        return inner.size.unwrap();
                    }
                }

                let view = element_state.view.read(cx);
                let text = SharedString::from(view.open_note_content.clone());
                let len = text.len();
                let selection = view.editor_selected_range.clone();
                let marked = view.editor_marked_range.clone();
                let highlight_spans = view.editor_highlight_spans.clone();
                let ui_theme = UiTheme::from_settings(view.settings_theme, view.settings_accent);

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
                for span in &highlight_spans {
                    boundaries.push(span.range.start.min(len));
                    boundaries.push(span.range.end.min(len));
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

                    if let Some(kind) = highlight_spans
                        .iter()
                        .find(|span| start >= span.range.start && end <= span.range.end)
                        .map(|span| span.kind)
                    {
                        run.color = match kind {
                            EditorHighlightKind::HeadingMarker => rgb(ui_theme.syntax_heading_marker),
                            EditorHighlightKind::HeadingText => rgb(ui_theme.syntax_heading_text),
                            EditorHighlightKind::CodeFence => rgb(ui_theme.syntax_code_fence),
                            EditorHighlightKind::CodeText => rgb(ui_theme.syntax_code_text),
                            EditorHighlightKind::QuoteMarker => rgb(ui_theme.syntax_quote_marker),
                            EditorHighlightKind::ListMarker => rgb(ui_theme.syntax_list_marker),
                            EditorHighlightKind::LinkText => rgb(ui_theme.syntax_link_text),
                            EditorHighlightKind::LinkUrl => rgb(ui_theme.syntax_link_url),
                        }
                        .into();
                    }

                    runs.push(run);
                }

                let lines = match window
                    .text_system()
                    .shape_text(text.clone(), font_size, &runs, text_wrap_width, None)
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

                let logical_line_numbers = compute_wrapped_line_numbers(&text, &lines);

                element_state.state.borrow_mut().replace(NoteEditorLayoutInner {
                    lines,
                    logical_line_numbers,
                    gutter_width,
                    gutter_digits,
                    ui_theme,
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
        if let Some(inner) = self.state.borrow_mut().as_mut() {
            inner.bounds = Some(bounds);
        }
    }

    fn line_height(&self) -> Option<Pixels> {
        self.state.borrow().as_ref().map(|inner| inner.line_height)
    }

    fn position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        let inner = self.state.borrow();
        let inner = inner.as_ref()?;
        let bounds = inner.bounds?;
        let line_height = inner.line_height;

        let text_x_offset = editor_text_x_offset(inner.gutter_width);
        let mut line_origin = point(bounds.origin.x + text_x_offset, bounds.origin.y);
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
        let inner = self.state.borrow();
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
        let text_x_offset = editor_text_x_offset(inner.gutter_width);
        let mut line_origin = point(bounds.origin.x + text_x_offset, bounds.origin.y);
        let mut line_start_ix = 0usize;

        for line in &inner.lines {
            let line_bottom = line_origin.y + line.size(line_height).height;
            if position.y > line_bottom {
                line_origin.y = line_bottom;
                line_start_ix += line.len() + 1;
                continue;
            }

            if position.x <= line_origin.x {
                return Err(line_start_ix);
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
        let inner = self.state.borrow();
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
        let layout = NoteEditorLayout::new(self.view.clone());
        let layout_id = layout.layout(window, cx);
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

        if let Some(inner) = layout.state.borrow().as_ref() {
            let line_height = inner.line_height;
            let text_style = window.text_style();
            let mut line_origin = bounds.origin;
            let gutter_width = inner.gutter_width;
            let gutter_bounds = Bounds::new(bounds.origin, size(gutter_width, bounds.size.height));
            window.paint_quad(fill(gutter_bounds, rgb(inner.ui_theme.editor_gutter_bg)));
            let text_x_offset = editor_text_x_offset(gutter_width);
            window.paint_quad(fill(
                Bounds::new(
                    point(
                        gutter_bounds.origin.x + gutter_bounds.size.width - px(1.),
                        gutter_bounds.origin.y,
                    ),
                    size(px(1.), gutter_bounds.size.height),
                ),
                rgb(inner.ui_theme.border),
            ));

            for line in &inner.lines {
                let _ = line.paint_background(
                    point(line_origin.x + text_x_offset, line_origin.y),
                    line_height,
                    text_style.text_align,
                    Some(bounds),
                    window,
                    cx,
                );
                let _ = line.paint(
                    point(line_origin.x + text_x_offset, line_origin.y),
                    line_height,
                    text_style.text_align,
                    Some(bounds),
                    window,
                    cx,
                );
                line_origin.y += line.size(line_height).height;
            }

            let view = self.view.read(cx);
            let diag_lines: HashSet<usize> = view
                .markdown_diagnostics
                .iter()
                .map(|diag| diag.line.max(1))
                .collect();
            let _ = view;

            let mut y = bounds.origin.y;
            for (line_ix, line) in inner.lines.iter().enumerate() {
                let line_number = inner.logical_line_numbers.get(line_ix).copied().unwrap_or(1);
                if let Some(line_text) = line_number_label(
                    line_number,
                    inner.gutter_digits,
                    gutter_width,
                    inner.ui_theme.editor_gutter_text,
                    window,
                    cx,
                ) {
                    let _ = line_text.paint(
                        point(bounds.origin.x, y),
                        px(EDITOR_GUTTER_LINE_HEIGHT),
                        text_style.text_align,
                        Some(bounds),
                        window,
                        cx,
                    );
                }

                if diag_lines.contains(&line_number) {
                    let marker_height = (line.size(line_height).height - px(8.)).max(px(6.));
                    let marker = fill(
                        Bounds::new(
                            point(bounds.origin.x + px(2.), y + px(4.)),
                            size(px(4.), marker_height),
                        ),
                        rgb(inner.ui_theme.diagnostic_error),
                    );
                    window.paint_quad(marker);
                }

                y += line.size(line_height).height;
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
            self.schedule_save_note(self.autosave_delay(), cx);
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

        self.apply_editor_transaction(range, new_text, EditorMutationSource::Ime, false, cx);
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

        self.apply_editor_transaction(range.clone(), new_text, EditorMutationSource::Ime, true, cx);

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
        let ui_theme = UiTheme::from_settings(self.settings_theme, self.settings_accent);

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
                        rgb(ui_theme.interactive_hover)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        on_click(this, cx);
                    }))
                    .child(div().w(px(3.)).h_full().bg(if active {
                        rgb(ui_theme.accent)
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
                    this.settings_backdrop_armed_until =
                        Some(Instant::now() + OVERLAY_BACKDROP_ARM_DELAY);
                    cx.notify();
                },
            ));

        let rail = div()
            .w(px(48.))
            .min_w(px(48.))
            .max_w(px(48.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(ui_theme.panel_bg))
            .border_r_1()
            .border_color(rgb(ui_theme.border))
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
                .bg(rgb(ui_theme.surface_alt_bg))
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(28.))
                        .px_3()
                        .flex()
                        .items_center()
                        .bg(rgb(ui_theme.panel_bg))
                        .gap(px(10.))
                        .child(
                            div()
                                .font_family("IBM Plex Mono")
                                .text_size(px(10.))
                                .font_weight(FontWeight(900.))
                                .text_color(rgb(ui_theme.text_secondary))
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
                                .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                                    this.set_panel_shell_collapsed(true, cx);
                                }))
                                .child(ui_icon(ICON_PANEL_LEFT_CLOSE, 16., ui_theme.text_muted)),
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
                                        .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                                        .on_click(cx.listener(
                                            |this, _ev: &ClickEvent, _window, cx| {
                                                this.create_new_note(cx);
                                            },
                                        ))
                                        .child(ui_icon(ICON_FILE_PLUS, 16., ui_theme.text_muted)),
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
                                        .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                                        .on_click(cx.listener(
                                            |this, _ev: &ClickEvent, _window, cx| {
                                                this.create_new_folder(cx);
                                            },
                                        ))
                                        .child(ui_icon(ICON_FOLDER_PLUS, 16., ui_theme.text_muted)),
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
                                        .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                                        .on_click(cx.listener(
                                            |this, _ev: &ClickEvent, _window, cx| {
                                                this.rescan_vault(cx);
                                            },
                                        ))
                                        .child(ui_icon(ICON_REFRESH_CW, 16., ui_theme.text_muted)),
                                ),
                        ),
                )
                .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)))
                .child(
                    div()
                        .id("explorer.filter")
                        .h(px(36.))
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .bg(if self.is_filtering() {
                            rgb(ui_theme.accent_soft)
                        } else {
                            rgb(ui_theme.surface_alt_bg)
                        })
                        .focusable()
                        .cursor_pointer()
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                            this.on_filter_key(ev, cx);
                        }))
                        .child(ui_icon(ICON_FUNNEL, 14., ui_theme.text_muted))
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
                                    rgb(ui_theme.accent)
                                } else {
                                    rgb(ui_theme.text_muted)
                                })
                                .child(SharedString::from(if self.explorer_filter.is_empty() {
                                    "Filter files…".to_string()
                                } else {
                                    format!("Filter: {}", self.explorer_filter)
                                })),
                        ),
                )
                .child(div().h(px(2.)).w_full().bg(if self.is_filtering() {
                    rgb(ui_theme.accent)
                } else {
                    rgb(ui_theme.accent_soft)
                }))
                .child(
                    div()
                        .id("explorer.list")
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .bg(rgb(ui_theme.surface_alt_bg))
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
                                cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
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
                        .when(is_selected, |this| this.bg(rgb(ui_theme.accent_soft)))
                        .when(!is_selected, |this| this.hover(|this| this.bg(rgb(ui_theme.interactive_hover))))
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                          this.open_note(selected_path.clone(), cx);
                        }))
                        .child(ui_icon(
                          ICON_FILE_TEXT,
                          14.,
                          if is_selected { ui_theme.accent } else { ui_theme.text_muted },
                        ))
                        .child(
                          div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(if is_selected { 900. } else { 750. }))
                            .text_color(rgb(if is_selected { ui_theme.accent } else { ui_theme.text_primary }))
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
                        .when(
                          this
                            .selected_explorer_folder
                            .as_deref()
                            .is_some_and(str::is_empty),
                          |this| this.bg(rgb(ui_theme.accent_soft)),
                        )
                        .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                          this.selected_explorer_folder = Some(String::new());
                          this.selected_note = None;
                          match this.vault_state {
                            VaultState::Opened { .. } => this.toggle_folder_expanded(&folder, cx),
                            VaultState::Opening { .. } => {}
                            _ => this.open_vault_prompt(cx),
                          }
                        }))
                        .child(ui_icon(chevron, 14., ui_theme.text_muted))
                        .child(ui_icon(ICON_VAULT, 14., ui_theme.text_muted))
                        .child(
                          div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(900.))
                            .text_color(rgb(ui_theme.text_primary))
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
                                                .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                                                .on_click(cx.listener(
                                                    |this, _ev: &ClickEvent, _window, cx| {
                                                        this.open_vault_prompt(cx);
                                                    },
                                                ))
                                                .child(ui_icon(ICON_FOLDER_OPEN, 14., ui_theme.text_muted))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .overflow_hidden()
                                                        .font_family("IBM Plex Mono")
                                                        .text_size(px(11.))
                                                        .font_weight(FontWeight(650.))
                                                        .text_color(rgb(ui_theme.text_muted))
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
                        .when(
                            this.selected_explorer_folder.as_deref() == Some(folder.as_str()),
                            |this| this.bg(rgb(ui_theme.accent_soft)),
                        )
                        .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                          this.selected_explorer_folder = Some(folder_path.clone());
                          this.selected_note = None;
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
                                  .bg(rgb(ui_theme.border)),
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
                            .text_color(rgb(if *expanded {
                                ui_theme.text_primary
                            } else {
                                ui_theme.text_secondary
                            }))
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

                                                let icon_color = if is_selected {
                                                    ui_theme.accent
                                                } else {
                                                    ui_theme.text_muted
                                                };
                                                let text_color = if is_selected {
                                                    ui_theme.accent
                                                } else {
                                                    ui_theme.text_primary
                                                };
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
                        .when(is_selected, |this| this.bg(rgb(ui_theme.accent_soft)))
                        .when(!is_selected && !is_drag_target, |this| {
                          this.hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                        })
                        .when(is_drag_target, |this| {
                          this
                            .bg(rgb(ui_theme.accent_soft))
                            .border_1()
                            .border_color(rgb(ui_theme.accent))
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
      .bg(rgb(ui_theme.surface_alt_bg))
      .flex()
      .flex_col()
      .child(
        div()
          .h(px(28.))
          .px_3()
          .flex()
          .items_center()
          .bg(rgb(ui_theme.panel_bg))
          .gap(px(10.))
          .child(
            div()
              .font_family("IBM Plex Mono")
              .text_size(px(10.))
              .font_weight(FontWeight(900.))
              .text_color(rgb(ui_theme.text_secondary))
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
              .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
              .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                this.set_panel_shell_collapsed(true, cx);
              }))
              .child(ui_icon(ICON_PANEL_LEFT_CLOSE, 16., ui_theme.text_muted)),
          )
      )
      .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)))
      .child(
        div()
          .id("search.input")
          .h(px(36.))
          .px_3()
          .flex()
          .items_center()
          .gap_2()
          .bg(if self.search_query.trim().is_empty() {
            rgb(ui_theme.surface_alt_bg)
          } else {
            rgb(ui_theme.accent_soft)
          })
          .focusable()
          .cursor_pointer()
          .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
            this.on_search_key(ev, cx);
          }))
          .child(ui_icon(ICON_SEARCH, 14., if self.search_query.trim().is_empty() {
            ui_theme.text_muted
          } else {
            ui_theme.accent
          }))
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
                rgb(ui_theme.text_muted)
              } else {
                rgb(ui_theme.accent)
              })
              .child(SharedString::from(if self.search_query.trim().is_empty() {
                "Search…".to_string()
              } else {
                format!("Search: {}", self.search_query.trim())
              })),
          ),
      )
      .child(div().h(px(2.)).w_full().bg(if self.search_query.trim().is_empty() {
        rgb(ui_theme.accent_soft)
      } else {
        rgb(ui_theme.accent)
      }))
      .child(
        div()
          .id("search.results")
          .flex_1()
          .min_h_0()
          .w_full()
          .bg(rgb(ui_theme.surface_alt_bg))
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
                  .text_color(rgb(ui_theme.text_secondary))
                  .child(search_vault_label.clone()),
              )
              .child(
                div()
                  .font_family("IBM Plex Mono")
                  .text_size(px(10.))
                  .font_weight(FontWeight(900.))
                  .text_color(rgb(ui_theme.text_muted))
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
                  cx.processor(move |this, range: std::ops::Range<usize>, _window, list_cx| {
                    if this.search_query.trim().is_empty() {
                      return range
                        .map(|ix| {
                          div()
                            .id(ElementId::named_usize("search.placeholder", ix))
                            .h(px(22.))
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(ui_theme.text_muted))
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
                            .text_color(rgb(ui_theme.text_muted))
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
                              .bg(if selected { rgb(ui_theme.accent_soft) } else { rgba(0x00000000) })
                              .when(!selected, |this| this.hover(|this| this.bg(rgb(ui_theme.interactive_hover))))
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
                                  .text_color(rgb(ui_theme.text_primary))
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
                              .bg(if selected { rgb(ui_theme.accent_soft) } else { rgba(0x00000000) })
                              .when(!selected, |this| this.hover(|this| this.bg(rgb(ui_theme.interactive_hover))))
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
                                  .text_color(rgb(ui_theme.text_secondary))
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
                .border_color(rgb(ui_theme.border))
                .bg(if active {
                    rgb(ui_theme.interactive_hover)
                } else {
                    rgb(ui_theme.panel_bg)
                })
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                    this.workspace_mode = mode;
                    cx.notify();
                }))
                .child(ui_icon(
                    icon,
                    16.,
                    if active {
                        ui_theme.accent
                    } else {
                        ui_theme.text_muted
                    },
                ))
        };

        let mode_bar = div()
            .h(px(28.))
            .w_full()
            .bg(rgb(ui_theme.panel_bg))
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
                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.set_workspace_collapsed(true, cx);
                    }))
                    .child(ui_icon(ICON_PANEL_RIGHT_CLOSE, 16., ui_theme.text_muted)),
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
                        .text_color(rgb(ui_theme.text_muted))
                        .child("No open editors"),
                );
            } else {
                for path in &self.open_editors {
                    let is_active = self.open_note_path.as_deref() == Some(path.as_str());
                    let icon_color = if is_active {
                        ui_theme.accent
                    } else {
                        ui_theme.text_muted
                    };
                    let text_color = if is_active {
                        ui_theme.text_primary
                    } else {
                        ui_theme.text_secondary
                    };
                    let close_color = if is_active {
                        ui_theme.text_muted
                    } else {
                        ui_theme.text_subtle
                    };
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
                        .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
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
                                rgb(ui_theme.interactive_hover)
                            } else {
                                rgba(0x00000000)
                            })
                            .cursor_pointer()
                            .when(!is_active, |this| {
                                this.hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                            })
                            .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                                this.open_note(path.clone(), cx);
                            }))
                            .child(div().w(px(3.)).h_full().bg(if is_active {
                                rgb(ui_theme.accent_soft)
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
                        .bg(rgb(ui_theme.panel_bg))
                        .px_3()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .font_family("IBM Plex Mono")
                                .text_size(px(10.))
                                .font_weight(FontWeight(900.))
                                .text_color(rgb(ui_theme.text_secondary))
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
                        .bg(rgb(ui_theme.surface_alt_bg))
                        .child(open_list),
                )
        };

        let references_view = {
            let mut links_list = div().w_full().flex().flex_col().gap(px(2.));
            let mut backlinks_list = div().w_full().flex().flex_col().gap(px(2.));
            let mut links_count = 0usize;
            let mut backlinks_count = 0usize;

            if let Some(open_path) = self.open_note_path.as_deref() {
                if let Some(index) = self.knowledge_index.as_ref() {
                    if let Some(summary) = index.note_summary(open_path) {
                        links_count = summary.links.len();
                        if summary.links.is_empty() {
                            links_list = links_list.child(
                                div()
                                    .h(px(24.))
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .px_2()
                                    .font_family("IBM Plex Mono")
                                    .text_size(px(10.))
                                    .font_weight(FontWeight(650.))
                                    .text_color(rgb(ui_theme.text_muted))
                                    .child("No outgoing links"),
                            );
                        } else {
                            for raw_link in summary.links.into_iter().take(64) {
                                let resolved = index.resolve_link_target(&raw_link);
                                let label = resolved
                                    .as_deref()
                                    .map(file_name)
                                    .unwrap_or_else(|| raw_link.clone());

                                let row = div()
                                    .id(ElementId::Name(SharedString::from(format!(
                                        "workspace.refs.link:{open_path}:{raw_link}"
                                    ))))
                                    .h(px(24.))
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .bg(rgb(ui_theme.surface_alt_bg))
                                    .border_1()
                                    .border_color(rgb(ui_theme.border))
                                    .child(ui_icon(ICON_LINK_2, 12., ui_theme.text_muted))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .font_family("IBM Plex Mono")
                                            .text_size(px(10.))
                                            .font_weight(FontWeight(700.))
                                            .text_color(rgb(if resolved.is_some() {
                                                ui_theme.text_secondary
                                            } else {
                                                0xef4444
                                            }))
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(label),
                                    );

                                links_list = if let Some(target_path) = resolved {
                                    links_list.child(
                                        row.cursor_pointer()
                                            .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                                            .on_click(cx.listener(
                                                move |this, _ev: &ClickEvent, _window, cx| {
                                                    this.open_note(target_path.clone(), cx);
                                                },
                                            )),
                                    )
                                } else {
                                    links_list.child(row)
                                };
                            }
                        }
                    }

                    let backlinks = index.backlinks_for(open_path, 64);
                    backlinks_count = backlinks.len();
                    if backlinks.is_empty() {
                        backlinks_list = backlinks_list.child(
                            div()
                                .h(px(24.))
                                .w_full()
                                .flex()
                                .items_center()
                                .px_2()
                                .font_family("IBM Plex Mono")
                                .text_size(px(10.))
                                .font_weight(FontWeight(650.))
                                .text_color(rgb(ui_theme.text_muted))
                                .child("No backlinks"),
                        );
                    } else {
                        for source_path in backlinks {
                            let source_label = self.note_title_for_path(&source_path);
                            backlinks_list = backlinks_list.child(
                                div()
                                    .id(ElementId::Name(SharedString::from(format!(
                                        "workspace.refs.backlink:{open_path}:{source_path}"
                                    ))))
                                    .h(px(24.))
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .bg(rgb(ui_theme.surface_alt_bg))
                                    .border_1()
                                    .border_color(rgb(ui_theme.border))
                                    .cursor_pointer()
                                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                                    .on_click(cx.listener(
                                        move |this, _ev: &ClickEvent, _window, cx| {
                                            this.open_note(source_path.clone(), cx);
                                        },
                                    ))
                                    .child(ui_icon(ICON_LINK_2, 12., ui_theme.text_muted))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .font_family("IBM Plex Mono")
                                            .text_size(px(10.))
                                            .font_weight(FontWeight(700.))
                                            .text_color(rgb(ui_theme.text_secondary))
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(source_label),
                                    ),
                            );
                        }
                    }
                } else {
                    links_list = links_list.child(
                        div()
                            .h(px(24.))
                            .w_full()
                            .flex()
                            .items_center()
                            .px_2()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child("Index building..."),
                    );
                    backlinks_list = backlinks_list.child(
                        div()
                            .h(px(24.))
                            .w_full()
                            .flex()
                            .items_center()
                            .px_2()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child("Index building..."),
                    );
                }
            } else {
                links_list = links_list.child(
                    div()
                        .h(px(24.))
                        .w_full()
                        .flex()
                        .items_center()
                        .px_2()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child("Open a note to inspect links"),
                );
                backlinks_list = backlinks_list.child(
                    div()
                        .h(px(24.))
                        .w_full()
                        .flex()
                        .items_center()
                        .px_2()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child("Open a note to inspect backlinks"),
                );
            }

            div()
                .id("workspace.refs")
                .flex()
                .flex_col()
                .w_full()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .gap_2()
                .bg(rgb(ui_theme.panel_bg))
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(900.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child("REFERENCES"),
                )
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(800.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child(SharedString::from(format!("LINKS ({links_count})"))),
                )
                .child(links_list)
                .child(
                    div()
                        .mt_2()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(800.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child(SharedString::from(format!("BACKLINKS ({backlinks_count})"))),
                )
                .child(backlinks_list)
        };

        let bookmarks_view = {
            let bookmark_paths = self.bookmarked_notes_snapshot();
            let mut bookmark_list = div().w_full().flex().flex_col().gap(px(2.)).pt(px(4.));

            if bookmark_paths.is_empty() {
                bookmark_list = bookmark_list.child(
                    div()
                        .h(px(24.))
                        .w_full()
                        .flex()
                        .items_center()
                        .px_2()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child("No bookmarks"),
                );
            } else {
                for bookmark_path in &bookmark_paths {
                    let exists = self.note_exists(bookmark_path);
                    let is_active = self.open_note_path.as_deref() == Some(bookmark_path.as_str());
                    let bookmark_title = self.note_title_for_path(bookmark_path);
                    let open_path = bookmark_path.clone();
                    let remove_path = bookmark_path.clone();

                    let remove_button = div()
                        .id(ElementId::Name(SharedString::from(format!(
                            "workspace.bookmarks.remove:{remove_path}"
                        ))))
                        .h(px(22.))
                        .w(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                        .occlude()
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                            this.app_settings
                                .bookmarked_notes
                                .retain(|path| path != &remove_path);
                            this.persist_settings();
                            this.status = SharedString::from("Bookmark removed");
                            cx.notify();
                        }))
                        .child(ui_icon(ICON_X, 12., ui_theme.text_muted));

                    let row = div()
                        .id(ElementId::Name(SharedString::from(format!(
                            "workspace.bookmarks.item:{bookmark_path}"
                        ))))
                        .h(px(28.))
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .bg(if is_active {
                            rgb(ui_theme.interactive_hover)
                        } else {
                            rgb(ui_theme.surface_alt_bg)
                        })
                        .border_1()
                        .border_color(rgb(ui_theme.border))
                        .child(ui_icon(
                            ICON_BOOKMARK,
                            12.,
                            if exists { ui_theme.accent } else { 0xef4444 },
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .font_family("IBM Plex Mono")
                                .text_size(px(10.))
                                .font_weight(FontWeight(if is_active { 800. } else { 700. }))
                                .text_color(rgb(if exists {
                                    ui_theme.text_secondary
                                } else {
                                    ui_theme.text_subtle
                                }))
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(bookmark_title),
                        )
                        .child(remove_button);

                    bookmark_list = if exists {
                        bookmark_list.child(
                            row.cursor_pointer()
                                .when(!is_active, |this| {
                                    this.hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                                })
                                .on_click(cx.listener(
                                    move |this, _ev: &ClickEvent, _window, cx| {
                                        this.open_note(open_path.clone(), cx);
                                    },
                                )),
                        )
                    } else {
                        bookmark_list.child(row)
                    };
                }
            }

            div()
                .id("workspace.bookmarks")
                .flex()
                .flex_col()
                .w_full()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .gap_2()
                .bg(rgb(ui_theme.panel_bg))
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(900.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child("BOOKMARKS"),
                )
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child(SharedString::from(format!(
                            "Pinned notes ({})",
                            bookmark_paths.len()
                        ))),
                )
                .child(bookmark_list)
        };

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
            .bg(rgb(ui_theme.panel_bg))
            .flex()
            .flex_col()
            .child(mode_bar)
            .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)))
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
                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        on_click(this, cx);
                    }))
                    .child(ui_icon(icon, 16., ui_theme.text_muted))
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
            .bg(rgb(ui_theme.panel_bg))
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
                .child(div().w(px(1.)).h(px(16.)).bg(rgb(ui_theme.border)));
        }

        for path in &self.open_editors {
            let is_active = self.open_note_path.as_deref() == Some(path.as_str());
            let icon_color = if is_active {
                ui_theme.accent
            } else {
                ui_theme.text_muted
            };
            let text_color = if is_active {
                ui_theme.text_primary
            } else {
                ui_theme.text_secondary
            };
            let close_color = if is_active {
                ui_theme.text_muted
            } else {
                ui_theme.text_subtle
            };
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
                .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
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
                        rgb(ui_theme.surface_bg)
                    } else {
                        rgb(ui_theme.panel_bg)
                    })
                    .cursor_pointer()
                    .when(!is_active, |this| {
                        this.hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    })
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

        let split_active = self.editor_view_mode == EditorViewMode::Split;
        let split_action = div()
            .id("editor.action.split")
            .h(px(28.))
            .w(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(if split_active {
                rgb(ui_theme.interactive_hover)
            } else {
                rgb(ui_theme.panel_bg)
            })
            .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                this.toggle_editor_split_mode(cx);
            }))
            .child(ui_icon(
                ICON_COLUMNS_2,
                16.,
                if split_active {
                    ui_theme.accent
                } else {
                    ui_theme.text_muted
                },
            ));

        tabs_bar = tabs_bar
            .child(div().flex_1())
            .child(tab_action("editor.new", ICON_PLUS, |this, cx| {
                this.create_new_note(cx);
            }))
            .child(tab_action("editor.bookmark", ICON_BOOKMARK, |this, cx| {
                this.toggle_current_note_bookmark(cx);
            }))
            .child(tab_action("editor.export", ICON_DOWNLOAD, |this, cx| {
                this.export_open_note(cx);
            }))
            .child(split_action)
            .child(div().w(px(4.)));

        let mode_toggle = |id: &'static str,
                           icon: &'static str,
                           mode: EditorViewMode,
                           this: &XnoteWindow,
                           cx: &mut Context<Self>| {
            let active = this.editor_view_mode == mode;
            div()
                .id(id)
                .h(px(28.))
                .w(px(28.))
                .min_w(px(28.))
                .max_w(px(28.))
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .bg(if active {
                    rgb(ui_theme.interactive_hover)
                } else {
                    rgb(ui_theme.surface_bg)
                })
                .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                    this.set_editor_view_mode(mode, cx);
                }))
                .child(ui_icon(
                    icon,
                    13.,
                    if active {
                        ui_theme.accent
                    } else {
                        ui_theme.text_muted
                    },
                ))
        };

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
                            .text_color(rgb(ui_theme.text_muted))
                            .child(folder),
                    )
                    .child(ui_icon(ICON_CHEVRON_RIGHT, 12., ui_theme.text_subtle));
            }
            segments = segments.child(
                div()
                    .font_family("IBM Plex Mono")
                    .text_size(px(11.))
                    .font_weight(FontWeight(850.))
                    .text_color(rgb(ui_theme.text_primary))
                    .child(file),
            );

            div()
                .h(px(28.))
                .w_full()
                .bg(rgb(ui_theme.surface_bg))
                .pl(px(18.))
                .pr(px(4.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().overflow_hidden().child(segments)),
                )
                .child(
                    div()
                        .w(px(120.))
                        .min_w(px(120.))
                        .max_w(px(120.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(0.))
                        .child(mode_toggle(
                            "editor.mode.edit",
                            ICON_BRUSH,
                            EditorViewMode::Edit,
                            self,
                            cx,
                        ))
                        .child(mode_toggle(
                            "editor.mode.preview",
                            ICON_EYE,
                            EditorViewMode::Preview,
                            self,
                            cx,
                        )),
                )
        };

        let editor_header = div()
            .h(px(64.))
            .w_full()
            .bg(rgb(ui_theme.surface_bg))
            .px(px(18.))
            .py(px(12.))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .font_family("Inter")
                    .text_size(px(20.))
                    .font_weight(FontWeight(900.))
                    .text_color(rgb(ui_theme.text_primary))
                    .child(note_title),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(SharedString::from(format!(
                                "H{}",
                                self.open_note_heading_count
                            ))),
                    )
                    .child(div().w(px(1.)).h(px(12.)).bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(SharedString::from(format!(
                                "L{}",
                                self.open_note_link_count
                            ))),
                    )
                    .child(div().w(px(1.)).h(px(12.)).bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(SharedString::from(format!(
                                "Code {}",
                                self.open_note_code_fence_count
                            ))),
                    )
                    .child(div().w(px(1.)).h(px(12.)).bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(SharedString::from(format!(
                                "Diag {}",
                                self.markdown_diagnostics.len()
                            ))),
                    ),
            );

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
                .text_color(rgb(ui_theme.text_primary))
                .bg(rgb(ui_theme.surface_bg));

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
                    .text_color(rgb(ui_theme.text_muted))
                    .child(editor_body_placeholder.clone())
                    .into_any_element()
            }
        };

        let preview_pane = || {
            let mut pane = div()
                .id("editor.preview")
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_y_scroll()
                .px(px(18.))
                .py(px(10.))
                .bg(rgb(ui_theme.surface_bg))
                .flex()
                .flex_col()
                .gap(px(10.));

            if self.markdown_preview.headings.is_empty() && self.markdown_preview.blocks.is_empty() {
                pane = pane.child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(11.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child("No markdown structure yet."),
                );
            } else {
                if !self.markdown_preview.headings.is_empty() {
                    let mut toc = div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .p(px(8.))
                        .border_1()
                        .border_color(rgb(ui_theme.border))
                        .bg(rgb(ui_theme.surface_alt_bg));

                    toc = toc.child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child("OUTLINE"),
                    );

                    for (level, heading) in self.markdown_preview.headings.iter().take(64) {
                        toc = toc.child(
                            div()
                                .pl(px(((*level as i32 - 1).max(0) * 10) as f32))
                                .font_family("Inter")
                                .text_size(px(12.))
                                .font_weight(FontWeight(700.))
                                .text_color(rgb(ui_theme.text_primary))
                                .child(heading.clone()),
                        );
                    }

                    pane = pane.child(toc);
                }

                let mut blocks = div().flex().flex_col().gap(px(8.));
                for block in self.markdown_preview.blocks.iter().take(240) {
                    let (font_family, text_size, font_weight, text_color, prefix) = match block.kind {
                        MarkdownPreviewBlockKind::Heading(level) => (
                            "Inter",
                            px((20_i32 - (level as i32 * 2)).max(12) as f32),
                            FontWeight(900.),
                            ui_theme.text_primary,
                            "",
                        ),
                        MarkdownPreviewBlockKind::Paragraph => {
                            ("Inter", px(13.), FontWeight(650.), ui_theme.text_secondary, "")
                        }
                        MarkdownPreviewBlockKind::CodeFence => (
                            "IBM Plex Mono",
                            px(12.),
                            FontWeight(650.),
                            ui_theme.text_primary,
                            "``` ",
                        ),
                        MarkdownPreviewBlockKind::Quote => (
                            "Inter",
                            px(13.),
                            FontWeight(650.),
                            ui_theme.text_muted,
                            "▍ ",
                        ),
                        MarkdownPreviewBlockKind::List => (
                            "Inter",
                            px(13.),
                            FontWeight(650.),
                            ui_theme.text_secondary,
                            "• ",
                        ),
                    };

                    blocks = blocks.child(
                        div()
                            .font_family(font_family)
                            .text_size(text_size)
                            .font_weight(font_weight)
                            .text_color(rgb(text_color))
                            .child(SharedString::from(format!("{prefix}{}", block.text))),
                    );
                }

                pane = pane.child(blocks);
            }

            pane.into_any_element()
        };

        let editor_body = if self.editor_view_mode == EditorViewMode::Split {
            div()
                .id("editor.split")
                .flex_1()
                .min_h_0()
                .flex()
                .flex_row()
                .child(editor_pane("editor.pane.left", true))
                .child(div().w(px(1.)).h_full().bg(rgb(ui_theme.border)))
                .child(preview_pane())
                .into_any_element()
        } else if self.editor_view_mode == EditorViewMode::Preview {
            preview_pane()
        } else {
            editor_pane("editor.pane.single", true)
        };

        let diagnostics_panel = {
            let mut panel = div()
                .id("editor.diagnostics")
                .h(px(112.))
                .w_full()
                .min_h(px(112.))
                .max_h(px(112.))
                .flex_shrink_0()
                .border_t_1()
                .border_color(rgb(ui_theme.border))
                .bg(rgb(ui_theme.surface_alt_bg))
                .px(px(12.))
                .py(px(8.))
                .flex()
                .flex_col()
                .gap(px(6.));

            panel = panel.child(
                div()
                    .font_family("IBM Plex Mono")
                    .text_size(px(10.))
                    .font_weight(FontWeight(800.))
                    .text_color(rgb(ui_theme.text_muted))
                    .child("DIAGNOSTICS"),
            );

            if self.markdown_diagnostics.is_empty() {
                panel = panel.child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_subtle))
                        .child("No diagnostics"),
                );
            } else {
                panel = panel.child(
                    div().flex_1().min_h_0().child(uniform_list(
                        "editor.diagnostics.list",
                        self.markdown_diagnostics.len().min(128),
                        cx.processor(move |this, range: std::ops::Range<usize>, _window, _cx| {
                            range
                                .map(|ix| {
                                    let Some(diag) = this.markdown_diagnostics.get(ix) else {
                                        return div()
                                            .id(ElementId::named_usize(
                                                "editor.diagnostics.missing",
                                                ix,
                                            ))
                                            .h(px(18.));
                                    };

                                    let ui_theme = UiTheme::from_settings(
                                        this.settings_theme,
                                        this.settings_accent,
                                    );
                                    let (label, color) = match diag.severity {
                                        MarkdownDiagnosticSeverity::Info => {
                                            ("INFO", ui_theme.diagnostic_info)
                                        }
                                        MarkdownDiagnosticSeverity::Warning => {
                                            ("WARN", ui_theme.diagnostic_warning)
                                        }
                                        MarkdownDiagnosticSeverity::Error => {
                                            ("ERROR", ui_theme.diagnostic_error)
                                        }
                                    };

                                    div()
                                        .id(ElementId::named_usize("editor.diagnostics.row", ix))
                                        .h(px(18.))
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .w(px(44.))
                                                .font_family("IBM Plex Mono")
                                                .text_size(px(10.))
                                                .font_weight(FontWeight(800.))
                                                .text_color(rgb(color))
                                                .child(label),
                                        )
                                        .child(
                                            div()
                                                .w(px(56.))
                                                .font_family("IBM Plex Mono")
                                                .text_size(px(10.))
                                                .font_weight(FontWeight(750.))
                                                .text_color(rgb(ui_theme.text_muted))
                                                .child(SharedString::from(format!(
                                                    "Ln {}",
                                                    diag.line
                                                ))),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .font_family("IBM Plex Mono")
                                                .text_size(px(10.))
                                                .font_weight(FontWeight(650.))
                                                .text_color(rgb(ui_theme.text_secondary))
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .child(diag.message.clone()),
                                        )
                                })
                                .collect()
                        }),
                    )),
                );
            }

            panel
        };

        let editor = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(rgb(ui_theme.surface_bg))
            .flex()
            .flex_col()
            .child(tabs_bar)
            .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)))
            .child(breadcrumbs)
            .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)))
            .child(editor_header)
            .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)))
            .child(editor_body)
            .child(diagnostics_panel);

        let titlebar_command = div()
            .id("titlebar.command")
            .w(px(520.))
            .h(px(24.))
            .bg(rgb(ui_theme.app_bg))
            .border_1()
            .border_color(rgb(ui_theme.border))
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
                    .child(ui_icon(ICON_SEARCH, 14., ui_theme.text_muted))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child("Search / Commands (Ctrl+K)"),
                    ),
            )
            .child(div().h(px(2.)).w_full().bg(rgb(ui_theme.accent_soft)));

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
                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    .on_click(
                        move |_ev: &ClickEvent, window: &mut Window, _cx: &mut App| {
                            on_click(window);
                        },
                    )
                    .child(ui_icon(icon, 16., ui_theme.text_muted))
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
            .bg(rgb(ui_theme.titlebar_bg))
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
            .bg(rgb(ui_theme.panel_bg))
            .flex()
            .flex_col()
            .child(menu_bar)
            .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)));

        let workspace_name = match &self.vault_state {
            VaultState::Opened { root_name, .. } => root_name.to_string(),
            _ => "None".to_string(),
        };

        let (cursor_line, cursor_col) = self.cursor_line_col();

        let (sync_status, sync_status_dot_color) = if self.open_note_loading {
            (SharedString::from("Loading"), ui_theme.status_loading)
        } else if self.open_note_dirty {
            (SharedString::from("Unsaved"), ui_theme.status_dirty)
        } else {
            (SharedString::from("Synced"), ui_theme.status_synced)
        };

        let index_hint = match &self.index_state {
            IndexState::Idle => SharedString::from("Index: Idle"),
            IndexState::Building => SharedString::from("Index: Building"),
            IndexState::Ready {
                note_count,
                duration_ms,
            } => SharedString::from(format!("Index: {note_count} ({duration_ms} ms)")),
            IndexState::Error { message } => SharedString::from(format!("Index error: {message}")),
        };

        let watch_hint = if let Some(err) = &self.watcher_status.last_error {
            SharedString::from(format!("Watch error: {err}"))
        } else {
            SharedString::from(format!("Watch rev {}", self.watcher_status.revision))
        };

        let search_total = self.cache_stats.search_hits + self.cache_stats.search_misses;
        let quick_total = self.cache_stats.quick_open_hits + self.cache_stats.quick_open_misses;
        let search_hit_rate = if search_total == 0 {
            0.0
        } else {
            (self.cache_stats.search_hits as f64 * 100.0) / search_total as f64
        };
        let quick_hit_rate = if quick_total == 0 {
            0.0
        } else {
            (self.cache_stats.quick_open_hits as f64 * 100.0) / quick_total as f64
        };
        let cache_hint = SharedString::from(format!(
            "Cache S {:.0}% ({}/{}) · Q {:.0}% ({}/{})",
            search_hit_rate,
            self.cache_stats.search_hits,
            search_total,
            quick_hit_rate,
            self.cache_stats.quick_open_hits,
            quick_total
        ));

        let status_bar = div()
            .h(px(28.))
            .w_full()
            .bg(rgb(ui_theme.panel_bg))
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
                            .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
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
                                    .text_color(rgb(ui_theme.text_primary))
                                    .child("Knowledge"),
                            )
                            .child(ui_icon(ICON_CHEVRON_DOWN, 14., 0x6b7280)),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .id("status.workspace")
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .cursor_pointer()
                            .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                            .px_2()
                            .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                                this.open_vault_prompt(cx);
                            }))
                            .child(
                                div()
                                    .font_family("IBM Plex Mono")
                                    .text_size(px(11.))
                                    .font_weight(FontWeight(750.))
                                    .text_color(rgb(ui_theme.text_primary))
                                    .child(SharedString::from(format!(
                                        "Workspace: {workspace_name}"
                                    ))),
                            )
                            .child(ui_icon(ICON_CHEVRON_DOWN, 16., 0x6b7280)),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_primary))
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
                            .text_color(rgb(ui_theme.text_muted))
                            .child(SharedString::from(format!(
                                "Ln {cursor_line}, Col {cursor_col}"
                            ))),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(SharedString::from(self.editor_mode_label())),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(700.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(SharedString::from(format!(
                                "Edit p50 {}ms p95 {}ms (n={})",
                                self.edit_latency_stats.p50_ms(),
                                self.edit_latency_stats.p95_ms(),
                                self.edit_latency_stats.sample_count()
                            ))),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(700.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(index_hint),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(700.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(watch_hint),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(ui_theme.border)))
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(700.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(cache_hint),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(ui_theme.border)))
                    .child(ui_icon(ICON_REFRESH_CW, 14., 0x6b7280))
                    .child(
                        div()
                            .w(px(STATUS_SYNC_SLOT_WIDTH))
                            .min_w(px(STATUS_SYNC_SLOT_WIDTH))
                            .max_w(px(STATUS_SYNC_SLOT_WIDTH))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .justify_start()
                            .child(
                                div()
                                    .w(px(6.))
                                    .h(px(6.))
                                    .rounded_md()
                                    .bg(rgb(sync_status_dot_color)),
                            )
                            .child(
                                div()
                                    .font_family("IBM Plex Mono")
                                    .text_size(px(10.))
                                    .font_weight(FontWeight(800.))
                                    .text_color(rgb(ui_theme.text_primary))
                                    .child(sync_status),
                            ),
                    ),
            );

        let splitter_handle = |id: &'static str, kind: SplitterKind| {
            let active = self.splitter_drag.is_some_and(|d| d.kind == kind);
            let line = rgb(ui_theme.border);
            div()
                .id(id)
                .w(splitter_w)
                .min_w(splitter_w)
                .max_w(splitter_w)
                .flex_shrink_0()
                .h_full()
                .relative()
                .bg(if active {
                    rgb(ui_theme.interactive_hover)
                } else {
                    rgb(ui_theme.app_bg)
                })
                .cursor_col_resize()
                .hover(|this| this.bg(rgb(ui_theme.panel_bg)))
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
            .bg(rgb(ui_theme.app_bg))
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

fn build_explorer_index(vault: &Vault, scan: &VaultScan) -> anyhow::Result<ExplorerIndex> {
    let mut all_note_paths = Vec::with_capacity(scan.notes.len());
    let mut all_note_paths_lower = Vec::with_capacity(scan.notes.len());
    for e in &scan.notes {
        all_note_paths.push(e.path.clone());
        all_note_paths_lower.push(e.path.to_lowercase());
    }

    let mut by_folder: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in &scan.notes {
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

    for folder in &scan.folders {
        if folder.is_empty() {
            continue;
        }

        let mut full = folder.clone();
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

fn compute_entries_fingerprint(paths: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();
    paths.len().hash(&mut hasher);
    for path in paths {
        path.hash(&mut hasher);
    }
    hasher.finish()
}

fn touch_cache_order(key: &str, order: &mut VecDeque<String>, capacity: usize) -> Option<String> {
    if let Some(pos) = order.iter().position(|existing| existing == key) {
        order.remove(pos);
    }
    order.push_back(key.to_string());
    if order.len() > capacity {
        return order.pop_front();
    }
    None
}

fn folder_of_note_path(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(folder, _)| folder.to_string())
        .unwrap_or_default()
}

fn resolve_base_folder_for_new_items(
    selected_explorer_folder: Option<&str>,
    selected_note: Option<&str>,
    folder_notes: &HashMap<String, Vec<String>>,
    folder_children: &HashMap<String, Vec<String>>,
) -> String {
    if let Some(folder) = selected_explorer_folder {
        if folder.is_empty()
            || folder_notes.contains_key(folder)
            || folder_children.contains_key(folder)
        {
            return folder.to_string();
        }
    }

    match selected_note.and_then(|selected| selected.rsplit_once('/')) {
        Some((folder, _)) => folder.to_string(),
        None => String::new(),
    }
}

fn ensure_folder_branch(
    folder: &str,
    folder_children: &mut HashMap<String, Vec<String>>,
    folder_notes: &mut HashMap<String, Vec<String>>,
) {
    folder_children.entry(String::new()).or_default();
    folder_notes.entry(String::new()).or_default();

    if folder.is_empty() {
        return;
    }

    let mut current = folder.to_string();
    loop {
        let parent = current
            .rsplit_once('/')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default();

        let children = folder_children.entry(parent.clone()).or_default();
        if !children.iter().any(|existing| existing == &current) {
            children.push(current.clone());
            children.sort();
        }

        folder_notes.entry(current.clone()).or_default();
        if parent.is_empty() {
            break;
        }
        current = parent;
    }
}

fn add_note_to_tree_structures(
    note_path: &str,
    folder_children: &mut HashMap<String, Vec<String>>,
    folder_notes: &mut HashMap<String, Vec<String>>,
) {
    let folder = folder_of_note_path(note_path);
    ensure_folder_branch(&folder, folder_children, folder_notes);
    let notes = folder_notes.entry(folder).or_default();
    if !notes.iter().any(|existing| existing == note_path) {
        notes.push(note_path.to_string());
        notes.sort();
    }
}

fn remove_note_from_tree_structures(
    note_path: &str,
    folder_children: &mut HashMap<String, Vec<String>>,
    folder_notes: &mut HashMap<String, Vec<String>>,
) {
    let folder = folder_of_note_path(note_path);
    if let Some(notes) = folder_notes.get_mut(&folder) {
        notes.retain(|existing| existing != note_path);
    }

    let mut current = folder;
    loop {
        if current.is_empty() {
            break;
        }

        let has_notes = folder_notes
            .get(&current)
            .is_some_and(|notes| !notes.is_empty());
        let has_children = folder_children
            .get(&current)
            .is_some_and(|children| !children.is_empty());

        if has_notes || has_children {
            break;
        }

        folder_notes.remove(&current);
        folder_children.remove(&current);

        let parent = current
            .rsplit_once('/')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default();
        if let Some(siblings) = folder_children.get_mut(&parent) {
            siblings.retain(|child| child != &current);
        }

        current = parent;
    }
}

fn rename_note_in_tree_structures(
    old_path: &str,
    new_path: &str,
    folder_children: &mut HashMap<String, Vec<String>>,
    folder_notes: &mut HashMap<String, Vec<String>>,
) {
    remove_note_from_tree_structures(old_path, folder_children, folder_notes);
    add_note_to_tree_structures(new_path, folder_children, folder_notes);
}

fn derive_prefix_moves(moved_pairs: &[(String, String)]) -> Option<Vec<(String, String)>> {
    let mut from_to = HashMap::<String, String>::new();
    let mut to_from = HashMap::<String, String>::new();

    for (from, to) in moved_pairs {
        let Some(from_folder) = from.rsplit_once('/').map(|(folder, _)| folder) else {
            continue;
        };
        let Some(to_folder) = to.rsplit_once('/').map(|(folder, _)| folder) else {
            continue;
        };
        if from_folder.is_empty() || to_folder.is_empty() {
            continue;
        }
        if from_folder == to_folder {
            continue;
        }

        let from_prefix = format!("{from_folder}/");
        let to_prefix = format!("{to_folder}/");

        if let Some(existing) = from_to.get(&from_prefix) {
            if existing != &to_prefix {
                return None;
            }
        } else {
            from_to.insert(from_prefix.clone(), to_prefix.clone());
        }

        if let Some(existing) = to_from.get(&to_prefix) {
            if existing != &from_prefix {
                return None;
            }
        } else {
            to_from.insert(to_prefix, from_prefix);
        }
    }

    if from_to.is_empty() {
        return Some(Vec::new());
    }

    let mut out = from_to.into_iter().collect::<Vec<_>>();
    out.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));

    for i in 0..out.len() {
        for j in (i + 1)..out.len() {
            let (old_i, new_i) = (&out[i].0, &out[i].1);
            let (old_j, new_j) = (&out[j].0, &out[j].1);

            let overlap = old_i.starts_with(old_j) || old_j.starts_with(old_i);
            if !overlap {
                continue;
            }

            let forward_compatible = new_i.starts_with(new_j) || new_j.starts_with(new_i);
            if !forward_compatible {
                return None;
            }
        }
    }

    Some(out)
}

fn rewrite_path_with_prefix(path: &str, old_prefix: &str, new_prefix: &str) -> Option<String> {
    let suffix = path.strip_prefix(old_prefix)?;
    Some(format!("{new_prefix}{suffix}"))
}

fn push_cache_stats_snapshot(
    snapshots: &mut VecDeque<CacheStatsSnapshot>,
    stats: &QueryCacheStats,
    epoch_secs: u64,
    max_snapshots: usize,
) {
    if let Some(last) = snapshots.back_mut() {
        if last.epoch_secs == epoch_secs {
            last.stats = stats.clone();
            return;
        }
    }

    snapshots.push_back(CacheStatsSnapshot {
        epoch_secs,
        stats: stats.clone(),
    });

    while snapshots.len() > max_snapshots {
        snapshots.pop_front();
    }
}

fn current_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn resolve_window_baseline(
    snapshots: &[CacheStatsSnapshot],
    generated_at_utc: u64,
    window_secs: u64,
) -> QueryCacheStats {
    let cutoff = generated_at_utc.saturating_sub(window_secs);
    snapshots
        .iter()
        .rev()
        .find(|snapshot| snapshot.epoch_secs <= cutoff)
        .map(|snapshot| snapshot.stats.clone())
        .or_else(|| snapshots.first().map(|snapshot| snapshot.stats.clone()))
        .unwrap_or_default()
}

fn diff_cache_stats(now: &QueryCacheStats, baseline: &QueryCacheStats) -> QueryCacheStats {
    QueryCacheStats {
        search_hits: now.search_hits.saturating_sub(baseline.search_hits),
        search_misses: now.search_misses.saturating_sub(baseline.search_misses),
        quick_open_hits: now.quick_open_hits.saturating_sub(baseline.quick_open_hits),
        quick_open_misses: now
            .quick_open_misses
            .saturating_sub(baseline.quick_open_misses),
    }
}

fn build_hit_rate_percent(hits: u64, misses: u64) -> f64 {
    let total = hits + misses;
    if total == 0 {
        0.0
    } else {
        (hits as f64 * 100.0) / total as f64
    }
}

fn build_cache_diagnostics_payload(
    stats: &QueryCacheStats,
    snapshots: &[CacheStatsSnapshot],
    generated_at_utc: u64,
) -> Vec<u8> {
    let search_hit_rate_percent = build_hit_rate_percent(stats.search_hits, stats.search_misses);
    let quick_hit_rate_percent =
        build_hit_rate_percent(stats.quick_open_hits, stats.quick_open_misses);

    let short_baseline = resolve_window_baseline(
        snapshots,
        generated_at_utc,
        CACHE_DIAGNOSTICS_SHORT_WINDOW_SECS,
    );
    let long_baseline = resolve_window_baseline(
        snapshots,
        generated_at_utc,
        CACHE_DIAGNOSTICS_LONG_WINDOW_SECS,
    );
    let short_delta = diff_cache_stats(stats, &short_baseline);
    let long_delta = diff_cache_stats(stats, &long_baseline);

    serde_json::to_vec_pretty(&json!({
        "generated_at_utc_epoch": generated_at_utc,
        "version": 2,
        "snapshots": {
            "count": snapshots.len(),
            "max": CACHE_DIAGNOSTICS_MAX_SNAPSHOTS
        },
        "search": {
            "hits": stats.search_hits,
            "misses": stats.search_misses,
            "hit_rate_percent": search_hit_rate_percent
        },
        "quick_open": {
            "hits": stats.quick_open_hits,
            "misses": stats.quick_open_misses,
            "hit_rate_percent": quick_hit_rate_percent
        },
        "windows": {
            "short": {
                "seconds": CACHE_DIAGNOSTICS_SHORT_WINDOW_SECS,
                "search": {
                    "hits": short_delta.search_hits,
                    "misses": short_delta.search_misses,
                    "total": short_delta.search_hits + short_delta.search_misses,
                    "hit_rate_percent": build_hit_rate_percent(short_delta.search_hits, short_delta.search_misses)
                },
                "quick_open": {
                    "hits": short_delta.quick_open_hits,
                    "misses": short_delta.quick_open_misses,
                    "total": short_delta.quick_open_hits + short_delta.quick_open_misses,
                    "hit_rate_percent": build_hit_rate_percent(short_delta.quick_open_hits, short_delta.quick_open_misses)
                }
            },
            "long": {
                "seconds": CACHE_DIAGNOSTICS_LONG_WINDOW_SECS,
                "search": {
                    "hits": long_delta.search_hits,
                    "misses": long_delta.search_misses,
                    "total": long_delta.search_hits + long_delta.search_misses,
                    "hit_rate_percent": build_hit_rate_percent(long_delta.search_hits, long_delta.search_misses)
                },
                "quick_open": {
                    "hits": long_delta.quick_open_hits,
                    "misses": long_delta.quick_open_misses,
                    "total": long_delta.quick_open_hits + long_delta.quick_open_misses,
                    "hit_rate_percent": build_hit_rate_percent(long_delta.quick_open_hits, long_delta.quick_open_misses)
                }
            }
        }
    }))
    .unwrap_or_else(|_| b"{}".to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_prefix_moves_extracts_folder_prefix_mapping() {
        let moved = vec![
            ("notes/old/a.md".to_string(), "notes/new/a.md".to_string()),
            ("notes/old/b.md".to_string(), "notes/new/b.md".to_string()),
        ];

        let prefixes = derive_prefix_moves(&moved).expect("valid prefix mapping");
        assert_eq!(
            prefixes,
            vec![("notes/old/".to_string(), "notes/new/".to_string())]
        );
    }

    #[test]
    fn derive_prefix_moves_rejects_conflicting_targets() {
        let moved = vec![
            ("notes/old/a.md".to_string(), "notes/new-a/a.md".to_string()),
            ("notes/old/b.md".to_string(), "notes/new-b/b.md".to_string()),
        ];
        assert!(derive_prefix_moves(&moved).is_none());
    }

    #[test]
    fn derive_prefix_moves_prefers_longest_prefix() {
        let moved = vec![
            (
                "notes/root/sub/a.md".to_string(),
                "notes/target/sub/a.md".to_string(),
            ),
            (
                "notes/root/b.md".to_string(),
                "notes/target/b.md".to_string(),
            ),
        ];
        let prefixes = derive_prefix_moves(&moved).expect("valid prefix mapping");
        assert_eq!(
            prefixes[0],
            (
                "notes/root/sub/".to_string(),
                "notes/target/sub/".to_string()
            )
        );
        assert_eq!(
            prefixes[1],
            ("notes/root/".to_string(), "notes/target/".to_string())
        );
    }

    #[test]
    fn rewrite_path_with_prefix_rewrites_nested_paths() {
        let out = rewrite_path_with_prefix("notes/old/sub/c.md", "notes/old/", "notes/new/")
            .expect("rewritten");
        assert_eq!(out, "notes/new/sub/c.md");
    }

    #[test]
    fn build_cache_diagnostics_payload_contains_hit_rates() {
        let now = QueryCacheStats {
            search_hits: 8,
            search_misses: 2,
            quick_open_hits: 3,
            quick_open_misses: 1,
        };
        let snapshots = vec![CacheStatsSnapshot {
            epoch_secs: 1_000,
            stats: QueryCacheStats {
                search_hits: 6,
                search_misses: 2,
                quick_open_hits: 2,
                quick_open_misses: 1,
            },
        }];
        let payload = build_cache_diagnostics_payload(&now, &snapshots, 1_000 + 600);
        let parsed: serde_json::Value = serde_json::from_slice(&payload).expect("json");
        assert_eq!(parsed["version"], 2);
        assert_eq!(parsed["search"]["hit_rate_percent"], 80.0);
        assert_eq!(parsed["quick_open"]["hit_rate_percent"], 75.0);
        assert_eq!(parsed["windows"]["short"]["search"]["hits"], 2);
        assert_eq!(parsed["windows"]["short"]["search"]["misses"], 0);
        assert_eq!(parsed["windows"]["short"]["quick_open"]["hits"], 1);
        assert_eq!(parsed["windows"]["short"]["quick_open"]["misses"], 0);
    }

    #[test]
    fn push_cache_stats_snapshot_keeps_bounded_recent_window() {
        let mut snapshots = VecDeque::new();
        let mut stats = QueryCacheStats::default();

        for i in 0..(CACHE_DIAGNOSTICS_MAX_SNAPSHOTS as u64 + 10) {
            stats.search_hits = i;
            push_cache_stats_snapshot(&mut snapshots, &stats, i, CACHE_DIAGNOSTICS_MAX_SNAPSHOTS);
        }

        assert_eq!(snapshots.len(), CACHE_DIAGNOSTICS_MAX_SNAPSHOTS);
        let first = snapshots.front().expect("first");
        let last = snapshots.back().expect("last");
        assert_eq!(last.epoch_secs, CACHE_DIAGNOSTICS_MAX_SNAPSHOTS as u64 + 9);
        assert_eq!(first.epoch_secs, 10);
    }

    #[test]
    fn resolve_base_folder_prefers_selected_folder_context() {
        let mut folder_notes = HashMap::<String, Vec<String>>::new();
        folder_notes.insert("notes/projects".to_string(), Vec::new());
        let folder_children = HashMap::<String, Vec<String>>::new();

        let folder = resolve_base_folder_for_new_items(
            Some("notes/projects"),
            Some("notes/other/a.md"),
            &folder_notes,
            &folder_children,
        );

        assert_eq!(folder, "notes/projects");
    }

    #[test]
    fn resolve_base_folder_falls_back_to_selected_note_folder() {
        let folder_notes = HashMap::<String, Vec<String>>::new();
        let folder_children = HashMap::<String, Vec<String>>::new();

        let folder = resolve_base_folder_for_new_items(
            Some("notes/missing"),
            Some("notes/active/a.md"),
            &folder_notes,
            &folder_children,
        );

        assert_eq!(folder, "notes/active");
    }

    #[test]
    fn resolve_base_folder_uses_root_when_no_context() {
        let folder_notes = HashMap::<String, Vec<String>>::new();
        let folder_children = HashMap::<String, Vec<String>>::new();

        let folder =
            resolve_base_folder_for_new_items(None, None, &folder_notes, &folder_children);

        assert!(folder.is_empty());
    }
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

fn percentile_u128(samples: &mut [u128], percentile: f64) -> u128 {
    if samples.is_empty() {
        return 0;
    }

    samples.sort_unstable();
    let pct = percentile.clamp(0.0, 100.0);
    let rank = ((pct / 100.0) * ((samples.len() - 1) as f64)).round() as usize;
    samples[rank]
}

fn line_number_label(
    line_number: usize,
    digits: usize,
    width: Pixels,
    color: u32,
    window: &mut Window,
    _cx: &mut App,
) -> Option<gpui::WrappedLine> {
    let text = SharedString::from(format!("{line_number:>digits$}"));
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    let runs = [TextRun {
        len: text.len(),
        font: text_style.font(),
        color: rgb(color).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    }];

    window
        .text_system()
        .shape_text(text, font_size, &runs, Some(width), Some(1))
        .ok()
        .and_then(|lines| lines.into_iter().next())
}

fn compute_wrapped_line_numbers(text: &str, lines: &[gpui::WrappedLine]) -> Vec<usize> {
    let mut out = Vec::with_capacity(lines.len());
    let mut logical_line = 1usize;
    let mut consumed = 0usize;

    for line in lines {
        out.push(logical_line);
        let len = line.len();
        consumed = consumed.saturating_add(len);
        if consumed < text.len() && text.as_bytes().get(consumed) == Some(&b'\n') {
            logical_line = logical_line.saturating_add(1);
            consumed = consumed.saturating_add(1);
        }
    }

    out
}

fn editor_text_x_offset(gutter_width: Pixels) -> Pixels {
    gutter_width + px(EDITOR_TEXT_LEFT_PADDING)
}

fn line_number_digits(max_line: usize) -> usize {
    max_line.max(1).to_string().len()
}

fn editor_gutter_width_for_digits(digits: usize) -> f32 {
    let digits = digits.max(1) as f32;
    EDITOR_GUTTER_BASE_WIDTH + digits * EDITOR_GUTTER_DIGIT_WIDTH
}

fn build_editor_highlight_spans(text: &str) -> Vec<EditorHighlightSpan> {
    if text.len() > MAX_EDITOR_HIGHLIGHT_BYTES {
        return Vec::new();
    }

    let mut spans = Vec::new();
    let mut in_code = false;
    let mut byte_cursor = 0usize;

    for line in text.lines() {
        let line_start = byte_cursor;
        let line_end = line_start + line.len();
        let trimmed = line.trim_start();
        let leading_ws = line.len().saturating_sub(trimmed.len());
        let content_start = line_start + leading_ws;

        if trimmed.starts_with("```") {
            spans.push(EditorHighlightSpan {
                range: content_start..line_end,
                kind: EditorHighlightKind::CodeFence,
            });
            in_code = !in_code;
        } else if in_code {
            spans.push(EditorHighlightSpan {
                range: line_start..line_end,
                kind: EditorHighlightKind::CodeText,
            });
        } else if let Some(level) = markdown_heading_level(trimmed) {
            let marker_len = usize::from(level);
            if content_start + marker_len <= line_end {
                spans.push(EditorHighlightSpan {
                    range: content_start..(content_start + marker_len),
                    kind: EditorHighlightKind::HeadingMarker,
                });
            }
            let text_start = (content_start + marker_len + 1).min(line_end);
            if text_start < line_end {
                spans.push(EditorHighlightSpan {
                    range: text_start..line_end,
                    kind: EditorHighlightKind::HeadingText,
                });
            }
        } else if trimmed.starts_with('>') {
            let marker_end = (content_start + 1).min(line_end);
            spans.push(EditorHighlightSpan {
                range: content_start..marker_end,
                kind: EditorHighlightKind::QuoteMarker,
            });
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            let marker_end = (content_start + 1).min(line_end);
            spans.push(EditorHighlightSpan {
                range: content_start..marker_end,
                kind: EditorHighlightKind::ListMarker,
            });
        }

        for (link_start, link_end, text_range, url_range) in markdown_link_ranges(line, line_start) {
            if link_start < link_end {
                spans.push(EditorHighlightSpan {
                    range: text_range,
                    kind: EditorHighlightKind::LinkText,
                });
                spans.push(EditorHighlightSpan {
                    range: url_range,
                    kind: EditorHighlightKind::LinkUrl,
                });
            }
        }

        byte_cursor = line_end.saturating_add(1);
    }

    spans.retain(|span| span.range.start < span.range.end);
    spans
}

fn markdown_heading_level(line: &str) -> Option<u8> {
    let mut count = 0u8;
    for c in line.chars() {
        if c == '#' {
            count = count.saturating_add(1);
            if count > 6 {
                return None;
            }
        } else {
            break;
        }
    }

    if count == 0 {
        return None;
    }

    let marker = "#".repeat(count as usize);
    if line.starts_with(&format!("{marker} ")) {
        Some(count)
    } else {
        None
    }
}

#[cfg(test)]
mod editor_highlight_tests {
    use super::*;

    #[test]
    fn highlight_spans_detect_heading_and_link() {
        let text = "# Title\nSee [x](https://example.com)\n";
        let spans = build_editor_highlight_spans(text);

        assert!(spans
            .iter()
            .any(|span| span.kind == EditorHighlightKind::HeadingMarker));
        assert!(spans
            .iter()
            .any(|span| span.kind == EditorHighlightKind::LinkText));
        assert!(spans
            .iter()
            .any(|span| span.kind == EditorHighlightKind::LinkUrl));
    }

    #[test]
    fn highlight_spans_disable_for_very_large_document() {
        let text = "a".repeat(MAX_EDITOR_HIGHLIGHT_BYTES + 16);
        let spans = build_editor_highlight_spans(&text);
        assert!(spans.is_empty());
    }
}

fn markdown_link_ranges(
    line: &str,
    line_start: usize,
) -> Vec<(usize, usize, Range<usize>, Range<usize>)> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        if bytes[i] == b'[' {
            let Some(close_bracket_rel) = line[i + 1..].find(']') else {
                i += 1;
                continue;
            };
            let close_bracket = i + 1 + close_bracket_rel;
            if close_bracket + 1 < bytes.len()
                && bytes[close_bracket + 1] == b'('
            {
                let Some(close_paren_rel) = line[close_bracket + 2..].find(')') else {
                    i += 1;
                    continue;
                };
                let close_paren = close_bracket + 2 + close_paren_rel;
                let full_start = line_start + i;
                let full_end = line_start + close_paren + 1;
                let text_start = line_start + i + 1;
                let text_end = line_start + close_bracket;
                let url_start = line_start + close_bracket + 2;
                let url_end = line_start + close_paren;
                out.push((full_start, full_end, text_start..text_end, url_start..url_end));
                i = close_paren + 1;
                continue;
            }
        }
        i += 1;
    }
    out
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
                |_, cx| cx.new(XnoteWindow::new),
            )
            .unwrap();
            cx.activate(true);
        });
}
