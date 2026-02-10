use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const NOTE_META_VERSION_V1: u32 = 1;
static NOTE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NoteMetaTarget {
    pub kind: String,
    pub id: String,
    #[serde(default, rename = "anchor", skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NoteMetaRelation {
    #[serde(rename = "type")]
    pub relation_type: String,
    pub to: NoteMetaTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(rename = "createdAt", default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(rename = "createdBy", default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NoteMetaPins {
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub infos: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NoteMetaV1 {
    pub version: u32,
    pub id: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: String,
    #[serde(default)]
    pub relations: Vec<NoteMetaRelation>,
    #[serde(default)]
    pub pins: NoteMetaPins,
    #[serde(default)]
    pub ext: Map<String, Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl NoteMetaV1 {
    pub fn new(note_id: impl Into<String>) -> Result<Self> {
        let id = normalize_note_id(&note_id.into())?;
        Ok(Self {
            version: NOTE_META_VERSION_V1,
            id,
            updated_at: String::new(),
            relations: Vec::new(),
            pins: NoteMetaPins::default(),
            ext: Map::new(),
            extra: Map::new(),
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != NOTE_META_VERSION_V1 {
            return Err(anyhow!("unsupported note meta version: {}", self.version));
        }

        normalize_note_id(&self.id)?;

        for relation in &self.relations {
            if relation.relation_type.trim().is_empty() {
                return Err(anyhow!("note meta relation type is empty"));
            }

            let kind = relation.to.kind.trim();
            if !matches!(kind, "knowledge" | "resource" | "info") {
                return Err(anyhow!("unsupported note meta target kind: {kind}"));
            }

            if relation.to.id.trim().is_empty() {
                return Err(anyhow!("note meta relation target id is empty"));
            }
        }

        Ok(())
    }

    pub fn canonical_json(&self) -> Result<String> {
        self.validate()?;
        let mut out = serde_json::to_string_pretty(self)?;
        if !out.ends_with('\n') {
            out.push('\n');
        }
        Ok(out)
    }
}

pub fn normalize_note_id(note_id: &str) -> Result<String> {
    let id = note_id.trim();
    if id.is_empty() {
        return Err(anyhow!("note id cannot be empty"));
    }

    if id
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
    {
        return Err(anyhow!(
            "note id must contain only [A-Za-z0-9_-], got: {id}"
        ));
    }

    Ok(id.to_string())
}

pub fn generate_note_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    let pid = std::process::id() as u64;
    let seq = NOTE_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("N{millis:011X}{pid:05X}{seq:04X}")
}

pub fn extract_note_id_from_frontmatter(content: &str) -> Option<String> {
    let (body_start, body_end) = frontmatter_body_bounds(content)?;
    let frontmatter = &content[body_start..body_end];
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("id") {
            continue;
        }
        let normalized = value.trim().trim_matches('"').trim_matches('\'').trim();
        if let Ok(note_id) = normalize_note_id(normalized) {
            return Some(note_id);
        }
    }
    None
}

pub fn ensure_frontmatter_note_id(
    content: &str,
    candidate_id: &str,
) -> Result<(String, String, bool)> {
    if let Some(existing_id) = extract_note_id_from_frontmatter(content) {
        return Ok((content.to_string(), existing_id, false));
    }

    let note_id = normalize_note_id(candidate_id)?;
    let newline = detect_line_ending(content);

    if let Some((_body_start, body_end, closing_start)) = frontmatter_bounds(content) {
        let mut out = String::with_capacity(content.len() + note_id.len() + 16);
        out.push_str(&content[..body_end]);
        out.push_str(&format!("id: {note_id}{newline}"));
        out.push_str(&content[closing_start..]);
        return Ok((out, note_id, true));
    }

    let out = format!("---{newline}id: {note_id}{newline}---{newline}{content}");
    Ok((out, note_id, true))
}

fn detect_line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn frontmatter_body_bounds(content: &str) -> Option<(usize, usize)> {
    let (body_start, body_end, _) = frontmatter_bounds(content)?;
    Some((body_start, body_end))
}

fn frontmatter_bounds(content: &str) -> Option<(usize, usize, usize)> {
    let first_line_end = content.find('\n')?;
    if content[..first_line_end].trim_end_matches('\r') != "---" {
        return None;
    }

    let body_start = first_line_end + 1;
    let mut cursor = body_start;

    while cursor <= content.len() {
        let line_end = content[cursor..]
            .find('\n')
            .map(|rel| cursor + rel)
            .unwrap_or(content.len());
        let line = &content[cursor..line_end];
        if line.trim_end_matches('\r') == "---" {
            return Some((body_start, cursor, cursor));
        }
        if line_end == content.len() {
            break;
        }
        cursor = line_end + 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_note_id_rejects_invalid_characters() {
        assert!(normalize_note_id("01HABC").is_ok());
        assert!(normalize_note_id("01H_ABC-123").is_ok());
        assert!(normalize_note_id("../bad").is_err());
        assert!(normalize_note_id("bad value").is_err());
    }

    #[test]
    fn note_meta_roundtrip_json() {
        let mut meta = NoteMetaV1::new("01HABCDE").expect("new note meta");
        meta.updated_at = "2026-02-10T12:00:00Z".to_string();
        meta.relations.push(NoteMetaRelation {
            relation_type: "xnote.explains".to_string(),
            to: NoteMetaTarget {
                kind: "knowledge".to_string(),
                id: "01HTARGET".to_string(),
                anchor: Some("h1".to_string()),
                extra: Map::new(),
            },
            note: Some("demo".to_string()),
            created_at: Some("2026-02-10T12:00:00Z".to_string()),
            created_by: Some("user".to_string()),
            extra: Map::new(),
        });
        meta.pins.notes.push("01HTARGET".to_string());

        let json = meta.canonical_json().expect("json");
        let parsed: NoteMetaV1 = serde_json::from_str(&json).expect("parse");

        assert_eq!(parsed.id, "01HABCDE");
        assert_eq!(parsed.relations.len(), 1);
        assert_eq!(parsed.pins.notes, vec!["01HTARGET"]);
    }

    #[test]
    fn ensure_frontmatter_note_id_inserts_into_existing_frontmatter() {
        let content = "---\naliases: [\"Guide\"]\n---\n# Title\n";
        let (next, id, changed) = ensure_frontmatter_note_id(content, "01HNOTE").expect("ensure");
        assert!(changed);
        assert_eq!(id, "01HNOTE");
        assert!(next.contains("aliases: [\"Guide\"]\nid: 01HNOTE\n---"));
    }

    #[test]
    fn ensure_frontmatter_note_id_prepends_when_missing_frontmatter() {
        let content = "# Title\nBody\n";
        let (next, id, changed) = ensure_frontmatter_note_id(content, "01HNOTE").expect("ensure");
        assert!(changed);
        assert_eq!(id, "01HNOTE");
        assert!(next.starts_with("---\nid: 01HNOTE\n---\n# Title\n"));
    }

    #[test]
    fn ensure_frontmatter_note_id_keeps_existing_id() {
        let content = "---\nid: 01HOLD\n---\n# Title\n";
        let (next, id, changed) =
            ensure_frontmatter_note_id(content, "01HNEW").expect("ensure existing");
        assert!(!changed);
        assert_eq!(id, "01HOLD");
        assert_eq!(next, content);
    }
}
