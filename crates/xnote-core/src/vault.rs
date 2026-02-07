use crate::paths::{
    join_inside, normalize_folder_rel_path, normalize_vault_rel_path, to_posix_path,
};
use anyhow::{Context as _, Result};
use ignore::WalkBuilder;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEntry {
    /// Vault-relative POSIX path, e.g. `notes/Intro.md`
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultScan {
    pub notes: Vec<NoteEntry>,
    pub folders: Vec<String>,
}

impl Vault {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        if !root.is_dir() {
            anyhow::bail!("vault root is not a directory");
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Stage A: fast scan for markdown files.
    pub fn fast_scan_notes(&self) -> Result<Vec<NoteEntry>> {
        Ok(self.fast_scan_notes_and_folders()?.notes)
    }

    /// Stage A+: fast scan for markdown files and folder paths.
    pub fn fast_scan_notes_and_folders(&self) -> Result<VaultScan> {
        let mut entries = Vec::new();
        let mut folders = BTreeSet::new();

        let mut builder = WalkBuilder::new(&self.root);
        builder
            .hidden(true)
            .follow_links(false)
            .ignore(false)
            .git_ignore(true)
            .git_exclude(true)
            .git_global(true);

        for result in builder.build() {
            let dent = match result {
                Ok(d) => d,
                Err(_) => continue,
            };

            let path = dent.path();
            if dent.file_type().is_some_and(|t| t.is_dir()) {
                if path == self.root {
                    continue;
                }
                let rel = path.strip_prefix(&self.root).unwrap_or(path);
                let rel_posix = to_posix_path(rel)?;
                if rel_posix == ".xnote" || rel_posix.starts_with(".xnote/") {
                    continue;
                }
                folders.insert(rel_posix.trim_end_matches('/').to_string());
                continue;
            }

            if !dent.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }

            if path
                .extension()
                .is_none_or(|ext| ext.to_string_lossy().to_lowercase() != "md")
            {
                continue;
            }

            let rel = path.strip_prefix(&self.root).unwrap_or(path);
            let rel_posix = to_posix_path(rel)?;
            // filter out internal metadata
            if rel_posix.starts_with(".xnote/") {
                continue;
            }

            entries.push(NoteEntry { path: rel_posix });
        }

        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(VaultScan {
            notes: entries,
            folders: folders.into_iter().collect(),
        })
    }

    pub fn read_note(&self, note_path: &str) -> Result<String> {
        let rel = normalize_vault_rel_path(note_path)?;
        let full = join_inside(&self.root, &rel)?;
        std::fs::read_to_string(&full).with_context(|| format!("read note: {rel}"))
    }

    pub fn write_note(&self, note_path: &str, content: &str) -> Result<()> {
        let rel = normalize_vault_rel_path(note_path)?;
        let full = join_inside(&self.root, &rel)?;
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).with_context(|| "create note parent dir")?;
        }
        std::fs::write(&full, content).with_context(|| format!("write note: {rel}"))?;
        Ok(())
    }

    pub fn order_file_path(&self, folder: &str) -> Result<PathBuf> {
        let folder = normalize_folder_rel_path(folder)?;
        let folder = folder.trim_end_matches('/').to_string();

        let mut p = self.root.join(".xnote").join("order");
        for part in folder.split('/') {
            p.push(part);
        }
        p.set_extension("order.md");
        Ok(p)
    }

    pub fn load_folder_order(&self, folder: &str) -> Result<Vec<String>> {
        let order_path = self.order_file_path(folder)?;
        let content = match std::fs::read_to_string(&order_path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e).with_context(|| format!("read order file: {:?}", order_path)),
        };

        Ok(parse_order_md(&content))
    }

    pub fn save_folder_order(&self, folder: &str, ordered_paths: &[String]) -> Result<()> {
        let folder_norm = normalize_folder_rel_path(folder)?;
        let order_path = self.order_file_path(&folder_norm)?;
        if let Some(parent) = order_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| "create order dir")?;
        }

        let content = format_order_md(&folder_norm, ordered_paths);
        std::fs::write(&order_path, content)
            .with_context(|| format!("write order file: {:?}", order_path))?;
        Ok(())
    }
}

pub fn parse_order_md(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with("- [[") {
            continue;
        }
        let inner = match line.strip_prefix("- [[") {
            Some(s) => s,
            None => continue,
        };
        let inner = match inner.strip_suffix("]]") {
            Some(s) => s,
            None => continue,
        };
        let inner = inner.trim();
        if let Some(path) = inner.strip_prefix("path:") {
            if let Ok(norm) = normalize_vault_rel_path(path) {
                out.push(norm);
            }
        }
    }
    out
}

pub fn format_order_md(folder: &str, ordered_paths: &[String]) -> String {
    let folder = folder.trim().trim_end_matches('/');
    let mut s = String::new();
    s.push_str(&format!("# Order for {folder}/\n"));
    for p in ordered_paths {
        s.push_str(&format!("- [[path:{p}]]\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_order_extracts_paths() {
        let content = r#"
# Order for notes/
- [[path:notes/Intro.md]]
- [[path:notes/Basics.md]]
- [[id:01H...]]
"#;
        let parsed = parse_order_md(content);
        assert_eq!(parsed, vec!["notes/Intro.md", "notes/Basics.md"]);
    }

    #[test]
    fn format_order_writes_markdown_list() {
        let out = format_order_md("notes", &["notes/A.md".into()]);
        assert!(out.contains("# Order for notes/"));
        assert!(out.contains("- [[path:notes/A.md]]"));
    }

    #[test]
    fn fast_scan_notes_and_folders_includes_empty_folders() {
        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_vault_scan_empty_folder_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);

        fs::create_dir_all(temp_dir.join("notes/empty")).expect("create empty dir");
        fs::create_dir_all(temp_dir.join("notes/content")).expect("create note dir");
        fs::write(temp_dir.join("notes/content/a.md"), "# A").expect("write note");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let scan = vault.fast_scan_notes_and_folders().expect("scan");

        assert!(scan.notes.iter().any(|n| n.path == "notes/content/a.md"));
        assert!(scan.folders.iter().any(|f| f == "notes/empty"));
        assert!(scan.folders.iter().any(|f| f == "notes/content"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn fast_scan_notes_still_returns_only_notes() {
        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_vault_scan_notes_only_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);

        fs::create_dir_all(temp_dir.join("notes/empty")).expect("create empty dir");
        fs::create_dir_all(temp_dir.join("notes/content")).expect("create note dir");
        fs::write(temp_dir.join("notes/content/a.md"), "# A").expect("write note");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let notes = vault.fast_scan_notes().expect("scan notes");

        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].path, "notes/content/a.md");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
