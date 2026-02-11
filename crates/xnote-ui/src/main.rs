mod i18n;

use gpui::{
    div, ease_in_out, fill, point, prelude::*, px, radians, relative, rgb, rgba, size, svg,
    uniform_list, Animation, AnimationExt as _, AnyView, App, Application, AssetSource,
    AvailableSpace, Bounds, ClickEvent, ClipboardItem, Context, CursorStyle, DragMoveEvent,
    Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, FontWeight,
    GlobalElementId, KeyDownEvent, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, Point, SharedString, Size, Style, Task, TextRun, Timer,
    Transformation, UTF16Selection, UnderlineStyle, Window, WindowBounds, WindowControlArea,
    WindowOptions,
};
use i18n::{I18n, Locale};
use serde_json::json;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::net::{TcpStream, ToSocketAddrs};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use xnote_core::ai::{
    build_provider_from_env, execute_rewrite_with_env_provider, execute_rewrite_with_vcp_tool_loop,
    execute_vcp_tool_orchestrator, AiPolicy, AiProvider, AiRewriteRequest, AiToolLoopStopReason,
    AiToolOrchestratorConfig, VcpCompatProvider, VcpToolPolicy, VcpToolRequest,
};
use xnote_core::command::{command_specs, CommandId};
use xnote_core::editor::{EditTransaction, EditorBuffer};
use xnote_core::keybind::KeyContext;
use xnote_core::keybind::Keymap;
use xnote_core::knowledge::{KnowledgeIndex, SearchOptions};
use xnote_core::markdown::{
    lint_markdown, parse_markdown, MarkdownDiagnostic, MarkdownDiagnosticSeverity,
    MarkdownInvalidationWindow, MarkdownParseResult,
};
use xnote_core::note_meta::{
    ensure_frontmatter_note_id, extract_note_id_from_frontmatter, generate_note_id,
    NoteMetaRelation, NoteMetaTarget, NoteMetaV1,
};
use xnote_core::paths::{join_inside, normalize_folder_rel_path, normalize_vault_rel_path};
use xnote_core::plugin::{
    PluginActivationEvent, PluginCapability, PluginLifecycleState, PluginManifest, PluginRegistry,
    PluginRuntimeMode,
};
use xnote_core::settings::{
    load_effective_settings, project_settings_path, save_project_settings, save_settings,
    settings_path, AppSettings, WindowLayoutSettings,
};
use xnote_core::vault::{NoteEntry, Vault, VaultScan};
use xnote_core::watch::{expand_note_move_pairs_with_prefix, VaultWatchChange, VaultWatcher};

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
const NOTE_CONTENT_CACHE_CAPACITY: usize = 96;
const RECENT_QUERY_HISTORY_CAPACITY: usize = 8;
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
const EDITOR_GUTTER_BASE_WIDTH: f32 = 3.0;
const EDITOR_GUTTER_DIGIT_WIDTH: f32 = 5.0;
const EDITOR_GUTTER_LINE_HEIGHT: f32 = 20.0;
const EDITOR_TEXT_LEFT_PADDING: f32 = 5.0;
const EDITOR_TEXT_MIN_WRAP_WIDTH: f32 = 40.0;
const EDITOR_SURFACE_LEFT_PADDING: f32 = 0.0;
const EDITOR_SURFACE_RIGHT_PADDING: f32 = 18.0;
const EDITOR_GUTTER_STABLE_DIGITS_MAX_9999: usize = 4;
const WINDOW_LAYOUT_PERSIST_DEBOUNCE: Duration = Duration::from_millis(320);
const WINDOW_DEFAULT_WIDTH_PX: u32 = 1200;
const WINDOW_DEFAULT_HEIGHT_PX: u32 = 760;
const WINDOW_MIN_WIDTH_PX: u32 = 820;
const WINDOW_MIN_HEIGHT_PX: u32 = 540;
const PANEL_SHELL_MIN_WIDTH: f32 = 150.0;
const WORKSPACE_MIN_WIDTH: f32 = 180.0;
const EXPLORER_HEADER_ACTION_SIZE: f32 = 20.0;
const EXPLORER_HEADER_ICON_SIZE: f32 = 14.0;
const EDITOR_SPLIT_MIN_RATIO: f32 = 0.25;
const EDITOR_SPLIT_MAX_RATIO: f32 = 0.75;
const EDITOR_GROUP_SPLITTER_WIDTH: f32 = 10.0;
const EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH: f32 = 80.0;
const EDITOR_GROUP_INITIAL_TOTAL_WIDTH: f32 = 960.0;
const INACTIVE_GROUP_PREVIEW_MAX_LINES: usize = 2000;
const INACTIVE_GROUP_PREVIEW_MAX_LINE_CHARS: usize = 4096;
const EDITOR_GROUP_MRU_CAPACITY: usize = 24;
const EDITOR_SPLIT_RATIO_SCALE: f32 = 1000.0;
const EDITOR_SPLIT_DIRECTION_DOWN: &str = "down";
const EDITOR_SPLIT_DIRECTION_RIGHT: &str = "right";
const AI_HUB_MAX_MESSAGES: usize = 96;
const DEFAULT_AI_PROVIDER: &str = "mock";
const DEFAULT_AI_VCP_URL: &str = "http://127.0.0.1:5890";
const DEFAULT_AI_VCP_MODEL: &str = "gemini-2.5-flash-preview-05-20";
const AI_ENDPOINT_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorViewMode {
    Edit,
    Preview,
    Split,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorSplitDirection {
    Right,
    Down,
}

impl EditorSplitDirection {
    fn to_tag(self) -> &'static str {
        match self {
            Self::Right => EDITOR_SPLIT_DIRECTION_RIGHT,
            Self::Down => EDITOR_SPLIT_DIRECTION_DOWN,
        }
    }

    fn from_tag(tag: &str) -> Self {
        match tag {
            EDITOR_SPLIT_DIRECTION_DOWN => Self::Down,
            _ => Self::Right,
        }
    }
}

impl EditorViewMode {
    fn to_tag(self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::Preview => "preview",
            Self::Split => "split",
        }
    }

    fn from_tag(tag: &str) -> Self {
        match tag {
            "preview" => Self::Preview,
            "split" => Self::Split,
            _ => Self::Edit,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct EditorTabViewState {
    mode: EditorViewMode,
    split_ratio: f32,
    split_direction: EditorSplitDirection,
    split_saved_mode: EditorViewMode,
}

impl EditorTabViewState {
    fn sanitize(mut self) -> Self {
        if self.mode == EditorViewMode::Split {
            self.mode = EditorViewMode::Edit;
        }
        self.split_ratio = self
            .split_ratio
            .clamp(EDITOR_SPLIT_MIN_RATIO, EDITOR_SPLIT_MAX_RATIO);
        if self.split_saved_mode == EditorViewMode::Split {
            self.split_saved_mode = EditorViewMode::Edit;
        }
        self
    }
}

fn default_editor_group_view_state() -> EditorTabViewState {
    EditorTabViewState {
        mode: EditorViewMode::Edit,
        split_ratio: 0.5,
        split_direction: EditorSplitDirection::Right,
        split_saved_mode: EditorViewMode::Edit,
    }
    .sanitize()
}

#[derive(Clone, Debug)]
struct EditorGroup {
    id: u64,
    note_path: Option<String>,
    tabs: Vec<String>,
    pinned_tabs: HashSet<String>,
    note_mru: VecDeque<String>,
    view_state: EditorTabViewState,
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
    LinkPicker,
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
struct DraggedEditorTab {
    path: String,
}

#[derive(Clone, Debug)]
struct DragOver {
    folder: String,
    target_path: String,
    insert_after: bool,
}

#[derive(Clone, Debug)]
struct TabDragOver {
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
#[allow(dead_code)]
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
enum WorkstationModule {
    Knowledge,
    Resources,
    Inbox,
    AiHub,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AiHubMessageRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, Debug)]
struct AiHubMessage {
    role: AiHubMessageRole,
    content: SharedString,
    timestamp_label: SharedString,
}

#[derive(Clone, Debug, Default)]
struct AiChatRunResult {
    provider: String,
    model: String,
    response: String,
    tool_calls: Vec<VcpToolRequest>,
    rounds_executed: usize,
    stop_reason: Option<AiToolLoopStopReason>,
}

#[derive(Clone, Copy, Debug)]
struct AiHubAgentItem {
    name_key: &'static str,
    meta_key: &'static str,
    instruction_key: &'static str,
}

impl WorkstationModule {
    const fn label(self) -> &'static str {
        match self {
            Self::Knowledge => "Knowledge",
            Self::Resources => "Resources",
            Self::Inbox => "Inbox",
            Self::AiHub => "AI Hub",
        }
    }

    const fn detail(self) -> &'static str {
        match self {
            Self::Knowledge => "Core notes workspace",
            Self::Resources => "Coming soon",
            Self::Inbox => "Coming soon",
            Self::AiHub => "AI chat + tool traces",
        }
    }

    const fn shortcut_hint(self) -> &'static str {
        match self {
            Self::Knowledge => "Alt+K",
            Self::Resources => "Alt+R",
            Self::Inbox => "Alt+I",
            Self::AiHub => "Alt+A",
        }
    }

    const fn disabled_tooltip(self) -> Option<&'static str> {
        match self {
            Self::Knowledge => None,
            Self::Resources => Some("Resources module is coming in next milestones"),
            Self::Inbox => Some("Inbox module is coming in next milestones"),
            Self::AiHub => None,
        }
    }

    const fn icon(self) -> &'static str {
        match self {
            Self::Knowledge => ICON_FILE_TEXT,
            Self::Resources => ICON_FOLDER,
            Self::Inbox => ICON_SEARCH,
            Self::AiHub => ICON_FILE_COG,
        }
    }

    const fn is_available(self) -> bool {
        matches!(self, Self::Knowledge | Self::AiHub)
    }

    fn from_shortcut_key(key: &str) -> Option<Self> {
        match key {
            "k" => Some(Self::Knowledge),
            "r" => Some(Self::Resources),
            "i" => Some(Self::Inbox),
            "a" => Some(Self::AiHub),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitterKind {
    PanelShell,
    Workspace,
    EditorGroup,
}

#[derive(Clone, Copy, Debug)]
struct SplitterDrag {
    kind: SplitterKind,
    start_x: Pixels,
    start_width: Pixels,
    group_split_index: Option<usize>,
    group_pair_total: Option<f32>,
    group_target_total: Option<f32>,
    pointer_initialized: bool,
}

#[derive(Clone, Debug)]
struct SplitLayoutState {
    widths: Vec<f32>,
    target_total_width: f32,
}

#[derive(Clone, Copy, Debug)]
struct SplitLayoutEngine {
    min_width: f32,
    default_total_width: f32,
}

impl SplitLayoutEngine {
    const fn new(min_width: f32, default_total_width: f32) -> Self {
        Self {
            min_width,
            default_total_width,
        }
    }

    fn sanitize_total_width(&self, group_len: usize, target_total_width: f32) -> f32 {
        let group_len = group_len.max(1);
        let min_total = self.min_width * group_len as f32;
        target_total_width
            .max(self.default_total_width)
            .max(min_total)
    }

    fn normalize(
        &self,
        widths: &[f32],
        group_len: usize,
        target_total_width: f32,
    ) -> SplitLayoutState {
        let group_len = group_len.max(1);
        let target_total_width = self.sanitize_total_width(group_len, target_total_width);
        let min_total = self.min_width * group_len as f32;

        let mut raw = Vec::with_capacity(group_len);
        for ix in 0..group_len {
            let value = widths.get(ix).copied().unwrap_or(1.0);
            let normalized = if value.is_finite() && value > 0.0 {
                value
            } else {
                1.0
            };
            raw.push(normalized);
        }

        let raw_sum = raw.iter().sum::<f32>();
        let ratios = if raw_sum > f32::EPSILON {
            raw.into_iter()
                .map(|value| value / raw_sum)
                .collect::<Vec<_>>()
        } else {
            vec![1.0 / group_len as f32; group_len]
        };

        let remaining = (target_total_width - min_total).max(0.0);
        let widths = ratios
            .into_iter()
            .map(|ratio| self.min_width + remaining * ratio)
            .collect();

        SplitLayoutState {
            widths,
            target_total_width,
        }
    }

    fn split_at(
        &self,
        widths: &[f32],
        group_len: usize,
        target_total_width: f32,
        source_ix: usize,
    ) -> SplitLayoutState {
        let mut state = self.normalize(widths, group_len, target_total_width);
        if source_ix >= state.widths.len() {
            return state;
        }

        let source_width = state.widths[source_ix].max(self.min_width * 2.0);
        let left = (source_width * 0.5).max(self.min_width);
        let right = (source_width - left).max(self.min_width);

        state.widths[source_ix] = left;
        state.widths.insert(source_ix + 1, right);
        let next_total = state.widths.iter().sum::<f32>();
        self.normalize(&state.widths, state.widths.len(), next_total)
    }

    fn close_at(
        &self,
        widths: &[f32],
        group_len: usize,
        target_total_width: f32,
        remove_ix: usize,
    ) -> SplitLayoutState {
        let mut state = self.normalize(widths, group_len, target_total_width);
        if state.widths.len() <= 1 || remove_ix >= state.widths.len() {
            return state;
        }

        let removed = state.widths.remove(remove_ix);
        if let Some(prev_ix) = remove_ix.checked_sub(1) {
            if let Some(prev) = state.widths.get_mut(prev_ix) {
                *prev += removed;
            }
        } else if let Some(first) = state.widths.first_mut() {
            *first += removed;
        }

        let next_total = state.widths.iter().sum::<f32>();
        self.normalize(&state.widths, state.widths.len(), next_total)
    }

    fn drag_pair(
        &self,
        widths: &[f32],
        group_len: usize,
        target_total_width: f32,
        split_index: usize,
        left_start: f32,
        pair_total: f32,
        delta: f32,
    ) -> SplitLayoutState {
        let mut state = self.normalize(widths, group_len, target_total_width);
        if split_index + 1 >= state.widths.len() {
            return state;
        }

        let pair_total = pair_total.max(self.min_width * 2.0);
        let max_left = (pair_total - self.min_width).max(self.min_width);
        let left_start = left_start.clamp(self.min_width, max_left);
        let left = (left_start + delta).clamp(self.min_width, max_left);
        let right = (pair_total - left).max(self.min_width);

        state.widths[split_index] = left;
        state.widths[split_index + 1] = right;
        state.target_total_width = state.widths.iter().sum();
        state
    }
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
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VaultPromptTarget {
    CurrentWindow,
    NewWindow,
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
        path_highlights: Vec<Range<usize>>,
    },
    Match {
        path: String,
        line: usize,
        preview: String,
        preview_highlights: Vec<Range<usize>>,
    },
}

#[derive(Clone, Debug)]
struct OpenPathMatch {
    path: String,
    title: String,
    path_highlights: Vec<Range<usize>>,
    title_highlights: Vec<Range<usize>>,
}

#[derive(Clone, Debug)]
struct NoteLinkHit {
    raw: String,
    target_path: String,
    display: String,
    range: Range<usize>,
}

#[derive(Clone, Debug)]
struct ResolvedOpenPathMatch {
    path: String,
    title: String,
    title_lower: String,
    stem_lower: String,
}

#[derive(Clone, Debug)]
struct SearchMatchEntry {
    line: usize,
    preview: String,
    preview_highlights: Vec<Range<usize>>,
}

#[derive(Clone, Debug)]
struct SearchResultGroup {
    path: String,
    match_count: usize,
    path_highlights: Vec<Range<usize>>,
    matches: Vec<SearchMatchEntry>,
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
        id: CommandId::OpenVaultInNewWindow,
        icon: ICON_VAULT,
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
    PaletteCommandSpec {
        id: CommandId::AiRewriteSelection,
        icon: ICON_BRUSH,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsSection {
    About,
    Appearance,
    Editor,
    Ai,
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
    editor_groups: Vec<EditorGroup>,
    active_editor_group_id: u64,
    next_editor_group_id: u64,
    editor_group_mru: VecDeque<u64>,
    editor_group_note_history: HashMap<u64, VecDeque<String>>,
    pinned_editors: HashSet<String>,
    tab_drag_over: Option<TabDragOver>,
    open_note_loading: bool,
    open_note_dirty: bool,
    open_note_content: String,
    editor_buffer: Option<EditorBuffer>,
    editor_focus_handle: FocusHandle,
    vault_prompt_focus_handle: FocusHandle,
    ai_hub_input_focus_handle: FocusHandle,
    editor_selected_range: Range<usize>,
    editor_selection_reversed: bool,
    editor_marked_range: Option<Range<usize>>,
    editor_is_selecting: bool,
    editor_preferred_x: Option<Pixels>,
    editor_layout: Option<NoteEditorLayout>,
    next_note_open_nonce: u64,
    current_note_open_nonce: u64,
    next_ai_rewrite_nonce: u64,
    pending_ai_rewrite_nonce: u64,
    next_ai_endpoint_check_nonce: u64,
    pending_ai_endpoint_check_nonce: u64,
    ai_chat_input: String,
    ai_hub_cursor_offset: usize,
    ai_hub_cursor_preferred_col: Option<usize>,
    ai_hub_messages: Vec<AiHubMessage>,
    ai_hub_session_title: SharedString,
    ai_hub_selected_agent_idx: usize,
    ai_hub_input_needs_focus: bool,
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
    active_module: WorkstationModule,
    module_switcher_open: bool,
    module_switcher_backdrop_armed_until: Option<Instant>,
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
    editor_tab_view_state: HashMap<String, EditorTabViewState>,
    palette_open: bool,
    palette_mode: PaletteMode,
    palette_query: String,
    palette_selected: usize,
    palette_results: Vec<OpenPathMatch>,
    link_picker_open: bool,
    link_picker_query: String,
    link_picker_selected: usize,
    link_picker_results: Vec<OpenPathMatch>,
    link_picker_anchor_range: Option<Range<usize>>,
    palette_search_groups: Vec<SearchResultGroup>,
    palette_search_results: Vec<SearchRow>,
    palette_search_collapsed_paths: HashSet<String>,
    recent_palette_quick_open_queries: VecDeque<String>,
    recent_palette_search_queries: VecDeque<String>,
    next_palette_nonce: u64,
    pending_palette_nonce: u64,
    palette_backdrop_armed_until: Option<Instant>,
    vault_prompt_open: bool,
    vault_prompt_target: VaultPromptTarget,
    vault_prompt_needs_focus: bool,
    vault_prompt_value: String,
    vault_prompt_error: Option<SharedString>,
    vault_prompt_backdrop_armed_until: Option<Instant>,
    search_query: String,
    search_selected: usize,
    search_groups: Vec<SearchResultGroup>,
    search_results: Vec<SearchRow>,
    search_collapsed_paths: HashSet<String>,
    recent_panel_search_queries: VecDeque<String>,
    recent_explorer_filter_queries: VecDeque<String>,
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
    editor_split_ratio: f32,
    editor_split_direction: EditorSplitDirection,
    editor_split_saved_mode: EditorViewMode,
    editor_group_width_weights: Vec<f32>,
    editor_group_target_total_width: f32,
    editor_group_drag: Option<SplitterDrag>,
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
    note_content_cache: HashMap<String, String>,
    note_content_cache_order: VecDeque<String>,
    pending_group_preview_loads: HashSet<String>,
    pending_external_note_reload: Option<String>,
    search_query_cache: HashMap<String, Vec<SearchResultGroup>>,
    search_query_cache_order: VecDeque<String>,
    quick_open_query_cache: HashMap<String, Vec<OpenPathMatch>>,
    quick_open_query_cache_order: VecDeque<String>,
    cache_stats: QueryCacheStats,
    cache_stats_last_flushed: QueryCacheStats,
    cache_stats_snapshots: VecDeque<CacheStatsSnapshot>,
    editor_autosave_delay_input: String,
    hotkey_editing_command: Option<CommandId>,
    hotkey_editing_value: String,
    next_window_layout_persist_nonce: u64,
    pending_window_layout_persist_nonce: u64,
    open_note_id: Option<String>,
    open_note_meta: Option<NoteMetaV1>,
    open_note_meta_loading: bool,
    next_note_meta_load_nonce: u64,
    pending_note_meta_load_nonce: u64,
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
        let i18n = I18n::new(boot.locale);
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
                PluginActivationEvent::OnCommand(CommandId::OpenVaultInNewWindow),
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
            editor_groups: vec![EditorGroup {
                id: 1,
                note_path: None,
                tabs: Vec::new(),
                pinned_tabs: HashSet::new(),
                note_mru: VecDeque::new(),
                view_state: default_editor_group_view_state(),
            }],
            active_editor_group_id: 1,
            next_editor_group_id: 2,
            editor_group_mru: VecDeque::from([1]),
            editor_group_note_history: HashMap::new(),
            pinned_editors: HashSet::new(),
            tab_drag_over: None,
            open_note_loading: false,
            open_note_dirty: false,
            open_note_content: String::new(),
            editor_buffer: None,
            editor_focus_handle: cx.focus_handle(),
            vault_prompt_focus_handle: cx.focus_handle(),
            ai_hub_input_focus_handle: cx.focus_handle(),
            editor_selected_range: 0..0,
            editor_selection_reversed: false,
            editor_marked_range: None,
            editor_is_selecting: false,
            editor_preferred_x: None,
            editor_layout: None,
            next_note_open_nonce: 0,
            current_note_open_nonce: 0,
            next_ai_rewrite_nonce: 0,
            pending_ai_rewrite_nonce: 0,
            next_ai_endpoint_check_nonce: 0,
            pending_ai_endpoint_check_nonce: 0,
            ai_chat_input: String::new(),
            ai_hub_cursor_offset: 0,
            ai_hub_cursor_preferred_col: None,
            ai_hub_messages: vec![AiHubMessage {
                role: AiHubMessageRole::System,
                content: SharedString::from(i18n.text("ai.hub.system.ready")),
                timestamp_label: SharedString::from(i18n.text("ai.hub.timestamp.now")),
            }],
            ai_hub_session_title: SharedString::from(i18n.text("ai.hub.session.title")),
            ai_hub_selected_agent_idx: 0,
            ai_hub_input_needs_focus: false,
            next_note_save_nonce: 0,
            pending_note_save_nonce: 0,
            status: SharedString::from(i18n.text("status.ready")),
            app_settings: boot.app_settings,
            settings_path: boot.settings_path,
            project_settings_path: boot.project_settings_path,
            i18n,
            keymap: boot.keymap,
            plugin_runtime_mode: boot.plugin_runtime_mode,
            plugin_registry,
            plugin_activation_state: PluginActivationState::Idle,
            active_module: WorkstationModule::Knowledge,
            module_switcher_open: false,
            module_switcher_backdrop_armed_until: None,
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
            editor_tab_view_state: HashMap::new(),
            palette_open: false,
            palette_mode: PaletteMode::Commands,
            palette_query: String::new(),
            palette_selected: 0,
            palette_results: Vec::new(),
            link_picker_open: false,
            link_picker_query: String::new(),
            link_picker_selected: 0,
            link_picker_results: Vec::new(),
            link_picker_anchor_range: None,
            palette_search_groups: Vec::new(),
            palette_search_results: Vec::new(),
            palette_search_collapsed_paths: HashSet::new(),
            recent_palette_quick_open_queries: VecDeque::new(),
            recent_palette_search_queries: VecDeque::new(),
            next_palette_nonce: 0,
            pending_palette_nonce: 0,
            palette_backdrop_armed_until: None,
            vault_prompt_open: false,
            vault_prompt_target: VaultPromptTarget::CurrentWindow,
            vault_prompt_needs_focus: false,
            vault_prompt_value: String::new(),
            vault_prompt_error: None,
            vault_prompt_backdrop_armed_until: None,
            search_query: String::new(),
            search_selected: 0,
            search_groups: Vec::new(),
            search_results: Vec::new(),
            search_collapsed_paths: HashSet::new(),
            recent_panel_search_queries: VecDeque::new(),
            recent_explorer_filter_queries: VecDeque::new(),
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
            editor_split_ratio: 0.5,
            editor_split_direction: EditorSplitDirection::Right,
            editor_split_saved_mode: EditorViewMode::Edit,
            editor_group_width_weights: vec![EDITOR_GROUP_INITIAL_TOTAL_WIDTH],
            editor_group_target_total_width: EDITOR_GROUP_INITIAL_TOTAL_WIDTH,
            editor_group_drag: None,
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
            note_content_cache: HashMap::new(),
            note_content_cache_order: VecDeque::new(),
            pending_group_preview_loads: HashSet::new(),
            pending_external_note_reload: None,
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
            next_window_layout_persist_nonce: 0,
            pending_window_layout_persist_nonce: 0,
            open_note_id: None,
            open_note_meta: None,
            open_note_meta_loading: false,
            next_note_meta_load_nonce: 0,
            pending_note_meta_load_nonce: 0,
        };

        this.apply_persisted_split_layout();

        this.status = SharedString::from(this.i18n.text("status.ready"));
        this.sync_ai_settings_env();

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
        self.editor_groups.clear();
        self.editor_groups.push(EditorGroup {
            id: 1,
            note_path: None,
            tabs: Vec::new(),
            pinned_tabs: HashSet::new(),
            note_mru: VecDeque::new(),
            view_state: default_editor_group_view_state(),
        });
        self.editor_group_width_weights.clear();
        self.editor_group_width_weights
            .push(EDITOR_GROUP_INITIAL_TOTAL_WIDTH);
        self.editor_group_target_total_width = EDITOR_GROUP_INITIAL_TOTAL_WIDTH;
        self.editor_group_drag = None;
        self.active_editor_group_id = 1;
        self.next_editor_group_id = 2;
        self.editor_group_mru.clear();
        self.editor_group_mru.push_back(1);
        self.editor_group_note_history.clear();
        self.pinned_editors.clear();
        self.tab_drag_over = None;
        self.editor_tab_view_state.clear();
        self.open_note_loading = false;
        self.open_note_dirty = false;
        self.open_note_content.clear();
        self.open_note_id = None;
        self.open_note_meta = None;
        self.open_note_meta_loading = false;
        self.pending_note_meta_load_nonce = 0;
        self.editor_buffer = None;
        self.editor_selected_range = 0..0;
        self.editor_selection_reversed = false;
        self.editor_marked_range = None;
        self.editor_is_selecting = false;
        self.editor_preferred_x = None;
        self.editor_layout = None;
        self.pending_note_save_nonce = 0;
        self.pending_ai_rewrite_nonce = 0;
        self.panel_mode = PanelMode::Explorer;
        self.workspace_mode = WorkspaceMode::OpenEditors;
        self.palette_open = false;
        self.palette_mode = PaletteMode::Commands;
        self.palette_query.clear();
        self.palette_selected = 0;
        self.palette_results.clear();
        self.link_picker_open = false;
        self.link_picker_query.clear();
        self.link_picker_selected = 0;
        self.link_picker_results.clear();
        self.link_picker_anchor_range = None;
        self.palette_search_groups.clear();
        self.palette_search_collapsed_paths.clear();
        self.refresh_palette_search_rows_from_groups();
        self.pending_palette_nonce = 0;
        self.palette_backdrop_armed_until = None;
        self.search_query.clear();
        self.search_selected = 0;
        self.search_groups.clear();
        self.search_collapsed_paths.clear();
        self.refresh_search_rows_from_groups();
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
        self.note_content_cache.clear();
        self.note_content_cache_order.clear();
        self.pending_group_preview_loads.clear();
        self.pending_external_note_reload = None;
        self.settings_section = SettingsSection::Appearance;
        self.settings_open = false;
        self.settings_language_menu_open = false;
        self.settings_backdrop_armed_until = None;
        self.editor_view_mode = EditorViewMode::Edit;
        self.editor_split_saved_mode = EditorViewMode::Edit;
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
                        vault.ensure_knowledge_structure()?;
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
                        this.rebuild_explorer_rows(cx);
                        this.apply_restored_group_layout_to_open_editors(cx);
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
                                && matches!(
                                    this.palette_mode,
                                    PaletteMode::QuickOpen | PaletteMode::Search
                                )
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

        let cursor = previous_char_boundary(
            &self.open_note_content,
            self.editor_cursor_offset()
                .min(self.open_note_content.len()),
        );
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

        self.editor_group_drag = None;

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
                                Ok::<_, anyhow::Error>((index, scan.notes, note_count, duration_ms))
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
                            this.rebuild_explorer_rows(cx);
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

                                    let note_id = generate_note_id();
                                    let initial =
                                        format!("---\nid: {note_id}\n---\n# New Note\n\n");
                                    vault.write_note(&rel, &initial)?;
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
        self.pinned_editors.remove(path);
        if let Some(active_group_id) = self.active_group().map(|group| group.id) {
            if let Some(history) = self.editor_group_note_history.get_mut(&active_group_id) {
                history.retain(|existing| existing != path);
            }
        }
        self.sync_active_group_tabs_from_open_editors();
        let still_open_in_any_group = self
            .editor_groups
            .iter()
            .any(|group| group.tabs.iter().any(|tab| tab == path));
        if !still_open_in_any_group {
            self.editor_tab_view_state.remove(path);
            for history in self.editor_group_note_history.values_mut() {
                history.retain(|existing| existing != path);
            }
        }
        if self.selected_note.as_deref() == Some(path) && !still_open_in_any_group {
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
            self.sync_active_group_note_path();
        } else {
            cx.notify();
        }
    }

    fn reorder_open_editors_with_pins(&mut self) {
        let mut pinned = Vec::new();
        let mut normal = Vec::new();
        let active_group_tabs = self.open_editors.clone();

        for path in &active_group_tabs {
            if self.pinned_editors.contains(path) {
                pinned.push(path.clone());
            } else {
                normal.push(path.clone());
            }
        }
        pinned.extend(normal);
        self.open_editors = pinned;
    }

    fn sanitize_group_interaction_state(group: &mut EditorGroup) {
        let tab_set = group.tabs.iter().cloned().collect::<HashSet<_>>();
        group.pinned_tabs.retain(|path| tab_set.contains(path));
        group.note_mru =
            Self::filtered_group_note_mru(std::mem::take(&mut group.note_mru), &group.tabs);
    }

    fn active_group_mut(&mut self) -> Option<&mut EditorGroup> {
        self.editor_groups
            .iter_mut()
            .find(|group| group.id == self.active_editor_group_id)
    }

    fn active_group(&self) -> Option<&EditorGroup> {
        self.editor_groups
            .iter()
            .find(|group| group.id == self.active_editor_group_id)
    }

    fn sync_active_group_tabs_from_open_editors(&mut self) {
        let tabs = self.open_editors.clone();
        let active_group_id = self.active_editor_group_id;
        let pinned_tabs = self
            .pinned_editors
            .iter()
            .filter(|path| tabs.iter().any(|tab| tab == *path))
            .cloned()
            .collect::<HashSet<_>>();
        let note_mru = Self::filtered_group_note_mru(
            self.editor_group_note_history
                .get(&active_group_id)
                .cloned()
                .or_else(|| self.active_group().map(|group| group.note_mru.clone()))
                .unwrap_or_default(),
            &tabs,
        );
        self.editor_group_note_history
            .insert(active_group_id, note_mru.clone());
        if let Some(group) = self.active_group_mut() {
            group.tabs = tabs;
            group.pinned_tabs = pinned_tabs;
            group.note_mru = note_mru;
            Self::sanitize_group_interaction_state(group);
        }
    }

    fn filtered_group_note_mru(raw: VecDeque<String>, tabs: &[String]) -> VecDeque<String> {
        let tab_set = tabs.iter().cloned().collect::<HashSet<_>>();
        let mut out = VecDeque::new();
        for path in raw {
            if tab_set.contains(&path) && !out.iter().any(|existing| existing == &path) {
                out.push_back(path);
            }
        }
        while out.len() > EDITOR_GROUP_MRU_CAPACITY {
            out.pop_front();
        }
        out
    }

    fn apply_active_group_interaction_state(&mut self) {
        if let Some((group_id, tabs, pinned_tabs, note_mru)) = self.active_group().map(|group| {
            (
                group.id,
                group.tabs.clone(),
                group.pinned_tabs.clone(),
                group.note_mru.clone(),
            )
        }) {
            self.pinned_editors = pinned_tabs
                .into_iter()
                .filter(|path| tabs.iter().any(|tab| tab == path))
                .collect();
            self.editor_group_note_history
                .insert(group_id, Self::filtered_group_note_mru(note_mru, &tabs));
        }
    }

    fn restore_active_group_runtime_state(&mut self) {
        let Some((tabs, saved)) = self
            .active_group()
            .map(|group| (group.tabs.clone(), group.view_state.sanitize()))
        else {
            self.open_editors.clear();
            self.pinned_editors.clear();
            self.editor_group_note_history
                .insert(self.active_editor_group_id, VecDeque::new());
            return;
        };

        self.open_editors = tabs;
        self.editor_view_mode = saved.mode;
        self.editor_split_ratio = saved.split_ratio;
        self.editor_split_direction = saved.split_direction;
        self.editor_split_saved_mode = saved.split_saved_mode;
        self.apply_active_group_interaction_state();
        self.reorder_open_editors_with_pins();
        self.sync_active_group_tabs_from_open_editors();
    }

    fn normalize_editor_group_weights(&mut self) {
        let group_len = self.editor_groups.len().max(1);
        let engine = SplitLayoutEngine::new(
            EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH,
            EDITOR_GROUP_INITIAL_TOTAL_WIDTH,
        );
        let state = engine.normalize(
            &self.editor_group_width_weights,
            group_len,
            self.editor_group_target_total_width,
        );
        self.editor_group_width_weights = state.widths;
        self.editor_group_target_total_width = state.target_total_width;
    }

    fn normalized_editor_group_weights_snapshot_for_total(
        &self,
        group_len: usize,
        target_total_width: f32,
    ) -> Vec<f32> {
        let engine = SplitLayoutEngine::new(
            EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH,
            EDITOR_GROUP_INITIAL_TOTAL_WIDTH,
        );
        engine
            .normalize(
                &self.editor_group_width_weights,
                group_len,
                target_total_width,
            )
            .widths
    }

    fn begin_editor_group_drag(
        &mut self,
        split_index: usize,
        target_total_width: f32,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        let engine = SplitLayoutEngine::new(
            EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH,
            EDITOR_GROUP_INITIAL_TOTAL_WIDTH,
        );
        let normalized = engine.normalize(
            &self.editor_group_width_weights,
            self.editor_groups.len().max(1),
            target_total_width,
        );
        if split_index + 1 >= normalized.widths.len() {
            return;
        }
        let start_width = normalized.widths[split_index];
        let pair_total = normalized.widths[split_index] + normalized.widths[split_index + 1];
        self.editor_group_width_weights = normalized.widths;
        self.editor_group_target_total_width = normalized.target_total_width;
        self.editor_group_drag = Some(SplitterDrag {
            kind: SplitterKind::EditorGroup,
            start_x: event.position.x,
            start_width: px(start_width),
            group_split_index: Some(split_index),
            group_pair_total: Some(pair_total),
            group_target_total: Some(self.editor_group_target_total_width),
            pointer_initialized: false,
        });
        cx.notify();
    }

    fn end_editor_group_drag(&mut self, cx: &mut Context<Self>) {
        if self.editor_group_drag.take().is_some() {
            cx.notify();
        }
    }

    fn on_editor_group_drag_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut drag) = self.editor_group_drag else {
            return;
        };
        let Some(split_index) = drag.group_split_index else {
            return;
        };
        let Some(total) = drag.group_pair_total else {
            return;
        };

        if self.editor_groups.len() < 2 || split_index + 1 >= self.editor_groups.len() {
            return;
        }

        if !drag.pointer_initialized {
            drag.start_x = event.position.x;
            drag.pointer_initialized = true;
            self.editor_group_drag = Some(drag);
            return;
        }

        let delta = f32::from(event.position.x - drag.start_x);
        let engine = SplitLayoutEngine::new(
            EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH,
            EDITOR_GROUP_INITIAL_TOTAL_WIDTH,
        );
        let next = engine.drag_pair(
            &self.editor_group_width_weights,
            self.editor_groups.len(),
            drag.group_target_total
                .unwrap_or(self.editor_group_target_total_width),
            split_index,
            f32::from(drag.start_width),
            total,
            delta,
        );
        self.editor_group_width_weights = next.widths;
        self.editor_group_target_total_width = next.target_total_width;
        self.editor_group_drag = Some(drag);

        cx.notify();
    }

    fn on_editor_group_drag_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.end_editor_group_drag(cx);
    }

    fn toggle_pin_editor(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.pinned_editors.contains(path) {
            self.pinned_editors.remove(path);
            self.status = SharedString::from("Tab unpinned");
        } else {
            self.pinned_editors.insert(path.to_string());
            self.status = SharedString::from("Tab pinned");
        }
        self.reorder_open_editors_with_pins();
        self.sync_active_group_tabs_from_open_editors();
        cx.notify();
    }

    fn set_tab_drag_over(
        &mut self,
        target_path: String,
        insert_after: bool,
        cx: &mut Context<Self>,
    ) {
        let should_update = self.tab_drag_over.as_ref().is_none_or(|current| {
            current.target_path != target_path || current.insert_after != insert_after
        });
        if should_update {
            self.tab_drag_over = Some(TabDragOver {
                target_path,
                insert_after,
            });
            cx.notify();
        }
    }

    fn clear_tab_drag_over(&mut self, cx: &mut Context<Self>) {
        if self.tab_drag_over.is_some() {
            self.tab_drag_over = None;
            cx.notify();
        }
    }

    fn reorder_editor_tab(&mut self, dragged_path: &str, target_path: &str, insert_after: bool) {
        let Some(from_ix) = self
            .open_editors
            .iter()
            .position(|path| path == dragged_path)
        else {
            return;
        };
        let Some(target_ix) = self
            .open_editors
            .iter()
            .position(|path| path == target_path)
        else {
            return;
        };
        if dragged_path == target_path {
            return;
        }

        let dragged = self.open_editors.remove(from_ix);
        let mut insert_ix = target_ix;
        if from_ix < target_ix {
            insert_ix = insert_ix.saturating_sub(1);
        }
        if insert_after {
            insert_ix = (insert_ix + 1).min(self.open_editors.len());
        }
        self.open_editors.insert(insert_ix, dragged);
        self.reorder_open_editors_with_pins();
        self.sync_active_group_tabs_from_open_editors();
    }

    fn handle_tab_drop(
        &mut self,
        dragged: &DraggedEditorTab,
        target_path: &str,
        target_group_id: u64,
        cx: &mut Context<Self>,
    ) {
        let drop_meta = self.tab_drag_over.clone();
        self.clear_tab_drag_over(cx);

        if let Some(meta) = &drop_meta {
            if meta.target_path != target_path {
                return;
            }
        }

        let insert_after = drop_meta
            .as_ref()
            .map(|meta| meta.insert_after)
            .unwrap_or(false);

        let current_group = self.group_id_for_note_path(&dragged.path);
        if current_group == Some(target_group_id) {
            self.reorder_editor_tab(&dragged.path, target_path, insert_after);
            self.status = SharedString::from("Tab reordered");
            cx.notify();
            return;
        }

        self.move_note_to_group(dragged.path.clone(), target_group_id, cx);
    }

    fn move_note_to_group(
        &mut self,
        note_path: String,
        target_group_id: u64,
        cx: &mut Context<Self>,
    ) {
        if self
            .editor_groups
            .iter()
            .all(|group| group.id != target_group_id)
        {
            return;
        }

        for group in &mut self.editor_groups {
            if group.id != target_group_id {
                group.tabs.retain(|path| path != &note_path);
                group.pinned_tabs.remove(&note_path);
                group.note_mru.retain(|path| path != &note_path);
            }
            if group.id != target_group_id && group.note_path.as_deref() == Some(note_path.as_str())
            {
                group.note_path = None;
            }
        }

        if let Some(group) = self
            .editor_groups
            .iter_mut()
            .find(|group| group.id == target_group_id)
        {
            if !group.tabs.iter().any(|path| path == &note_path) {
                group.tabs.push(note_path.clone());
            }
            group.note_path = Some(note_path.clone());
            if self.pinned_editors.contains(&note_path) {
                group.pinned_tabs.insert(note_path.clone());
            }
            group.note_mru.retain(|path| path != &note_path);
            group.note_mru.push_back(note_path.clone());
            while group.note_mru.len() > EDITOR_GROUP_MRU_CAPACITY {
                group.note_mru.pop_front();
            }
            Self::sanitize_group_interaction_state(group);
        }

        self.active_editor_group_id = target_group_id;
        self.touch_group_mru(target_group_id);
        self.restore_active_group_runtime_state();
        self.open_note(note_path, cx);
    }

    fn group_id_for_note_path(&self, note_path: &str) -> Option<u64> {
        self.editor_groups
            .iter()
            .find(|group| group.tabs.iter().any(|path| path == note_path))
            .map(|group| group.id)
    }

    fn move_current_editor_to_next_group(&mut self, cx: &mut Context<Self>) {
        let Some(note_path) = self.open_note_path.clone() else {
            return;
        };

        self.ensure_active_group_exists();
        let Some(active_ix) = self.active_group_index() else {
            return;
        };

        let target_group_id = if active_ix + 1 < self.editor_groups.len() {
            self.editor_groups[active_ix + 1].id
        } else {
            let new_id = self.next_editor_group_id.max(1);
            self.next_editor_group_id = new_id.saturating_add(1);
            self.editor_groups.insert(
                active_ix + 1,
                EditorGroup {
                    id: new_id,
                    note_path: None,
                    tabs: Vec::new(),
                    pinned_tabs: HashSet::new(),
                    note_mru: VecDeque::new(),
                    view_state: default_editor_group_view_state(),
                },
            );
            let engine = SplitLayoutEngine::new(
                EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH,
                EDITOR_GROUP_INITIAL_TOTAL_WIDTH,
            );
            let next = engine.split_at(
                &self.editor_group_width_weights,
                self.editor_groups.len().saturating_sub(1).max(1),
                self.editor_group_target_total_width,
                active_ix,
            );
            self.editor_group_width_weights = next.widths;
            self.editor_group_target_total_width = next.target_total_width;
            new_id
        };

        self.move_note_to_group(note_path, target_group_id, cx);
        self.status = SharedString::from("Moved editor to next group");
    }

    fn open_palette(&mut self, mode: PaletteMode, cx: &mut Context<Self>) {
        self.palette_open = true;
        self.palette_mode = mode;
        self.palette_query.clear();
        self.palette_selected = 0;
        self.palette_results.clear();
        self.palette_search_groups.clear();
        self.palette_search_collapsed_paths.clear();
        self.refresh_palette_search_rows_from_groups();
        self.pending_palette_nonce = 0;
        self.palette_backdrop_armed_until = Some(Instant::now() + OVERLAY_BACKDROP_ARM_DELAY);
        self.module_switcher_open = false;
        self.module_switcher_backdrop_armed_until = None;
        cx.notify();
    }

    fn close_palette(&mut self, cx: &mut Context<Self>) {
        self.palette_open = false;
        self.palette_query.clear();
        self.palette_selected = 0;
        self.palette_results.clear();
        self.palette_search_groups.clear();
        self.palette_search_collapsed_paths.clear();
        self.refresh_palette_search_rows_from_groups();
        self.pending_palette_nonce = 0;
        self.palette_backdrop_armed_until = None;
        cx.notify();
    }

    fn open_link_picker(&mut self, anchor_range: Range<usize>, cx: &mut Context<Self>) {
        if self.open_note_loading || self.open_note_path.is_none() {
            return;
        }
        self.link_picker_open = true;
        self.link_picker_anchor_range = Some(anchor_range);
        self.link_picker_query.clear();
        self.link_picker_selected = 0;
        self.link_picker_results.clear();
        self.schedule_apply_link_picker_results(Duration::ZERO, cx);
        cx.notify();
    }

    fn close_link_picker(&mut self, cx: &mut Context<Self>) {
        self.link_picker_open = false;
        self.link_picker_query.clear();
        self.link_picker_selected = 0;
        self.link_picker_results.clear();
        self.link_picker_anchor_range = None;
        cx.notify();
    }

    fn schedule_apply_link_picker_results(&mut self, _delay: Duration, cx: &mut Context<Self>) {
        let query = self.link_picker_query.trim().to_lowercase();
        let Some(index) = self.knowledge_index.as_ref() else {
            self.link_picker_results.clear();
            self.link_picker_selected = 0;
            cx.notify();
            return;
        };

        let paths = if query.is_empty() {
            index.quick_open_paths("", 200)
        } else {
            index.quick_open_paths(&query, 300)
        };
        self.link_picker_results =
            apply_quick_open_weighted_ranking_with_titles(&query, paths, index, 120);
        if self.link_picker_selected >= self.link_picker_results.len() {
            self.link_picker_selected = 0;
        }
        cx.notify();
    }

    fn insert_link_picker_selection(&mut self, cx: &mut Context<Self>) {
        let Some(selected) = self
            .link_picker_results
            .get(self.link_picker_selected)
            .cloned()
        else {
            return;
        };
        let Some(anchor_range) = self.link_picker_anchor_range.clone() else {
            return;
        };

        let insert_text = self.format_wikilink_for_insert(&selected.path, &selected.title);
        self.apply_editor_transaction(
            anchor_range,
            &insert_text,
            EditorMutationSource::LinkPicker,
            false,
            cx,
        );
        self.close_link_picker(cx);
    }

    fn format_wikilink_for_insert(&self, path: &str, title: &str) -> String {
        let mut prefer_title = self.app_settings.files_links.prefer_wikilink_titles;
        let fallback = file_name(path);
        let normalized_title = title.trim();

        if normalized_title.is_empty() || normalized_title.eq_ignore_ascii_case(&fallback) {
            prefer_title = false;
        }

        if prefer_title {
            format!("[[{}|{}]]", path, normalized_title)
        } else {
            format!("[[{}]]", path)
        }
    }

    fn normalize_link_lookup_key(raw: &str) -> String {
        let trimmed = raw
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>')
            .trim();
        let without_alias = trimmed
            .split_once('|')
            .map(|(target, _)| target)
            .unwrap_or(trimmed)
            .trim();
        let without_heading = without_alias
            .split_once('#')
            .map(|(target, _)| target)
            .unwrap_or(without_alias)
            .trim();
        without_heading.to_string()
    }

    fn note_link_hits(&self) -> Vec<NoteLinkHit> {
        let mut out = Vec::new();
        let Some(index) = self.knowledge_index.as_ref() else {
            return out;
        };

        for (link_start, link_end, text_range, url_range) in
            markdown_link_ranges(&self.open_note_content, 0)
        {
            if url_range.start >= url_range.end || url_range.end > self.open_note_content.len() {
                continue;
            }
            let Some(raw_url) = self.open_note_content.get(url_range.clone()) else {
                continue;
            };
            let lookup = Self::normalize_link_lookup_key(raw_url);
            if lookup.is_empty() {
                continue;
            }
            let Some(target_path) = index.resolve_link_target(&lookup) else {
                continue;
            };
            let display = self
                .open_note_content
                .get(text_range.clone())
                .unwrap_or(raw_url)
                .trim()
                .to_string();

            out.push(NoteLinkHit {
                raw: raw_url.trim().to_string(),
                target_path,
                display,
                range: link_start..link_end,
            });
        }

        let mut remain = self.open_note_content.as_str();
        let mut base = 0usize;
        while let Some(start_rel) = remain.find("[[") {
            let start = base + start_rel;
            let after_start = start + 2;
            let after = &self.open_note_content[after_start..];
            let Some(end_rel) = after.find("]]") else {
                break;
            };
            let end = after_start + end_rel + 2;

            let Some(inner) = self
                .open_note_content
                .get(after_start..(after_start + end_rel))
            else {
                break;
            };
            let lookup = Self::normalize_link_lookup_key(inner);
            if !lookup.is_empty() {
                if let Some(target_path) = index.resolve_link_target(&lookup) {
                    out.push(NoteLinkHit {
                        raw: inner.trim().to_string(),
                        target_path,
                        display: lookup,
                        range: start..end,
                    });
                }
            }

            base = end;
            if base >= self.open_note_content.len() {
                break;
            }
            remain = &self.open_note_content[base..];
        }

        out
    }

    fn link_hit_at_offset(&self, offset: usize) -> Option<NoteLinkHit> {
        self.note_link_hits()
            .into_iter()
            .find(|hit| offset >= hit.range.start && offset < hit.range.end)
    }

    fn open_link_under_editor_point(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.open_note_loading || self.open_note_path.is_none() {
            return false;
        }
        let Some(offset) = self.editor_index_for_point(position) else {
            return false;
        };
        let Some(hit) = self.link_hit_at_offset(offset) else {
            return false;
        };

        self.open_note(hit.target_path.clone(), cx);
        self.status = SharedString::from(format!("Open link: {}", hit.display));
        cx.notify();
        true
    }

    fn open_vault_prompt_with_target(&mut self, target: VaultPromptTarget, cx: &mut Context<Self>) {
        let default_value = match &self.vault_state {
            VaultState::Opened { vault, .. } => vault.root().to_string_lossy().to_string(),
            VaultState::Opening { path } => path.to_string_lossy().to_string(),
            _ => resolve_vault_path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        };

        self.vault_prompt_open = true;
        self.vault_prompt_target = target;
        self.vault_prompt_needs_focus = true;
        self.vault_prompt_value = default_value;
        self.vault_prompt_error = None;
        self.vault_prompt_backdrop_armed_until = Some(Instant::now() + OVERLAY_BACKDROP_ARM_DELAY);
        self.palette_open = false;
        self.palette_backdrop_armed_until = None;
        self.settings_open = false;
        self.settings_language_menu_open = false;
        self.settings_backdrop_armed_until = None;
        self.module_switcher_open = false;
        self.module_switcher_backdrop_armed_until = None;
        cx.notify();
    }

    fn open_vault_prompt(&mut self, cx: &mut Context<Self>) {
        self.open_vault_prompt_with_target(VaultPromptTarget::CurrentWindow, cx);
    }

    fn open_vault_prompt_new_window(&mut self, cx: &mut Context<Self>) {
        self.open_vault_prompt_with_target(VaultPromptTarget::NewWindow, cx);
    }

    fn open_vault_in_new_window(&mut self, vault_path: &Path, cx: &mut Context<Self>) {
        let current_exe = match std::env::current_exe() {
            Ok(path) => path,
            Err(err) => {
                self.status = SharedString::from(format!(
                    "Open in new window failed: cannot resolve executable ({err})"
                ));
                cx.notify();
                return;
            }
        };

        let mut command = ProcessCommand::new(current_exe);
        command.arg("--vault");
        command.arg(vault_path.as_os_str());

        match command.spawn() {
            Ok(_child) => {
                self.status = SharedString::from(format!(
                    "Opened vault in new window: {}",
                    vault_path.display()
                ));
            }
            Err(err) => {
                self.status = SharedString::from(format!("Open in new window failed: {err}"));
            }
        }
        cx.notify();
    }

    fn cycle_vault_prompt_target(&mut self, cx: &mut Context<Self>) {
        self.vault_prompt_target = match self.vault_prompt_target {
            VaultPromptTarget::CurrentWindow => VaultPromptTarget::NewWindow,
            VaultPromptTarget::NewWindow => VaultPromptTarget::CurrentWindow,
        };
        self.vault_prompt_error = None;
        cx.notify();
    }

    fn apply_vault_prompt_open(
        &mut self,
        path: PathBuf,
        target: VaultPromptTarget,
        cx: &mut Context<Self>,
    ) {
        self.close_vault_prompt(cx);
        match target {
            VaultPromptTarget::CurrentWindow => {
                self.open_vault(path, cx).detach();
            }
            VaultPromptTarget::NewWindow => {
                self.open_vault_in_new_window(&path, cx);
            }
        }
    }

    fn vault_prompt_mode_hint(&self) -> &'static str {
        match self.vault_prompt_target {
            VaultPromptTarget::CurrentWindow => "Current window",
            VaultPromptTarget::NewWindow => "New window",
        }
    }

    fn vault_prompt_help_hint(&self) -> &'static str {
        "Enter open · Ctrl+Shift+O toggle target · Esc cancel"
    }

    fn on_vault_prompt_submit(&mut self, cx: &mut Context<Self>) {
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

        let target = self.vault_prompt_target;
        self.apply_vault_prompt_open(path, target, cx);
    }

    fn open_module_switcher(&mut self, cx: &mut Context<Self>) {
        self.palette_open = false;
        self.palette_backdrop_armed_until = None;
        self.settings_open = false;
        self.settings_language_menu_open = false;
        self.settings_backdrop_armed_until = None;
        self.vault_prompt_open = false;
        self.vault_prompt_needs_focus = false;
        self.vault_prompt_error = None;
        self.vault_prompt_backdrop_armed_until = None;
        self.module_switcher_open = true;
        self.module_switcher_backdrop_armed_until =
            Some(Instant::now() + OVERLAY_BACKDROP_ARM_DELAY);
        cx.notify();
    }

    fn close_module_switcher(&mut self, cx: &mut Context<Self>) {
        self.module_switcher_open = false;
        self.module_switcher_backdrop_armed_until = None;
        cx.notify();
    }

    fn select_module(&mut self, module: WorkstationModule, cx: &mut Context<Self>) {
        if module.is_available() {
            self.active_module = module;
            self.status = SharedString::from(format!("Module: {}", module.label()));
            self.ai_hub_input_needs_focus = module == WorkstationModule::AiHub;
            self.close_module_switcher(cx);
        } else {
            self.status = SharedString::from(format!(
                "{} module is not available in current MVP",
                module.label()
            ));
            cx.notify();
        }
    }

    fn handle_module_shortcut_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(module) = WorkstationModule::from_shortcut_key(key) else {
            return false;
        };
        self.select_module(module, cx);
        true
    }

    fn close_vault_prompt(&mut self, cx: &mut Context<Self>) {
        self.vault_prompt_open = false;
        self.vault_prompt_target = VaultPromptTarget::CurrentWindow;
        self.vault_prompt_needs_focus = false;
        self.vault_prompt_error = None;
        self.vault_prompt_backdrop_armed_until = None;
        cx.notify();
    }

    fn refresh_search_rows_from_groups(&mut self) {
        self.search_results =
            flatten_search_groups(&self.search_groups, &self.search_collapsed_paths);
        if self.search_results.is_empty() {
            self.search_selected = 0;
        } else if self.search_selected >= self.search_results.len() {
            self.search_selected = 0;
        }
    }

    fn refresh_palette_search_rows_from_groups(&mut self) {
        self.palette_search_results = flatten_search_groups(
            &self.palette_search_groups,
            &self.palette_search_collapsed_paths,
        );
        if self.palette_search_results.is_empty() {
            self.palette_selected = 0;
        } else if self.palette_selected >= self.palette_search_results.len() {
            self.palette_selected = 0;
        }
    }

    fn toggle_search_group_collapsed(&mut self, path: &str) {
        if !self.search_groups.iter().any(|group| group.path == path) {
            return;
        }
        if !self.search_collapsed_paths.remove(path) {
            self.search_collapsed_paths.insert(path.to_string());
        }
        self.refresh_search_rows_from_groups();
        self.search_selected = self
            .search_results
            .iter()
            .position(
                |row| matches!(row, SearchRow::File { path: row_path, .. } if row_path == path),
            )
            .unwrap_or(0);
    }

    fn toggle_palette_search_group_collapsed(&mut self, path: &str) {
        if !self
            .palette_search_groups
            .iter()
            .any(|group| group.path == path)
        {
            return;
        }
        if !self.palette_search_collapsed_paths.remove(path) {
            self.palette_search_collapsed_paths.insert(path.to_string());
        }
        self.refresh_palette_search_rows_from_groups();
        self.palette_selected = self
            .palette_search_results
            .iter()
            .position(
                |row| matches!(row, SearchRow::File { path: row_path, .. } if row_path == path),
            )
            .unwrap_or(0);
    }

    fn focus_next_editor_group(&mut self, cx: &mut Context<Self>) {
        self.ensure_active_group_exists();
        let len = self.editor_groups.len();
        if len <= 1 {
            return;
        }
        let ix = self.active_group_index().unwrap_or(0);
        let next_ix = (ix + 1) % len;
        let group_id = self.editor_groups[next_ix].id;
        self.set_active_editor_group(group_id, cx);
    }

    fn touch_group_mru(&mut self, group_id: u64) {
        if let Some(pos) = self.editor_group_mru.iter().position(|id| *id == group_id) {
            self.editor_group_mru.remove(pos);
        }
        self.editor_group_mru.push_back(group_id);
        while self.editor_group_mru.len() > EDITOR_GROUP_MRU_CAPACITY {
            self.editor_group_mru.pop_front();
        }
    }

    fn prune_group_mru(&mut self) {
        let valid = self
            .editor_groups
            .iter()
            .map(|group| group.id)
            .collect::<HashSet<_>>();
        self.editor_group_mru.retain(|id| valid.contains(id));
        if self.editor_group_mru.is_empty() {
            self.touch_group_mru(self.active_editor_group_id);
        }
    }

    fn record_group_note_history(&mut self, group_id: u64, note_path: &str) {
        let history = self.editor_group_note_history.entry(group_id).or_default();
        history.retain(|existing| existing != note_path);
        history.push_back(note_path.to_string());
        while history.len() > EDITOR_GROUP_MRU_CAPACITY {
            history.pop_front();
        }
        if let Some(group) = self
            .editor_groups
            .iter_mut()
            .find(|group| group.id == group_id)
        {
            group.note_mru = Self::filtered_group_note_mru(history.clone(), &group.tabs);
        }
    }

    fn swap_group_note_history(&mut self, group_id: u64, cx: &mut Context<Self>) {
        let Some(current_path) = self.open_note_path.clone() else {
            return;
        };

        let Some(history) = self.editor_group_note_history.get_mut(&group_id) else {
            return;
        };

        let Some(target_ix) = history.iter().rposition(|path| path != &current_path) else {
            return;
        };

        let target = history[target_ix].clone();
        if !self.open_editors.iter().any(|path| path == &target) {
            return;
        }

        history.retain(|path| path != &target);
        history.push_back(current_path);
        history.push_back(target.clone());
        while history.len() > EDITOR_GROUP_MRU_CAPACITY {
            history.pop_front();
        }

        self.open_note(target, cx);
    }

    fn focus_previous_editor_group(&mut self, cx: &mut Context<Self>) {
        self.ensure_active_group_exists();
        let len = self.editor_groups.len();
        if len <= 1 {
            return;
        }
        let ix = self.active_group_index().unwrap_or(0);
        let prev_ix = if ix == 0 { len - 1 } else { ix - 1 };
        let group_id = self.editor_groups[prev_ix].id;
        self.set_active_editor_group(group_id, cx);
    }

    fn focus_last_editor_group(&mut self, cx: &mut Context<Self>) {
        self.ensure_active_group_exists();
        if self.editor_groups.len() <= 1 {
            return;
        }
        self.prune_group_mru();
        let active = self.active_editor_group_id;
        if let Some(target) = self
            .editor_group_mru
            .iter()
            .rev()
            .find(|id| **id != active)
            .copied()
        {
            self.set_active_editor_group(target, cx);
        }
    }

    fn split_active_group_to_new_note(&mut self, cx: &mut Context<Self>) {
        self.split_active_editor_group(cx);
        self.create_new_note(cx);
    }

    fn close_other_editor_groups(&mut self, cx: &mut Context<Self>) {
        self.ensure_active_group_exists();
        let active = self.active_editor_group_id;
        if self.editor_groups.len() <= 1 {
            return;
        }

        let active_note = self
            .editor_groups
            .iter()
            .find(|group| group.id == active)
            .and_then(|group| group.note_path.clone());

        self.editor_groups.retain(|group| group.id == active);
        self.normalize_editor_group_weights();
        self.prune_group_mru();
        self.active_editor_group_id = active;
        self.touch_group_mru(active);
        self.restore_active_group_runtime_state();
        self.open_note_path = active_note.clone();
        self.selected_note = active_note.clone();

        self.status = SharedString::from("Other editor groups closed");
        cx.notify();
    }

    fn refresh_active_group_after_external_layout_mutation(&mut self) {
        if let Some(group) = self.active_group() {
            self.open_editors = group.tabs.clone();
        }
        self.apply_active_group_interaction_state();
        self.reorder_open_editors_with_pins();
        self.sync_active_group_tabs_from_open_editors();
    }

    fn close_groups_to_right(&mut self, cx: &mut Context<Self>) {
        self.ensure_active_group_exists();
        let Some(active_ix) = self.active_group_index() else {
            return;
        };
        if active_ix + 1 >= self.editor_groups.len() {
            return;
        }

        self.editor_groups.truncate(active_ix + 1);
        self.normalize_editor_group_weights();
        self.prune_group_mru();
        let active = self.active_editor_group_id;
        self.touch_group_mru(active);
        self.restore_active_group_runtime_state();
        let active_note = self
            .editor_groups
            .iter()
            .find(|group| group.id == active)
            .and_then(|group| group.note_path.clone());
        self.open_note_path = active_note.clone();
        self.selected_note = active_note.clone();

        self.status = SharedString::from("Editor groups to the right closed");
        cx.notify();
    }

    fn reopen_external_current_note(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.pending_external_note_reload.take() else {
            return;
        };
        if self.open_note_path.as_deref() != Some(path.as_str()) {
            return;
        }
        self.open_note(path, cx);
    }

    fn cache_note_content(&mut self, path: &str, content: String) {
        self.note_content_cache.insert(path.to_string(), content);
        if let Some(evicted) = touch_cache_order(
            path,
            &mut self.note_content_cache_order,
            NOTE_CONTENT_CACHE_CAPACITY,
        ) {
            self.note_content_cache.remove(&evicted);
        }
    }

    fn cached_note_content(&mut self, path: &str) -> Option<String> {
        let content = self.note_content_cache.get(path).cloned();
        if content.is_some() {
            let _ = touch_cache_order(
                path,
                &mut self.note_content_cache_order,
                NOTE_CONTENT_CACHE_CAPACITY,
            );
        }
        content
    }

    fn evict_note_content_cache_path(&mut self, path: &str) {
        self.note_content_cache.remove(path);
        self.pending_group_preview_loads.remove(path);
        if let Some(pos) = self
            .note_content_cache_order
            .iter()
            .position(|existing| existing == path)
        {
            self.note_content_cache_order.remove(pos);
        }
    }

    fn move_note_content_cache_path(&mut self, from: &str, to: &str) {
        if from == to {
            return;
        }

        if let Some(content) = self.note_content_cache.remove(from) {
            self.note_content_cache.insert(to.to_string(), content);
        }

        if let Some(pos) = self
            .note_content_cache_order
            .iter()
            .position(|existing| existing == from)
        {
            self.note_content_cache_order.remove(pos);
        }

        if self.note_content_cache.contains_key(to) {
            let _ = touch_cache_order(
                to,
                &mut self.note_content_cache_order,
                NOTE_CONTENT_CACHE_CAPACITY,
            );
        }

        if self.pending_group_preview_loads.remove(from) {
            self.pending_group_preview_loads.insert(to.to_string());
        }
    }

    fn schedule_group_preview_load_if_needed(&mut self, note_path: &str, cx: &mut Context<Self>) {
        if self.note_content_cache.contains_key(note_path)
            || self.pending_group_preview_loads.contains(note_path)
        {
            return;
        }
        let Some(vault) = self.vault() else {
            return;
        };

        let path = note_path.to_string();
        self.pending_group_preview_loads.insert(path.clone());

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                let vault = vault.clone();
                let path = path.clone();
                async move {
                    let read_result: anyhow::Result<String> = cx
                        .background_executor()
                        .spawn({
                            let vault = vault.clone();
                            let path = path.clone();
                            async move { vault.read_note(&path) }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        this.pending_group_preview_loads.remove(&path);
                        if let Ok(content) = read_result {
                            this.cache_note_content(&path, content);
                            cx.notify();
                        }
                    })
                    .ok();
                }
            },
        )
        .detach();
    }

    fn schedule_non_active_group_preview_loads(&mut self, cx: &mut Context<Self>) {
        let active_group = self.active_editor_group_id;
        let paths = self
            .editor_groups
            .iter()
            .filter(|group| group.id != active_group)
            .filter_map(|group| group.note_path.clone())
            .filter(|path| {
                !self.note_content_cache.contains_key(path)
                    && !self.pending_group_preview_loads.contains(path)
            })
            .collect::<Vec<_>>();

        for path in paths {
            self.schedule_group_preview_load_if_needed(&path, cx);
        }
    }

    fn apply_restored_group_layout_to_open_editors(&mut self, cx: &mut Context<Self>) {
        self.ensure_active_group_exists();
        let target_len = self.editor_groups.len().max(1);
        let available_paths = self
            .explorer_all_note_paths
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let persisted_note_paths = self
            .editor_groups
            .iter()
            .map(|group| group.note_path.clone())
            .collect::<Vec<_>>();
        let persisted_view_states = self
            .editor_groups
            .iter()
            .map(|group| group.view_state.sanitize())
            .collect::<Vec<_>>();
        let persisted_pinned_tabs = self
            .editor_groups
            .iter()
            .map(|group| group.pinned_tabs.clone())
            .collect::<Vec<_>>();
        let persisted_note_mru = self
            .editor_groups
            .iter()
            .map(|group| group.note_mru.clone())
            .collect::<Vec<_>>();

        let normalize_existing_path = |raw: Option<String>| -> Option<String> {
            raw.and_then(|value| normalize_vault_rel_path(&value).ok())
                .filter(|value| available_paths.contains(value))
        };

        let mut assigned_paths = HashSet::new();
        let mut recovered_tabs = vec![Vec::<String>::new(); target_len];

        for ix in 0..target_len {
            if let Some(group) = self.editor_groups.get(ix) {
                for raw_tab in &group.tabs {
                    let Ok(path) = normalize_vault_rel_path(raw_tab) else {
                        continue;
                    };
                    if !available_paths.contains(&path) {
                        continue;
                    }
                    if !assigned_paths.insert(path.clone()) {
                        continue;
                    }
                    recovered_tabs[ix].push(path);
                }
            }
        }

        for ix in 0..target_len {
            if let Some(persisted_path) =
                normalize_existing_path(persisted_note_paths.get(ix).cloned().unwrap_or_default())
            {
                if !recovered_tabs[ix].iter().any(|tab| tab == &persisted_path)
                    && assigned_paths.insert(persisted_path.clone())
                {
                    recovered_tabs[ix].push(persisted_path);
                }
            }
        }

        if recovered_tabs.iter().all(|tabs| tabs.is_empty()) && !self.open_editors.is_empty() {
            for (ix, raw_path) in self.open_editors.clone().into_iter().enumerate() {
                let Ok(path) = normalize_vault_rel_path(&raw_path) else {
                    continue;
                };
                if !available_paths.contains(&path) {
                    continue;
                }
                if !assigned_paths.insert(path.clone()) {
                    continue;
                }
                let group_ix = ix.min(target_len.saturating_sub(1));
                recovered_tabs[group_ix].push(path);
            }
        }

        for ix in 0..target_len {
            if let Some(group) = self.editor_groups.get_mut(ix) {
                group.view_state = persisted_view_states
                    .get(ix)
                    .copied()
                    .unwrap_or_else(default_editor_group_view_state)
                    .sanitize();
                group.tabs = recovered_tabs[ix].clone();
                group.pinned_tabs = persisted_pinned_tabs
                    .get(ix)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|path| group.tabs.iter().any(|tab| tab == path))
                    .collect();
                group.note_mru = Self::filtered_group_note_mru(
                    persisted_note_mru.get(ix).cloned().unwrap_or_default(),
                    &group.tabs,
                );
                Self::sanitize_group_interaction_state(group);

                let persisted = normalize_existing_path(
                    persisted_note_paths.get(ix).cloned().unwrap_or_default(),
                );

                group.note_path = match persisted {
                    Some(path) if group.tabs.iter().any(|tab| tab == &path) => Some(path),
                    _ => group.tabs.last().cloned(),
                };
            }
        }

        if self
            .editor_groups
            .iter()
            .all(|group| group.id != self.active_editor_group_id)
        {
            self.active_editor_group_id = self.editor_groups[0].id;
        }

        if let Some((tabs, note_path, saved)) = self.active_group().map(|group| {
            (
                group.tabs.clone(),
                group.note_path.clone(),
                group.view_state.sanitize(),
            )
        }) {
            self.open_editors = tabs;
            self.open_note_path = note_path.clone();
            self.selected_note = note_path;
            self.editor_view_mode = saved.mode;
            self.editor_split_ratio = saved.split_ratio;
            self.editor_split_direction = saved.split_direction;
            self.editor_split_saved_mode = saved.split_saved_mode;
        }
        self.apply_active_group_interaction_state();
        self.reorder_open_editors_with_pins();
        self.sync_active_group_tabs_from_open_editors();

        self.prune_group_mru();
        if !self.editor_group_mru.contains(&self.active_editor_group_id) {
            self.editor_group_mru.push_back(self.active_editor_group_id);
        }
        self.normalize_editor_group_weights();

        if let Some(path) = self.open_note_path.clone() {
            self.open_note(path, cx);
        }
    }

    fn apply_loaded_note_content(
        &mut self,
        note_path: &str,
        content: String,
        cx: &mut Context<Self>,
    ) {
        let previous_content = content.clone();
        let note_id = self.ensure_note_id_for_current_note(note_path, content, cx);
        self.open_note_id = Some(note_id.clone());
        self.schedule_load_note_meta(note_id, cx);

        if self.open_note_content != previous_content {
            self.open_note_dirty = true;
            self.schedule_save_note(Duration::from_millis(0), cx);
        }

        self.open_note_word_count = count_words(&self.open_note_content);
        self.pending_markdown_invalidation = Some(MarkdownInvalidationWindow::new(
            0,
            self.open_note_content.len(),
        ));
        self.schedule_markdown_parse(Duration::ZERO, cx);

        if let Some((pending_path, pending_line)) = self.pending_open_note_cursor.take() {
            if pending_path == note_path {
                let offset = byte_offset_for_line(&self.open_note_content, pending_line);
                self.editor_selected_range = offset..offset;
                self.editor_selection_reversed = false;
                self.editor_preferred_x = None;
            } else {
                self.pending_open_note_cursor = Some((pending_path, pending_line));
            }
        }
    }

    fn ensure_note_id_for_current_note(
        &mut self,
        _note_path: &str,
        content: String,
        cx: &mut Context<Self>,
    ) -> String {
        let existing = extract_note_id_from_frontmatter(&content);
        let candidate = existing.clone().unwrap_or_else(generate_note_id);

        match ensure_frontmatter_note_id(&content, &candidate) {
            Ok((normalized_content, normalized_id, changed)) => {
                self.open_note_content = normalized_content;
                self.editor_buffer = Some(EditorBuffer::new(&self.open_note_content));
                if changed {
                    self.status = SharedString::from(format!("Assigned note id: {normalized_id}"));
                    cx.notify();
                }
                normalized_id
            }
            Err(err) => {
                self.open_note_content = content;
                self.editor_buffer = Some(EditorBuffer::new(&self.open_note_content));
                self.status = SharedString::from(format!("Note id normalize failed: {err}"));
                candidate
            }
        }
    }

    fn schedule_load_note_meta(&mut self, note_id: String, cx: &mut Context<Self>) {
        let Some(vault) = self.vault() else {
            return;
        };
        let Some(open_path) = self.open_note_path.clone() else {
            return;
        };

        self.next_note_meta_load_nonce = self.next_note_meta_load_nonce.wrapping_add(1);
        let nonce = self.next_note_meta_load_nonce;
        self.pending_note_meta_load_nonce = nonce;
        self.open_note_meta_loading = true;
        cx.notify();

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let note_id_for_load = note_id.clone();
                    let meta_result: anyhow::Result<Option<NoteMetaV1>> = cx
                        .background_executor()
                        .spawn({
                            let vault = vault.clone();
                            async move {
                                if let Some(meta) = vault.load_note_meta(&note_id_for_load)? {
                                    return Ok(Some(meta));
                                }

                                let meta = NoteMetaV1::new(note_id_for_load.clone())?;
                                vault.save_note_meta(&meta)?;
                                vault.load_note_meta(&note_id_for_load)
                            }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_note_meta_load_nonce != nonce
                            || this.open_note_path.as_deref() != Some(open_path.as_str())
                        {
                            return;
                        }

                        this.open_note_meta_loading = false;
                        match meta_result {
                            Ok(meta) => {
                                this.open_note_meta = meta;
                            }
                            Err(err) => {
                                this.open_note_meta = None;
                                this.status =
                                    SharedString::from(format!("NoteMeta load failed: {err}"));
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

    fn save_current_note_meta(&mut self, mut meta: NoteMetaV1, cx: &mut Context<Self>) {
        let Some(vault) = self.vault() else {
            return;
        };
        let Some(open_path) = self.open_note_path.clone() else {
            return;
        };

        let note_id = meta.id.clone();
        self.next_note_meta_load_nonce = self.next_note_meta_load_nonce.wrapping_add(1);
        let nonce = self.next_note_meta_load_nonce;
        self.pending_note_meta_load_nonce = nonce;
        self.open_note_meta_loading = true;
        meta.updated_at = current_epoch_secs().to_string();

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let save_result: anyhow::Result<Option<NoteMetaV1>> = cx
                        .background_executor()
                        .spawn({
                            let vault = vault.clone();
                            let note_id_for_save = note_id.clone();
                            async move {
                                vault.save_note_meta(&meta)?;
                                vault.load_note_meta(&note_id_for_save)
                            }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_note_meta_load_nonce != nonce
                            || this.open_note_path.as_deref() != Some(open_path.as_str())
                        {
                            return;
                        }

                        this.open_note_meta_loading = false;
                        match save_result {
                            Ok(saved_meta) => {
                                this.open_note_meta = saved_meta;
                                this.status = SharedString::from("Note metadata saved");
                            }
                            Err(err) => {
                                this.status =
                                    SharedString::from(format!("Note metadata save failed: {err}"));
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

    fn add_relation_from_link_target(&mut self, target_path: &str, cx: &mut Context<Self>) {
        let Some(note_id) = self.open_note_id.clone() else {
            self.status = SharedString::from("Current note id unavailable");
            cx.notify();
            return;
        };

        let Some(index) = self.knowledge_index.as_ref() else {
            self.status = SharedString::from("Index unavailable");
            cx.notify();
            return;
        };

        let Some(summary) = index.note_summary(target_path) else {
            self.status = SharedString::from("Target note unavailable");
            cx.notify();
            return;
        };

        let Some(target_id) = summary.note_id else {
            self.status = SharedString::from("Target note has no id yet");
            cx.notify();
            return;
        };

        let mut meta = self.open_note_meta.clone().unwrap_or_else(|| {
            NoteMetaV1::new(note_id.clone()).unwrap_or(NoteMetaV1 {
                version: 1,
                id: note_id,
                updated_at: String::new(),
                relations: Vec::new(),
                pins: Default::default(),
                ext: serde_json::Map::new(),
                extra: serde_json::Map::new(),
            })
        });

        let exists = meta.relations.iter().any(|relation| {
            relation.relation_type == "xnote.references"
                && relation.to.kind == "knowledge"
                && relation.to.id == target_id
        });
        if exists {
            self.status = SharedString::from("Relation already exists");
            cx.notify();
            return;
        }

        meta.relations.push(NoteMetaRelation {
            relation_type: "xnote.references".to_string(),
            to: NoteMetaTarget {
                kind: "knowledge".to_string(),
                id: target_id,
                anchor: None,
                extra: serde_json::Map::new(),
            },
            note: None,
            created_at: None,
            created_by: Some("user".to_string()),
            extra: serde_json::Map::new(),
        });

        self.save_current_note_meta(meta, cx);
    }

    fn toggle_pin_note_from_link_target(&mut self, target_path: &str, cx: &mut Context<Self>) {
        let Some(note_id) = self.open_note_id.clone() else {
            self.status = SharedString::from("Current note id unavailable");
            cx.notify();
            return;
        };

        let Some(index) = self.knowledge_index.as_ref() else {
            self.status = SharedString::from("Index unavailable");
            cx.notify();
            return;
        };

        let Some(summary) = index.note_summary(target_path) else {
            self.status = SharedString::from("Target note unavailable");
            cx.notify();
            return;
        };

        let Some(target_id) = summary.note_id else {
            self.status = SharedString::from("Target note has no id yet");
            cx.notify();
            return;
        };

        let mut meta = self.open_note_meta.clone().unwrap_or_else(|| {
            NoteMetaV1::new(note_id.clone()).unwrap_or(NoteMetaV1 {
                version: 1,
                id: note_id,
                updated_at: String::new(),
                relations: Vec::new(),
                pins: Default::default(),
                ext: serde_json::Map::new(),
                extra: serde_json::Map::new(),
            })
        });

        if meta.pins.notes.iter().any(|item| item == &target_id) {
            meta.pins.notes.retain(|item| item != &target_id);
        } else {
            meta.pins.notes.push(target_id);
        }

        self.save_current_note_meta(meta, cx);
    }

    fn ensure_active_group_exists(&mut self) {
        if self
            .editor_groups
            .iter()
            .any(|group| group.id == self.active_editor_group_id)
        {
            return;
        }

        if let Some(first) = self.editor_groups.first() {
            self.active_editor_group_id = first.id;
        } else {
            let id = self.next_editor_group_id.max(1);
            self.next_editor_group_id = id.saturating_add(1);
            self.editor_groups.push(EditorGroup {
                id,
                note_path: None,
                tabs: Vec::new(),
                pinned_tabs: HashSet::new(),
                note_mru: VecDeque::new(),
                view_state: default_editor_group_view_state(),
            });
            self.active_editor_group_id = id;
        }
        self.normalize_editor_group_weights();
    }

    fn active_group_index(&self) -> Option<usize> {
        self.editor_groups
            .iter()
            .position(|group| group.id == self.active_editor_group_id)
    }

    fn set_active_editor_group(&mut self, group_id: u64, cx: &mut Context<Self>) {
        if self.editor_groups.iter().all(|group| group.id != group_id) {
            return;
        }

        if self.active_editor_group_id == group_id {
            return;
        }

        self.active_editor_group_id = group_id;
        self.touch_group_mru(group_id);
        self.restore_active_group_runtime_state();

        if let Some(group) = self.editor_groups.iter().find(|group| group.id == group_id) {
            let target = group.note_path.clone();
            self.selected_note = target.clone();

            if let Some(path) = target {
                if self.open_note_path.as_deref() != Some(path.as_str()) || self.open_note_loading {
                    self.open_note(path, cx);
                    return;
                }
                self.open_note_path = Some(path.clone());
                self.sync_active_group_note_path();
            } else {
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
            }
        }
        cx.notify();
    }

    fn sync_active_group_note_path(&mut self) {
        if let Some(ix) = self.active_group_index() {
            self.editor_groups[ix].note_path = self.open_note_path.clone();
            self.editor_groups[ix].view_state = self.current_tab_view_state();
            self.editor_groups[ix].tabs = self.open_editors.clone();
        }
    }

    fn split_active_editor_group(&mut self, cx: &mut Context<Self>) {
        self.ensure_active_group_exists();
        let source_ix = self.active_group_index().unwrap_or(0);
        let source_note = self
            .editor_groups
            .get(source_ix)
            .and_then(|group| group.note_path.clone());
        let new_id = self.next_editor_group_id;
        self.next_editor_group_id = self.next_editor_group_id.wrapping_add(1).max(1);

        self.editor_groups.insert(
            source_ix + 1,
            EditorGroup {
                id: new_id,
                note_path: source_note,
                tabs: self.open_editors.clone(),
                pinned_tabs: self.pinned_editors.clone(),
                note_mru: self
                    .editor_group_note_history
                    .get(&self.active_editor_group_id)
                    .cloned()
                    .unwrap_or_default(),
                view_state: self.current_tab_view_state(),
            },
        );
        let engine = SplitLayoutEngine::new(
            EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH,
            EDITOR_GROUP_INITIAL_TOTAL_WIDTH,
        );
        let next = engine.split_at(
            &self.editor_group_width_weights,
            self.editor_groups.len().saturating_sub(1).max(1),
            self.editor_group_target_total_width,
            source_ix,
        );
        self.editor_group_width_weights = next.widths;
        self.editor_group_target_total_width = next.target_total_width;
        self.active_editor_group_id = new_id;
        self.touch_group_mru(new_id);
        self.restore_active_group_runtime_state();
        self.open_note_path = self
            .editor_groups
            .get(source_ix + 1)
            .and_then(|group| group.note_path.clone());
        self.selected_note = self.open_note_path.clone();
        cx.notify();
    }

    fn close_editor_group(&mut self, group_id: u64, cx: &mut Context<Self>) {
        if self.editor_groups.len() <= 1 {
            return;
        }

        let Some(ix) = self
            .editor_groups
            .iter()
            .position(|group| group.id == group_id)
        else {
            return;
        };

        let pre_close_group_len = self.editor_groups.len();
        self.editor_groups.remove(ix);
        let engine = SplitLayoutEngine::new(
            EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH,
            EDITOR_GROUP_INITIAL_TOTAL_WIDTH,
        );
        let next = engine.close_at(
            &self.editor_group_width_weights,
            pre_close_group_len,
            self.editor_group_target_total_width,
            ix,
        );
        self.editor_group_width_weights = next.widths;
        self.editor_group_target_total_width = next.target_total_width;
        self.prune_group_mru();
        self.ensure_active_group_exists();
        if self.active_editor_group_id == group_id {
            let fallback_ix = ix
                .saturating_sub(1)
                .min(self.editor_groups.len().saturating_sub(1));
            let fallback_id = self.editor_groups[fallback_ix].id;
            self.active_editor_group_id = fallback_id;
            self.touch_group_mru(fallback_id);
        }

        if let Some(group) = self
            .editor_groups
            .iter()
            .find(|group| group.id == self.active_editor_group_id)
        {
            self.open_note_path = group.note_path.clone();
            self.selected_note = group.note_path.clone();
        }
        self.restore_active_group_runtime_state();

        cx.notify();
    }

    fn push_recent_query(recent: &mut VecDeque<String>, raw_query: &str) {
        let query = raw_query.trim();
        if query.is_empty() {
            return;
        }

        recent.retain(|existing| !existing.eq_ignore_ascii_case(query));
        recent.push_front(query.to_string());
        while recent.len() > RECENT_QUERY_HISTORY_CAPACITY {
            recent.pop_back();
        }
    }

    fn command_spec_by_id(
        &self,
        id: CommandId,
    ) -> Option<&'static xnote_core::command::CommandSpec> {
        command_specs().iter().find(|spec| spec.id == id)
    }

    fn command_label(&self, id: CommandId) -> String {
        self.command_spec_by_id(id)
            .map(|spec| {
                let text = self.i18n.text(spec.label_key);
                if text == spec.label_key {
                    self.command_label_fallback(id)
                } else {
                    text
                }
            })
            .unwrap_or_else(|| self.command_label_fallback(id))
    }

    fn command_detail(&self, id: CommandId) -> String {
        self.command_spec_by_id(id)
            .map(|spec| {
                let text = self.i18n.text(spec.detail_key);
                if text == spec.detail_key {
                    self.command_detail_fallback(id)
                } else {
                    text
                }
            })
            .unwrap_or_else(|| self.command_detail_fallback(id))
    }

    fn command_label_fallback(&self, id: CommandId) -> String {
        match id {
            CommandId::OpenVaultInNewWindow => "Open vault in new window".to_string(),
            _ => id.as_str().to_string(),
        }
    }

    fn command_detail_fallback(&self, id: CommandId) -> String {
        match id {
            CommandId::OpenVaultInNewWindow => {
                "Choose a folder and open it in a new app window".to_string()
            }
            _ => String::new(),
        }
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

    fn sync_ai_settings_env(&self) {
        let provider = self.app_settings.ai.provider.trim();
        let provider = if provider.is_empty() {
            DEFAULT_AI_PROVIDER
        } else {
            provider
        };
        std::env::set_var("XNOTE_AI_PROVIDER", provider);
        std::env::set_var("XNOTE_AI_BACKEND", provider);

        let vcp_url = self.app_settings.ai.vcp_url.trim();
        if vcp_url.is_empty() {
            std::env::remove_var("XNOTE_AI_VCP_URL");
        } else {
            std::env::set_var("XNOTE_AI_VCP_URL", vcp_url);
        }

        let vcp_key = self.app_settings.ai.vcp_key.trim();
        if vcp_key.is_empty() {
            std::env::remove_var("XNOTE_AI_VCP_KEY");
        } else {
            std::env::set_var("XNOTE_AI_VCP_KEY", vcp_key);
        }

        let vcp_model = self.app_settings.ai.vcp_model.trim();
        if vcp_model.is_empty() {
            std::env::remove_var("XNOTE_AI_VCP_MODEL");
        } else {
            std::env::set_var("XNOTE_AI_VCP_MODEL", vcp_model);
        }

        std::env::set_var(
            "XNOTE_AI_VCP_TOOL_INJECTION",
            if self.app_settings.ai.vcp_tool_injection {
                "true"
            } else {
                "false"
            },
        );
    }

    fn run_ai_endpoint_check(&mut self, cx: &mut Context<Self>) {
        let endpoint = if self.app_settings.ai.vcp_url.trim().is_empty() {
            DEFAULT_AI_VCP_URL.to_string()
        } else {
            self.app_settings.ai.vcp_url.trim().to_string()
        };

        self.next_ai_endpoint_check_nonce = self.next_ai_endpoint_check_nonce.wrapping_add(1);
        let check_nonce = self.next_ai_endpoint_check_nonce;
        self.pending_ai_endpoint_check_nonce = check_nonce;

        self.status = SharedString::from(self.i18n.text("settings.ai.status.endpoint_checking"));
        cx.notify();

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                let endpoint_for_check = endpoint.clone();
                async move {
                    let result = cx
                        .background_executor()
                        .spawn(async move { probe_vcp_endpoint(endpoint_for_check.as_str()) })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_ai_endpoint_check_nonce != check_nonce {
                            return;
                        }

                        this.pending_ai_endpoint_check_nonce = 0;

                        match result {
                            Ok(()) => {
                                this.status = SharedString::from(format!(
                                    "{} {}",
                                    this.i18n.text("settings.ai.status.endpoint_ok"),
                                    endpoint
                                ));
                            }
                            Err(err) => {
                                this.status = SharedString::from(format!(
                                    "{} {} ({err})",
                                    this.i18n.text("settings.ai.status.endpoint_failed"),
                                    endpoint
                                ));
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

    fn apply_persisted_split_layout(&mut self) {
        let layout = self.app_settings.window_layout.clone();

        if let Some(width_px) = layout.panel_shell_width_px {
            let width = px(width_px as f32).max(px(PANEL_SHELL_MIN_WIDTH));
            self.panel_shell_width = width;
            self.panel_shell_saved_width = width;
        }

        if let Some(width_px) = layout.workspace_width_px {
            let width = px(width_px as f32).max(px(WORKSPACE_MIN_WIDTH));
            self.workspace_width = width;
            self.workspace_saved_width = width;
        }

        if let Some(collapsed) = layout.panel_shell_collapsed {
            self.panel_shell_collapsed = collapsed;
            if !collapsed {
                self.panel_shell_width =
                    self.panel_shell_saved_width.max(px(PANEL_SHELL_MIN_WIDTH));
            }
        }

        if let Some(collapsed) = layout.workspace_collapsed {
            self.workspace_collapsed = collapsed;
            if !collapsed {
                self.workspace_width = self.workspace_saved_width.max(px(WORKSPACE_MIN_WIDTH));
            }
        }

        if let Some(ratio_milli) = layout.editor_split_ratio_milli {
            let ratio = ratio_milli as f32 / EDITOR_SPLIT_RATIO_SCALE;
            self.editor_split_ratio = ratio.clamp(EDITOR_SPLIT_MIN_RATIO, EDITOR_SPLIT_MAX_RATIO);
        }

        if let Some(direction) = layout.editor_split_direction.as_deref() {
            self.editor_split_direction = EditorSplitDirection::from_tag(direction);
        }

        if let Some(group_count_raw) = layout.editor_group_count {
            let group_count = usize::from(group_count_raw.clamp(1, 8));
            self.editor_groups.clear();
            self.editor_group_width_weights.clear();

            let mut restored_weights = layout
                .editor_group_width_weights_px
                .unwrap_or_default()
                .into_iter()
                .map(|value| (value as f32).max(EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH))
                .collect::<Vec<_>>();

            let legacy_compact_width = restored_weights
                .iter()
                .all(|value| *value <= EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH + 1.0);

            if restored_weights.len() < group_count {
                restored_weights.resize(
                    group_count,
                    EDITOR_GROUP_INITIAL_TOTAL_WIDTH / group_count as f32,
                );
            } else if restored_weights.len() > group_count {
                restored_weights.truncate(group_count);
            }

            if restored_weights.is_empty() || legacy_compact_width {
                restored_weights =
                    vec![EDITOR_GROUP_INITIAL_TOTAL_WIDTH / group_count as f32; group_count];
            }

            let min_total = EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH * group_count as f32;
            self.editor_group_target_total_width = restored_weights
                .iter()
                .sum::<f32>()
                .max(EDITOR_GROUP_INITIAL_TOTAL_WIDTH)
                .max(min_total);

            self.editor_group_width_weights = restored_weights;
            self.editor_group_mru.clear();
            self.editor_group_note_history.clear();
            let persisted_modes = layout.editor_group_view_modes.unwrap_or_default();
            let persisted_active_paths = layout.editor_group_active_note_paths.unwrap_or_default();
            let persisted_group_tabs = layout.editor_group_tabs.unwrap_or_default();
            let persisted_group_pinned_tabs = layout.editor_group_pinned_tabs.unwrap_or_default();
            let persisted_group_note_mru = layout.editor_group_note_mru.unwrap_or_default();

            for ix in 0..group_count {
                let id = (ix as u64) + 1;
                let mut view_state = default_editor_group_view_state();
                if let Some(mode_tag) = persisted_modes.get(ix) {
                    view_state.mode = EditorViewMode::from_tag(mode_tag);
                    view_state = view_state.sanitize();
                }
                let tabs = persisted_group_tabs.get(ix).cloned().unwrap_or_default();
                let pinned_tabs = persisted_group_pinned_tabs
                    .get(ix)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect::<HashSet<_>>();
                let note_mru = persisted_group_note_mru
                    .get(ix)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect::<VecDeque<_>>();
                self.editor_groups.push(EditorGroup {
                    id,
                    note_path: persisted_active_paths.get(ix).cloned().unwrap_or_default(),
                    tabs,
                    pinned_tabs,
                    note_mru,
                    view_state,
                });
                if let Some(group) = self.editor_groups.last_mut() {
                    Self::sanitize_group_interaction_state(group);
                }
                self.editor_group_mru.push_back(id);
            }

            let active_ix = usize::from(layout.editor_active_group_index.unwrap_or(0))
                .min(group_count.saturating_sub(1));
            self.active_editor_group_id = (active_ix as u64) + 1;
            self.next_editor_group_id = (group_count as u64).saturating_add(1).max(2);
            self.normalize_editor_group_weights();
        }
    }

    fn window_layout_snapshot(&self, window: &Window) -> WindowLayoutSettings {
        let mut layout = self.app_settings.window_layout.clone();
        let panel_width = self.panel_shell_saved_width.max(px(PANEL_SHELL_MIN_WIDTH));
        let workspace_width = self.workspace_saved_width.max(px(WORKSPACE_MIN_WIDTH));

        match window.window_bounds() {
            WindowBounds::Windowed(bounds) => {
                layout.window_x_px = Some((f32::from(bounds.origin.x).round()) as i32);
                layout.window_y_px = Some((f32::from(bounds.origin.y).round()) as i32);
                layout.window_width_px =
                    Some((f32::from(bounds.size.width).round() as u32).max(WINDOW_MIN_WIDTH_PX));
                layout.window_height_px =
                    Some((f32::from(bounds.size.height).round() as u32).max(WINDOW_MIN_HEIGHT_PX));
            }
            WindowBounds::Maximized(bounds) | WindowBounds::Fullscreen(bounds) => {
                layout.window_width_px =
                    Some((f32::from(bounds.size.width).round() as u32).max(WINDOW_MIN_WIDTH_PX));
                layout.window_height_px =
                    Some((f32::from(bounds.size.height).round() as u32).max(WINDOW_MIN_HEIGHT_PX));
            }
        }

        layout.panel_shell_width_px = Some(u32::from(panel_width.round()));
        layout.workspace_width_px = Some(u32::from(workspace_width.round()));
        layout.panel_shell_collapsed = Some(self.panel_shell_collapsed);
        layout.workspace_collapsed = Some(self.workspace_collapsed);
        layout.editor_split_ratio_milli = Some(
            (self.editor_split_ratio.clamp(0.0, 1.0) * EDITOR_SPLIT_RATIO_SCALE).round() as u16,
        );
        layout.editor_split_direction = Some(self.editor_split_direction.to_tag().to_string());
        layout.editor_group_count = Some(self.editor_groups.len().clamp(1, 8) as u8);
        layout.editor_active_group_index = Some(self.active_group_index().unwrap_or(0) as u8);
        let group_len = self.editor_groups.len().max(1);
        let min_total = EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH * group_len as f32;
        let persisted_total = self
            .editor_group_target_total_width
            .max(EDITOR_GROUP_INITIAL_TOTAL_WIDTH)
            .max(min_total);
        layout.editor_group_width_weights_px = Some(
            self.normalized_editor_group_weights_snapshot_for_total(group_len, persisted_total)
                .into_iter()
                .map(|value| value.round().max(1.0) as u32)
                .collect(),
        );
        layout.editor_group_view_modes = Some(
            self.editor_groups
                .iter()
                .map(|group| group.view_state.sanitize().mode.to_tag().to_string())
                .collect(),
        );
        layout.editor_group_active_note_paths = Some(
            self.editor_groups
                .iter()
                .map(|group| group.note_path.clone())
                .collect(),
        );
        layout.editor_group_tabs = Some(
            self.editor_groups
                .iter()
                .map(|group| group.tabs.clone())
                .collect(),
        );
        layout.editor_group_pinned_tabs = Some(
            self.editor_groups
                .iter()
                .map(|group| {
                    group
                        .tabs
                        .iter()
                        .filter(|path| group.pinned_tabs.contains(*path))
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .collect(),
        );
        layout.editor_group_note_mru = Some(
            self.editor_groups
                .iter()
                .map(|group| Self::filtered_group_note_mru(group.note_mru.clone(), &group.tabs))
                .map(|mru| mru.into_iter().collect::<Vec<_>>())
                .collect(),
        );

        layout
    }

    fn current_tab_view_state(&self) -> EditorTabViewState {
        EditorTabViewState {
            mode: self.editor_view_mode,
            split_ratio: self.editor_split_ratio,
            split_direction: self.editor_split_direction,
            split_saved_mode: self.editor_split_saved_mode,
        }
        .sanitize()
    }

    fn remember_current_tab_view_state(&mut self) {
        let state = self.current_tab_view_state();
        if let Some(group) = self.active_group_mut() {
            group.view_state = state;
        }
        if let Some(path) = self.open_note_path.clone() {
            self.editor_tab_view_state
                .insert(path, self.current_tab_view_state());
        }
    }

    fn restore_tab_view_state_or_default(&mut self, path: &str) {
        if let Some(saved) = self.editor_tab_view_state.get(path).copied() {
            let saved = saved.sanitize();
            self.editor_view_mode = saved.mode;
            self.editor_split_ratio = saved.split_ratio;
            self.editor_split_direction = saved.split_direction;
            self.editor_split_saved_mode = saved.split_saved_mode;
            return;
        }

        let fallback = self
            .active_group()
            .map(|group| group.view_state.sanitize())
            .unwrap_or_else(default_editor_group_view_state);
        self.editor_view_mode = fallback.mode;
        self.editor_split_saved_mode = fallback.split_saved_mode;
        self.editor_split_ratio = self
            .editor_split_ratio
            .clamp(EDITOR_SPLIT_MIN_RATIO, EDITOR_SPLIT_MAX_RATIO);
    }

    fn schedule_window_layout_persist_if_changed(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let snapshot = self.window_layout_snapshot(window);
        if snapshot == self.app_settings.window_layout {
            return;
        }

        self.app_settings.window_layout = snapshot;
        self.next_window_layout_persist_nonce =
            self.next_window_layout_persist_nonce.wrapping_add(1);
        let nonce = self.next_window_layout_persist_nonce;
        self.pending_window_layout_persist_nonce = nonce;

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    Timer::after(WINDOW_LAYOUT_PERSIST_DEBOUNCE).await;
                    this.update(&mut cx, |this, _cx| {
                        if this.pending_window_layout_persist_nonce == nonce {
                            this.persist_settings();
                        }
                    })
                    .ok();
                }
            },
        )
        .detach();
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
            CommandId::OpenVaultInNewWindow => {
                self.close_palette(cx);
                self.open_vault_prompt_new_window(cx);
            }
            CommandId::QuickOpen => self.open_palette(PaletteMode::QuickOpen, cx),
            CommandId::CommandPalette => self.open_palette(PaletteMode::Commands, cx),
            CommandId::Settings => {
                self.close_palette(cx);
                self.settings_open = true;
                self.settings_language_menu_open = false;
                self.settings_backdrop_armed_until =
                    Some(Instant::now() + OVERLAY_BACKDROP_ARM_DELAY);
                self.module_switcher_open = false;
                self.module_switcher_backdrop_armed_until = None;
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
                if self.palette_open {
                    self.open_palette(PaletteMode::Search, cx);
                } else {
                    self.close_palette(cx);
                }
                self.panel_mode = PanelMode::Search;
                cx.notify();
            }
            CommandId::AiRewriteSelection => {
                self.close_palette(cx);
                self.ai_rewrite_selection(cx);
            }
        }
    }

    fn ai_rewrite_selection(&mut self, cx: &mut Context<Self>) {
        self.run_ai_rewrite_instruction(
            "Polish the selected note text while keeping original meaning.".to_string(),
            false,
            cx,
        );
    }

    fn ai_hub_submit_input(&mut self, cx: &mut Context<Self>) {
        let prompt = self.ai_chat_input.trim().to_string();
        if prompt.is_empty() {
            self.status = SharedString::from(self.i18n.text("ai.hub.error.input_empty"));
            cx.notify();
            return;
        }

        self.ai_hub_push_message(AiHubMessageRole::User, prompt.clone());
        self.ai_chat_input.clear();
        self.ai_hub_cursor_offset = 0;
        self.ai_hub_cursor_preferred_col = None;
        self.run_ai_chat_prompt(prompt, cx);
    }

    fn run_ai_chat_prompt(&mut self, prompt: String, cx: &mut Context<Self>) {
        let Some(note_path) = self.open_note_path.clone() else {
            let message = self.i18n.text("ai.hub.error.no_open_note");
            self.status = SharedString::from(message.clone());
            self.ai_hub_push_message(AiHubMessageRole::System, message);
            cx.notify();
            return;
        };

        let selection_range = clamp_range_to_char_boundaries(
            &self.open_note_content,
            self.editor_selected_range.clone(),
        );
        let selection_text = if selection_range.start < selection_range.end {
            self.open_note_content
                .get(selection_range)
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };

        let request = AiRewriteRequest {
            note_path,
            selection: selection_text,
            instruction: self.build_ai_chat_instruction(
                prompt.as_str(),
                self.collect_ai_context_excerpt(8, 2_200).as_str(),
            ),
        };

        self.next_ai_rewrite_nonce = self.next_ai_rewrite_nonce.wrapping_add(1);
        let ai_nonce = self.next_ai_rewrite_nonce;
        self.pending_ai_rewrite_nonce = ai_nonce;
        self.status = SharedString::from(self.i18n.text("ai.hub.status.chat_in_progress"));
        cx.notify();

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                let request = request.clone();
                let note_path = request.note_path.clone();
                let vault_result = this.read_with(&cx, |this, _| this.vault());
                let knowledge_index_result =
                    this.read_with(&cx, |this, _| this.knowledge_index.clone());
                async move {
                    let vault = match vault_result {
                        Ok(Some(vault)) => vault,
                        Ok(None) => {
                            this.update(&mut cx, |this, cx| {
                                if this.pending_ai_rewrite_nonce == ai_nonce {
                                    this.pending_ai_rewrite_nonce = 0;
                                    let msg = this.i18n.text("ai.hub.error.vault_not_opened");
                                    this.status = SharedString::from(msg.clone());
                                    this.ai_hub_push_message(AiHubMessageRole::System, msg);
                                    cx.notify();
                                }
                            })
                            .ok();
                            return;
                        }
                        Err(err) => {
                            this.update(&mut cx, |this, cx| {
                                if this.pending_ai_rewrite_nonce == ai_nonce {
                                    this.pending_ai_rewrite_nonce = 0;
                                    let msg = format!(
                                        "{}: {err}",
                                        this.i18n.text("ai.hub.error.read_vault_state")
                                    );
                                    this.status = SharedString::from(msg.clone());
                                    this.ai_hub_push_message(AiHubMessageRole::System, msg);
                                    cx.notify();
                                }
                            })
                            .ok();
                            return;
                        }
                    };

                    let knowledge_index = match knowledge_index_result {
                        Ok(value) => value,
                        Err(_) => None,
                    };

                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let provider_kind = std::env::var("XNOTE_AI_PROVIDER")
                                .ok()
                                .unwrap_or_else(|| DEFAULT_AI_PROVIDER.to_string())
                                .to_ascii_lowercase();

                            if provider_kind == "vcp"
                                || provider_kind == "vcp_compat"
                                || provider_kind == "vcp_toolbox"
                            {
                                let provider = VcpCompatProvider::from_env();
                                let orchestrator = execute_vcp_tool_orchestrator(
                                    &request,
                                    &provider,
                                    &vault,
                                    knowledge_index.as_ref().map(|index| index.as_ref()),
                                    &VcpToolPolicy::default(),
                                    &AiToolOrchestratorConfig {
                                        max_rounds: 4,
                                        request_id: None,
                                        scenario: "chat".to_string(),
                                        final_response_instruction:
                                            "Now return your final user-facing answer.".to_string(),
                                    },
                                )?;

                                Ok::<AiChatRunResult, anyhow::Error>(AiChatRunResult {
                                    provider: provider.provider_name().to_string(),
                                    model: provider.model_name().to_string(),
                                    response: orchestrator.final_response,
                                    tool_calls: orchestrator.tool_calls,
                                    rounds_executed: orchestrator.rounds_executed,
                                    stop_reason: Some(orchestrator.stop_reason),
                                })
                            } else {
                                let provider = build_provider_from_env()?;
                                let response = provider.rewrite_selection(&request)?;
                                Ok::<AiChatRunResult, anyhow::Error>(AiChatRunResult {
                                    provider: provider.provider_name().to_string(),
                                    model: provider.model_name().to_string(),
                                    response,
                                    tool_calls: Vec::new(),
                                    rounds_executed: 1,
                                    stop_reason: Some(AiToolLoopStopReason::FinalResponse),
                                })
                            }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_ai_rewrite_nonce != ai_nonce
                            || this.open_note_path.as_deref() != Some(note_path.as_str())
                        {
                            return;
                        }

                        this.pending_ai_rewrite_nonce = 0;
                        match result {
                            Ok(chat) => {
                                let response = chat.response.trim().to_string();
                                if response.is_empty() {
                                    this.ai_hub_push_message(
                                        AiHubMessageRole::System,
                                        this.i18n.text("ai.hub.status.chat_empty_response"),
                                    );
                                    this.status = SharedString::from(
                                        this.i18n.text("ai.hub.status.chat_completed_empty"),
                                    );
                                } else {
                                    this.ai_hub_push_message(
                                        AiHubMessageRole::Assistant,
                                        truncate_message(response.as_str(), 2_400),
                                    );
                                    if chat.tool_calls.is_empty() {
                                        this.status = SharedString::from(format!(
                                            "{} ({}/{})",
                                            this.i18n.text("ai.hub.status.chat_done"),
                                            chat.provider,
                                            chat.model
                                        ));
                                    } else {
                                        this.ai_hub_push_message(
                                            AiHubMessageRole::System,
                                            this.summarize_tool_trace(
                                                chat.tool_calls.as_slice(),
                                                chat.rounds_executed,
                                                chat.stop_reason,
                                            ),
                                        );
                                        this.status = SharedString::from(format!(
                                            "{} ({}/{}, {} {})",
                                            this.i18n.text("ai.hub.status.chat_done"),
                                            chat.provider,
                                            chat.model,
                                            chat.tool_calls.len(),
                                            this.i18n.text("ai.hub.trace.calls_suffix")
                                        ));
                                    }
                                }
                            }
                            Err(err) => {
                                let msg = format!(
                                    "{}: {err}",
                                    this.i18n.text("ai.hub.status.chat_failed")
                                );
                                this.status = SharedString::from(msg.clone());
                                this.ai_hub_push_message(AiHubMessageRole::System, msg);
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

    fn ai_hub_push_message(&mut self, role: AiHubMessageRole, content: impl Into<String>) {
        let mut content = content.into();
        if content.trim().is_empty() {
            return;
        }
        if content.chars().count() > 4_000 {
            content = truncate_message(content.as_str(), 4_000);
        }

        self.ai_hub_messages.push(AiHubMessage {
            role,
            content: SharedString::from(content),
            timestamp_label: SharedString::from(build_clock_label(current_epoch_secs())),
        });

        if self.ai_hub_messages.len() > AI_HUB_MAX_MESSAGES {
            let overflow = self.ai_hub_messages.len() - AI_HUB_MAX_MESSAGES;
            self.ai_hub_messages.drain(0..overflow);
        }
    }

    fn collect_ai_context_excerpt(&self, max_messages: usize, max_chars: usize) -> String {
        if self.ai_hub_messages.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        let total = self.ai_hub_messages.len();
        let start = total.saturating_sub(max_messages);

        for message in self.ai_hub_messages.iter().skip(start) {
            let role = match message.role {
                AiHubMessageRole::User => "user",
                AiHubMessageRole::Assistant => "assistant",
                AiHubMessageRole::System => "system",
            };
            out.push_str(role);
            out.push_str(": ");
            out.push_str(message.content.as_ref());
            out.push('\n');
        }

        truncate_message(out.trim(), max_chars)
    }

    fn ai_hub_agent_items() -> &'static [AiHubAgentItem; 4] {
        &[
            AiHubAgentItem {
                name_key: "ai.hub.ui.agent.writer.name",
                meta_key: "ai.hub.ui.agent.writer.meta",
                instruction_key: "ai.hub.agent.instruction.writer",
            },
            AiHubAgentItem {
                name_key: "ai.hub.ui.agent.researcher.name",
                meta_key: "ai.hub.ui.agent.researcher.meta",
                instruction_key: "ai.hub.agent.instruction.researcher",
            },
            AiHubAgentItem {
                name_key: "ai.hub.ui.agent.planner.name",
                meta_key: "ai.hub.ui.agent.planner.meta",
                instruction_key: "ai.hub.agent.instruction.planner",
            },
            AiHubAgentItem {
                name_key: "ai.hub.ui.agent.coder.name",
                meta_key: "ai.hub.ui.agent.coder.meta",
                instruction_key: "ai.hub.agent.instruction.coder",
            },
        ]
    }

    fn selected_ai_hub_agent(&self) -> AiHubAgentItem {
        let items = Self::ai_hub_agent_items();
        let idx = self
            .ai_hub_selected_agent_idx
            .min(items.len().saturating_sub(1));
        items[idx]
    }

    fn ai_hub_normalize_cursor(&mut self) {
        let len = self.ai_chat_input.len();
        self.ai_hub_cursor_offset =
            previous_char_boundary(&self.ai_chat_input, self.ai_hub_cursor_offset.min(len));
    }

    fn ai_hub_line_start(&self, offset: usize) -> usize {
        let offset =
            previous_char_boundary(&self.ai_chat_input, offset.min(self.ai_chat_input.len()));
        let prefix = &self.ai_chat_input[..offset];
        match prefix.rfind('\n') {
            Some(ix) => ix + 1,
            None => 0,
        }
    }

    fn ai_hub_line_end(&self, offset: usize) -> usize {
        let offset =
            previous_char_boundary(&self.ai_chat_input, offset.min(self.ai_chat_input.len()));
        let suffix = &self.ai_chat_input[offset..];
        match suffix.find('\n') {
            Some(rel) => offset + rel,
            None => self.ai_chat_input.len(),
        }
    }

    fn ai_hub_col_in_line(&self, offset: usize) -> usize {
        let offset =
            previous_char_boundary(&self.ai_chat_input, offset.min(self.ai_chat_input.len()));
        let line_start = self.ai_hub_line_start(offset);
        self.ai_chat_input[line_start..offset].chars().count()
    }

    fn ai_hub_offset_for_line_col(&self, line_start: usize, col: usize) -> usize {
        let line_start = previous_char_boundary(
            &self.ai_chat_input,
            line_start.min(self.ai_chat_input.len()),
        );
        let line_end = self.ai_hub_line_end(line_start);
        let line = &self.ai_chat_input[line_start..line_end];
        if col == 0 {
            return line_start;
        }

        let mut current_col = 0usize;
        for (byte_ix, _ch) in line.char_indices() {
            if current_col == col {
                return line_start + byte_ix;
            }
            current_col += 1;
        }
        line_end
    }

    fn ai_hub_insert_text_at_cursor(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }

        self.ai_hub_normalize_cursor();
        let cursor = self.ai_hub_cursor_offset;
        self.ai_chat_input.insert_str(cursor, text);
        self.ai_hub_cursor_offset = cursor + text.len();
        self.ai_hub_cursor_preferred_col = None;
        true
    }

    fn ai_hub_delete_backward_char(&mut self) -> bool {
        self.ai_hub_normalize_cursor();
        let cursor = self.ai_hub_cursor_offset;
        if cursor == 0 {
            return false;
        }

        let start = previous_char_boundary(&self.ai_chat_input, cursor.saturating_sub(1));
        if start >= cursor {
            return false;
        }

        self.ai_chat_input.replace_range(start..cursor, "");
        self.ai_hub_cursor_offset = start;
        self.ai_hub_cursor_preferred_col = None;
        true
    }

    fn ai_hub_delete_forward_char(&mut self) -> bool {
        self.ai_hub_normalize_cursor();
        let cursor = self.ai_hub_cursor_offset;
        if cursor >= self.ai_chat_input.len() {
            return false;
        }

        let Some(next_char) = self.ai_chat_input[cursor..].chars().next() else {
            return false;
        };
        let end = cursor + next_char.len_utf8();
        self.ai_chat_input.replace_range(cursor..end, "");
        self.ai_hub_cursor_preferred_col = None;
        true
    }

    fn ai_hub_move_cursor_vertical(&mut self, direction: i32) -> bool {
        if direction == 0 {
            return false;
        }

        self.ai_hub_normalize_cursor();
        let cursor = self.ai_hub_cursor_offset;
        let line_start = self.ai_hub_line_start(cursor);
        let line_end = self.ai_hub_line_end(cursor);
        let current_col = self.ai_hub_col_in_line(cursor);
        let target_col = self.ai_hub_cursor_preferred_col.unwrap_or(current_col);

        let target = if direction < 0 {
            if line_start == 0 {
                0
            } else {
                let prev_line_anchor = line_start.saturating_sub(1);
                let prev_line_start = self.ai_hub_line_start(prev_line_anchor);
                self.ai_hub_offset_for_line_col(prev_line_start, target_col)
            }
        } else if line_end >= self.ai_chat_input.len() {
            self.ai_chat_input.len()
        } else {
            let next_line_start = line_end + 1;
            self.ai_hub_offset_for_line_col(next_line_start, target_col)
        };

        self.ai_hub_cursor_offset = target;
        self.ai_hub_cursor_preferred_col = Some(target_col);
        target != cursor
    }

    fn build_ai_chat_instruction(&self, user_prompt: &str, context_excerpt: &str) -> String {
        let mut instruction = String::new();
        instruction.push_str(self.i18n.text("ai.hub.chat.instruction.base").as_str());
        instruction.push_str("\n\n");

        instruction.push_str(
            self.i18n
                .text(self.selected_ai_hub_agent().instruction_key)
                .as_str(),
        );
        instruction.push_str("\n\n");

        if !context_excerpt.trim().is_empty() {
            instruction.push_str(
                self.i18n
                    .text("ai.hub.chat.instruction.context_header")
                    .as_str(),
            );
            instruction.push('\n');
            instruction.push_str(context_excerpt.trim());
            instruction.push_str("\n\n");
        }

        instruction.push_str(
            self.i18n
                .text("ai.hub.chat.instruction.user_prompt")
                .as_str(),
        );
        instruction.push('\n');
        instruction.push_str(user_prompt.trim());
        instruction
    }

    fn summarize_tool_trace(
        &self,
        tool_calls: &[VcpToolRequest],
        rounds_executed: usize,
        stop_reason: Option<AiToolLoopStopReason>,
    ) -> String {
        let stop_reason_label = match stop_reason {
            Some(AiToolLoopStopReason::FinalResponse) => {
                self.i18n.text("ai.hub.trace.stop.final_response")
            }
            Some(AiToolLoopStopReason::MaxRoundsReached) => {
                self.i18n.text("ai.hub.trace.stop.max_rounds")
            }
            None => self.i18n.text("ai.hub.trace.stop.unknown"),
        };

        let mut lines = vec![format!(
            "{} {} | {} {} | {} {}",
            self.i18n.text("ai.hub.trace.calls"),
            tool_calls.len(),
            self.i18n.text("ai.hub.trace.rounds"),
            rounds_executed,
            self.i18n.text("ai.hub.trace.stop"),
            stop_reason_label,
        )];

        for (idx, call) in tool_calls.iter().enumerate() {
            let args_preview = if call.args.is_empty() {
                self.i18n.text("ai.hub.trace.args.none")
            } else {
                let preview = call
                    .args
                    .iter()
                    .map(|(k, v)| format!("{k}={}", truncate_message(v, 48)))
                    .collect::<Vec<_>>()
                    .join(", ");
                truncate_message(preview.as_str(), 140)
            };

            lines.push(format!(
                "#{} {} | {}",
                idx + 1,
                call.tool_name,
                args_preview
            ));
        }

        lines.join("\n")
    }

    fn run_ai_rewrite_instruction(
        &mut self,
        instruction: String,
        emit_chat_messages: bool,
        cx: &mut Context<Self>,
    ) {
        if self.open_note_loading || self.open_note_path.is_none() {
            let message = self.i18n.text("ai.hub.error.no_open_note");
            self.status = SharedString::from(message.clone());
            if emit_chat_messages {
                self.ai_hub_push_message(AiHubMessageRole::System, message);
            }
            cx.notify();
            return;
        }

        let selection_range = if self.editor_selected_range.is_empty() {
            let cursor = self.editor_cursor_offset();
            self.editor_line_start(cursor)..self.editor_line_end(cursor)
        } else {
            self.editor_selected_range.clone()
        };

        let selection_range =
            clamp_range_to_char_boundaries(&self.open_note_content, selection_range);
        if selection_range.start >= selection_range.end {
            let message = self.i18n.text("ai.hub.error.empty_selection");
            self.status = SharedString::from(message.clone());
            if emit_chat_messages {
                self.ai_hub_push_message(AiHubMessageRole::System, message);
            }
            cx.notify();
            return;
        }

        let Some(selection_text) = self.open_note_content.get(selection_range.clone()) else {
            let message = self.i18n.text("ai.hub.error.invalid_selection_range");
            self.status = SharedString::from(message.clone());
            if emit_chat_messages {
                self.ai_hub_push_message(AiHubMessageRole::System, message);
            }
            cx.notify();
            return;
        };

        let Some(note_path) = self.open_note_path.clone() else {
            let message = self.i18n.text("ai.hub.error.no_open_note");
            self.status = SharedString::from(message.clone());
            if emit_chat_messages {
                self.ai_hub_push_message(AiHubMessageRole::System, message);
            }
            cx.notify();
            return;
        };

        let request = AiRewriteRequest {
            note_path,
            selection: selection_text.to_string(),
            instruction,
        };

        self.next_ai_rewrite_nonce = self.next_ai_rewrite_nonce.wrapping_add(1);
        let ai_nonce = self.next_ai_rewrite_nonce;
        self.pending_ai_rewrite_nonce = ai_nonce;
        let open_nonce = self.current_note_open_nonce;
        self.status = SharedString::from(self.i18n.text("ai.hub.status.rewrite_in_progress"));
        cx.notify();

        cx.spawn(
            move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                let request = request.clone();
                let selection_range = selection_range.clone();
                let note_path = request.note_path.clone();
                let vault_result = this.read_with(&cx, |this, _| this.vault());
                let knowledge_index_result =
                    this.read_with(&cx, |this, _| this.knowledge_index.clone());
                async move {
                    let vault = match vault_result {
                        Ok(Some(vault)) => vault,
                        Ok(None) => {
                            this.update(&mut cx, |this, cx| {
                                if this.pending_ai_rewrite_nonce == ai_nonce {
                                    this.pending_ai_rewrite_nonce = 0;
                                    let msg =
                                        this.i18n.text("ai.hub.error.rewrite_vault_not_opened");
                                    this.status = SharedString::from(msg.clone());
                                    if emit_chat_messages {
                                        this.ai_hub_push_message(AiHubMessageRole::System, msg);
                                    }
                                    cx.notify();
                                }
                            })
                            .ok();
                            return;
                        }
                        Err(err) => {
                            this.update(&mut cx, |this, cx| {
                                if this.pending_ai_rewrite_nonce == ai_nonce {
                                    this.pending_ai_rewrite_nonce = 0;
                                    let msg = format!(
                                        "{}: {err}",
                                        this.i18n.text("ai.hub.error.rewrite_read_vault_state")
                                    );
                                    this.status = SharedString::from(msg.clone());
                                    if emit_chat_messages {
                                        this.ai_hub_push_message(AiHubMessageRole::System, msg);
                                    }
                                    cx.notify();
                                }
                            })
                            .ok();
                            return;
                        }
                    };

                    let knowledge_index = match knowledge_index_result {
                        Ok(value) => value,
                        Err(_) => None,
                    };

                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let provider_kind = std::env::var("XNOTE_AI_PROVIDER")
                                .ok()
                                .unwrap_or_else(|| "mock".to_string())
                                .to_ascii_lowercase();

                            if provider_kind == "vcp"
                                || provider_kind == "vcp_compat"
                                || provider_kind == "vcp_toolbox"
                            {
                                execute_rewrite_with_vcp_tool_loop(
                                    &request,
                                    &vault,
                                    knowledge_index.as_ref().map(|index| index.as_ref()),
                                    &VcpToolPolicy::default(),
                                    2,
                                )
                                .map(|loop_result| {
                                    let rationale = if loop_result.tool_calls.is_empty() {
                                        "rewrite with VCP provider".to_string()
                                    } else {
                                        format!(
                                            "rewrite with {} tool call(s)",
                                            loop_result.tool_calls.len()
                                        )
                                    };

                                    xnote_core::ai::AiExecutionResult {
                                        proposal: xnote_core::ai::AiRewriteProposal {
                                            replacement: loop_result.replacement,
                                            rationale,
                                            provider: loop_result.provider,
                                            model: loop_result.model,
                                        },
                                        dry_run: true,
                                        applied: false,
                                    }
                                })
                            } else {
                                execute_rewrite_with_env_provider(
                                    &request,
                                    false,
                                    AiPolicy::default(),
                                )
                            }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_ai_rewrite_nonce != ai_nonce
                            || this.current_note_open_nonce != open_nonce
                            || this.open_note_path.as_deref() != Some(note_path.as_str())
                        {
                            return;
                        }

                        this.pending_ai_rewrite_nonce = 0;
                        match result {
                            Ok(result) => {
                                let replacement = result.proposal.replacement;
                                this.apply_editor_transaction(
                                    selection_range,
                                    &replacement,
                                    EditorMutationSource::Keyboard,
                                    false,
                                    cx,
                                );
                                this.status = SharedString::from(format!(
                                    "{} ({}/{})",
                                    this.i18n.text("ai.hub.status.rewrite_applied"),
                                    result.proposal.provider,
                                    result.proposal.model
                                ));
                                if emit_chat_messages {
                                    this.ai_hub_push_message(
                                        AiHubMessageRole::Assistant,
                                        format!(
                                            "{}\n\n{}",
                                            result.proposal.rationale,
                                            truncate_message(replacement.as_str(), 1_200)
                                        ),
                                    );
                                }
                            }
                            Err(err) => {
                                let msg = format!(
                                    "{}: {err}",
                                    this.i18n.text("ai.hub.status.rewrite_failed")
                                );
                                this.status = SharedString::from(msg.clone());
                                if emit_chat_messages {
                                    this.ai_hub_push_message(AiHubMessageRole::System, msg);
                                }
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
        let panel_min_w = px(PANEL_SHELL_MIN_WIDTH);
        let workspace_min_w = px(WORKSPACE_MIN_WIDTH);

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
            self.panel_shell_width = self.panel_shell_saved_width.max(px(PANEL_SHELL_MIN_WIDTH));
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
            self.workspace_width = self.workspace_saved_width.max(px(WORKSPACE_MIN_WIDTH));
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
            SplitterKind::EditorGroup => px(0.),
        };
        self.splitter_drag = Some(SplitterDrag {
            kind,
            start_x: event.position.x,
            start_width,
            group_split_index: None,
            group_pair_total: None,
            group_target_total: None,
            pointer_initialized: false,
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
        let Some(mut drag) = self.splitter_drag else {
            return;
        };

        if !drag.pointer_initialized {
            drag.start_x = event.position.x;
            drag.pointer_initialized = true;
            self.splitter_drag = Some(drag);
            return;
        }

        let rail_w = px(48.);
        let splitter_w = px(6.);
        let editor_min_w = px(320.);
        let panel_min_w = px(PANEL_SHELL_MIN_WIDTH);
        let workspace_min_w = px(WORKSPACE_MIN_WIDTH);

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
                self.splitter_drag = Some(drag);
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
                self.splitter_drag = Some(drag);
                cx.notify();
            }
            SplitterKind::EditorGroup => return,
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
        let shift = ev.keystroke.modifiers.shift;
        let key = ev.keystroke.key.to_lowercase();

        if ctrl && shift && key.as_str() == "o" {
            self.cycle_vault_prompt_target(cx);
            return;
        }

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
                self.on_vault_prompt_submit(cx);
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
            PaletteMode::Search => (
                self.i18n.text("palette.title.search"),
                self.i18n.text("palette.placeholder.search"),
                self.i18n.text("palette.group.search"),
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
            PaletteMode::Search => self.palette_search_results.len(),
        };
        let palette_hint = match self.palette_mode {
            PaletteMode::Commands => SharedString::from("Esc close · ↑/↓ navigate · Enter run"),
            PaletteMode::QuickOpen => {
                if query_empty {
                    if let Some(recent) = self.recent_palette_quick_open_queries.front() {
                        SharedString::from(format!(
                            "Esc close · ↑/↓ navigate · Enter open · Recent: {recent}"
                        ))
                    } else {
                        SharedString::from("Esc close · ↑/↓ navigate · Enter open")
                    }
                } else {
                    SharedString::from("Esc close · ↑/↓ navigate · Enter open")
                }
            }
            PaletteMode::Search => {
                if query_empty {
                    if let Some(recent) = self.recent_palette_search_queries.front() {
                        SharedString::from(format!(
                            "Esc close · ↑/↓ navigate · Enter open · Recent: {recent}"
                        ))
                    } else {
                        SharedString::from("Esc close · ↑/↓ navigate · Enter open")
                    }
                } else {
                    SharedString::from("Esc close · ↑/↓ navigate · Enter open")
                }
            }
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
                                if let Some(recent) = this.recent_palette_quick_open_queries.front()
                                {
                                    if recent.is_empty() {
                                        "Type to search…"
                                    } else {
                                        "Type to search… (Ctrl+V to paste)"
                                    }
                                } else {
                                    "Type to search…"
                                }
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
                                let title = open_match.title.clone();
                                let path_highlights = open_match.path_highlights.clone();
                                let title_highlights = open_match.title_highlights.clone();

                                let selected = ix == this.palette_selected;
                                let open_path = path.clone();
                                let row_path = path.clone();

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
                                    .tooltip({
                                        let tooltip_path = row_path.clone();
                                        let tooltip_theme = ui_theme;
                                        move |_window, cx| {
                                            AnyView::from(cx.new(|_| TooltipPreview {
                                                label: tooltip_path.clone().into(),
                                                ui_theme: tooltip_theme,
                                            }))
                                        }
                                    })
                                    .child(ui_icon(ICON_FILE_TEXT, 16., ui_theme.text_muted))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .overflow_hidden()
                                            .flex()
                                            .flex_col()
                                            .justify_center()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .overflow_hidden()
                                                    .font_family("Inter")
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight(780.))
                                                    .whitespace_nowrap()
                                                    .text_ellipsis()
                                                    .children(render_highlighted_segments(
                                                        &title,
                                                        &title_highlights,
                                                        ui_theme.text_primary,
                                                        ui_theme.accent,
                                                        FontWeight(760.),
                                                        FontWeight(900.),
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .overflow_hidden()
                                                    .font_family("IBM Plex Mono")
                                                    .text_size(px(10.))
                                                    .font_weight(FontWeight(650.))
                                                    .whitespace_nowrap()
                                                    .text_ellipsis()
                                                    .children(render_highlighted_segments(
                                                        &path,
                                                        &path_highlights,
                                                        ui_theme.text_muted,
                                                        ui_theme.text_secondary,
                                                        FontWeight(650.),
                                                        FontWeight(820.),
                                                    )),
                                            ),
                                    )
                            })
                            .collect::<Vec<_>>()
                    }
                    PaletteMode::Search => {
                        if this.palette_search_results.is_empty() {
                            let msg = if this.palette_query.trim().is_empty() {
                                "Type keywords to search content…"
                            } else {
                                "No search matches"
                            };
                            return range
                                .map(|ix| {
                                    div()
                                        .id(ElementId::named_usize("palette.search.empty", ix))
                                        .h(px(44.))
                                        .px(px(10.))
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
                                let Some(row) = this.palette_search_results.get(ix) else {
                                    return div()
                                        .id(ElementId::named_usize("palette.search.missing", ix))
                                        .h(px(44.))
                                        .px(px(10.))
                                        .child("");
                                };

                                let selected = ix == this.palette_selected;

                                match row {
                                    SearchRow::File {
                                        path,
                                        match_count,
                                        path_highlights,
                                    } => {
                                        let row_path = path.clone();
                                        let toggle_path = path.clone();
                                        let is_collapsed =
                                            this.palette_search_collapsed_paths.contains(path);
                                        let count_label =
                                            SharedString::from(format!("{match_count}"));
                                        div()
                                            .id(ElementId::Name(SharedString::from(format!(
                                                "palette.search.file:{path}"
                                            ))))
                                            .h(px(44.))
                                            .w_full()
                                            .px(px(10.))
                                            .flex()
                                            .items_center()
                                            .cursor_pointer()
                                            .bg(if selected {
                                                rgb(ui_theme.accent_soft)
                                            } else {
                                                rgba(0x00000000)
                                            })
                                            .when(!selected, |this| {
                                                this.hover(|this| {
                                                    this.bg(rgb(ui_theme.interactive_hover))
                                                })
                                            })
                                            .on_click(cx.listener(
                                                move |this, _ev: &ClickEvent, _window, cx| {
                                                    this.palette_selected = ix;
                                                    this.toggle_palette_search_group_collapsed(
                                                        &toggle_path,
                                                    );
                                                    cx.notify();
                                                },
                                            ))
                                            .tooltip({
                                                let tooltip_path = row_path.clone();
                                                let tooltip_theme = ui_theme;
                                                move |_window, cx| {
                                                    AnyView::from(cx.new(|_| TooltipPreview {
                                                        label: tooltip_path.clone().into(),
                                                        ui_theme: tooltip_theme,
                                                    }))
                                                }
                                            })
                                            .child(ui_icon(
                                                ICON_FILE_TEXT,
                                                16.,
                                                ui_theme.text_muted,
                                            ))
                                            .child(
                                                div()
                                                    .h(px(20.))
                                                    .w(px(20.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(ui_icon(
                                                        if is_collapsed {
                                                            ICON_CHEVRON_RIGHT
                                                        } else {
                                                            ICON_CHEVRON_DOWN
                                                        },
                                                        14.,
                                                        ui_theme.text_muted,
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .flex()
                                                    .items_center()
                                                    .font_family("IBM Plex Mono")
                                                    .text_size(px(11.))
                                                    .font_weight(FontWeight(800.))
                                                    .whitespace_nowrap()
                                                    .text_ellipsis()
                                                    .children(render_highlighted_segments(
                                                        path,
                                                        path_highlights,
                                                        ui_theme.text_primary,
                                                        ui_theme.accent,
                                                        FontWeight(800.),
                                                        FontWeight(900.),
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .font_family("IBM Plex Mono")
                                                    .text_size(px(10.))
                                                    .font_weight(FontWeight(700.))
                                                    .text_color(rgb(ui_theme.text_muted))
                                                    .child(count_label),
                                            )
                                    }
                                    SearchRow::Match {
                                        path,
                                        line,
                                        preview,
                                        preview_highlights,
                                    } => {
                                        let row_path = path.clone();
                                        let path_for_click = path.clone();
                                        let line_for_click = *line;
                                        let line_label = SharedString::from(format!("{line}"));

                                        div()
                                            .id(ElementId::Name(SharedString::from(format!(
                                                "palette.search.match:{path}:{line}"
                                            ))))
                                            .h(px(44.))
                                            .w_full()
                                            .px(px(10.))
                                            .flex()
                                            .items_center()
                                            .cursor_pointer()
                                            .bg(if selected {
                                                rgb(ui_theme.accent_soft)
                                            } else {
                                                rgba(0x00000000)
                                            })
                                            .when(!selected, |this| {
                                                this.hover(|this| {
                                                    this.bg(rgb(ui_theme.interactive_hover))
                                                })
                                            })
                                            .on_click(cx.listener(
                                                move |this, _ev: &ClickEvent, _window, cx| {
                                                    this.palette_selected = ix;
                                                    this.close_palette(cx);
                                                    this.open_note_at_line(
                                                        path_for_click.clone(),
                                                        line_for_click,
                                                        cx,
                                                    );
                                                },
                                            ))
                                            .tooltip({
                                                let tooltip_text = SharedString::from(format!(
                                                    "{}:{} {}",
                                                    row_path, line, preview
                                                ));
                                                let tooltip_theme = ui_theme;
                                                move |_window, cx| {
                                                    AnyView::from(cx.new(|_| TooltipPreview {
                                                        label: tooltip_text.clone(),
                                                        ui_theme: tooltip_theme,
                                                    }))
                                                }
                                            })
                                            .child(ui_icon(ICON_SEARCH, 16., ui_theme.text_muted))
                                            .child(
                                                div()
                                                    .font_family("IBM Plex Mono")
                                                    .text_size(px(10.))
                                                    .font_weight(FontWeight(700.))
                                                    .text_color(rgb(ui_theme.text_muted))
                                                    .child(line_label),
                                            )
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .flex()
                                                    .items_center()
                                                    .font_family("IBM Plex Mono")
                                                    .text_size(px(11.))
                                                    .whitespace_nowrap()
                                                    .text_ellipsis()
                                                    .children(render_highlighted_segments(
                                                        preview,
                                                        preview_highlights,
                                                        ui_theme.text_secondary,
                                                        ui_theme.accent,
                                                        FontWeight(650.),
                                                        FontWeight(850.),
                                                    )),
                                            )
                                    }
                                }
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
            .occlude()
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
                    .px_3()
                    .pb(px(6.))
                    .font_family("IBM Plex Mono")
                    .text_size(px(10.))
                    .font_weight(FontWeight(650.))
                    .text_color(rgb(ui_theme.text_muted))
                    .child(palette_hint),
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
                    .occlude()
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
                    .child(palette_box),
            )
    }

    fn module_switcher_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_theme = UiTheme::from_settings(self.settings_theme, self.settings_accent);
        let module_item = |id: &'static str, module: WorkstationModule| {
            let active = self.active_module == module;
            let available = module.is_available();
            let disabled = !available;
            let shortcut_hint = module.shortcut_hint();

            let row = div()
                .id(id)
                .h(px(36.))
                .w_full()
                .px(px(10.))
                .rounded_sm()
                .flex()
                .items_center()
                .gap(px(8.))
                .bg(if active {
                    rgb(ui_theme.interactive_hover)
                } else {
                    rgba(0x00000000)
                })
                .when(!disabled, |this| this.cursor_pointer())
                .when(disabled, |this| this.opacity(0.82))
                .when(!disabled, |this| {
                    this.hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                })
                .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                    this.select_module(module, cx);
                }))
                .child(ui_icon(
                    module.icon(),
                    14.,
                    if available {
                        if active {
                            ui_theme.accent
                        } else {
                            ui_theme.text_secondary
                        }
                    } else {
                        ui_theme.text_subtle
                    },
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family("IBM Plex Mono")
                        .text_size(px(11.))
                        .font_weight(FontWeight(if active { 800. } else { 700. }))
                        .text_color(rgb(if available {
                            ui_theme.text_primary
                        } else {
                            ui_theme.text_muted
                        }))
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(module.label()),
                )
                .child(
                    div()
                        .w(px(56.))
                        .flex()
                        .justify_end()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_subtle))
                        .child(shortcut_hint),
                )
                .child(
                    div()
                        .w(px(112.))
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_subtle))
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(if disabled {
                            "Coming soon"
                        } else {
                            module.detail()
                        }),
                );

            if let Some(message) = module.disabled_tooltip() {
                row.tooltip({
                    let tooltip_message: SharedString = message.into();
                    let tooltip_theme = ui_theme;
                    move |_window, cx| {
                        AnyView::from(cx.new(|_| TooltipPreview {
                            label: tooltip_message.clone(),
                            ui_theme: tooltip_theme,
                        }))
                    }
                })
            } else {
                row
            }
        };

        let menu = div()
            .id("module.switcher.menu")
            .w(px(320.))
            .p(px(8.))
            .rounded_md()
            .border_1()
            .border_color(rgb(ui_theme.border))
            .bg(rgb(ui_theme.surface_bg))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .px(px(4.))
                    .pb(px(4.))
                    .font_family("IBM Plex Mono")
                    .text_size(px(10.))
                    .font_weight(FontWeight(800.))
                    .text_color(rgb(ui_theme.text_muted))
                    .child("MODULES"),
            )
            .child(module_item(
                "module.switcher.knowledge",
                WorkstationModule::Knowledge,
            ))
            .child(module_item(
                "module.switcher.resources",
                WorkstationModule::Resources,
            ))
            .child(module_item(
                "module.switcher.inbox",
                WorkstationModule::Inbox,
            ))
            .child(module_item(
                "module.switcher.ai_hub",
                WorkstationModule::AiHub,
            ))
            .child(
                div()
                    .mt(px(4.))
                    .px(px(10.))
                    .pt(px(6.))
                    .border_t_1()
                    .border_color(rgb(ui_theme.border))
                    .font_family("IBM Plex Mono")
                    .text_size(px(10.))
                    .font_weight(FontWeight(700.))
                    .text_color(rgb(ui_theme.text_subtle))
                    .child("Tip: Alt+K / Alt+R / Alt+I / Alt+A"),
            );

        div()
            .id("module.switcher.overlay")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .occlude()
            .focusable()
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                let key = ev.keystroke.key.to_lowercase();
                if key == "escape" {
                    this.close_module_switcher(cx);
                    return;
                }

                if ev.keystroke.modifiers.alt {
                    this.handle_module_shortcut_key(&key, cx);
                }
            }))
            .child(
                div()
                    .id("module.switcher.backdrop")
                    .size_full()
                    .bg(rgba(0x00000000))
                    .absolute()
                    .top_0()
                    .left_0()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        let armed = this
                            .module_switcher_backdrop_armed_until
                            .is_none_or(|deadline| Instant::now() >= deadline);
                        if armed {
                            this.close_module_switcher(cx);
                        }
                    })),
            )
            .child(
                div()
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .child(div().absolute().left(px(8.)).bottom(px(34.)).child(menu)),
            )
    }

    fn render_ai_hub_panel(
        &mut self,
        ui_theme: UiTheme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let agent_row = |name: SharedString,
                         meta: SharedString,
                         active: bool,
                         row_idx: usize,
                         cx: &mut Context<Self>| {
            div()
                .h(px(58.))
                .w_full()
                .px(px(10.))
                .py(px(8.))
                .bg(if active {
                    rgb(ui_theme.accent_soft)
                } else {
                    rgb(ui_theme.surface_alt_bg)
                })
                .border_1()
                .border_color(rgb(if active {
                    ui_theme.accent
                } else {
                    ui_theme.border
                }))
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev: &MouseDownEvent, window, cx| {
                        this.ai_hub_selected_agent_idx = row_idx;
                        this.ai_hub_input_needs_focus = true;
                        this.ai_hub_cursor_offset = this.ai_chat_input.len();
                        this.ai_hub_cursor_preferred_col = None;
                        window.focus(&this.ai_hub_input_focus_handle);
                        cx.notify();
                    }),
                )
                .flex()
                .flex_col()
                .justify_center()
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(13.))
                        .font_weight(FontWeight(800.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(name.clone()),
                )
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child(meta.clone()),
                )
        };

        let agent_panel_title = SharedString::from(self.i18n.text("ai.hub.ui.agent_panel.title"));
        let agent_panel_subtitle =
            SharedString::from(self.i18n.text("ai.hub.ui.agent_panel.subtitle"));
        let agent_items = Self::ai_hub_agent_items();

        let agent_rows = agent_items
            .into_iter()
            .enumerate()
            .map(|(row_idx, item)| {
                agent_row(
                    SharedString::from(self.i18n.text(item.name_key)),
                    SharedString::from(self.i18n.text(item.meta_key)),
                    self.ai_hub_selected_agent_idx == row_idx,
                    row_idx,
                    cx,
                )
            })
            .collect::<Vec<_>>();

        let left_panel = div()
            .w(px(260.))
            .min_w(px(260.))
            .max_w(px(260.))
            .h_full()
            .bg(rgb(ui_theme.surface_bg))
            .border_r_1()
            .border_color(rgb(ui_theme.border))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .font_family("Inter")
                    .text_size(px(20.))
                    .font_weight(FontWeight(800.))
                    .text_color(rgb(ui_theme.text_primary))
                    .child(agent_panel_title),
            )
            .child(
                div()
                    .font_family("IBM Plex Mono")
                    .text_size(px(10.))
                    .font_weight(FontWeight(700.))
                    .text_color(rgb(ui_theme.text_muted))
                    .child(agent_panel_subtitle),
            )
            .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(agent_rows),
            );

        let role_user = self.i18n.text("ai.hub.ui.role.user");
        let role_assistant = self.i18n.text("ai.hub.ui.role.assistant");
        let role_system = self.i18n.text("ai.hub.ui.role.system");

        let messages = self
            .ai_hub_messages
            .iter()
            .enumerate()
            .map(|(ix, message)| {
                let (bubble_bg, bubble_border, bubble_text, prefix) = match message.role {
                    AiHubMessageRole::User => (
                        ui_theme.accent_soft,
                        ui_theme.accent,
                        ui_theme.text_primary,
                        role_user.clone(),
                    ),
                    AiHubMessageRole::Assistant => (
                        ui_theme.surface_bg,
                        ui_theme.border,
                        ui_theme.text_primary,
                        role_assistant.clone(),
                    ),
                    AiHubMessageRole::System => (
                        ui_theme.surface_alt_bg,
                        ui_theme.border,
                        ui_theme.text_muted,
                        role_system.clone(),
                    ),
                };

                div()
                    .id(ElementId::named_usize("ai_hub.message", ix))
                    .w_full()
                    .bg(rgb(bubble_bg))
                    .border_1()
                    .border_color(rgb(bubble_border))
                    .rounded_md()
                    .p(px(10.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .font_family("Inter")
                                    .text_size(px(12.))
                                    .font_weight(FontWeight(800.))
                                    .text_color(rgb(ui_theme.text_secondary))
                                    .child(prefix),
                            )
                            .child(
                                div()
                                    .font_family("IBM Plex Mono")
                                    .text_size(px(10.))
                                    .font_weight(FontWeight(700.))
                                    .text_color(rgb(ui_theme.text_muted))
                                    .child(message.timestamp_label.clone()),
                            ),
                    )
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(13.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(bubble_text))
                            .child(message.content.clone()),
                    )
            })
            .collect::<Vec<_>>();

        self.ai_hub_normalize_cursor();
        let input_empty = self.ai_chat_input.is_empty();
        let cursor = self.ai_hub_cursor_offset.min(self.ai_chat_input.len());
        let ai_input_focused = self.ai_hub_input_focus_handle.is_focused(window);

        let (input_left, input_right) = if input_empty {
            (String::new(), String::new())
        } else {
            (
                self.ai_chat_input[..cursor].to_string(),
                self.ai_chat_input[cursor..].to_string(),
            )
        };
        let input_placeholder = self.i18n.text("ai.hub.ui.input.placeholder");

        let center_panel = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(rgb(ui_theme.app_bg))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .h(px(44.))
                    .w_full()
                    .px(px(12.))
                    .bg(rgb(ui_theme.surface_bg))
                    .border_1()
                    .border_color(rgb(ui_theme.border))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(16.))
                            .font_weight(FontWeight(800.))
                            .text_color(rgb(ui_theme.text_primary))
                            .child(self.ai_hub_session_title.clone()),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .bg(rgb(ui_theme.surface_bg))
                    .border_1()
                    .border_color(rgb(ui_theme.border))
                    .p(px(10.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .children(messages),
            )
            .child(
                div()
                    .id("ai_hub.input")
                    .h(px(56.))
                    .w_full()
                    .px(px(10.))
                    .bg(rgb(ui_theme.surface_bg))
                    .border_1()
                    .border_color(rgb(if ai_input_focused {
                        ui_theme.accent
                    } else {
                        ui_theme.border
                    }))
                    .track_focus(&self.ai_hub_input_focus_handle)
                    .focusable()
                    .cursor_text()
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                        this.ai_hub_input_needs_focus = true;
                        this.on_ai_hub_input_key(ev, cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _ev: &MouseDownEvent, window, cx| {
                            this.ai_hub_input_needs_focus = true;
                            this.ai_hub_cursor_offset = this.ai_chat_input.len();
                            this.ai_hub_cursor_preferred_col = None;
                            window.focus(&this.ai_hub_input_focus_handle);
                            cx.notify();
                        }),
                    )
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(ui_icon(ICON_COMMAND, 14., ui_theme.text_muted))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .w_full()
                            .h_full()
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .font_family("IBM Plex Mono")
                            .text_size(px(12.))
                            .font_weight(FontWeight(if input_empty { 650. } else { 750. }))
                            .child(if input_empty {
                                if ai_input_focused {
                                    div()
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .font_family("IBM Plex Mono")
                                        .text_size(px(12.))
                                        .font_weight(FontWeight(650.))
                                        .text_color(rgb(ui_theme.text_subtle))
                                        .child(div().w(px(1.)).h(px(16.)).bg(rgb(ui_theme.accent)))
                                        .child(input_placeholder)
                                        .into_any_element()
                                } else {
                                    div()
                                        .w_full()
                                        .font_family("IBM Plex Mono")
                                        .text_size(px(12.))
                                        .font_weight(FontWeight(650.))
                                        .text_color(rgb(ui_theme.text_subtle))
                                        .child(input_placeholder)
                                        .into_any_element()
                                }
                            } else {
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .font_family("IBM Plex Mono")
                                    .text_size(px(12.))
                                    .font_weight(FontWeight(750.))
                                    .text_color(rgb(ui_theme.text_primary))
                                    .child(SharedString::from(input_left))
                                    .child(div().w(px(1.)).h(px(16.)).bg(rgb(
                                        if ai_input_focused {
                                            ui_theme.accent
                                        } else {
                                            ui_theme.text_subtle
                                        },
                                    )))
                                    .child(SharedString::from(input_right))
                                    .into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .h(px(34.))
                            .w(px(44.))
                            .bg(rgb(ui_theme.accent_soft))
                            .border_1()
                            .border_color(rgb(ui_theme.accent))
                            .cursor_pointer()
                            .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                                    this.ai_hub_submit_input(cx);
                                }),
                            )
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(ui_icon(ICON_CHEVRON_RIGHT, 14., ui_theme.text_primary)),
                    ),
            );

        div()
            .id("ai_hub.main")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .flex()
            .child(left_panel)
            .child(center_panel)
    }

    fn link_picker_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_theme = UiTheme::from_settings(self.settings_theme, self.settings_accent);
        let input_empty = self.link_picker_query.trim().is_empty();
        let input_text = if input_empty {
            SharedString::from("Type to link notes…")
        } else {
            SharedString::from(self.link_picker_query.clone())
        };
        let input_color = if input_empty {
            ui_theme.text_subtle
        } else {
            ui_theme.text_primary
        };

        let results = if self.link_picker_results.is_empty() {
            vec![div()
                .id(ElementId::named_usize("link_picker.empty", 0))
                .h(px(40.))
                .px(px(10.))
                .font_family("Inter")
                .text_size(px(13.))
                .font_weight(FontWeight(700.))
                .text_color(rgb(ui_theme.text_muted))
                .child(if input_empty {
                    "Type to search notes…"
                } else {
                    "No matches"
                })]
        } else {
            self.link_picker_results
                .iter()
                .enumerate()
                .map(|(ix, item)| {
                    let selected = ix == self.link_picker_selected;
                    let open_ix = ix;
                    let title = item.title.clone();
                    let path = item.path.clone();
                    let title_highlights = item.title_highlights.clone();
                    let path_highlights = item.path_highlights.clone();
                    div()
                        .id(ElementId::Name(SharedString::from(format!(
                            "link_picker.item:{}",
                            item.path
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
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                            this.link_picker_selected = open_ix;
                            this.insert_link_picker_selection(cx);
                        }))
                        .child(ui_icon(ICON_LINK_2, 14., ui_theme.text_muted))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                .justify_center()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .font_family("Inter")
                                        .text_size(px(12.))
                                        .font_weight(FontWeight(780.))
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .children(render_highlighted_segments(
                                            &title,
                                            &title_highlights,
                                            ui_theme.text_primary,
                                            ui_theme.accent,
                                            FontWeight(760.),
                                            FontWeight(900.),
                                        )),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .font_family("IBM Plex Mono")
                                        .text_size(px(10.))
                                        .font_weight(FontWeight(650.))
                                        .text_color(rgb(ui_theme.text_muted))
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .children(render_highlighted_segments(
                                            &path,
                                            &path_highlights,
                                            ui_theme.text_muted,
                                            ui_theme.text_secondary,
                                            FontWeight(650.),
                                            FontWeight(820.),
                                        )),
                                ),
                        )
                })
                .collect::<Vec<_>>()
        };

        let picker_box = div()
            .w(px(640.))
            .h(px(360.))
            .bg(rgb(ui_theme.surface_alt_bg))
            .border_1()
            .border_color(rgb(ui_theme.border))
            .occlude()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(44.))
                    .w_full()
                    .px_3()
                    .bg(rgb(ui_theme.surface_bg))
                    .border_b_1()
                    .border_color(rgb(ui_theme.border))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui_icon(ICON_LINK_2, 15., ui_theme.text_subtle))
                    .child(
                        div()
                            .font_family("Inter")
                            .text_size(px(13.))
                            .font_weight(FontWeight(700.))
                            .text_color(rgb(input_color))
                            .child(input_text),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(700.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child("ESC"),
                    ),
            )
            .child(
                div()
                    .id("link_picker.list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .children(results),
            );

        div()
            .id("link_picker.overlay")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .occlude()
            .child(
                div()
                    .id("link_picker.backdrop")
                    .size_full()
                    .bg(rgba(0x00000024))
                    .absolute()
                    .top_0()
                    .left_0()
                    .occlude()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.close_link_picker(cx);
                    })),
            )
            .child(
                div()
                    .size_full()
                    .absolute()
                    .top_0()
                    .left_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(picker_box),
            )
    }

    fn render_link_hit_hint(&self, hit: &NoteLinkHit, ui_theme: UiTheme) -> gpui::AnyElement {
        let label = if hit.raw.is_empty() {
            hit.target_path.clone()
        } else {
            format!("{} → {}", hit.raw, hit.target_path)
        };

        div()
            .id("editor.link.hint")
            .absolute()
            .right(px(12.))
            .top(px(8.))
            .px(px(10.))
            .h(px(22.))
            .rounded_md()
            .border_1()
            .border_color(rgb(ui_theme.accent_soft))
            .bg(rgb(ui_theme.surface_alt_bg))
            .text_color(rgb(ui_theme.text_secondary))
            .font_family("IBM Plex Mono")
            .text_size(px(10.))
            .font_weight(FontWeight(700.))
            .whitespace_nowrap()
            .text_ellipsis()
            .child(SharedString::from(label))
            .into_any_element()
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
        let mode_hint = self.vault_prompt_mode_hint();
        let help_hint = self.vault_prompt_help_hint();

        let prompt_box = div()
            .w(px(720.))
            .bg(rgb(ui_theme.surface_alt_bg))
            .border_1()
            .border_color(rgb(ui_theme.border))
            .occlude()
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
                            .font_family("IBM Plex Mono")
                            .text_size(px(10.))
                            .font_weight(FontWeight(700.))
                            .text_color(rgb(ui_theme.text_subtle))
                            .child(SharedString::from(format!("Target: {mode_hint}"))),
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
                            .child(help_hint),
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
            SettingsSection::Ai => self.i18n.text("settings.nav.ai"),
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
                .unwrap_or_else(|| self.i18n.text("settings.common.none"));
            let note_count = self.explorer_all_note_paths.len();
            let bookmark_count = self.app_settings.bookmarked_notes.len();
            let runtime = match &self.plugin_activation_state {
                PluginActivationState::Idle => self.i18n.text("settings.about.runtime_status.idle"),
                PluginActivationState::Activating => {
                    self.i18n.text("settings.about.runtime_status.activating")
                }
                PluginActivationState::Ready { active_count } => format!(
                    "{} ({active_count})",
                    self.i18n.text("settings.about.runtime_status.ready")
                ),
                PluginActivationState::Error { message } => format!(
                    "{} ({message})",
                    self.i18n.text("settings.about.runtime_status.error")
                ),
            };

            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(card(
                    self.i18n.text("settings.about.card.workspace.title"),
                    self.i18n.text("settings.about.card.workspace.desc"),
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(12.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_secondary))
                        .child(format!(
                            "{}: {note_count}\n{}: {open_note}\n{}: {bookmark_count}",
                            self.i18n.text("settings.about.workspace.notes"),
                            self.i18n.text("settings.about.workspace.open_note"),
                            self.i18n.text("settings.about.workspace.bookmarks"),
                        ))
                        .into_any_element(),
                ))
                .child(card(
                    self.i18n.text("settings.about.card.runtime.title"),
                    self.i18n.text("settings.about.card.runtime.desc"),
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(12.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_secondary))
                        .child(format!(
                            "{}: {}\n{}: {}\n{}: {}",
                            self.i18n.text("settings.about.runtime.plugins"),
                            self.plugin_registry.list().len(),
                            self.i18n.text("settings.about.runtime.mode"),
                            self.plugin_runtime_mode.as_tag(),
                            self.i18n.text("settings.about.runtime.status"),
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
                    this.status = SharedString::from(format!(
                        "{} {next} {}",
                        this.i18n.text("settings.editor.status.autosave_set"),
                        this.i18n.text("settings.editor.unit.ms")
                    ));
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
                    self.i18n.text("settings.editor.card.autosave.title"),
                    self.i18n.text("settings.editor.card.autosave.desc"),
                    autosave_select.into_any_element(),
                ))
                .child(card(
                    self.i18n.text("settings.editor.card.behavior.title"),
                    self.i18n.text("settings.editor.card.behavior.desc"),
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(12.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_secondary))
                        .child(self.i18n.text("settings.editor.card.behavior.hint"))
                        .into_any_element(),
                ))
                .into_any_element()
        };

        let ai_content = {
            let provider_label = if self.app_settings.ai.provider.trim().is_empty() {
                DEFAULT_AI_PROVIDER.to_string()
            } else {
                self.app_settings.ai.provider.trim().to_string()
            };
            let endpoint_label = if self.app_settings.ai.vcp_url.trim().is_empty() {
                DEFAULT_AI_VCP_URL.to_string()
            } else {
                self.app_settings.ai.vcp_url.trim().to_string()
            };
            let model_label = if self.app_settings.ai.vcp_model.trim().is_empty() {
                DEFAULT_AI_VCP_MODEL.to_string()
            } else {
                self.app_settings.ai.vcp_model.trim().to_string()
            };

            let provider_toggle = div()
                .id("settings.ai.provider")
                .h(px(34.))
                .px(px(10.))
                .bg(rgb(ui_theme.surface_alt_bg))
                .border_1()
                .border_color(rgb(ui_theme.border))
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                .flex()
                .items_center()
                .justify_between()
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    let next = if this
                        .app_settings
                        .ai
                        .provider
                        .trim()
                        .eq_ignore_ascii_case("vcp")
                    {
                        "mock"
                    } else {
                        "vcp"
                    };
                    this.app_settings.ai.provider = next.to_string();
                    this.persist_settings();
                    this.sync_ai_settings_env();
                    this.status = SharedString::from(format!(
                        "{} {next}",
                        this.i18n.text("settings.ai.status.provider_set")
                    ));
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(13.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(provider_label),
                )
                .child(ui_icon(ICON_CHEVRON_DOWN, 16., ui_theme.text_muted));

            let endpoint_toggle = div()
                .id("settings.ai.endpoint")
                .h(px(34.))
                .px(px(10.))
                .bg(rgb(ui_theme.surface_alt_bg))
                .border_1()
                .border_color(rgb(ui_theme.border))
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                .flex()
                .items_center()
                .justify_between()
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    let current = this.app_settings.ai.vcp_url.trim().to_string();
                    let next = if current.eq_ignore_ascii_case(DEFAULT_AI_VCP_URL) {
                        "http://localhost:5890".to_string()
                    } else {
                        DEFAULT_AI_VCP_URL.to_string()
                    };
                    this.app_settings.ai.vcp_url = next.clone();
                    this.persist_settings();
                    this.sync_ai_settings_env();
                    this.status = SharedString::from(format!(
                        "{} {next}",
                        this.i18n.text("settings.ai.status.endpoint_set")
                    ));
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(11.))
                        .font_weight(FontWeight(750.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(endpoint_label),
                )
                .child(ui_icon(ICON_REFRESH_CW, 14., ui_theme.text_muted));

            let model_toggle = div()
                .id("settings.ai.model")
                .h(px(34.))
                .px(px(10.))
                .bg(rgb(ui_theme.surface_alt_bg))
                .border_1()
                .border_color(rgb(ui_theme.border))
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.accent_soft)))
                .flex()
                .items_center()
                .justify_between()
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    let current = this.app_settings.ai.vcp_model.trim().to_string();
                    let next = if current.eq_ignore_ascii_case(DEFAULT_AI_VCP_MODEL) {
                        "gemini-2.5-pro-preview".to_string()
                    } else {
                        DEFAULT_AI_VCP_MODEL.to_string()
                    };
                    this.app_settings.ai.vcp_model = next.clone();
                    this.persist_settings();
                    this.sync_ai_settings_env();
                    this.status = SharedString::from(format!(
                        "{} {next}",
                        this.i18n.text("settings.ai.status.model_set")
                    ));
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(11.))
                        .font_weight(FontWeight(750.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(model_label),
                )
                .child(ui_icon(ICON_CHEVRON_DOWN, 16., ui_theme.text_muted));

            let tool_injection_toggle = div()
                .id("settings.ai.tool_injection")
                .h(px(34.))
                .px(px(10.))
                .bg(if self.app_settings.ai.vcp_tool_injection {
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
                .justify_between()
                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    this.app_settings.ai.vcp_tool_injection =
                        !this.app_settings.ai.vcp_tool_injection;
                    this.persist_settings();
                    this.sync_ai_settings_env();
                    this.status = SharedString::from(if this.app_settings.ai.vcp_tool_injection {
                        this.i18n.text("settings.ai.status.tool_injection_enabled")
                    } else {
                        this.i18n.text("settings.ai.status.tool_injection_disabled")
                    });
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(if self.app_settings.ai.vcp_tool_injection {
                            self.i18n.text("settings.ai.toggle.tool_injection_on")
                        } else {
                            self.i18n.text("settings.ai.toggle.tool_injection_off")
                        }),
                )
                .child(ui_icon(ICON_FILE_COG, 14., ui_theme.text_muted));

            let apply_btn = div()
                .id("settings.ai.apply")
                .h(px(34.))
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
                    this.sync_ai_settings_env();
                    let endpoint = if this.app_settings.ai.vcp_url.trim().is_empty() {
                        DEFAULT_AI_VCP_URL
                    } else {
                        this.app_settings.ai.vcp_url.trim()
                    };
                    this.status = SharedString::from(format!(
                        "{} {}",
                        this.i18n.text("settings.ai.status.endpoint_ready"),
                        endpoint
                    ));
                    this.run_ai_endpoint_check(cx);
                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.))
                        .font_weight(FontWeight(800.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(self.i18n.text("settings.ai.button.apply_check")),
                );

            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(card(
                    self.i18n.text("settings.ai.card.provider.title"),
                    self.i18n.text("settings.ai.card.provider.desc"),
                    provider_toggle.into_any_element(),
                ))
                .child(card(
                    self.i18n.text("settings.ai.card.endpoint.title"),
                    self.i18n.text("settings.ai.card.endpoint.desc"),
                    endpoint_toggle.into_any_element(),
                ))
                .child(card(
                    self.i18n.text("settings.ai.card.model.title"),
                    self.i18n.text("settings.ai.card.model.desc"),
                    model_toggle.into_any_element(),
                ))
                .child(card(
                    self.i18n.text("settings.ai.card.tool_loop.title"),
                    self.i18n.text("settings.ai.card.tool_loop.desc"),
                    tool_injection_toggle.into_any_element(),
                ))
                .child(card(
                    self.i18n.text("settings.ai.card.connection.title"),
                    self.i18n.text("settings.ai.card.connection.desc"),
                    apply_btn.into_any_element(),
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
                            this.i18n
                                .text("settings.files.status.external_sync_enabled")
                        } else {
                            this.i18n
                                .text("settings.files.status.external_sync_disabled")
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
                            self.i18n.text("settings.common.enabled")
                        } else {
                            self.i18n.text("settings.common.disabled")
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
                            this.i18n
                                .text("settings.files.status.prefer_wikilink_enabled")
                        } else {
                            this.i18n
                                .text("settings.files.status.prefer_wikilink_disabled")
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
                            self.i18n.text("settings.common.enabled")
                        } else {
                            self.i18n.text("settings.common.disabled")
                        }),
                );

            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(card(
                    self.i18n.text("settings.files.card.external_sync.title"),
                    self.i18n.text("settings.files.card.external_sync.desc"),
                    external_sync_toggle.into_any_element(),
                ))
                .child(card(
                    self.i18n.text("settings.files.card.wiki_pref.title"),
                    self.i18n.text("settings.files.card.wiki_pref.desc"),
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
                                    self.i18n.text("settings.hotkeys.placeholder.press_keys")
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
                                    .child(if is_editing {
                                        self.i18n.text("settings.hotkeys.button.cancel")
                                    } else {
                                        self.i18n.text("settings.hotkeys.button.edit")
                                    }),
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
                                                "{}: {}",
                                                this.i18n.text("settings.hotkeys.status.reset_one"),
                                                command_id.as_str()
                                            ));
                                        }
                                        Err(err) => {
                                            this.status = SharedString::from(format!(
                                                "{} ({}): {err}",
                                                this.i18n.text(
                                                    "settings.hotkeys.status.reset_one_failed"
                                                ),
                                                command_id.as_str()
                                            ));
                                        }
                                    }
                                } else {
                                    this.status = SharedString::from(format!(
                                        "{}: {}",
                                        this.i18n.text(
                                            "settings.hotkeys.status.shortcut_already_default"
                                        ),
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
                                    .child(self.i18n.text("settings.hotkeys.button.reset")),
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
                                this.status = SharedString::from(
                                    this.i18n.text("settings.hotkeys.status.reset_all"),
                                );
                            }
                            Err(err) => {
                                this.status = SharedString::from(format!(
                                    "{}: {err}",
                                    this.i18n.text("settings.hotkeys.status.reset_all_failed")
                                ));
                            }
                        }
                    } else {
                        this.status = SharedString::from(
                            this.i18n
                                .text("settings.hotkeys.status.all_shortcuts_already_default"),
                        );
                    }

                    cx.notify();
                }))
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.))
                        .font_weight(FontWeight(800.))
                        .text_color(rgb(ui_theme.text_primary))
                        .child(self.i18n.text("settings.hotkeys.button.reset_all")),
                );

            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(card(
                    self.i18n.text("settings.hotkeys.card.shortcuts.title"),
                    self.i18n.text("settings.hotkeys.card.shortcuts.desc"),
                    list.into_any_element(),
                ))
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(11.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child(self.i18n.text("settings.hotkeys.tip")),
                )
                .child(reset_all)
                .into_any_element()
        };

        let advanced_content = {
            let runtime_mode_label = if self.plugin_runtime_mode == PluginRuntimeMode::Process {
                self.i18n.text("settings.advanced.runtime_mode.process")
            } else {
                self.i18n.text("settings.advanced.runtime_mode.in_process")
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
                        "{} {}",
                        this.i18n.text("settings.advanced.status.runtime_mode_set"),
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
                            self.i18n.text("settings.advanced.watcher.enabled")
                        } else {
                            self.i18n.text("settings.advanced.watcher.disabled")
                        }),
                );

            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(card(
                    self.i18n.text("settings.advanced.card.runtime_mode.title"),
                    self.i18n.text("settings.advanced.card.runtime_mode.desc"),
                    runtime_mode_toggle.into_any_element(),
                ))
                .child(card(
                    self.i18n.text("settings.advanced.card.file_watcher.title"),
                    self.i18n.text("settings.advanced.card.file_watcher.desc"),
                    watcher_toggle.into_any_element(),
                ))
                .into_any_element()
        };

        let page_content = match self.settings_section {
            SettingsSection::Appearance => appearance_content,
            SettingsSection::About => about_content,
            SettingsSection::Editor => editor_content,
            SettingsSection::Ai => ai_content,
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
            .occlude()
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
                                "settings.nav.ai",
                                ICON_FILE_COG,
                                self.i18n.text("settings.nav.ai"),
                                SettingsSection::Ai,
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
                    .occlude()
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

                    let Some((query, tokens, paths_lower)) = this
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

                            Some((
                                query,
                                split_query_tokens_lowercase(&this.explorer_filter),
                                this.explorer_all_note_paths_lower.clone(),
                            ))
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
                                let matched = if tokens.is_empty() {
                                    path_lower.contains(&query)
                                } else {
                                    tokens.iter().all(|token| path_lower.contains(token))
                                };
                                if matched {
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
                                this.search_groups.clear();
                                this.search_collapsed_paths.clear();
                                this.refresh_search_rows_from_groups();
                                cx.notify();
                                return None;
                            }

                            if let Some(cached) = this.search_query_cache.get(&query).cloned() {
                                this.cache_stats.search_hits =
                                    this.cache_stats.search_hits.wrapping_add(1);
                                this.search_groups = cached;
                                this.search_collapsed_paths.retain(|path| {
                                    this.search_groups.iter().any(|group| group.path == *path)
                                });
                                this.search_selected = 0;
                                this.refresh_search_rows_from_groups();
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
                                this.search_groups.clear();
                                this.search_collapsed_paths.clear();
                                this.refresh_search_rows_from_groups();
                                cx.notify();
                                return None;
                            };

                            let Some(knowledge_index) = this.knowledge_index.clone() else {
                                this.search_selected = 0;
                                this.search_groups.clear();
                                this.search_collapsed_paths.clear();
                                this.refresh_search_rows_from_groups();
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

                    let search_groups: Vec<SearchResultGroup> = cx
                        .background_executor()
                        .spawn(async move {
                            let query_tokens = unique_case_insensitive_tokens(&query_for_task);
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

                                let path_highlights =
                                    collect_highlight_ranges_lowercase(&hit.path, &query_tokens);
                                let mut matches = Vec::new();

                                for preview in hit.previews {
                                    if rows >= max_rows {
                                        break;
                                    }
                                    let preview_text = preview.preview;
                                    let preview_highlights = collect_highlight_ranges_lowercase(
                                        &preview_text,
                                        &query_tokens,
                                    );
                                    matches.push(SearchMatchEntry {
                                        line: preview.line,
                                        preview: preview_text,
                                        preview_highlights,
                                    });
                                    rows += 1;
                                }

                                out.push(SearchResultGroup {
                                    path: hit.path,
                                    match_count: hit.match_count,
                                    path_highlights,
                                    matches,
                                });
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
                            .insert(query.clone(), search_groups.clone());
                        if let Some(evicted) = touch_cache_order(
                            &query,
                            &mut this.search_query_cache_order,
                            SEARCH_QUERY_CACHE_CAPACITY,
                        ) {
                            this.search_query_cache.remove(&evicted);
                        }

                        this.search_groups = search_groups;
                        this.search_collapsed_paths.retain(|path| {
                            this.search_groups.iter().any(|group| group.path == *path)
                        });
                        this.refresh_search_rows_from_groups();
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

                    let Some((query, vault, knowledge_index, search_options, generation, mode)) =
                        this.update(&mut cx, |this, cx| {
                            if this.pending_palette_nonce != nonce {
                                return None;
                            }

                            let query = this.palette_query.trim().to_lowercase();
                            if query.is_empty() {
                                this.palette_selected = 0;
                                this.palette_results.clear();
                                this.palette_search_groups.clear();
                                this.palette_search_collapsed_paths.clear();
                                this.refresh_palette_search_rows_from_groups();
                                cx.notify();
                                return None;
                            }

                            if this.palette_mode == PaletteMode::QuickOpen {
                                if let Some(cached) =
                                    this.quick_open_query_cache.get(&query).cloned()
                                {
                                    this.cache_stats.quick_open_hits =
                                        this.cache_stats.quick_open_hits.wrapping_add(1);
                                    this.palette_selected = 0;
                                    this.palette_results = cached;
                                    this.palette_search_groups.clear();
                                    this.palette_search_collapsed_paths.clear();
                                    this.refresh_palette_search_rows_from_groups();
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
                            }

                            let Some(knowledge_index) = this.knowledge_index.clone() else {
                                this.palette_selected = 0;
                                this.palette_results.clear();
                                this.palette_search_groups.clear();
                                this.palette_search_collapsed_paths.clear();
                                this.refresh_palette_search_rows_from_groups();
                                cx.notify();
                                return None;
                            };

                            Some((
                                query,
                                this.vault(),
                                knowledge_index,
                                this.search_options.clone(),
                                this.index_generation,
                                this.palette_mode,
                            ))
                        })
                        .ok()
                        .flatten()
                    else {
                        return;
                    };

                    let query_for_task = query.clone();

                    let (matched_paths, search_groups): (
                        Vec<OpenPathMatch>,
                        Vec<SearchResultGroup>,
                    ) = cx
                        .background_executor()
                        .spawn(async move {
                            let query_tokens = unique_case_insensitive_tokens(&query_for_task);
                            match mode {
                                PaletteMode::QuickOpen => (
                                    apply_quick_open_weighted_ranking_with_titles(
                                        &query_for_task,
                                        knowledge_index.quick_open_paths(&query_for_task, 300),
                                        &knowledge_index,
                                        200,
                                    ),
                                    Vec::new(),
                                ),
                                PaletteMode::Search => {
                                    let Some(vault) = vault else {
                                        return (Vec::new(), Vec::new());
                                    };
                                    let mut out = Vec::new();
                                    let max_rows = search_options.max_match_rows;
                                    let mut rows = 0usize;
                                    let outcome = knowledge_index.search(
                                        &vault,
                                        &query_for_task,
                                        search_options.clone(),
                                    );
                                    for hit in outcome.hits {
                                        if rows >= max_rows {
                                            break;
                                        }
                                        let path_highlights = collect_highlight_ranges_lowercase(
                                            &hit.path,
                                            &query_tokens,
                                        );
                                        let mut matches = Vec::new();

                                        for preview in hit.previews {
                                            if rows >= max_rows {
                                                break;
                                            }
                                            let preview_text = preview.preview;
                                            let preview_highlights =
                                                collect_highlight_ranges_lowercase(
                                                    &preview_text,
                                                    &query_tokens,
                                                );
                                            matches.push(SearchMatchEntry {
                                                line: preview.line,
                                                preview: preview_text,
                                                preview_highlights,
                                            });
                                            rows += 1;
                                        }

                                        out.push(SearchResultGroup {
                                            path: hit.path,
                                            match_count: hit.match_count,
                                            path_highlights,
                                            matches,
                                        });
                                    }
                                    (Vec::new(), out)
                                }
                                PaletteMode::Commands => (Vec::new(), Vec::new()),
                            }
                        })
                        .await;

                    this.update(&mut cx, |this, cx| {
                        if this.pending_palette_nonce != nonce {
                            return;
                        }
                        if this.index_generation != generation {
                            return;
                        }

                        if mode == PaletteMode::QuickOpen {
                            this.quick_open_query_cache
                                .insert(query.clone(), matched_paths.clone());
                            if let Some(evicted) = touch_cache_order(
                                &query,
                                &mut this.quick_open_query_cache_order,
                                QUICK_OPEN_CACHE_CAPACITY,
                            ) {
                                this.quick_open_query_cache.remove(&evicted);
                            }
                        }

                        this.palette_results = matched_paths;
                        this.palette_search_groups = search_groups;
                        this.palette_search_collapsed_paths.retain(|path| {
                            this.palette_search_groups
                                .iter()
                                .any(|group| group.path == *path)
                        });
                        this.refresh_palette_search_rows_from_groups();
                        let len = match mode {
                            PaletteMode::QuickOpen => this.palette_results.len(),
                            PaletteMode::Search => this.palette_search_results.len(),
                            PaletteMode::Commands => this.filtered_palette_command_indices().len(),
                        };
                        if this.palette_selected >= len {
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
                    let (removed, folder_bookmarks_changed) =
                        self.apply_folder_removed_change(&path);
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
            match expand_note_move_pairs_with_prefix(&existing_paths_vec, &moved_note_pairs) {
                Some(expanded) => moved_note_pairs = expanded,
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
                    self.remember_current_tab_view_state();
                    self.open_note_path = Some(to.clone());
                }
                if self.pinned_editors.remove(from) {
                    self.pinned_editors.insert(to.clone());
                }
                for history in self.editor_group_note_history.values_mut() {
                    for path in history.iter_mut() {
                        if path == from {
                            *path = to.clone();
                        }
                    }
                }
                if self.pending_external_note_reload.as_deref() == Some(from.as_str()) {
                    self.pending_external_note_reload = Some(to.clone());
                }
                self.move_note_content_cache_path(from, to);
                for group in &mut self.editor_groups {
                    if group.note_path.as_deref() == Some(from.as_str()) {
                        group.note_path = Some(to.clone());
                    }
                    for tab in &mut group.tabs {
                        if tab == from {
                            *tab = to.clone();
                        }
                    }
                    if group.pinned_tabs.remove(from) {
                        group.pinned_tabs.insert(to.clone());
                    }
                    for path in group.note_mru.iter_mut() {
                        if path == from {
                            *path = to.clone();
                        }
                    }
                    Self::sanitize_group_interaction_state(group);
                }

                self.refresh_active_group_after_external_layout_mutation();

                if let Some(state) = self.editor_tab_view_state.remove(from) {
                    self.editor_tab_view_state.insert(to.clone(), state);
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
                self.evict_note_content_cache_path(path);
                self.pinned_editors.remove(path);
                for history in self.editor_group_note_history.values_mut() {
                    history.retain(|entry| entry != path);
                }
                if self.pending_external_note_reload.as_deref() == Some(path.as_str()) {
                    self.pending_external_note_reload = None;
                }
                if self.open_note_path.as_deref() == Some(path.as_str()) {
                    self.open_note_path = None;
                    self.open_note_content.clear();
                    self.editor_buffer = None;
                    self.editor_view_mode = EditorViewMode::Edit;
                    self.editor_split_saved_mode = EditorViewMode::Edit;
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
                for group in &mut self.editor_groups {
                    group.tabs.retain(|tab| tab != path);
                    group.pinned_tabs.remove(path);
                    group.note_mru.retain(|existing| existing != path);
                    Self::sanitize_group_interaction_state(group);
                    if group.note_path.as_deref() == Some(path.as_str()) {
                        group.note_path = None;
                    }
                }
                self.refresh_active_group_after_external_layout_mutation();
                self.editor_tab_view_state.remove(path);
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
            self.evict_note_content_cache_path(&path);
            if let Err(err) = next_index.upsert_note(&vault, &path) {
                self.watcher_status.last_error =
                    Some(SharedString::from(format!("watch upsert failed: {err}")));
                self.rescan_vault(cx);
                return;
            }

            if self.open_note_path.as_deref() == Some(path.as_str()) {
                if self.open_note_dirty {
                    self.pending_external_note_reload = Some(path.clone());
                    self.status =
                        SharedString::from("External change pending; save or revert to reload");
                } else {
                    self.pending_external_note_reload = None;
                    self.open_note(path.clone(), cx);
                }
            }
        }

        for path in &new_note_paths {
            self.evict_note_content_cache_path(path);
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
        self.rebuild_explorer_rows(cx);
        self.status = SharedString::from("External note content updated");

        if !self.search_query.trim().is_empty() {
            self.schedule_apply_search(Duration::ZERO, cx);
        }
        if self.palette_open
            && matches!(
                self.palette_mode,
                PaletteMode::QuickOpen | PaletteMode::Search
            )
            && !self.palette_query.trim().is_empty()
        {
            self.schedule_apply_palette_results(Duration::ZERO, cx);
        }
    }

    fn bump_index_generation(&mut self) {
        self.index_generation = self.index_generation.wrapping_add(1);
        self.search_query_cache.clear();
        self.search_query_cache_order.clear();
        self.search_groups.clear();
        self.search_collapsed_paths.clear();
        self.refresh_search_rows_from_groups();
        self.quick_open_query_cache.clear();
        self.quick_open_query_cache_order.clear();
        self.palette_search_groups.clear();
        self.palette_search_collapsed_paths.clear();
        self.refresh_palette_search_rows_from_groups();
        self.recent_palette_quick_open_queries.clear();
        self.recent_palette_search_queries.clear();
        self.recent_panel_search_queries.clear();
        self.recent_explorer_filter_queries.clear();
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

        if self.module_switcher_open {
            let key = ev.keystroke.key.to_lowercase();
            if key == "escape" {
                self.close_module_switcher(cx);
                return;
            }
            if ev.keystroke.modifiers.alt {
                self.handle_module_shortcut_key(&key, cx);
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
                | CommandId::OpenVaultInNewWindow
                | CommandId::FocusExplorer
                | CommandId::FocusSearch
                | CommandId::AiRewriteSelection => {
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
            "enter" | "return" => {
                Self::push_recent_query(
                    &mut self.recent_explorer_filter_queries,
                    self.explorer_filter.trim(),
                );
                cx.notify();
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

        if self.link_picker_open {
            if key == "escape" {
                self.close_link_picker(cx);
            }
            return true;
        }

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

        if self.module_switcher_open {
            let key = ev.keystroke.key.to_lowercase();
            if key == "escape" {
                self.close_module_switcher(cx);
                return;
            }
            if ev.keystroke.modifiers.alt {
                self.handle_module_shortcut_key(&key, cx);
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
                | CommandId::OpenVaultInNewWindow
                | CommandId::FocusExplorer
                | CommandId::FocusSearch
                | CommandId::AiRewriteSelection => {
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
                    self.search_groups.clear();
                    self.search_collapsed_paths.clear();
                    self.refresh_search_rows_from_groups();
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
            "left" => {
                let selected_path = self
                    .search_results
                    .get(self.search_selected)
                    .and_then(|row| match row {
                        SearchRow::File { path, .. } => Some(path.clone()),
                        _ => None,
                    });
                if let Some(path) = selected_path {
                    if !self.search_collapsed_paths.contains(&path) {
                        self.toggle_search_group_collapsed(&path);
                        cx.notify();
                    }
                }
            }
            "right" => {
                let selected_path = self
                    .search_results
                    .get(self.search_selected)
                    .and_then(|row| match row {
                        SearchRow::File { path, .. } => Some(path.clone()),
                        _ => None,
                    });
                if let Some(path) = selected_path {
                    if self.search_collapsed_paths.contains(&path) {
                        self.toggle_search_group_collapsed(&path);
                        cx.notify();
                    }
                }
            }
            "enter" | "return" => {
                if let Some(row) = self.search_results.get(self.search_selected).cloned() {
                    Self::push_recent_query(
                        &mut self.recent_panel_search_queries,
                        self.search_query.trim(),
                    );
                    match row {
                        SearchRow::File { path, .. } => {
                            self.toggle_search_group_collapsed(&path);
                            cx.notify();
                        }
                        SearchRow::Match { path, line, .. } => {
                            self.open_note_at_line(path, line, cx);
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
                | CommandId::OpenVaultInNewWindow
                | CommandId::ReloadVault
                | CommandId::Settings
                | CommandId::NewNote
                | CommandId::SaveFile
                | CommandId::Undo
                | CommandId::Redo
                | CommandId::ToggleSplit
                | CommandId::FocusExplorer
                | CommandId::FocusSearch
                | CommandId::AiRewriteSelection => {
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
                    self.palette_search_groups.clear();
                    self.palette_search_collapsed_paths.clear();
                    self.refresh_palette_search_rows_from_groups();
                    self.pending_palette_nonce = 0;
                    cx.notify();
                    return;
                }
                "p" => {
                    self.palette_mode = PaletteMode::QuickOpen;
                    self.palette_query.clear();
                    self.palette_selected = 0;
                    self.palette_results.clear();
                    self.palette_search_groups.clear();
                    self.palette_search_collapsed_paths.clear();
                    self.refresh_palette_search_rows_from_groups();
                    self.pending_palette_nonce = 0;
                    cx.notify();
                    return;
                }
                "f" => {
                    self.palette_mode = PaletteMode::Search;
                    self.palette_query.clear();
                    self.palette_selected = 0;
                    self.palette_results.clear();
                    self.palette_search_groups.clear();
                    self.palette_search_collapsed_paths.clear();
                    self.refresh_palette_search_rows_from_groups();
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
                                    PaletteMode::QuickOpen | PaletteMode::Search => {
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
            PaletteMode::Search => self.palette_search_results.len(),
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
            "left" => {
                if self.palette_mode == PaletteMode::Search {
                    let selected_path = self
                        .palette_search_results
                        .get(self.palette_selected)
                        .and_then(|row| match row {
                            SearchRow::File { path, .. } => Some(path.clone()),
                            _ => None,
                        });
                    if let Some(path) = selected_path {
                        if !self.palette_search_collapsed_paths.contains(&path) {
                            self.toggle_palette_search_group_collapsed(&path);
                            cx.notify();
                        }
                    }
                }
            }
            "right" => {
                if self.palette_mode == PaletteMode::Search {
                    let selected_path = self
                        .palette_search_results
                        .get(self.palette_selected)
                        .and_then(|row| match row {
                            SearchRow::File { path, .. } => Some(path.clone()),
                            _ => None,
                        });
                    if let Some(path) = selected_path {
                        if self.palette_search_collapsed_paths.contains(&path) {
                            self.toggle_palette_search_group_collapsed(&path);
                            cx.notify();
                        }
                    }
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
                    Self::push_recent_query(
                        &mut self.recent_palette_quick_open_queries,
                        self.palette_query.trim(),
                    );
                    let path = open_match.path.clone();
                    self.close_palette(cx);
                    self.open_note(path, cx);
                }
                PaletteMode::Search => {
                    let Some(row) = self
                        .palette_search_results
                        .get(self.palette_selected)
                        .cloned()
                    else {
                        return;
                    };
                    Self::push_recent_query(
                        &mut self.recent_palette_search_queries,
                        self.palette_query.trim(),
                    );
                    match row {
                        SearchRow::File { path, .. } => {
                            self.toggle_palette_search_group_collapsed(&path);
                            cx.notify();
                        }
                        SearchRow::Match { path, line, .. } => {
                            self.close_palette(cx);
                            self.open_note_at_line(path, line, cx);
                        }
                    }
                }
            },
            "backspace" => {
                if self.palette_query.pop().is_some() {
                    self.palette_selected = 0;
                    match self.palette_mode {
                        PaletteMode::Commands => cx.notify(),
                        PaletteMode::QuickOpen | PaletteMode::Search => {
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
                    PaletteMode::QuickOpen | PaletteMode::Search => {
                        self.schedule_apply_palette_results(Duration::from_millis(60), cx);
                        cx.notify();
                    }
                }
            }
        }
    }

    fn rebuild_explorer_rows(&mut self, _cx: &mut Context<Self>) {
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
        self.rebuild_explorer_rows(cx);
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
        self.watch_scan_fingerprint =
            compute_entries_fingerprint(self.explorer_all_note_paths.as_ref());

        self.rebuild_explorer_rows(cx);
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

        self.rebuild_explorer_rows(cx);
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
            self.rebuild_explorer_rows(cx);

            if self.is_filtering() {
                self.schedule_apply_filter(Duration::ZERO, cx);
            }
            if !self.search_query.trim().is_empty() {
                self.schedule_apply_search(Duration::ZERO, cx);
            }
            if self.palette_open
                && matches!(
                    self.palette_mode,
                    PaletteMode::QuickOpen | PaletteMode::Search
                )
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
            .filter(|path| path == &folder || path.starts_with(&format!("{folder}/")))
            .cloned()
            .collect::<Vec<_>>();

        for note_path in &notes_to_remove {
            remove_note_from_tree_structures(
                &note_path,
                &mut self.explorer_folder_children,
                &mut self.folder_notes,
            );
            self.pinned_editors.remove(note_path);
            self.evict_note_content_cache_path(note_path);
            for history in self.editor_group_note_history.values_mut() {
                history.retain(|existing| existing != note_path);
            }
            if self.pending_external_note_reload.as_deref() == Some(note_path.as_str()) {
                self.pending_external_note_reload = None;
            }
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
            for group in &mut self.editor_groups {
                group.tabs.retain(|tab| tab != note_path);
                group.pinned_tabs.remove(note_path);
                group.note_mru.retain(|existing| existing != note_path);
                Self::sanitize_group_interaction_state(group);
                if group.note_path.as_deref() == Some(note_path.as_str()) {
                    group.note_path = None;
                }
            }
            self.editor_tab_view_state.remove(note_path);
            self.app_settings.bookmarked_notes.retain(|bookmarked| {
                let keep = bookmarked != note_path;
                if !keep {
                    bookmarks_changed = true;
                }
                keep
            });
        }

        let active_snapshot = self.active_group().map(|group| {
            (
                group.tabs.clone(),
                group.note_path.is_none(),
                group.view_state.sanitize(),
            )
        });
        if let Some((tabs, note_empty, saved)) = active_snapshot {
            self.open_editors = tabs;
            if note_empty {
                self.editor_view_mode = saved.mode;
                self.editor_split_ratio = saved.split_ratio;
                self.editor_split_direction = saved.split_direction;
                self.editor_split_saved_mode = saved.split_saved_mode;
            }
        }
        self.apply_active_group_interaction_state();
        self.reorder_open_editors_with_pins();
        self.sync_active_group_tabs_from_open_editors();

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
                .filter(|path| !(path == &folder || path.starts_with(&format!("{folder}/"))))
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
                self.remember_current_tab_view_state();
                self.open_note_path = Some(new_path.clone());
            }
            for group in &mut self.editor_groups {
                if group.note_path.as_deref() == Some(old_path.as_str()) {
                    group.note_path = Some(new_path.clone());
                }
                for tab in &mut group.tabs {
                    if tab == old_path {
                        *tab = new_path.clone();
                    }
                }
                if group.pinned_tabs.remove(old_path) {
                    group.pinned_tabs.insert(new_path.clone());
                }
                for entry in group.note_mru.iter_mut() {
                    if entry == old_path {
                        *entry = new_path.clone();
                    }
                }
                Self::sanitize_group_interaction_state(group);
            }
            if self.selected_note.as_deref() == Some(old_path.as_str()) {
                self.selected_note = Some(new_path.clone());
            }
            if self.pinned_editors.remove(old_path) {
                self.pinned_editors.insert(new_path.clone());
            }
            self.move_note_content_cache_path(old_path, new_path);
            if self.pending_external_note_reload.as_deref() == Some(old_path.as_str()) {
                self.pending_external_note_reload = Some(new_path.clone());
            }
            for history in self.editor_group_note_history.values_mut() {
                for entry in history.iter_mut() {
                    if entry == old_path {
                        *entry = new_path.clone();
                    }
                }
                history.retain(|entry| !entry.is_empty());
            }
            if let Some(state) = self.editor_tab_view_state.remove(old_path) {
                self.editor_tab_view_state.insert(new_path.clone(), state);
            }
            for bookmarked in &mut self.app_settings.bookmarked_notes {
                if bookmarked == old_path {
                    *bookmarked = new_path.clone();
                    bookmarks_changed = true;
                }
            }
        }

        self.refresh_active_group_after_external_layout_mutation();

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

        self.ensure_active_group_exists();

        self.remember_current_tab_view_state();

        self.pending_open_note_cursor = self
            .pending_open_note_cursor
            .take()
            .filter(|(path, _line)| path == note_path.as_str());

        self.selected_note = Some(note_path.clone());
        self.selected_explorer_folder = Some(folder_of_note_path(&note_path));
        if self.expand_note_ancestors(&note_path) {
            self.rebuild_explorer_rows(cx);
        }
        if !self.open_editors.iter().any(|p| p == &note_path) {
            self.open_editors.push(note_path.clone());
        }
        self.sync_active_group_tabs_from_open_editors();
        self.open_note_path = Some(note_path.clone());
        self.sync_active_group_note_path();
        self.record_group_note_history(self.active_editor_group_id, &note_path);
        self.sync_active_group_tabs_from_open_editors();
        let cached_content = self.cached_note_content(&note_path);
        let had_cached_content = cached_content.is_some();
        self.open_note_loading = !had_cached_content;
        self.open_note_dirty = false;
        self.open_note_content.clear();
        self.editor_buffer = None;
        self.restore_tab_view_state_or_default(&note_path);
        self.edit_latency_stats = EditLatencyStats::default();
        self.open_note_heading_count = 0;
        self.open_note_link_count = 0;
        self.open_note_code_fence_count = 0;
        self.open_note_id = None;
        self.open_note_meta = None;
        self.open_note_meta_loading = false;
        self.pending_note_meta_load_nonce = 0;
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
        self.pending_ai_rewrite_nonce = 0;

        self.next_note_open_nonce = self.next_note_open_nonce.wrapping_add(1);
        let open_nonce = self.next_note_open_nonce;
        self.current_note_open_nonce = open_nonce;

        if let Some(content) = cached_content {
            self.apply_loaded_note_content(&note_path, content, cx);
            self.status = SharedString::from("Ready");
        } else {
            self.status = SharedString::from(format!("Loading note: {note_path}"));
        }
        self.sync_active_group_note_path();
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
                                this.apply_loaded_note_content(&note_path, content, cx);
                                this.status = SharedString::from("Ready");
                            }
                            Err(err) => {
                                if !had_cached_content {
                                    this.open_note_content = format!("Failed to load note: {err}");
                                    this.editor_buffer =
                                        Some(EditorBuffer::new(&this.open_note_content));
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
                                } else {
                                    this.status = SharedString::from(format!(
                                        "Read failed; showing cached content: {err}"
                                    ));
                                }
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
        let offset = previous_char_boundary(&self.open_note_content, offset);
        self.editor_selected_range = offset..offset;
        self.editor_selection_reversed = false;
        cx.notify();
    }

    fn editor_select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = previous_char_boundary(&self.open_note_content, offset);
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
        let offset = previous_char_boundary(&self.open_note_content, offset);
        let prefix = &self.open_note_content[..offset];
        match prefix.rfind('\n') {
            Some(ix) => ix + 1,
            None => 0,
        }
    }

    fn editor_line_end(&self, offset: usize) -> usize {
        let offset = previous_char_boundary(&self.open_note_content, offset);
        let suffix = &self.open_note_content[offset..];
        match suffix.find('\n') {
            Some(rel) => offset + rel,
            None => self.open_note_content.len(),
        }
    }

    fn editor_index_for_point(&self, position: Point<Pixels>) -> Option<usize> {
        let layout = self.editor_layout.as_ref()?;
        match layout.index_for_position(position) {
            Ok(ix) | Err(ix) => Some(previous_char_boundary(
                &self.open_note_content,
                ix.min(self.open_note_content.len()),
            )),
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

        self.apply_editor_transaction(range, new_text, EditorMutationSource::Keyboard, false, cx);
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

        if matches!(source, EditorMutationSource::Keyboard)
            && new_text == "["
            && cursor >= 2
            && self.open_note_content.get(cursor - 2..cursor) == Some("[[")
        {
            self.open_link_picker((cursor - 2)..cursor, cx);
            return;
        }

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
                    xnote_core::markdown::MarkdownBlockKind::Quote => {
                        MarkdownPreviewBlockKind::Quote
                    }
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
        let previous = self.editor_view_mode;
        if mode == EditorViewMode::Split {
            if previous != EditorViewMode::Split {
                self.editor_split_saved_mode = previous;
            }
        } else if mode != EditorViewMode::Split {
            self.editor_split_saved_mode = mode;
        }

        self.editor_view_mode = mode;
        self.remember_current_tab_view_state();
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
        self.split_active_editor_group(cx);
    }

    fn on_active_split_drag_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.on_splitter_drag_mouse_move(event, window, cx);
    }

    fn on_active_split_drag_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.on_splitter_drag_mouse_up(event, window, cx);
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

        if event.modifiers.control || event.modifiers.platform {
            if self.open_link_under_editor_point(event.position, cx) {
                self.editor_is_selecting = false;
                return;
            }
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

    fn on_ai_hub_input_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.to_lowercase();
        let ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
        let shift = ev.keystroke.modifiers.shift;

        match key.as_str() {
            "enter" | "return" => {
                if !shift {
                    self.ai_hub_submit_input(cx);
                } else {
                    if self.ai_hub_insert_text_at_cursor("\n") {
                        cx.notify();
                    }
                }
            }
            "escape" => {
                self.ai_chat_input.clear();
                self.ai_hub_cursor_offset = 0;
                self.ai_hub_cursor_preferred_col = None;
                cx.notify();
            }
            "backspace" => {
                if self.ai_hub_delete_backward_char() {
                    cx.notify();
                }
            }
            "delete" => {
                if self.ai_hub_delete_forward_char() {
                    cx.notify();
                }
            }
            "left" => {
                self.ai_hub_normalize_cursor();
                let cursor = self.ai_hub_cursor_offset;
                let next = previous_char_boundary(&self.ai_chat_input, cursor.saturating_sub(1));
                if next != cursor {
                    self.ai_hub_cursor_offset = next;
                    self.ai_hub_cursor_preferred_col = None;
                    cx.notify();
                }
            }
            "right" => {
                self.ai_hub_normalize_cursor();
                let cursor = self.ai_hub_cursor_offset;
                let next = next_char_boundary(&self.ai_chat_input, cursor.saturating_add(1));
                if next != cursor {
                    self.ai_hub_cursor_offset = next.min(self.ai_chat_input.len());
                    self.ai_hub_cursor_preferred_col = None;
                    cx.notify();
                }
            }
            "home" => {
                self.ai_hub_normalize_cursor();
                let next = self.ai_hub_line_start(self.ai_hub_cursor_offset);
                if next != self.ai_hub_cursor_offset {
                    self.ai_hub_cursor_offset = next;
                    self.ai_hub_cursor_preferred_col = None;
                    cx.notify();
                }
            }
            "end" => {
                self.ai_hub_normalize_cursor();
                let next = self.ai_hub_line_end(self.ai_hub_cursor_offset);
                if next != self.ai_hub_cursor_offset {
                    self.ai_hub_cursor_offset = next;
                    self.ai_hub_cursor_preferred_col = None;
                    cx.notify();
                }
            }
            "up" => {
                if self.ai_hub_move_cursor_vertical(-1) {
                    cx.notify();
                }
            }
            "down" => {
                if self.ai_hub_move_cursor_vertical(1) {
                    cx.notify();
                }
            }
            _ => {
                if ctrl {
                    match key.as_str() {
                        "a" => {
                            self.ai_hub_cursor_offset = 0;
                            self.ai_hub_cursor_preferred_col = None;
                            cx.notify();
                        }
                        "e" => {
                            self.ai_hub_cursor_offset = self.ai_chat_input.len();
                            self.ai_hub_cursor_preferred_col = None;
                            cx.notify();
                        }
                        "v" => {
                            if let Some(text) =
                                cx.read_from_clipboard().and_then(|item| item.text())
                            {
                                let text = text.replace("\r\n", "\n");
                                if self.ai_hub_insert_text_at_cursor(&text) {
                                    cx.notify();
                                }
                            }
                        }
                        _ => {}
                    }
                    return;
                }

                if ctrl {
                    return;
                }
                let Some(text) = ev.keystroke.key_char.as_ref() else {
                    return;
                };
                if text.is_empty() {
                    return;
                }
                if self.ai_hub_insert_text_at_cursor(text) {
                    cx.notify();
                }
            }
        }
    }

    fn on_editor_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
        let shift = ev.keystroke.modifiers.shift;
        let alt = ev.keystroke.modifiers.alt;

        let key = ev.keystroke.key.to_lowercase();

        if self.link_picker_open {
            match key.as_str() {
                "escape" => {
                    self.close_link_picker(cx);
                }
                "up" => {
                    if self.link_picker_selected > 0 {
                        self.link_picker_selected -= 1;
                        cx.notify();
                    }
                }
                "down" => {
                    if self.link_picker_selected + 1 < self.link_picker_results.len() {
                        self.link_picker_selected += 1;
                        cx.notify();
                    }
                }
                "enter" | "return" => {
                    self.insert_link_picker_selection(cx);
                }
                "backspace" => {
                    if self.link_picker_query.pop().is_some() {
                        self.link_picker_selected = 0;
                        self.schedule_apply_link_picker_results(Duration::ZERO, cx);
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
                    self.link_picker_query.push_str(text);
                    self.link_picker_selected = 0;
                    self.schedule_apply_link_picker_results(Duration::ZERO, cx);
                }
            }
            return;
        }

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

        if self.module_switcher_open {
            if key == "escape" {
                self.close_module_switcher(cx);
                return;
            }
            if alt {
                self.handle_module_shortcut_key(&key, cx);
            }
            return;
        }

        if let Some(command) = self.command_from_event(ev) {
            match command {
                CommandId::CommandPalette
                | CommandId::QuickOpen
                | CommandId::OpenVault
                | CommandId::OpenVaultInNewWindow
                | CommandId::ReloadVault
                | CommandId::NewNote
                | CommandId::Settings
                | CommandId::ToggleSplit
                | CommandId::SaveFile
                | CommandId::Undo
                | CommandId::Redo
                | CommandId::FocusExplorer
                | CommandId::FocusSearch
                | CommandId::AiRewriteSelection => {
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
                "tab" => {
                    if shift {
                        self.focus_last_editor_group(cx);
                    } else {
                        let group_id = self.active_editor_group_id;
                        self.swap_group_note_history(group_id, cx);
                    }
                    return;
                }
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

        if ctrl && shift {
            match key.as_str() {
                "\\" => {
                    self.split_active_group_to_new_note(cx);
                    return;
                }
                "w" => {
                    self.close_editor_group(self.active_editor_group_id, cx);
                    return;
                }
                "p" => {
                    if let Some(path) = self.open_note_path.clone() {
                        self.toggle_pin_editor(&path, cx);
                    }
                    return;
                }
                _ => {}
            }
        }

        if alt {
            match key.as_str() {
                "left" => {
                    self.focus_previous_editor_group(cx);
                    return;
                }
                "right" => {
                    self.focus_next_editor_group(cx);
                    return;
                }
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

        if ctrl && alt {
            match key.as_str() {
                "right" => {
                    self.move_current_editor_to_next_group(cx);
                    return;
                }
                "left" => {
                    self.focus_last_editor_group(cx);
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
            "tab" if !ctrl && !alt => self.editor_replace_selection("\t", cx),
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

                    let persisted_content = content_to_save.clone();

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
                                this.cache_note_content(&note_path, persisted_content);
                                this.reopen_external_current_note(cx);
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

        if self.reorder_folder(target_folder, &dragged.path, target_path, insert_after, cx) {
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
        cx: &mut Context<Self>,
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

        self.rebuild_explorer_rows(cx);
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
                let gutter_digits =
                    line_number_digits(max_line_number).max(EDITOR_GUTTER_STABLE_DIGITS_MAX_9999);
                let gutter_width = px(editor_gutter_width_for_digits(gutter_digits));
                let text_x_offset = editor_text_x_offset(gutter_width);
                let text_wrap_width =
                    wrap_width.map(|w| (w - text_x_offset).max(px(EDITOR_TEXT_MIN_WRAP_WIDTH)));

                if let Some(inner) = element_state.state.borrow().as_ref() {
                    if inner.size.is_some() && wrap_width == inner.wrap_width {
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
                            EditorHighlightKind::HeadingMarker => {
                                rgb(ui_theme.syntax_heading_marker)
                            }
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

                let lines = match window.text_system().shape_text(
                    text.clone(),
                    font_size,
                    &runs,
                    text_wrap_width,
                    None,
                ) {
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

                element_state
                    .state
                    .borrow_mut()
                    .replace(NoteEditorLayoutInner {
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
                let line_number = inner
                    .logical_line_numbers
                    .get(line_ix)
                    .copied()
                    .unwrap_or(1);
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

struct TooltipPreview {
    label: SharedString,
    ui_theme: UiTheme,
}

impl Render for TooltipPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(self.ui_theme.surface_bg))
            .border_1()
            .border_color(rgb(self.ui_theme.border))
            .text_color(rgb(self.ui_theme.text_primary))
            .font_family("IBM Plex Mono")
            .text_size(px(11.))
            .child(self.label.clone())
    }
}

impl Render for XnoteWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_theme = UiTheme::from_settings(self.settings_theme, self.settings_accent);

        self.schedule_non_active_group_preview_loads(cx);

        if self.active_module == WorkstationModule::AiHub && self.ai_hub_input_needs_focus {
            window.focus(&self.ai_hub_input_focus_handle);
            self.ai_hub_input_needs_focus = false;
        }

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
                    this.module_switcher_open = false;
                    this.module_switcher_backdrop_armed_until = None;
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
        let panel_min_w = px(PANEL_SHELL_MIN_WIDTH);
        let workspace_min_w = px(WORKSPACE_MIN_WIDTH);

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
        let editor_surface_width =
            (window_w - rail_w - panel_shell_width - workspace_width - splitter_w * splitter_count)
                .max(editor_min_w);

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

        self.schedule_window_layout_persist_if_changed(window, cx);

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
                        .gap(px(6.))
                        .overflow_hidden()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .font_family("IBM Plex Mono")
                                        .text_size(px(10.))
                                        .font_weight(FontWeight(900.))
                                        .text_color(rgb(ui_theme.text_secondary))
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child("EXPLORER"),
                                ),
                        )
                        .child(
                            div()
                                .id("panel_shell.collapse.explorer")
                                .h(px(EXPLORER_HEADER_ACTION_SIZE))
                                .w(px(EXPLORER_HEADER_ACTION_SIZE))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                                .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                                    this.set_panel_shell_collapsed(true, cx);
                                }))
                                .child(ui_icon(
                                    ICON_PANEL_LEFT_CLOSE,
                                    EXPLORER_HEADER_ICON_SIZE,
                                    ui_theme.text_muted,
                                )),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .id("explorer.new_file")
                                        .h(px(EXPLORER_HEADER_ACTION_SIZE))
                                        .w(px(EXPLORER_HEADER_ACTION_SIZE))
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
                                        .child(ui_icon(
                                            ICON_FILE_PLUS,
                                            EXPLORER_HEADER_ICON_SIZE,
                                            ui_theme.text_muted,
                                        )),
                                )
                                .child(
                                    div()
                                        .id("explorer.new_folder")
                                        .h(px(EXPLORER_HEADER_ACTION_SIZE))
                                        .w(px(EXPLORER_HEADER_ACTION_SIZE))
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
                                        .child(ui_icon(
                                            ICON_FOLDER_PLUS,
                                            EXPLORER_HEADER_ICON_SIZE,
                                            ui_theme.text_muted,
                                        )),
                                )
                                .child(
                                    div()
                                        .id("explorer.refresh")
                                        .h(px(EXPLORER_HEADER_ACTION_SIZE))
                                        .w(px(EXPLORER_HEADER_ACTION_SIZE))
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
                                        .child(ui_icon(
                                            ICON_REFRESH_CW,
                                            EXPLORER_HEADER_ICON_SIZE,
                                            ui_theme.text_muted,
                                        )),
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
                                    format!(
                                        "Filter: {} ({})",
                                        self.explorer_filter,
                                        self.explorer_rows_filtered.len()
                                    )
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
                        .overflow_hidden()
                        .bg(rgb(ui_theme.surface_alt_bg))
                        .py(px(10.))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _ev, _window, cx| this.clear_drag_over(cx)),
                        )
                        .child(
                            div()
                                .w_full()
                                .h_full()
                                .child(uniform_list(
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
                                                let tooltip_text = path.clone();

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
                        .tooltip({
                            let tooltip_text = tooltip_text.clone();
                            let tooltip_theme = ui_theme;
                            move |_window, cx| {
                                AnyView::from(cx.new(|_| TooltipPreview {
                                    label: tooltip_text.clone().into(),
                                    ui_theme: tooltip_theme,
                                }))
                            }
                        })
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
                                                let tooltip_text = root_name.clone();
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
                        .tooltip({
                            let tooltip_text = tooltip_text.clone();
                            let tooltip_theme = ui_theme;
                            move |_window, cx| {
                                AnyView::from(cx.new(|_| TooltipPreview {
                                    label: tooltip_text.clone().into(),
                                    ui_theme: tooltip_theme,
                                }))
                            }
                        })
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
                                                .tooltip({
                                                    let tooltip_text = text.clone();
                                                    let tooltip_theme = ui_theme;
                                                    move |_window, cx| {
                                                        AnyView::from(cx.new(|_| TooltipPreview {
                                                            label: tooltip_text.clone(),
                                                            ui_theme: tooltip_theme,
                                                        }))
                                                    }
                                                })
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
                                                let tooltip_text = folder.clone();
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
                        .tooltip({
                            let tooltip_text = tooltip_text.clone();
                            let tooltip_theme = ui_theme;
                            move |_window, cx| {
                                AnyView::from(cx.new(|_| TooltipPreview {
                                    label: tooltip_text.clone().into(),
                                    ui_theme: tooltip_theme,
                                }))
                            }
                        })
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
                                                let tooltip_text = path.clone();
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
                        .tooltip({
                            let tooltip_text = tooltip_text.clone();
                            let tooltip_theme = ui_theme;
                            move |_window, cx| {
                                AnyView::from(cx.new(|_| TooltipPreview {
                                    label: tooltip_text.clone().into(),
                                    ui_theme: tooltip_theme,
                                }))
                            }
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
                                        .h_full()),
                        ),
                );

        let search_vault_label = match &self.vault_state {
            VaultState::Opened { root_name, .. } => root_name.clone(),
            _ => SharedString::from("None"),
        };
        let search_panel_hint = if self.search_query.trim().is_empty() {
            if let Some(recent) = self.recent_panel_search_queries.front() {
                SharedString::from(format!(
                    "Type to search… · Esc clear/close · Enter open · Recent: {recent}"
                ))
            } else {
                SharedString::from("Type to search… · Esc clear/close · Enter open")
            }
        } else {
            SharedString::from("Esc clear/close · ↑/↓ navigate · Enter open")
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
              )
              .child(
                div()
                  .font_family("IBM Plex Mono")
                  .text_size(px(10.))
                  .font_weight(FontWeight(650.))
                  .text_color(rgb(ui_theme.text_muted))
                  .child(search_panel_hint),
              )
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
                      let recent_hint = this
                        .recent_panel_search_queries
                        .front()
                        .map(|q| SharedString::from(format!("Recent: {q}")));
                      return range
                        .map(|ix| {
                          div()
                            .id(ElementId::named_usize("search.placeholder", ix))
                            .h(px(22.))
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child(
                              if ix == 0 {
                                SharedString::from("Type to search…")
                              } else if ix == 1 {
                                recent_hint
                                  .clone()
                                  .unwrap_or_else(|| SharedString::from(""))
                              } else {
                                SharedString::from("")
                              }
                            )
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
                          SearchRow::File {
                            path,
                            match_count,
                            path_highlights,
                          } => {
                            let row_path = path.clone();
                            let toggle_path = path.clone();
                            let is_collapsed = this.search_collapsed_paths.contains(path);
                            let count_label = SharedString::from(format!("{match_count}"));

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
                                this.toggle_search_group_collapsed(&toggle_path);
                                cx.notify();
                              }))
                              .tooltip({
                                let tooltip_path = row_path.clone();
                                let tooltip_theme = ui_theme;
                                move |_window, cx| {
                                  AnyView::from(cx.new(|_| TooltipPreview {
                                    label: tooltip_path.clone().into(),
                                    ui_theme: tooltip_theme,
                                  }))
                                }
                              })
                              .child(ui_icon(
                                if is_collapsed {
                                  ICON_CHEVRON_RIGHT
                                } else {
                                  ICON_CHEVRON_DOWN
                                },
                                14.,
                                ui_theme.text_muted,
                              ))
                              .child(
                                div()
                                  .min_w_0()
                                  .flex_1()
                                  .overflow_hidden()
                                  .flex()
                                  .items_center()
                                  .font_family("IBM Plex Mono")
                                  .text_size(px(11.))
                                  .font_weight(FontWeight(900.))
                                  .whitespace_nowrap()
                                  .text_ellipsis()
                                  .children(render_highlighted_segments(
                                    path,
                                    path_highlights,
                                    ui_theme.text_primary,
                                    ui_theme.accent,
                                    FontWeight(900.),
                                    FontWeight(950.),
                                  )),
                              )
                              .child(
                                div()
                                  .font_family("IBM Plex Mono")
                                  .text_size(px(10.))
                                  .font_weight(FontWeight(700.))
                                  .text_color(rgb(ui_theme.text_muted))
                                  .child(count_label),
                              )
                          }
                          SearchRow::Match {
                            path,
                            line,
                            preview,
                            preview_highlights,
                          } => {
                            let row_path = path.clone();
                            let path_for_click = path.clone();
                            let line_for_click = *line;
                            let line_label = SharedString::from(format!("{line}"));

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
                              .tooltip({
                                let tooltip_text = SharedString::from(format!(
                                  "{}:{} {}",
                                  row_path,
                                  line,
                                  preview
                                ));
                                let tooltip_theme = ui_theme;
                                move |_window, cx| {
                                  AnyView::from(cx.new(|_| TooltipPreview {
                                    label: tooltip_text.clone(),
                                    ui_theme: tooltip_theme,
                                  }))
                                }
                              })
                              .child(
                                div()
                                  .font_family("IBM Plex Mono")
                                  .text_size(px(10.))
                                  .font_weight(FontWeight(700.))
                                  .text_color(rgb(ui_theme.text_muted))
                                  .child(line_label),
                              )
                              .child(
                                div()
                                  .min_w_0()
                                  .flex_1()
                                  .overflow_hidden()
                                  .flex()
                                  .items_center()
                                  .font_family("IBM Plex Mono")
                                  .text_size(px(10.))
                                  .whitespace_nowrap()
                                  .text_ellipsis()
                                  .children(render_highlighted_segments(
                                    preview,
                                    preview_highlights,
                                    ui_theme.text_secondary,
                                    ui_theme.accent,
                                    FontWeight(650.),
                                    FontWeight(850.),
                                  )),
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
            let mut note_meta_relations_list = div().w_full().flex().flex_col().gap(px(2.));
            let mut note_meta_pins_list = div().w_full().flex().flex_col().gap(px(2.));
            let mut links_count = 0usize;
            let mut backlinks_count = 0usize;
            let mut note_meta_relations_count = 0usize;
            let mut note_meta_pins_count = 0usize;
            let mut note_id_value = self.open_note_id.clone().unwrap_or_else(|| "-".to_string());

            if let Some(open_path) = self.open_note_path.as_deref() {
                if let Some(index) = self.knowledge_index.as_ref() {
                    if let Some(summary) = index.note_summary(open_path) {
                        if let Some(note_id) = summary.note_id.clone() {
                            note_id_value = note_id;
                        }
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
                                    let target_for_relation = target_path.clone();
                                    let target_for_pin = target_path.clone();
                                    links_list.child(
                                        row.cursor_pointer()
                                            .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                                            .on_click(cx.listener(
                                                move |this, _ev: &ClickEvent, _window, cx| {
                                                    this.open_note(target_path.clone(), cx);
                                                },
                                            ))
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(move |this, _ev, _window, cx| {
                                                    this.add_relation_from_link_target(
                                                        &target_for_relation,
                                                        cx,
                                                    );
                                                }),
                                            )
                                            .on_mouse_up(
                                                MouseButton::Middle,
                                                cx.listener(move |this, _ev, _window, cx| {
                                                    this.toggle_pin_note_from_link_target(
                                                        &target_for_pin,
                                                        cx,
                                                    );
                                                }),
                                            ),
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

                    note_meta_relations_list = note_meta_relations_list.child(
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
                    note_meta_pins_list = note_meta_pins_list.child(
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

                    if self.open_note_meta_loading {
                        note_meta_relations_list = note_meta_relations_list.child(
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
                                .child("Loading note metadata..."),
                        );
                        note_meta_pins_list = note_meta_pins_list.child(
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
                                .child("Loading note metadata..."),
                        );
                    } else if let Some(meta) = self.open_note_meta.as_ref() {
                        note_meta_relations_count = meta.relations.len();
                        let mut pin_count = 0usize;
                        pin_count = pin_count.saturating_add(meta.pins.notes.len());
                        pin_count = pin_count.saturating_add(meta.pins.resources.len());
                        pin_count = pin_count.saturating_add(meta.pins.infos.len());
                        note_meta_pins_count = pin_count;

                        if meta.relations.is_empty() {
                            note_meta_relations_list = note_meta_relations_list.child(
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
                                    .child("No typed relations"),
                            );
                        } else {
                            for relation in meta.relations.iter().take(32) {
                                let relation_label = format!(
                                    "{} -> {}:{}",
                                    relation.relation_type, relation.to.kind, relation.to.id
                                );
                                note_meta_relations_list = note_meta_relations_list.child(
                                    div()
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
                                                .text_color(rgb(ui_theme.text_secondary))
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .child(relation_label),
                                        ),
                                );
                            }
                        }

                        if note_meta_pins_count == 0 {
                            note_meta_pins_list = note_meta_pins_list.child(
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
                                    .child("No pins"),
                            );
                        } else {
                            for note_pin in meta.pins.notes.iter().take(16) {
                                let label = format!("note:{note_pin}");
                                note_meta_pins_list = note_meta_pins_list.child(
                                    div()
                                        .h(px(24.))
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_2()
                                        .bg(rgb(ui_theme.surface_alt_bg))
                                        .border_1()
                                        .border_color(rgb(ui_theme.border))
                                        .child(ui_icon(ICON_BOOKMARK, 12., ui_theme.accent))
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
                                                .child(label),
                                        ),
                                );
                            }
                            for resource_pin in meta.pins.resources.iter().take(16) {
                                let label = format!("resource:{resource_pin}");
                                note_meta_pins_list = note_meta_pins_list.child(
                                    div()
                                        .h(px(24.))
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_2()
                                        .bg(rgb(ui_theme.surface_alt_bg))
                                        .border_1()
                                        .border_color(rgb(ui_theme.border))
                                        .child(ui_icon(ICON_BOOKMARK, 12., ui_theme.accent))
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
                                                .child(label),
                                        ),
                                );
                            }
                            for info_pin in meta.pins.infos.iter().take(16) {
                                let label = format!("info:{info_pin}");
                                note_meta_pins_list = note_meta_pins_list.child(
                                    div()
                                        .h(px(24.))
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_2()
                                        .bg(rgb(ui_theme.surface_alt_bg))
                                        .border_1()
                                        .border_color(rgb(ui_theme.border))
                                        .child(ui_icon(ICON_BOOKMARK, 12., ui_theme.accent))
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
                                                .child(label),
                                        ),
                                );
                            }
                        }
                    } else {
                        note_meta_relations_list = note_meta_relations_list.child(
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
                                .child("No note metadata"),
                        );
                        note_meta_pins_list = note_meta_pins_list.child(
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
                                .child("No note metadata"),
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
                note_meta_relations_list = note_meta_relations_list.child(
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
                        .child("Open a note to inspect metadata"),
                );
                note_meta_pins_list = note_meta_pins_list.child(
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
                        .child("Open a note to inspect metadata"),
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
                .child(
                    div()
                        .mt_2()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(800.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child(SharedString::from("NOTE META")),
                )
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child(SharedString::from(format!("ID {note_id_value}"))),
                )
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child(SharedString::from(format!(
                            "RELATIONS ({note_meta_relations_count})"
                        ))),
                )
                .child(note_meta_relations_list)
                .child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(10.))
                        .font_weight(FontWeight(700.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child(SharedString::from(format!("PINS ({note_meta_pins_count})"))),
                )
                .child(note_meta_pins_list)
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

        tabs_bar = tabs_bar.on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _ev, _window, cx| this.clear_tab_drag_over(cx)),
        );

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
            let pin_path = path.clone();
            let dragged_tab = DraggedEditorTab { path: path.clone() };
            let is_pinned = self.pinned_editors.contains(&path);
            let drop_compare_path = path.clone();
            let tab_path_for_drop = path.clone();
            let tab_path_for_drag_move = path.clone();
            let tab_path_for_drop_handler = path.clone();
            let tab_path_for_click = path.clone();

            let is_drop_target = self
                .tab_drag_over
                .as_ref()
                .is_some_and(|meta| meta.target_path == tab_path_for_drop);
            let drop_insert_after = self
                .tab_drag_over
                .as_ref()
                .filter(|meta| meta.target_path == tab_path_for_drop)
                .map(|meta| meta.insert_after)
                .unwrap_or(false);

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

            let pin_button = div()
                .id(ElementId::Name(SharedString::from(format!(
                    "tab.pin:{pin_path}"
                ))))
                .h(px(28.))
                .w(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                    this.toggle_pin_editor(&pin_path, cx);
                }))
                .child(ui_icon(
                    ICON_BOOKMARK,
                    12.,
                    if is_pinned {
                        ui_theme.accent
                    } else {
                        ui_theme.text_subtle
                    },
                ));

            let tab_group_id = self
                .group_id_for_note_path(&path)
                .unwrap_or(self.active_editor_group_id);

            tabs_bar = tabs_bar.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!("tab:{path}"))))
                    .relative()
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
                    .on_drag(dragged_tab, |dragged, _offset, _window, cx| {
                        cx.new(|_| DragPreview {
                            label: SharedString::from(dragged.path.clone()),
                        })
                    })
                    .on_drag_move::<DraggedEditorTab>(cx.listener({
                        let target_path = tab_path_for_drag_move;
                        move |this, ev: &DragMoveEvent<DraggedEditorTab>, _window, cx| {
                            let Some(dragged) =
                                ev.dragged_item().downcast_ref::<DraggedEditorTab>()
                            else {
                                return;
                            };
                            if dragged.path == target_path {
                                return;
                            }
                            let mid_x = ev.bounds.origin.x + ev.bounds.size.width * 0.5;
                            let insert_after = ev.event.position.x >= mid_x;
                            this.set_tab_drag_over(target_path.clone(), insert_after, cx);
                        }
                    }))
                    .can_drop(move |dragged, _window, _cx| {
                        dragged
                            .downcast_ref::<DraggedEditorTab>()
                            .is_some_and(|tab| tab.path != drop_compare_path)
                    })
                    .on_drop::<DraggedEditorTab>(cx.listener(
                        move |this, dragged: &DraggedEditorTab, _window, cx| {
                            this.handle_tab_drop(
                                dragged,
                                &tab_path_for_drop_handler,
                                tab_group_id,
                                cx,
                            );
                        },
                    ))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _ev, _window, cx| this.clear_tab_drag_over(cx)),
                    )
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                        this.open_note(tab_path_for_click.clone(), cx);
                    }))
                    .when(is_drop_target, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .w(px(2.))
                                .bg(rgb(ui_theme.accent))
                                .when(drop_insert_after, |this| this.right(px(0.)))
                                .when(!drop_insert_after, |this| this.left(px(0.))),
                        )
                    })
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
                    .child(pin_button)
                    .child(close_button),
            );
        }

        let split_active = self.editor_groups.len() > 1;
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
                this.split_active_editor_group(cx);
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

        let split_dir_action = if split_active {
            Some(
                div()
                    .id("editor.action.split_direction")
                    .h(px(28.))
                    .w(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(rgb(ui_theme.panel_bg))
                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.editor_split_direction =
                            if this.editor_split_direction == EditorSplitDirection::Right {
                                EditorSplitDirection::Down
                            } else {
                                EditorSplitDirection::Right
                            };
                        this.remember_current_tab_view_state();
                        cx.notify();
                    }))
                    .child(ui_icon(
                        if self.editor_split_direction == EditorSplitDirection::Right {
                            ICON_PANEL_RIGHT_OPEN
                        } else {
                            ICON_PANEL_LEFT_OPEN
                        },
                        16.,
                        ui_theme.text_muted,
                    )),
            )
        } else {
            None
        };

        let close_group_action = if split_active {
            Some(
                div()
                    .id("editor.action.close_group")
                    .h(px(28.))
                    .w(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(rgb(ui_theme.panel_bg))
                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        let group_id = this.active_editor_group_id;
                        this.close_editor_group(group_id, cx);
                    }))
                    .child(ui_icon(ICON_X, 16., ui_theme.text_muted)),
            )
        } else {
            None
        };

        let close_other_groups_action = if split_active {
            Some(
                div()
                    .id("editor.action.close_other_groups")
                    .h(px(28.))
                    .w(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(rgb(ui_theme.panel_bg))
                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.close_other_editor_groups(cx);
                    }))
                    .tooltip({
                        let tooltip_theme = ui_theme;
                        move |_window, cx| {
                            AnyView::from(cx.new(|_| TooltipPreview {
                                label: SharedString::from("Close Other Groups"),
                                ui_theme: tooltip_theme,
                            }))
                        }
                    })
                    .child(ui_icon(ICON_SQUARE, 14., ui_theme.text_muted)),
            )
        } else {
            None
        };

        let close_groups_right_action = if split_active {
            Some(
                div()
                    .id("editor.action.close_groups_right")
                    .h(px(28.))
                    .w(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(rgb(ui_theme.panel_bg))
                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.close_groups_to_right(cx);
                    }))
                    .tooltip({
                        let tooltip_theme = ui_theme;
                        move |_window, cx| {
                            AnyView::from(cx.new(|_| TooltipPreview {
                                label: SharedString::from("Close Groups to Right"),
                                ui_theme: tooltip_theme,
                            }))
                        }
                    })
                    .child(ui_icon(ICON_CHEVRON_RIGHT, 14., ui_theme.text_muted)),
            )
        } else {
            None
        };

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
            .child(
                div()
                    .id("editor.mode.toggle.edit")
                    .h(px(28.))
                    .w(px(28.))
                    .min_w(px(28.))
                    .max_w(px(28.))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(if self.editor_view_mode == EditorViewMode::Edit {
                        rgb(ui_theme.interactive_hover)
                    } else {
                        rgb(ui_theme.panel_bg)
                    })
                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.set_editor_view_mode(EditorViewMode::Edit, cx);
                    }))
                    .child(ui_icon(
                        ICON_BRUSH,
                        13.,
                        if self.editor_view_mode == EditorViewMode::Edit {
                            ui_theme.accent
                        } else {
                            ui_theme.text_muted
                        },
                    )),
            )
            .child(
                div()
                    .id("editor.mode.toggle.preview")
                    .h(px(28.))
                    .w(px(28.))
                    .min_w(px(28.))
                    .max_w(px(28.))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(if self.editor_view_mode == EditorViewMode::Preview {
                        rgb(ui_theme.interactive_hover)
                    } else {
                        rgb(ui_theme.panel_bg)
                    })
                    .hover(|this| this.bg(rgb(ui_theme.interactive_hover)))
                    .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                        this.set_editor_view_mode(EditorViewMode::Preview, cx);
                    }))
                    .child(ui_icon(
                        ICON_EYE,
                        13.,
                        if self.editor_view_mode == EditorViewMode::Preview {
                            ui_theme.accent
                        } else {
                            ui_theme.text_muted
                        },
                    )),
            )
            .child(split_action)
            .when_some(split_dir_action, |this, action| this.child(action))
            .when_some(close_other_groups_action, |this, action| this.child(action))
            .when_some(close_groups_right_action, |this, action| this.child(action))
            .when_some(close_group_action, |this, action| this.child(action))
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

        let group_count = self.editor_groups.len().max(1);
        let group_splitter_total =
            (group_count.saturating_sub(1) as f32) * EDITOR_GROUP_SPLITTER_WIDTH;
        let group_target_total_width = (f32::from(editor_surface_width) - group_splitter_total)
            .max(EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH * group_count as f32);

        let editor_pane = |id: SharedString, interactive: bool, content: String| {
            let mut pane = div()
                .id(id)
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_y_scroll()
                .pl(px(EDITOR_SURFACE_LEFT_PADDING))
                .pr(px(EDITOR_SURFACE_RIGHT_PADDING))
                .pt(px(0.))
                .pb(px(10.))
                .font_family("IBM Plex Mono")
                .text_size(px(13.))
                .text_color(rgb(ui_theme.text_primary))
                .bg(rgb(ui_theme.surface_bg));

            if !content.is_empty() {
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
                if interactive {
                    pane.child(NoteEditorElement { view: cx.entity() })
                        .into_any_element()
                } else {
                    let preview_content = if content.trim().is_empty() {
                        div()
                            .font_family("IBM Plex Mono")
                            .text_size(px(11.))
                            .font_weight(FontWeight(650.))
                            .text_color(rgb(ui_theme.text_muted))
                            .child("No cached content yet (click to focus)")
                            .into_any_element()
                    } else {
                        div()
                            .id("editor.group.preview.content")
                            .w_full()
                            .font_family("IBM Plex Mono")
                            .text_size(px(13.))
                            .font_weight(FontWeight(450.))
                            .text_color(rgb(ui_theme.text_secondary))
                            .child(SharedString::from(content))
                            .into_any_element()
                    };

                    pane.child(preview_content).into_any_element()
                }
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
                .pl(px(EDITOR_SURFACE_LEFT_PADDING))
                .pr(px(EDITOR_SURFACE_RIGHT_PADDING))
                .pt(px(0.))
                .pb(px(10.))
                .bg(rgb(ui_theme.surface_bg))
                .flex()
                .flex_col()
                .gap(px(10.));

            if self.markdown_preview.headings.is_empty() && self.markdown_preview.blocks.is_empty()
            {
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
                    let (font_family, text_size, font_weight, text_color, prefix) = match block.kind
                    {
                        MarkdownPreviewBlockKind::Heading(level) => (
                            "Inter",
                            px((20_i32 - (level as i32 * 2)).max(12) as f32),
                            FontWeight(900.),
                            ui_theme.text_primary,
                            "",
                        ),
                        MarkdownPreviewBlockKind::Paragraph => (
                            "Inter",
                            px(13.),
                            FontWeight(650.),
                            ui_theme.text_secondary,
                            "",
                        ),
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

        let preview_pane_for_content = |id: SharedString, content: String| {
            let parsed = parse_markdown(&content);
            let mut pane = div()
                .id(id)
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_y_scroll()
                .pl(px(EDITOR_SURFACE_LEFT_PADDING))
                .pr(px(EDITOR_SURFACE_RIGHT_PADDING))
                .pt(px(0.))
                .pb(px(10.))
                .bg(rgb(ui_theme.surface_bg))
                .flex()
                .flex_col()
                .gap(px(10.));

            if parsed.blocks.is_empty() {
                pane = pane.child(
                    div()
                        .font_family("IBM Plex Mono")
                        .text_size(px(11.))
                        .font_weight(FontWeight(650.))
                        .text_color(rgb(ui_theme.text_muted))
                        .child("No markdown structure yet."),
                );
            } else {
                let mut blocks = div().flex().flex_col().gap(px(8.));
                for block in parsed.blocks.iter().take(160) {
                    let (font_family, text_size, font_weight, text_color, prefix) = match block.kind
                    {
                        xnote_core::markdown::MarkdownBlockKind::Heading(level) => (
                            "Inter",
                            px((20_i32 - (level as i32 * 2)).max(12) as f32),
                            FontWeight(900.),
                            ui_theme.text_primary,
                            "",
                        ),
                        xnote_core::markdown::MarkdownBlockKind::Paragraph => (
                            "Inter",
                            px(13.),
                            FontWeight(600.),
                            ui_theme.text_secondary,
                            "",
                        ),
                        xnote_core::markdown::MarkdownBlockKind::CodeFence => (
                            "IBM Plex Mono",
                            px(12.),
                            FontWeight(650.),
                            ui_theme.syntax_code_text,
                            "``` ",
                        ),
                        xnote_core::markdown::MarkdownBlockKind::Quote => (
                            "Inter",
                            px(13.),
                            FontWeight(700.),
                            ui_theme.syntax_quote_marker,
                            "> ",
                        ),
                        xnote_core::markdown::MarkdownBlockKind::List => (
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

        let editor_body = {
            let mut row = div()
                .id("editor.groups")
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .flex_row();

            let mut groups = self.editor_groups.clone();
            if groups.is_empty() {
                groups.push(EditorGroup {
                    id: self.active_editor_group_id,
                    note_path: self.open_note_path.clone(),
                    tabs: self.open_editors.clone(),
                    pinned_tabs: self.pinned_editors.clone(),
                    note_mru: self
                        .editor_group_note_history
                        .get(&self.active_editor_group_id)
                        .cloned()
                        .unwrap_or_default(),
                    view_state: self.current_tab_view_state(),
                });
            }

            let group_weights = self.normalized_editor_group_weights_snapshot_for_total(
                group_count,
                group_target_total_width,
            );

            for (ix, group) in groups.iter().enumerate() {
                let group_id = group.id;
                let is_active_group = group_id == self.active_editor_group_id;
                let group_mode = if is_active_group {
                    self.editor_view_mode
                } else {
                    group.view_state.mode
                };
                let group_note_path = group.note_path.clone();
                let group_title = group_note_path
                    .as_deref()
                    .map(|path| self.derive_note_title(path))
                    .unwrap_or_else(|| "No note selected".to_string());
                let group_path_label = group_note_path
                    .clone()
                    .unwrap_or_else(|| "Select a note in Explorer to open.".to_string());
                let group_mode_label = match group_mode {
                    EditorViewMode::Edit => "Edit",
                    EditorViewMode::Preview => "Preview",
                    EditorViewMode::Split => "Split",
                };
                let group_diag_count = if is_active_group {
                    self.markdown_diagnostics.len()
                } else {
                    0
                };
                let group_content = group_note_path
                    .as_deref()
                    .map(|path| {
                        if self.open_note_path.as_deref() == Some(path) {
                            self.open_note_content.clone()
                        } else {
                            self.note_content_cache
                                .get(path)
                                .cloned()
                                .unwrap_or_default()
                        }
                    })
                    .unwrap_or_default();
                let group_content = if is_active_group {
                    self.open_note_content.clone()
                } else {
                    full_preview_from_content(
                        &group_content,
                        INACTIVE_GROUP_PREVIEW_MAX_LINES,
                        INACTIVE_GROUP_PREVIEW_MAX_LINE_CHARS,
                    )
                };
                let group_body = match (is_active_group, group_mode) {
                    (true, EditorViewMode::Preview) => preview_pane(),
                    (true, _) => editor_pane(
                        SharedString::from("editor.pane.active"),
                        true,
                        group_content,
                    ),
                    (false, EditorViewMode::Preview) => preview_pane_for_content(
                        SharedString::from(format!("editor.preview.group:{group_id}")),
                        group_content,
                    ),
                    (false, _) => editor_pane(
                        SharedString::from(format!("editor.pane.group:{group_id}")),
                        false,
                        group_content,
                    ),
                };
                row = row.child(
                    div()
                        .id(ElementId::Name(SharedString::from(format!(
                            "editor.group:{group_id}"
                        ))))
                        .w(px(group_weights
                            .get(ix)
                            .copied()
                            .unwrap_or(EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH)))
                        .min_w(px(EDITOR_GROUP_MIN_VISIBLE_PANE_WIDTH))
                        .flex_shrink_0()
                        .min_h_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .when(group_count == 1, |this| this.flex_1().w_full())
                        .border_l_1()
                        .border_color(rgb(if ix == 0 {
                            ui_theme.border
                        } else if is_active_group {
                            ui_theme.accent
                        } else {
                            ui_theme.border
                        }))
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                            this.set_active_editor_group(group_id, cx);
                        }))
                        .child(
                            div()
                                .h_full()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .h(px(26.))
                                        .w_full()
                                        .px(px(8.))
                                        .bg(rgb(ui_theme.surface_bg))
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .font_family("IBM Plex Mono")
                                                .text_size(px(10.))
                                                .font_weight(FontWeight(700.))
                                                .text_color(rgb(ui_theme.text_muted))
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .child(group_path_label),
                                        ),
                                )
                                .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)))
                                .child(
                                    div()
                                        .h(px(42.))
                                        .w_full()
                                        .px(px(10.))
                                        .bg(rgb(ui_theme.surface_bg))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .font_family("Inter")
                                                .text_size(px(18.))
                                                .font_weight(FontWeight(850.))
                                                .text_color(rgb(ui_theme.text_primary))
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .child(group_title),
                                        )
                                        .child(
                                            div()
                                                .font_family("IBM Plex Mono")
                                                .text_size(px(10.))
                                                .font_weight(FontWeight(750.))
                                                .text_color(rgb(ui_theme.text_muted))
                                                .child(group_mode_label),
                                        ),
                                )
                                .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)))
                                .child(group_body)
                                .child(
                                    div()
                                        .h(px(24.))
                                        .w_full()
                                        .px(px(8.))
                                        .bg(rgb(ui_theme.surface_alt_bg))
                                        .border_t_1()
                                        .border_color(rgb(ui_theme.border))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .font_family("IBM Plex Mono")
                                                .text_size(px(10.))
                                                .font_weight(FontWeight(700.))
                                                .text_color(rgb(ui_theme.text_muted))
                                                .child(SharedString::from(format!(
                                                    "{}{}",
                                                    if is_active_group {
                                                        "Focused · "
                                                    } else {
                                                        ""
                                                    },
                                                    group_mode_label
                                                ))),
                                        )
                                        .child(
                                            div()
                                                .font_family("IBM Plex Mono")
                                                .text_size(px(10.))
                                                .font_weight(FontWeight(700.))
                                                .text_color(rgb(ui_theme.text_muted))
                                                .child(SharedString::from(format!(
                                                    "Diag {}",
                                                    group_diag_count
                                                ))),
                                        ),
                                ),
                        ),
                );

                if ix + 1 < group_count {
                    let split_index = ix;
                    row = row.child(
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "editor.group.separator:{}",
                                ix + 1
                            ))))
                            .relative()
                            .w(px(EDITOR_GROUP_SPLITTER_WIDTH))
                            .min_w(px(EDITOR_GROUP_SPLITTER_WIDTH))
                            .max_w(px(EDITOR_GROUP_SPLITTER_WIDTH))
                            .h_full()
                            .flex_shrink_0()
                            .bg(rgb(ui_theme.app_bg))
                            .cursor_col_resize()
                            .hover(|this| this.bg(rgb(ui_theme.panel_bg)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                                    this.begin_editor_group_drag(
                                        split_index,
                                        group_target_total_width,
                                        ev,
                                        cx,
                                    );
                                }),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(px((EDITOR_GROUP_SPLITTER_WIDTH - 1.0) * 0.5))
                                    .top_0()
                                    .bottom_0()
                                    .w(px(1.))
                                    .bg(rgb(ui_theme.border)),
                            ),
                    );
                }
            }

            row.into_any_element()
        };

        let _diagnostics_panel = {
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
                panel = panel.child(div().flex_1().min_h_0().child(uniform_list(
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
                                            .child(SharedString::from(format!("Ln {}", diag.line))),
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
                )));
            }

            panel
        };

        let active_link_hint = if self.open_note_loading || self.open_note_path.is_none() {
            None
        } else {
            let cursor = previous_char_boundary(
                &self.open_note_content,
                self.editor_cursor_offset()
                    .min(self.open_note_content.len()),
            );
            self.link_hit_at_offset(cursor)
        };

        let mut editor = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(rgb(ui_theme.surface_bg))
            .relative()
            .flex()
            .flex_col()
            .child(tabs_bar)
            .child(div().h(px(1.)).w_full().bg(rgb(ui_theme.border)))
            .child(editor_body);

        if let Some(hit) = active_link_hint.as_ref() {
            editor = editor.child(self.render_link_hit_hint(hit, ui_theme));
        }

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
                    .on_mouse_down(MouseButton::Right, cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                        this.open_palette(PaletteMode::QuickOpen, cx);
                    }))
                    .on_mouse_down(MouseButton::Middle, cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                        this.open_palette(PaletteMode::Search, cx);
                        this.panel_mode = PanelMode::Search;
                        cx.notify();
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
                            .child("Commands (Ctrl+K) · Quick Open (Ctrl+P) · Search (Ctrl+F in palette)"),
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
        } else if self.pending_external_note_reload.is_some() {
            (
                SharedString::from("External Pending"),
                ui_theme.status_loading,
            )
        } else if self.open_note_dirty {
            (SharedString::from("Unsaved"), ui_theme.status_dirty)
        } else {
            (SharedString::from("Synced"), ui_theme.status_synced)
        };

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
                                this.open_module_switcher(cx);
                            }))
                            .child(ui_icon(ICON_GRID_2X2, 14., 0x6b7280))
                            .child(
                                div()
                                    .font_family("IBM Plex Mono")
                                    .text_size(px(10.))
                                    .font_weight(FontWeight(800.))
                                    .text_color(rgb(ui_theme.text_primary))
                                    .child(self.active_module.label()),
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
            let active = self.splitter_drag.is_some_and(|d| d.kind == kind)
                || self.editor_group_drag.is_some_and(|d| d.kind == kind);
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
        if self.active_module == WorkstationModule::AiHub {
            main_row = main_row.child(self.render_ai_hub_panel(ui_theme, window, cx));
        } else {
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
        }

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
        if self.module_switcher_open {
            root = root.child(self.module_switcher_overlay(cx));
        }
        if self.settings_open {
            root = root.child(self.settings_overlay(cx));
        }
        if self.vault_prompt_open {
            root = root.child(self.vault_prompt_overlay(cx));
        }
        if self.link_picker_open {
            root = root.child(self.link_picker_overlay(cx));
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
                    .on_mouse_move(cx.listener(Self::on_active_split_drag_mouse_move))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(Self::on_active_split_drag_mouse_up),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(Self::on_active_split_drag_mouse_up),
                    ),
            );
        }

        if self.editor_group_drag.is_some() {
            root = root.child(
                div()
                    .id("editor.group.drag_overlay")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .right(px(0.))
                    .bg(rgba(0x00000000))
                    .cursor_col_resize()
                    .on_mouse_move(cx.listener(Self::on_editor_group_drag_mouse_move))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(Self::on_editor_group_drag_mouse_up),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(Self::on_editor_group_drag_mouse_up),
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

fn build_clock_label(epoch_secs: u64) -> String {
    let total = epoch_secs % 86_400;
    let hours = total / 3_600;
    let minutes = (total % 3_600) / 60;
    format!("{hours:02}:{minutes:02}")
}

fn probe_vcp_endpoint(endpoint: &str) -> std::io::Result<()> {
    let host_port = parse_host_port_from_endpoint(endpoint)?;
    let mut addrs = host_port.to_socket_addrs()?;
    let Some(addr) = addrs.next() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "endpoint resolved no address",
        ));
    };
    TcpStream::connect_timeout(&addr, AI_ENDPOINT_CHECK_TIMEOUT).map(|_| ())
}

fn parse_host_port_from_endpoint(endpoint: &str) -> std::io::Result<String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "empty endpoint",
        ));
    }

    let without_scheme = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .unwrap_or(trimmed);
    let host_and_path = without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme)
        .trim();
    if host_and_path.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid endpoint host",
        ));
    }

    if host_and_path.contains(':') {
        return Ok(host_and_path.to_string());
    }

    Ok(format!("{host_and_path}:80"))
}

fn truncate_message(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }

    let mut out = String::with_capacity(max_chars + 3);
    for ch in input.chars().take(max_chars) {
        out.push(ch);
    }
    out.push_str("...");
    out
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
    fn expand_note_move_pairs_with_prefix_is_exposed_via_core_watch() {
        let existing = vec![
            "notes/old/a.md".to_string(),
            "notes/old/sub/b.md".to_string(),
        ];
        let moved = vec![("notes/old/a.md".to_string(), "notes/new/a.md".to_string())];
        let out = expand_note_move_pairs_with_prefix(&existing, &moved).expect("expanded");
        assert_eq!(
            out,
            vec![
                ("notes/old/a.md".to_string(), "notes/new/a.md".to_string()),
                (
                    "notes/old/sub/b.md".to_string(),
                    "notes/new/sub/b.md".to_string()
                )
            ]
        );
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

        let folder = resolve_base_folder_for_new_items(None, None, &folder_notes, &folder_children);

        assert!(folder.is_empty());
    }

    #[test]
    fn editor_gutter_digits_are_stable_for_1_to_9999_lines() {
        assert_eq!(
            line_number_digits(1).max(EDITOR_GUTTER_STABLE_DIGITS_MAX_9999),
            4
        );
        assert_eq!(
            line_number_digits(376).max(EDITOR_GUTTER_STABLE_DIGITS_MAX_9999),
            4
        );
        assert_eq!(
            line_number_digits(9999).max(EDITOR_GUTTER_STABLE_DIGITS_MAX_9999),
            4
        );
        assert_eq!(
            line_number_digits(10_000).max(EDITOR_GUTTER_STABLE_DIGITS_MAX_9999),
            5
        );
    }

    #[test]
    fn quick_open_weighted_ranking_prefers_stem_then_short_path() {
        let query = "plan";
        let ranked = apply_quick_open_weighted_ranking(
            query,
            vec![
                "notes/deep/project-plan.md".to_string(),
                "notes/plan.md".to_string(),
                "archive/planning/index.md".to_string(),
            ],
            10,
        );

        assert_eq!(ranked.first().map(String::as_str), Some("notes/plan.md"));
        assert!(
            ranked
                .iter()
                .position(|p| p == "notes/deep/project-plan.md")
                .expect("deep plan present")
                < ranked
                    .iter()
                    .position(|p| p == "archive/planning/index.md")
                    .expect("planning present")
        );
    }

    #[test]
    fn flatten_search_groups_hides_matches_for_collapsed_group() {
        let groups = vec![SearchResultGroup {
            path: "notes/a.md".to_string(),
            match_count: 2,
            path_highlights: vec![0..4],
            matches: vec![
                SearchMatchEntry {
                    line: 3,
                    preview: "alpha one".to_string(),
                    preview_highlights: vec![0..5],
                },
                SearchMatchEntry {
                    line: 9,
                    preview: "alpha two".to_string(),
                    preview_highlights: vec![0..5],
                },
            ],
        }];

        let collapsed = HashSet::from(["notes/a.md".to_string()]);
        let rows = flatten_search_groups(&groups, &collapsed);
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0], SearchRow::File { .. }));

        let expanded = HashSet::new();
        let rows = flatten_search_groups(&groups, &expanded);
        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[1], SearchRow::Match { .. }));
    }

    #[test]
    fn collect_highlight_ranges_merges_overlaps() {
        let ranges = collect_highlight_ranges_lowercase(
            "project planning",
            &["plan".to_string(), "anning".to_string()],
        );
        assert_eq!(ranges, vec![8..16]);
    }

    #[test]
    fn touch_cache_order_moves_recent_and_evicts_oldest() {
        let mut order = VecDeque::new();
        assert_eq!(touch_cache_order("a", &mut order, 2), None);
        assert_eq!(touch_cache_order("b", &mut order, 2), None);
        assert_eq!(order.iter().cloned().collect::<Vec<_>>(), vec!["a", "b"]);

        assert_eq!(touch_cache_order("a", &mut order, 2), None);
        assert_eq!(order.iter().cloned().collect::<Vec<_>>(), vec!["b", "a"]);

        let evicted = touch_cache_order("c", &mut order, 2);
        assert_eq!(evicted.as_deref(), Some("b"));
        assert_eq!(order.iter().cloned().collect::<Vec<_>>(), vec!["a", "c"]);
    }

    #[test]
    fn close_groups_to_right_keeps_left_prefix_and_active() {
        let mut groups = vec![
            EditorGroup {
                id: 1,
                note_path: Some("notes/a.md".to_string()),
                tabs: vec!["notes/a.md".to_string()],
                pinned_tabs: HashSet::new(),
                note_mru: VecDeque::new(),
                view_state: default_editor_group_view_state(),
            },
            EditorGroup {
                id: 2,
                note_path: Some("notes/b.md".to_string()),
                tabs: vec!["notes/b.md".to_string()],
                pinned_tabs: HashSet::new(),
                note_mru: VecDeque::new(),
                view_state: default_editor_group_view_state(),
            },
            EditorGroup {
                id: 3,
                note_path: Some("notes/c.md".to_string()),
                tabs: vec!["notes/c.md".to_string()],
                pinned_tabs: HashSet::new(),
                note_mru: VecDeque::new(),
                view_state: default_editor_group_view_state(),
            },
        ];
        let active_id = 2_u64;

        let Some(active_ix) = groups.iter().position(|g| g.id == active_id) else {
            panic!("active group missing");
        };
        groups.truncate(active_ix + 1);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].id, 1);
        assert_eq!(groups[1].id, 2);
    }

    #[test]
    fn move_note_content_cache_path_transfers_entry() {
        let mut cache = HashMap::new();
        let mut order = VecDeque::new();
        cache.insert("notes/old.md".to_string(), "hello".to_string());
        let _ = touch_cache_order("notes/old.md", &mut order, NOTE_CONTENT_CACHE_CAPACITY);

        let from = "notes/old.md";
        let to = "notes/new.md";
        if let Some(content) = cache.remove(from) {
            cache.insert(to.to_string(), content);
        }
        if let Some(pos) = order.iter().position(|existing| existing == from) {
            order.remove(pos);
        }
        let _ = touch_cache_order(to, &mut order, NOTE_CONTENT_CACHE_CAPACITY);

        assert!(!cache.contains_key(from));
        assert_eq!(cache.get(to).map(String::as_str), Some("hello"));
        assert_eq!(order.back().map(String::as_str), Some(to));
    }

    #[test]
    fn compact_preview_from_content_limits_lines_and_chars() {
        let content = "1234567890\nabcdefg\nthird line";
        let preview = compact_preview_from_content(content, 2, 5);
        assert_eq!(preview, "12345…\nabcde…\n…");
    }

    #[test]
    fn pin_reorder_places_pinned_before_unpinned() {
        let mut open = vec![
            "notes/a.md".to_string(),
            "notes/b.md".to_string(),
            "notes/c.md".to_string(),
        ];
        let pinned = HashSet::from(["notes/c.md".to_string(), "notes/a.md".to_string()]);

        let mut pinned_part = Vec::new();
        let mut normal_part = Vec::new();
        for path in &open {
            if pinned.contains(path) {
                pinned_part.push(path.clone());
            } else {
                normal_part.push(path.clone());
            }
        }
        pinned_part.extend(normal_part);
        open = pinned_part;

        assert_eq!(open, vec!["notes/a.md", "notes/c.md", "notes/b.md"]);
    }

    #[test]
    fn splitter_start_position_jitter_baseline_is_zero_when_aligned() {
        let jitter = detect_splitter_start_position_jitter(320.0, 320.0, 320.0);
        assert_eq!(jitter, 0.0);
    }

    #[test]
    fn splitter_start_position_jitter_is_detectable_when_overlay_origin_differs() {
        let jitter = detect_splitter_start_position_jitter(320.0, 320.0, 356.0);
        assert!(jitter > 30.0);
    }

    #[test]
    fn quick_open_weighted_ranking_with_titles_prefers_title_matches() {
        let vault_root = std::env::temp_dir().join(format!(
            "xnote_ui_quick_open_title_pref_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&vault_root);
        std::fs::create_dir_all(vault_root.join("notes")).expect("mkdir");
        std::fs::write(vault_root.join("notes/a.md"), "# Deep Systems\n").expect("write a");
        std::fs::write(vault_root.join("notes/deep.md"), "# Generic\n").expect("write deep");

        let vault = Vault::open(&vault_root).expect("open vault");
        let mut index = KnowledgeIndex::empty();
        index.upsert_note(&vault, "notes/a.md").expect("upsert a");
        index
            .upsert_note(&vault, "notes/deep.md")
            .expect("upsert deep");

        let ranked = apply_quick_open_weighted_ranking_with_titles(
            "systems",
            vec!["notes/deep.md".to_string(), "notes/a.md".to_string()],
            &index,
            10,
        );
        assert_eq!(
            ranked.first().map(|item| item.path.as_str()),
            Some("notes/a.md")
        );

        let _ = std::fs::remove_dir_all(&vault_root);
    }

    #[test]
    fn split_layout_engine_split_creates_balanced_pair() {
        let engine = SplitLayoutEngine::new(80.0, 960.0);
        let state = engine.split_at(&[960.0], 1, 960.0, 0);
        assert_eq!(state.widths.len(), 2);
        assert!((state.widths[0] - 480.0).abs() < 0.001);
        assert!((state.widths[1] - 480.0).abs() < 0.001);
    }

    #[test]
    fn split_layout_engine_drag_pair_respects_min_width() {
        let engine = SplitLayoutEngine::new(80.0, 960.0);
        let state = engine.drag_pair(&[480.0, 480.0], 2, 960.0, 0, 480.0, 960.0, -1000.0);
        assert!((state.widths[0] - 80.0).abs() < 0.001);
        assert!((state.widths[1] - 880.0).abs() < 0.001);
    }

    #[test]
    fn split_layout_engine_close_returns_width_to_neighbor() {
        let engine = SplitLayoutEngine::new(80.0, 960.0);
        let state = engine.close_at(&[240.0, 300.0, 420.0], 3, 960.0, 1);
        assert_eq!(state.widths.len(), 2);
        let sum = state.widths.iter().sum::<f32>();
        assert!((sum - 960.0).abs() < 0.001);
        assert!(state.widths.iter().all(|width| *width >= 80.0));
        assert!(state.widths[0] > state.widths[1]);
    }

    #[test]
    fn split_layout_engine_normalize_preserves_target_total() {
        let engine = SplitLayoutEngine::new(80.0, 960.0);
        let state = engine.normalize(&[1.0, 1.0, 1.0], 3, 1200.0);
        let sum = state.widths.iter().sum::<f32>();
        assert!((sum - 1200.0).abs() < 0.001);
        assert!(state.widths.iter().all(|width| *width >= 80.0));
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

fn compact_preview_from_content(content: &str, max_lines: usize, max_line_chars: usize) -> String {
    if content.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for (ix, line) in content.lines().enumerate() {
        if ix >= max_lines {
            out.push('\n');
            out.push('…');
            break;
        }

        if ix > 0 {
            out.push('\n');
        }

        let mut chars = line.chars();
        let clipped: String = chars.by_ref().take(max_line_chars).collect();
        out.push_str(&clipped);
        if chars.next().is_some() {
            out.push('…');
        }
    }

    out
}

fn full_preview_from_content(content: &str, max_lines: usize, max_line_chars: usize) -> String {
    compact_preview_from_content(content, max_lines, max_line_chars)
}

fn split_query_tokens_lowercase(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect()
}

fn unique_case_insensitive_tokens(query: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for token in split_query_tokens_lowercase(query) {
        if seen.insert(token.clone()) {
            out.push(token);
        }
    }
    out
}

fn collect_highlight_ranges_lowercase(haystack: &str, tokens: &[String]) -> Vec<Range<usize>> {
    if haystack.is_empty() || tokens.is_empty() {
        return Vec::new();
    }

    let lowered = haystack.to_lowercase();
    let char_map = lowered
        .char_indices()
        .map(|(ix, _)| ix)
        .chain(std::iter::once(lowered.len()))
        .collect::<Vec<_>>();

    let to_byte = |char_ix: usize| -> usize {
        if char_ix >= char_map.len() {
            haystack.len()
        } else {
            char_map[char_ix]
        }
    };
    let mut ranges = Vec::<Range<usize>>::new();
    for token in tokens {
        if token.is_empty() {
            continue;
        }
        let mut search_start = 0usize;
        while search_start < lowered.len() {
            let Some(rel_ix) = lowered[search_start..].find(token) else {
                break;
            };
            let start = search_start + rel_ix;
            let end = start + token.len();
            ranges.push(start..end);
            search_start = end;
        }
    }

    if ranges.is_empty() {
        return ranges;
    }

    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end {
                if range.end > last.end {
                    last.end = range.end;
                }
                continue;
            }
        }
        merged.push(range);
    }

    merged
        .into_iter()
        .map(|range| to_byte(range.start)..to_byte(range.end))
        .collect()
}

fn flatten_search_groups(
    groups: &[SearchResultGroup],
    collapsed_paths: &HashSet<String>,
) -> Vec<SearchRow> {
    let mut rows = Vec::new();
    for group in groups {
        rows.push(SearchRow::File {
            path: group.path.clone(),
            match_count: group.match_count,
            path_highlights: group.path_highlights.clone(),
        });
        if collapsed_paths.contains(&group.path) {
            continue;
        }
        for entry in &group.matches {
            rows.push(SearchRow::Match {
                path: group.path.clone(),
                line: entry.line,
                preview: entry.preview.clone(),
                preview_highlights: entry.preview_highlights.clone(),
            });
        }
    }
    rows
}

#[cfg(test)]
fn apply_quick_open_weighted_ranking(
    query: &str,
    mut paths: Vec<String>,
    max_results: usize,
) -> Vec<String> {
    if paths.is_empty() {
        return paths;
    }

    let query_lower = query.trim().to_lowercase();
    if query_lower.is_empty() {
        paths.sort();
        if paths.len() > max_results {
            paths.truncate(max_results);
        }
        return paths;
    }

    let query_tokens = unique_case_insensitive_tokens(&query_lower);
    let mut scored = paths
        .into_iter()
        .map(|path| {
            let path_lower = path.to_lowercase();
            let file_name = path_lower
                .rsplit_once('/')
                .map(|(_, name)| name)
                .unwrap_or(path_lower.as_str());
            let file_stem = file_name.trim_end_matches(".md");

            let mut score = 0usize;
            if file_stem == query_lower {
                score += 300;
            }
            if file_stem.starts_with(&query_lower) {
                score += 180;
            }
            if file_stem.contains(&query_lower) {
                score += 130;
            }
            if file_name.starts_with(&query_lower) {
                score += 90;
            }
            if path_lower.starts_with(&query_lower) {
                score += 60;
            }
            if path_lower.contains(&query_lower) {
                score += 40;
            }

            if let Some(fuzzy) = subsequence_score_simple(file_stem, &query_lower) {
                score += fuzzy.saturating_mul(8);
            }
            if let Some(fuzzy) = subsequence_score_simple(&path_lower, &query_lower) {
                score += fuzzy;
            }

            for token in &query_tokens {
                if file_stem.contains(token) {
                    score += 18;
                } else if path_lower.contains(token) {
                    score += 8;
                }
            }

            (score, path)
        })
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.len().cmp(&b.1.len()))
            .then_with(|| a.1.cmp(&b.1))
    });

    scored
        .into_iter()
        .take(max_results)
        .map(|(_, path)| path)
        .collect()
}

fn resolve_open_path_match(path: &str, index: &KnowledgeIndex) -> ResolvedOpenPathMatch {
    let summary = index.note_summary(path);
    let title = summary
        .map(|item| item.title)
        .unwrap_or_else(|| file_name(path).trim_end_matches(".md").to_string());
    let title_lower = title.to_lowercase();
    let stem_lower = path
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(path)
        .trim_end_matches(".md")
        .to_lowercase();
    ResolvedOpenPathMatch {
        path: path.to_string(),
        title,
        title_lower,
        stem_lower,
    }
}

fn apply_quick_open_weighted_ranking_with_titles(
    query: &str,
    paths: Vec<String>,
    index: &KnowledgeIndex,
    max_results: usize,
) -> Vec<OpenPathMatch> {
    if paths.is_empty() {
        return Vec::new();
    }

    let query_lower = query.trim().to_lowercase();
    let query_tokens = unique_case_insensitive_tokens(&query_lower);
    if query_lower.is_empty() {
        let mut resolved = paths
            .into_iter()
            .map(|path| resolve_open_path_match(&path, index))
            .collect::<Vec<_>>();
        resolved.sort_by(|a, b| a.path.cmp(&b.path));
        return resolved
            .into_iter()
            .take(max_results)
            .map(|item| OpenPathMatch {
                path: item.path,
                title: item.title,
                path_highlights: Vec::new(),
                title_highlights: Vec::new(),
            })
            .collect();
    }

    let mut scored = paths
        .into_iter()
        .map(|path| {
            let resolved = resolve_open_path_match(&path, index);
            let path_lower = resolved.path.to_lowercase();
            let file_name = path_lower
                .rsplit_once('/')
                .map(|(_, name)| name)
                .unwrap_or(path_lower.as_str());
            let file_stem = file_name.trim_end_matches(".md");

            let mut score = 0usize;

            if resolved.title_lower == query_lower {
                score += 340;
            }
            if resolved.title_lower.starts_with(&query_lower) {
                score += 220;
            }
            if resolved.title_lower.contains(&query_lower) {
                score += 160;
            }

            if resolved.stem_lower == query_lower {
                score += 320;
            }
            if resolved.stem_lower.starts_with(&query_lower) {
                score += 190;
            }
            if resolved.stem_lower.contains(&query_lower) {
                score += 140;
            }
            if file_name.starts_with(&query_lower) {
                score += 90;
            }
            if path_lower.starts_with(&query_lower) {
                score += 60;
            }
            if path_lower.contains(&query_lower) {
                score += 40;
            }

            if let Some(fuzzy) = subsequence_score_simple(&resolved.title_lower, &query_lower) {
                score += fuzzy.saturating_mul(10);
            }
            if let Some(fuzzy) = subsequence_score_simple(file_stem, &query_lower) {
                score += fuzzy.saturating_mul(8);
            }
            if let Some(fuzzy) = subsequence_score_simple(&path_lower, &query_lower) {
                score += fuzzy;
            }

            for token in &query_tokens {
                if resolved.title_lower.contains(token) {
                    score += 22;
                }
                if file_stem.contains(token) {
                    score += 16;
                } else if path_lower.contains(token) {
                    score += 8;
                }
            }

            (score, resolved)
        })
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.path.len().cmp(&b.1.path.len()))
            .then_with(|| a.1.path.cmp(&b.1.path))
    });

    scored
        .into_iter()
        .take(max_results)
        .map(|(_, item)| OpenPathMatch {
            path_highlights: collect_highlight_ranges_lowercase(&item.path, &query_tokens),
            title_highlights: collect_highlight_ranges_lowercase(&item.title, &query_tokens),
            path: item.path,
            title: item.title,
        })
        .collect()
}

#[cfg(test)]
fn detect_splitter_start_position_jitter(
    initial_separator_x: f32,
    pointer_down_x: f32,
    first_overlay_move_x: f32,
) -> f32 {
    (first_overlay_move_x - pointer_down_x).abs() + (pointer_down_x - initial_separator_x).abs()
}

fn subsequence_score_simple(haystack: &str, query: &str) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }

    let mut score = 0usize;
    let mut search_start = 0usize;
    let mut prev = None;
    for qch in query.chars() {
        let found = haystack[search_start..]
            .char_indices()
            .find(|(_, hch)| *hch == qch)
            .map(|(rel_ix, hch)| (search_start + rel_ix, hch.len_utf8()));
        let (ix, len) = found?;
        score = score.saturating_add(2);
        if let Some(prev_ix) = prev {
            if ix == prev_ix + 1 {
                score = score.saturating_add(3);
            }
        }
        prev = Some(ix);
        search_start = ix + len;
    }
    Some(score)
}

fn render_highlighted_segments(
    text: &str,
    ranges: &[Range<usize>],
    normal_color: u32,
    highlight_color: u32,
    normal_weight: FontWeight,
    highlight_weight: FontWeight,
) -> Vec<gpui::AnyElement> {
    let mut segments = Vec::new();
    if text.is_empty() {
        return segments;
    }

    if ranges.is_empty() {
        segments.push(
            div()
                .text_color(rgb(normal_color))
                .font_weight(normal_weight)
                .child(SharedString::from(text.to_string()))
                .into_any_element(),
        );
        return segments;
    }

    let to_char_boundary = |s: &str, mut ix: usize| {
        if ix >= s.len() {
            return s.len();
        }
        while ix > 0 && !s.is_char_boundary(ix) {
            ix -= 1;
        }
        ix
    };

    let mut cursor = 0usize;
    for range in ranges {
        let start = to_char_boundary(text, range.start.min(text.len()));
        let end = to_char_boundary(text, range.end.min(text.len()));
        if end <= cursor {
            continue;
        }
        if start > cursor {
            segments.push(
                div()
                    .text_color(rgb(normal_color))
                    .font_weight(normal_weight)
                    .child(SharedString::from(text[cursor..start].to_string()))
                    .into_any_element(),
            );
        }
        if end > start {
            segments.push(
                div()
                    .text_color(rgb(highlight_color))
                    .font_weight(highlight_weight)
                    .child(SharedString::from(text[start..end].to_string()))
                    .into_any_element(),
            );
        }
        cursor = end;
    }

    if cursor < text.len() {
        segments.push(
            div()
                .text_color(rgb(normal_color))
                .font_weight(normal_weight)
                .child(SharedString::from(text[cursor..].to_string()))
                .into_any_element(),
        );
    }

    segments
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
        } else if trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
        {
            let marker_end = (content_start + 1).min(line_end);
            spans.push(EditorHighlightSpan {
                range: content_start..marker_end,
                kind: EditorHighlightKind::ListMarker,
            });
        }

        for (link_start, link_end, text_range, url_range) in markdown_link_ranges(line, line_start)
        {
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
    for span in &mut spans {
        span.range = clamp_range_to_char_boundaries(text, span.range.clone());
    }
    spans.retain(|span| span.range.start < span.range.end);
    spans
}

fn previous_char_boundary(text: &str, index: usize) -> usize {
    let index = index.min(text.len());
    if text.is_char_boundary(index) {
        return index;
    }
    let mut prev = 0usize;
    for (ix, _ch) in text.char_indices() {
        if ix >= index {
            break;
        }
        prev = ix;
    }
    prev
}

fn next_char_boundary(text: &str, index: usize) -> usize {
    let index = index.min(text.len());
    if text.is_char_boundary(index) {
        return index;
    }
    for (ix, _ch) in text.char_indices() {
        if ix > index {
            return ix;
        }
    }
    text.len()
}

fn clamp_range_to_char_boundaries(text: &str, range: Range<usize>) -> Range<usize> {
    if text.is_empty() {
        return 0..0;
    }
    let start = previous_char_boundary(text, range.start);
    let mut end = next_char_boundary(text, range.end);
    if end < start {
        end = start;
    }
    start..end.min(text.len())
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

    #[test]
    fn highlight_spans_remain_on_char_boundaries_for_cjk() {
        let text = "# N 撒发放\nSee [撒发](https://example.com)\n";
        let spans = build_editor_highlight_spans(text);
        assert!(!spans.is_empty());
        for span in spans {
            assert!(text.is_char_boundary(span.range.start));
            assert!(text.is_char_boundary(span.range.end));
            let _ = &text[span.range];
        }
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
            if close_bracket + 1 < bytes.len() && bytes[close_bracket + 1] == b'(' {
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
                out.push((
                    full_start,
                    full_end,
                    text_start..text_end,
                    url_start..url_end,
                ));
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

fn initial_window_bounds_from_layout(layout: &WindowLayoutSettings, cx: &mut App) -> WindowBounds {
    let width_px = layout
        .window_width_px
        .unwrap_or(WINDOW_DEFAULT_WIDTH_PX)
        .max(WINDOW_MIN_WIDTH_PX);
    let height_px = layout
        .window_height_px
        .unwrap_or(WINDOW_DEFAULT_HEIGHT_PX)
        .max(WINDOW_MIN_HEIGHT_PX);
    let restored_size = size(px(width_px as f32), px(height_px as f32));

    match (layout.window_x_px, layout.window_y_px) {
        (Some(x), Some(y)) => WindowBounds::Windowed(Bounds::new(
            point(px(x as f32), px(y as f32)),
            restored_size,
        )),
        _ => WindowBounds::Windowed(Bounds::centered(None, restored_size, cx)),
    }
}

fn main() {
    Application::new()
        .with_assets(UiAssets {
            base: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        })
        .run(|cx: &mut App| {
            let boot = load_boot_context();
            let bounds = initial_window_bounds_from_layout(&boot.app_settings.window_layout, cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(bounds),
                    window_min_size: Some(size(
                        px(WINDOW_MIN_WIDTH_PX as f32),
                        px(WINDOW_MIN_HEIGHT_PX as f32),
                    )),
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
