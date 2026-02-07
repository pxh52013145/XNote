use crate::keybind::Keymap;
use crate::plugin::PluginPolicy;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_PLUGIN_RUNTIME_MODE: &str = "in_process";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_locale_string")]
    pub locale: String,
    #[serde(default)]
    pub appearance: AppearanceSettings,
    #[serde(default)]
    pub editor: EditorSettings,
    #[serde(default)]
    pub files_links: FilesLinksSettings,
    #[serde(default)]
    pub bookmarked_notes: Vec<String>,
    #[serde(default)]
    pub keymap_overrides: HashMap<String, String>,
    #[serde(default)]
    pub keymap_contextual: Vec<KeymapRule>,
    #[serde(default)]
    pub plugin_policy: AppPluginPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeymapRule {
    pub command: String,
    pub chord: String,
    #[serde(default)]
    pub when: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppPluginPolicy {
    #[serde(default)]
    pub allow_network: bool,
    #[serde(default = "default_max_failed_activations")]
    pub max_failed_activations: u32,
    #[serde(default = "default_activation_timeout_ms")]
    pub activation_timeout_ms: u64,
    #[serde(default = "default_plugin_runtime_mode")]
    pub runtime_mode: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppearanceSettings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_accent")]
    pub accent: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorSettings {
    #[serde(default = "default_autosave_delay_ms")]
    pub autosave_delay_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilesLinksSettings {
    #[serde(default = "default_external_sync")]
    pub external_sync: bool,
    #[serde(default = "default_prefer_wikilink_titles")]
    pub prefer_wikilink_titles: bool,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            accent: default_accent(),
        }
    }
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            autosave_delay_ms: default_autosave_delay_ms(),
        }
    }
}

impl Default for FilesLinksSettings {
    fn default() -> Self {
        Self {
            external_sync: default_external_sync(),
            prefer_wikilink_titles: default_prefer_wikilink_titles(),
        }
    }
}

impl Default for AppPluginPolicy {
    fn default() -> Self {
        Self {
            allow_network: false,
            max_failed_activations: default_max_failed_activations(),
            activation_timeout_ms: default_activation_timeout_ms(),
            runtime_mode: default_plugin_runtime_mode(),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            locale: default_locale_string(),
            appearance: AppearanceSettings::default(),
            editor: EditorSettings::default(),
            files_links: FilesLinksSettings::default(),
            bookmarked_notes: Vec::new(),
            keymap_overrides: HashMap::new(),
            keymap_contextual: Vec::new(),
            plugin_policy: AppPluginPolicy::default(),
        }
    }
}

impl AppSettings {
    pub fn to_plugin_policy(&self) -> PluginPolicy {
        PluginPolicy {
            allow_network: self.plugin_policy.allow_network,
            max_failed_activations: self.plugin_policy.max_failed_activations.max(1),
            activation_timeout_ms: self.plugin_policy.activation_timeout_ms.max(10),
        }
    }

    pub fn build_keymap(&self) -> Result<Keymap> {
        let mut keymap = Keymap::default_keymap();
        keymap
            .apply_overrides(
                self.keymap_overrides
                    .iter()
                    .map(|(command, shortcut)| (command.as_str(), shortcut.as_str())),
            )
            .map_err(anyhow::Error::msg)?;

        keymap
            .apply_contextual_overrides(
                self.keymap_contextual.iter().map(|rule| {
                    (
                        rule.command.as_str(),
                        rule.chord.as_str(),
                        rule.when.as_str(),
                    )
                }),
                120,
            )
            .map_err(anyhow::Error::msg)?;

        Ok(keymap)
    }

    pub fn merge_overlay(&self, overlay: &AppSettings) -> AppSettings {
        let mut merged = self.clone();
        merged.schema_version = self.schema_version.max(overlay.schema_version);

        if !overlay.locale.trim().is_empty() {
            merged.locale = overlay.locale.clone();
        }

        if !overlay.appearance.theme.trim().is_empty() {
            merged.appearance.theme = overlay.appearance.theme.clone();
        }
        if !overlay.appearance.accent.trim().is_empty() {
            merged.appearance.accent = overlay.appearance.accent.clone();
        }

        merged.editor.autosave_delay_ms = overlay.editor.autosave_delay_ms.max(100);
        merged.files_links.external_sync = overlay.files_links.external_sync;
        merged.files_links.prefer_wikilink_titles = overlay.files_links.prefer_wikilink_titles;

        if !overlay.bookmarked_notes.is_empty() {
            for note in &overlay.bookmarked_notes {
                if !merged
                    .bookmarked_notes
                    .iter()
                    .any(|existing| existing == note)
                {
                    merged.bookmarked_notes.push(note.clone());
                }
            }
        }

        for (command, chord) in &overlay.keymap_overrides {
            merged
                .keymap_overrides
                .insert(command.clone(), chord.clone());
        }

        if !overlay.keymap_contextual.is_empty() {
            merged
                .keymap_contextual
                .extend(overlay.keymap_contextual.clone());
        }

        merged.plugin_policy = overlay.plugin_policy.clone();
        merged
    }
}

pub fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join("settings.json")
}

pub fn project_settings_path(project_root: &Path) -> PathBuf {
    project_root.join(".xnote").join("settings.json")
}

pub fn load_settings(config_dir: &Path) -> Result<AppSettings> {
    load_settings_from_path(&settings_path(config_dir))
}

pub fn load_project_settings(project_root: &Path) -> Result<Option<AppSettings>> {
    let path = project_settings_path(project_root);
    if !path.exists() {
        return Ok(None);
    }
    let settings = load_settings_from_path(&path)?;
    Ok(Some(settings))
}

pub fn load_effective_settings(
    config_dir: &Path,
    project_root: Option<&Path>,
) -> Result<AppSettings> {
    let user = load_settings(config_dir).unwrap_or_default();
    if let Some(project_root) = project_root {
        if let Some(project) = load_project_settings(project_root)? {
            return Ok(user.merge_overlay(&project));
        }
    }
    Ok(user)
}

pub fn save_settings(config_dir: &Path, settings: &AppSettings) -> Result<()> {
    save_settings_to_path(&settings_path(config_dir), settings)
}

pub fn save_project_settings(project_root: &Path, settings: &AppSettings) -> Result<()> {
    save_settings_to_path(&project_settings_path(project_root), settings)
}

fn load_settings_from_path(path: &Path) -> Result<AppSettings> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("read settings file: {}", path.display()))?;
    let settings: AppSettings = serde_json::from_str(&raw)
        .with_context(|| format!("parse settings file: {}", path.display()))?;
    Ok(settings)
}

fn save_settings_to_path(path: &Path, settings: &AppSettings) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create config dir: {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(settings)?;
    fs::write(path, json).with_context(|| format!("write settings file: {}", path.display()))?;
    Ok(())
}

const fn default_schema_version() -> u32 {
    1
}

fn default_locale_string() -> String {
    "en-US".to_string()
}

fn default_plugin_runtime_mode() -> String {
    DEFAULT_PLUGIN_RUNTIME_MODE.to_string()
}

fn default_theme() -> String {
    "light".to_string()
}

fn default_accent() -> String {
    "default".to_string()
}

const fn default_autosave_delay_ms() -> u64 {
    500
}

const fn default_external_sync() -> bool {
    true
}

const fn default_prefer_wikilink_titles() -> bool {
    true
}

const fn default_max_failed_activations() -> u32 {
    3
}

const fn default_activation_timeout_ms() -> u64 {
    2000
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "xnote_core_settings_test_{}_{}",
            name,
            std::process::id()
        ))
    }

    #[test]
    fn load_defaults_when_missing() {
        let dir = temp_dir("missing");
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }

        let settings = load_settings(&dir).expect("load defaults");
        assert_eq!(settings.locale, "en-US");
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = temp_dir("roundtrip");
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }

        let mut settings = AppSettings {
            locale: "zh-CN".to_string(),
            appearance: AppearanceSettings {
                theme: "dark".to_string(),
                accent: "blue".to_string(),
            },
            ..AppSettings::default()
        };
        settings.editor.autosave_delay_ms = 1200;
        settings.files_links.external_sync = false;
        settings.files_links.prefer_wikilink_titles = false;
        settings.bookmarked_notes.push("notes/Alpha.md".to_string());
        settings
            .keymap_overrides
            .insert("open_vault".to_string(), "Ctrl+Shift+O".to_string());
        settings.plugin_policy.allow_network = true;

        save_settings(&dir, &settings).expect("save settings");
        let loaded = load_settings(&dir).expect("load settings");
        assert_eq!(loaded, settings);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_keymap_from_overrides_and_context_rules() {
        let mut settings = AppSettings::default();
        settings
            .keymap_overrides
            .insert("open_vault".to_string(), "Ctrl+Shift+O".to_string());
        settings.keymap_contextual.push(KeymapRule {
            command: "focus_search".to_string(),
            chord: "Alt+1".to_string(),
            when: "search_panel".to_string(),
        });

        let keymap = settings.build_keymap().expect("build keymap");
        assert_eq!(
            keymap.resolve("ctrl+shift+o"),
            Some(crate::command::CommandId::OpenVault)
        );
    }

    #[test]
    fn layered_settings_project_overrides_user() {
        let user_dir = temp_dir("layered_user");
        let project_dir = temp_dir("layered_project");
        if user_dir.exists() {
            let _ = fs::remove_dir_all(&user_dir);
        }
        if project_dir.exists() {
            let _ = fs::remove_dir_all(&project_dir);
        }

        let mut user = AppSettings {
            locale: "en-US".to_string(),
            ..AppSettings::default()
        };
        user.keymap_overrides
            .insert("open_vault".to_string(), "Ctrl+O".to_string());
        save_settings(&user_dir, &user).expect("save user settings");

        let mut project = AppSettings {
            locale: "zh-CN".to_string(),
            appearance: AppearanceSettings {
                theme: "dark".to_string(),
                ..AppearanceSettings::default()
            },
            ..AppSettings::default()
        };
        project.editor.autosave_delay_ms = 900;
        project.bookmarked_notes.push("notes/Beta.md".to_string());
        project
            .keymap_overrides
            .insert("open_vault".to_string(), "Ctrl+Shift+O".to_string());
        save_project_settings(&project_dir, &project).expect("save project settings");

        let effective = load_effective_settings(&user_dir, Some(&project_dir))
            .expect("load effective settings");
        assert_eq!(effective.locale, "zh-CN");
        assert_eq!(effective.appearance.theme, "dark");
        assert_eq!(effective.editor.autosave_delay_ms, 900);
        assert!(effective
            .bookmarked_notes
            .iter()
            .any(|path| path == "notes/Beta.md"));
        assert_eq!(
            effective
                .keymap_overrides
                .get("open_vault")
                .map(String::as_str),
            Some("Ctrl+Shift+O")
        );

        let _ = fs::remove_dir_all(&user_dir);
        let _ = fs::remove_dir_all(&project_dir);
    }
}
