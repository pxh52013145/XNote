use crate::command::{command_specs, CommandId};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyChord {
    pub mods: KeyModifiers,
    pub key: String,
}

impl KeyChord {
    pub fn parse(input: &str) -> Option<Self> {
        let raw = input.trim();
        if raw.is_empty() {
            return None;
        }

        let mut mods = KeyModifiers::default();
        let mut key: Option<String> = None;

        for part in raw.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()) {
            let token = part.to_ascii_lowercase();
            match token.as_str() {
                "ctrl" | "control" => mods.ctrl = true,
                "alt" | "option" => mods.alt = true,
                "shift" => mods.shift = true,
                "meta" | "cmd" | "command" | "super" | "win" => mods.meta = true,
                _ => {
                    if key.is_some() {
                        return None;
                    }
                    key = Some(token);
                }
            }
        }

        let key = key?;
        Some(Self { mods, key })
    }

    pub fn normalize_string(input: &str) -> Option<String> {
        Self::parse(input).map(|chord| chord.to_string())
    }

    pub fn matches_event(
        &self,
        event_key: &str,
        control_or_platform: bool,
        alt: bool,
        shift: bool,
        meta: bool,
    ) -> bool {
        self.mods.ctrl == control_or_platform
            && self.mods.alt == alt
            && self.mods.shift == shift
            && self.mods.meta == meta
            && self.key == event_key.to_ascii_lowercase()
    }
}

impl std::fmt::Display for KeyChord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts: Vec<&str> = Vec::new();
        if self.mods.ctrl {
            parts.push("Ctrl");
        }
        if self.mods.alt {
            parts.push("Alt");
        }
        if self.mods.shift {
            parts.push("Shift");
        }
        if self.mods.meta {
            parts.push("Meta");
        }

        if self.key == "," {
            parts.push(",");
        } else if self.key == "\\" {
            parts.push("\\");
        } else {
            parts.push(&self.key);
        }

        write!(f, "{}", parts.join("+"))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyContext {
    values: HashMap<String, bool>,
}

impl KeyContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, key: impl Into<String>, value: bool) -> Self {
        self.values.insert(key.into(), value);
        self
    }

    pub fn set(&mut self, key: impl Into<String>, value: bool) {
        self.values.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> bool {
        self.values.get(key).copied().unwrap_or(false)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ContextExpr {
    Always,
    Key(String),
    NotKey(String),
    And(Vec<ContextExpr>),
}

impl ContextExpr {
    fn evaluate(&self, context: &KeyContext) -> bool {
        match self {
            Self::Always => true,
            Self::Key(key) => context.get(key),
            Self::NotKey(key) => !context.get(key),
            Self::And(items) => items.iter().all(|item| item.evaluate(context)),
        }
    }

    fn parse(input: Option<&str>) -> Result<Self, String> {
        let Some(raw) = input else {
            return Ok(Self::Always);
        };

        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(Self::Always);
        }

        let mut items = Vec::new();
        for token in trimmed
            .split("&&")
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
        {
            if let Some(key) = token.strip_prefix('!') {
                let key = key.trim();
                if key.is_empty() {
                    return Err(format!("invalid context expression: {trimmed}"));
                }
                items.push(ContextExpr::NotKey(key.to_string()));
            } else {
                items.push(ContextExpr::Key(token.to_string()));
            }
        }

        if items.is_empty() {
            Ok(Self::Always)
        } else if items.len() == 1 {
            Ok(items.remove(0))
        } else {
            Ok(Self::And(items))
        }
    }
}

#[derive(Clone, Debug)]
struct KeybindingEntry {
    command: CommandId,
    chord: KeyChord,
    chord_text: String,
    when: ContextExpr,
    when_text: String,
    source_priority: u8,
    source_order: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Keymap {
    entries: Vec<KeybindingEntry>,
}

impl Keymap {
    pub fn default_keymap() -> Self {
        let mut out = Self::default();
        for (order, spec) in command_specs().iter().enumerate() {
            let _ = out.bind_with_when(spec.default_shortcut, spec.id, None, 10, order, true);
        }
        out
    }

    pub fn bind(&mut self, chord: &str, command: CommandId) {
        let _ = self.bind_with_when(chord, command, None, 100, self.entries.len(), true);
    }

    fn bind_with_when(
        &mut self,
        chord: &str,
        command: CommandId,
        when: Option<&str>,
        source_priority: u8,
        source_order: usize,
        clear_existing_command: bool,
    ) -> Result<(), String> {
        let Some(normalized) = KeyChord::normalize_string(chord) else {
            return Err(format!("invalid key chord: {chord}"));
        };
        let chord =
            KeyChord::parse(&normalized).ok_or_else(|| format!("invalid key chord: {chord}"))?;
        let when_expr = ContextExpr::parse(when)?;
        let when_text = when.unwrap_or("").trim().to_string();

        if clear_existing_command {
            self.entries.retain(|entry| entry.command != command);
        }

        self.entries.push(KeybindingEntry {
            command,
            chord,
            chord_text: normalized,
            when: when_expr,
            when_text,
            source_priority,
            source_order,
        });
        Ok(())
    }

    pub fn resolve(&self, chord: &str) -> Option<CommandId> {
        let normalized = KeyChord::normalize_string(chord)?;
        self.resolve_with_context_text(&normalized, &KeyContext::default())
            .map(|entry| entry.command)
    }

    pub fn resolve_event(
        &self,
        event_key: &str,
        control_or_platform: bool,
        alt: bool,
        shift: bool,
        meta: bool,
    ) -> Option<CommandId> {
        self.resolve_event_in_context(
            event_key,
            control_or_platform,
            alt,
            shift,
            meta,
            &KeyContext::default(),
        )
    }

    pub fn resolve_event_in_context(
        &self,
        event_key: &str,
        control_or_platform: bool,
        alt: bool,
        shift: bool,
        meta: bool,
        context: &KeyContext,
    ) -> Option<CommandId> {
        self.entries
            .iter()
            .filter(|entry| {
                entry
                    .chord
                    .matches_event(event_key, control_or_platform, alt, shift, meta)
            })
            .filter(|entry| entry.when.evaluate(context))
            .max_by_key(|entry| (entry.source_priority, entry.source_order))
            .map(|entry| entry.command)
    }

    pub fn shortcut_for(&self, command: CommandId) -> Option<&str> {
        self.entries
            .iter()
            .filter(|entry| entry.command == command)
            .max_by_key(|entry| (entry.source_priority, entry.source_order))
            .map(|entry| entry.chord_text.as_str())
    }

    pub fn apply_overrides<I, K, V>(&mut self, entries: I) -> Result<(), String>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        for (order, (command_id, chord_text)) in entries.into_iter().enumerate() {
            let Some(command) = CommandId::from_str(command_id.as_ref()) else {
                return Err(format!("unknown command id: {}", command_id.as_ref()));
            };
            self.bind_with_when(chord_text.as_ref(), command, None, 100, order, true)?;
        }
        Ok(())
    }

    pub fn apply_contextual_overrides<I, K, C, W>(
        &mut self,
        entries: I,
        source_priority: u8,
    ) -> Result<(), String>
    where
        I: IntoIterator<Item = (K, C, W)>,
        K: AsRef<str>,
        C: AsRef<str>,
        W: AsRef<str>,
    {
        for (order, (command_id, chord_text, when)) in entries.into_iter().enumerate() {
            let Some(command) = CommandId::from_str(command_id.as_ref()) else {
                return Err(format!("unknown command id: {}", command_id.as_ref()));
            };
            self.bind_with_when(
                chord_text.as_ref(),
                command,
                Some(when.as_ref()),
                source_priority,
                order,
                false,
            )?;
        }
        Ok(())
    }

    fn resolve_with_context_text<'a>(
        &'a self,
        chord_text: &str,
        context: &KeyContext,
    ) -> Option<&'a KeybindingEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.chord_text.eq_ignore_ascii_case(chord_text))
            .filter(|entry| entry.when.evaluate(context))
            .max_by_key(|entry| (entry.source_priority, entry.source_order))
    }

    pub fn effective_when_for(&self, command: CommandId) -> Option<&str> {
        self.entries
            .iter()
            .filter(|entry| entry.command == command)
            .max_by_key(|entry| (entry.source_priority, entry.source_order))
            .map(|entry| entry.when_text.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_normalize_chords() {
        let chord = KeyChord::parse(" Ctrl + Shift + K ").expect("parse chord");
        assert!(chord.mods.ctrl);
        assert!(chord.mods.shift);
        assert_eq!(chord.key, "k");
        assert_eq!(chord.to_string(), "Ctrl+Shift+k");
    }

    #[test]
    fn default_keymap_resolve_and_override() {
        let mut keymap = Keymap::default_keymap();
        assert_eq!(keymap.resolve("ctrl+o"), Some(CommandId::OpenVault));

        keymap.bind("Ctrl+Shift+O", CommandId::OpenVault);
        assert_eq!(keymap.resolve("ctrl+o"), None);
        assert_eq!(keymap.resolve("ctrl+shift+o"), Some(CommandId::OpenVault));
    }

    #[test]
    fn resolve_event_matches_platform_control() {
        let keymap = Keymap::default_keymap();
        let cmd = keymap.resolve_event("s", true, false, false, false);
        assert_eq!(cmd, Some(CommandId::SaveFile));
    }

    #[test]
    fn apply_overrides_with_validation() {
        let mut keymap = Keymap::default_keymap();
        keymap
            .apply_overrides(vec![("open_vault", "Ctrl+Shift+O")])
            .expect("apply override");
        assert_eq!(keymap.resolve("ctrl+shift+o"), Some(CommandId::OpenVault));

        let invalid = keymap.apply_overrides(vec![("open_vault", "Ctrl+Shift+O+P")]);
        assert!(invalid.is_err());
    }

    #[test]
    fn resolve_with_context_expressions() {
        let mut keymap = Keymap::default_keymap();
        keymap
            .apply_contextual_overrides(
                vec![("quick_open", "Ctrl+P", "in_editor && !palette_open")],
                120,
            )
            .expect("apply contextual override");

        let editor_ctx = KeyContext::new()
            .with("in_editor", true)
            .with("palette_open", false);
        let palette_ctx = KeyContext::new()
            .with("in_editor", true)
            .with("palette_open", true);

        let resolved_editor =
            keymap.resolve_event_in_context("p", true, false, false, false, &editor_ctx);
        let resolved_palette =
            keymap.resolve_event_in_context("p", true, false, false, false, &palette_ctx);

        assert_eq!(resolved_editor, Some(CommandId::QuickOpen));
        assert_eq!(resolved_palette, Some(CommandId::QuickOpen));
    }

    #[test]
    fn context_specific_binding_wins_by_priority() {
        let mut keymap = Keymap::default_keymap();
        keymap
            .apply_contextual_overrides(vec![("focus_search", "Alt+1", "search_panel")], 150)
            .expect("apply contextual override");

        let default_ctx = KeyContext::new();
        let search_ctx = KeyContext::new().with("search_panel", true);

        let default_cmd =
            keymap.resolve_event_in_context("1", false, true, false, false, &default_ctx);
        let search_cmd =
            keymap.resolve_event_in_context("1", false, true, false, false, &search_ctx);

        assert_eq!(default_cmd, Some(CommandId::FocusExplorer));
        assert_eq!(search_cmd, Some(CommandId::FocusSearch));
    }
}
