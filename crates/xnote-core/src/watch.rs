use crate::paths::to_posix_path;
use anyhow::Result;
use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VaultWatchChange {
    NoteChanged { path: String },
    NoteRemoved { path: String },
    NoteMoved { from: String, to: String },
    FolderCreated { path: String },
    FolderRemoved { path: String },
    FolderMoved { from: String, to: String },
    RescanRequired,
}

pub struct VaultWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<notify::Event>>,
    root: PathBuf,
}

impl VaultWatcher {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = tx.send(event);
        })?;
        watcher.watch(&root, RecursiveMode::Recursive)?;

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
            root,
        })
    }

    pub fn recv_batch(
        &self,
        debounce: Duration,
        max_batch: usize,
    ) -> Result<Vec<VaultWatchChange>> {
        let first = match self.receiver.recv() {
            Ok(event) => event,
            Err(err) => anyhow::bail!("watch receiver closed: {err}"),
        };

        let mut out = Vec::new();
        self.push_event_changes(first, &mut out);

        let started = Instant::now();
        while out.len() < max_batch && started.elapsed() < debounce {
            let remain = debounce
                .checked_sub(started.elapsed())
                .unwrap_or(Duration::ZERO);
            if remain.is_zero() {
                break;
            }

            match self.receiver.recv_timeout(remain) {
                Ok(event) => self.push_event_changes(event, &mut out),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    anyhow::bail!("watch receiver disconnected")
                }
            }
        }

        dedup_changes(out)
    }

    fn push_event_changes(
        &self,
        event: notify::Result<notify::Event>,
        out: &mut Vec<VaultWatchChange>,
    ) {
        let event = match event {
            Ok(event) => event,
            Err(_) => {
                out.push(VaultWatchChange::RescanRequired);
                return;
            }
        };

        if let EventKind::Modify(ModifyKind::Name(rename_mode)) = &event.kind {
            self.push_rename_changes(*rename_mode, &event.paths, out);
            return;
        }

        for path in &event.paths {
            match event.kind {
                EventKind::Create(CreateKind::Folder) => {
                    if let Some(rel) = self.to_vault_rel_folder_path(path, true) {
                        out.push(VaultWatchChange::FolderCreated { path: rel });
                    }
                }
                EventKind::Remove(RemoveKind::Folder) => {
                    if let Some(rel) = self.to_vault_rel_folder_path(path, false) {
                        out.push(VaultWatchChange::FolderRemoved { path: rel });
                    }
                }
                EventKind::Remove(_) => {
                    if let Some(rel) = self.to_vault_rel_note_path(path) {
                        out.push(VaultWatchChange::NoteRemoved { path: rel });
                    }
                }
                EventKind::Access(_) => continue,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any | EventKind::Other => {
                    if let Some(rel) = self.to_vault_rel_note_path(path) {
                        out.push(VaultWatchChange::NoteChanged { path: rel });
                        continue;
                    }

                    if let Some(rel) = self.to_vault_rel_folder_path(path, true) {
                        out.push(VaultWatchChange::FolderCreated { path: rel });
                    }
                }
            }
        }
    }

    fn push_rename_changes(
        &self,
        rename_mode: RenameMode,
        paths: &[PathBuf],
        out: &mut Vec<VaultWatchChange>,
    ) {
        match rename_mode {
            RenameMode::Both if paths.len() >= 2 => {
                if let (Some(from), Some(to)) = (
                    self.to_vault_rel_note_path(&paths[0]),
                    self.to_vault_rel_note_path(&paths[1]),
                ) {
                    out.push(VaultWatchChange::NoteMoved { from, to });
                    return;
                }

                if let (Some(from), Some(to)) = (
                    self.to_vault_rel_folder_path(&paths[0], false),
                    self.to_vault_rel_folder_path(&paths[1], true),
                ) {
                    out.push(VaultWatchChange::FolderMoved { from, to });
                }
            }
            RenameMode::From => {
                if let Some(path) = paths.first().and_then(|p| self.to_vault_rel_note_path(p)) {
                    out.push(VaultWatchChange::NoteRemoved { path });
                    return;
                }

                if let Some(path) = paths
                    .first()
                    .and_then(|p| self.to_vault_rel_folder_path(p, false))
                {
                    out.push(VaultWatchChange::FolderRemoved { path });
                }
            }
            RenameMode::To => {
                if let Some(path) = paths.first().and_then(|p| self.to_vault_rel_note_path(p)) {
                    out.push(VaultWatchChange::NoteChanged { path });
                    return;
                }

                if let Some(path) = paths
                    .first()
                    .and_then(|p| self.to_vault_rel_folder_path(p, true))
                {
                    out.push(VaultWatchChange::FolderCreated { path });
                }
            }
            _ => {
                if paths.len() >= 2 {
                    if let (Some(from), Some(to)) = (
                        self.to_vault_rel_note_path(&paths[0]),
                        self.to_vault_rel_note_path(&paths[1]),
                    ) {
                        out.push(VaultWatchChange::NoteMoved { from, to });
                        return;
                    }

                    if let (Some(from), Some(to)) = (
                        self.to_vault_rel_folder_path(&paths[0], false),
                        self.to_vault_rel_folder_path(&paths[1], true),
                    ) {
                        out.push(VaultWatchChange::FolderMoved { from, to });
                    }
                }
            }
        }
    }

    fn to_vault_rel_note_path(&self, abs_path: &Path) -> Option<String> {
        let rel = abs_path.strip_prefix(&self.root).ok()?;
        let rel_posix = to_posix_path(rel).ok()?;
        if !rel_posix.ends_with(".md") {
            return None;
        }
        if rel_posix.starts_with(".xnote/") {
            return None;
        }
        Some(rel_posix)
    }

    fn to_vault_rel_folder_path(&self, abs_path: &Path, require_dir_metadata: bool) -> Option<String> {
        let rel = abs_path.strip_prefix(&self.root).ok()?;
        let rel_posix = to_posix_path(rel).ok()?;
        let rel_posix = rel_posix.trim_end_matches('/').to_string();

        if rel_posix.is_empty() {
            return None;
        }
        if rel_posix == ".xnote" || rel_posix.starts_with(".xnote/") {
            return None;
        }

        if require_dir_metadata && !abs_path.is_dir() {
            return None;
        }

        Some(rel_posix)
    }
}

fn dedup_changes(changes: Vec<VaultWatchChange>) -> Result<Vec<VaultWatchChange>> {
    let mut removed = std::collections::HashSet::new();
    let mut changed = std::collections::HashSet::new();
    let mut moved = std::collections::HashMap::new();
    let mut folder_created = std::collections::HashSet::new();
    let mut folder_removed = std::collections::HashSet::new();
    let mut folder_moved = std::collections::HashMap::new();
    let mut requires_rescan = false;

    for change in changes {
        match change {
            VaultWatchChange::RescanRequired => requires_rescan = true,
            VaultWatchChange::NoteMoved { from, to } => {
                if from == to {
                    continue;
                }
                changed.remove(&from);
                changed.remove(&to);
                removed.remove(&from);
                removed.remove(&to);
                moved.insert(from, to);
            }
            VaultWatchChange::NoteRemoved { path } => {
                changed.remove(&path);
                moved.remove(&path);
                moved.retain(|_, to| to != &path);
                removed.insert(path);
            }
            VaultWatchChange::NoteChanged { path } => {
                if !removed.contains(&path)
                    && !moved.contains_key(&path)
                    && !moved.values().any(|to| to == &path)
                {
                    changed.insert(path);
                }
            }
            VaultWatchChange::FolderMoved { from, to } => {
                if from == to {
                    continue;
                }
                folder_created.remove(&from);
                folder_created.remove(&to);
                folder_removed.remove(&from);
                folder_removed.remove(&to);
                folder_moved.insert(from, to);
            }
            VaultWatchChange::FolderRemoved { path } => {
                folder_created.remove(&path);
                folder_moved.remove(&path);
                folder_moved.retain(|_, to| to != &path);
                folder_removed.insert(path);
            }
            VaultWatchChange::FolderCreated { path } => {
                if !folder_removed.contains(&path)
                    && !folder_moved.contains_key(&path)
                    && !folder_moved.values().any(|to| to == &path)
                {
                    folder_created.insert(path);
                }
            }
        }
    }

    if requires_rescan {
        return Ok(vec![VaultWatchChange::RescanRequired]);
    }

    let collapsed_moved = collapse_moved_map(moved);
    let collapsed_folder_moved = collapse_moved_map(folder_moved);
    let mut out = Vec::with_capacity(
        collapsed_moved.len()
            + changed.len()
            + removed.len()
            + collapsed_folder_moved.len()
            + folder_created.len()
            + folder_removed.len(),
    );

    let mut moved_sorted = collapsed_moved.into_iter().collect::<Vec<_>>();
    moved_sorted.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    for (from, to) in moved_sorted {
        out.push(VaultWatchChange::NoteMoved { from, to });
    }

    let mut changed_sorted = changed.into_iter().collect::<Vec<_>>();
    changed_sorted.sort();
    for path in changed_sorted {
        out.push(VaultWatchChange::NoteChanged { path });
    }

    let mut removed_sorted = removed.into_iter().collect::<Vec<_>>();
    removed_sorted.sort();
    for path in removed_sorted {
        out.push(VaultWatchChange::NoteRemoved { path });
    }

    let mut folder_moved_sorted = collapsed_folder_moved.into_iter().collect::<Vec<_>>();
    folder_moved_sorted.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    for (from, to) in folder_moved_sorted {
        out.push(VaultWatchChange::FolderMoved { from, to });
    }

    let mut folder_created_sorted = folder_created.into_iter().collect::<Vec<_>>();
    folder_created_sorted.sort();
    for path in folder_created_sorted {
        out.push(VaultWatchChange::FolderCreated { path });
    }

    let mut folder_removed_sorted = folder_removed.into_iter().collect::<Vec<_>>();
    folder_removed_sorted.sort();
    for path in folder_removed_sorted {
        out.push(VaultWatchChange::FolderRemoved { path });
    }

    Ok(out)
}

fn collapse_moved_map(
    moved: std::collections::HashMap<String, String>,
) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();

    for (from, mut to) in moved.clone() {
        let mut hops = 0usize;
        while let Some(next) = moved.get(&to) {
            if next == &to || hops > moved.len() {
                break;
            }
            to = next.clone();
            hops += 1;
        }
        if from != to {
            out.insert(from, to);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_prefers_removed_over_changed_for_same_note() {
        let out = dedup_changes(vec![
            VaultWatchChange::NoteChanged {
                path: "notes/A.md".to_string(),
            },
            VaultWatchChange::NoteRemoved {
                path: "notes/A.md".to_string(),
            },
            VaultWatchChange::NoteChanged {
                path: "notes/B.md".to_string(),
            },
        ])
        .expect("dedup");

        assert_eq!(
            out,
            vec![
                VaultWatchChange::NoteChanged {
                    path: "notes/B.md".to_string()
                },
                VaultWatchChange::NoteRemoved {
                    path: "notes/A.md".to_string()
                }
            ]
        );
    }

    #[test]
    fn dedup_keeps_rescan_as_single_signal() {
        let out = dedup_changes(vec![
            VaultWatchChange::NoteChanged {
                path: "notes/A.md".to_string(),
            },
            VaultWatchChange::RescanRequired,
            VaultWatchChange::NoteRemoved {
                path: "notes/B.md".to_string(),
            },
        ])
        .expect("dedup");

        assert_eq!(out, vec![VaultWatchChange::RescanRequired]);
    }

    #[test]
    fn dedup_keeps_move_event() {
        let out = dedup_changes(vec![
            VaultWatchChange::NoteMoved {
                from: "notes/A.md".to_string(),
                to: "notes/B.md".to_string(),
            },
            VaultWatchChange::NoteChanged {
                path: "notes/B.md".to_string(),
            },
        ])
        .expect("dedup");

        assert_eq!(
            out,
            vec![VaultWatchChange::NoteMoved {
                from: "notes/A.md".to_string(),
                to: "notes/B.md".to_string()
            }]
        );
    }

    #[test]
    fn dedup_collapses_move_chain() {
        let out = dedup_changes(vec![
            VaultWatchChange::NoteMoved {
                from: "notes/A.md".to_string(),
                to: "notes/B.md".to_string(),
            },
            VaultWatchChange::NoteMoved {
                from: "notes/B.md".to_string(),
                to: "notes/C.md".to_string(),
            },
        ])
        .expect("dedup");

        assert_eq!(
            out,
            vec![
                VaultWatchChange::NoteMoved {
                    from: "notes/A.md".to_string(),
                    to: "notes/C.md".to_string()
                },
                VaultWatchChange::NoteMoved {
                    from: "notes/B.md".to_string(),
                    to: "notes/C.md".to_string()
                }
            ]
        );
    }

    #[test]
    fn dedup_keeps_folder_events() {
        let out = dedup_changes(vec![
            VaultWatchChange::FolderCreated {
                path: "notes/new".to_string(),
            },
            VaultWatchChange::FolderCreated {
                path: "notes/new".to_string(),
            },
            VaultWatchChange::FolderRemoved {
                path: "notes/old".to_string(),
            },
        ])
        .expect("dedup");

        assert_eq!(
            out,
            vec![
                VaultWatchChange::FolderCreated {
                    path: "notes/new".to_string()
                },
                VaultWatchChange::FolderRemoved {
                    path: "notes/old".to_string()
                }
            ]
        );
    }

    #[test]
    fn dedup_collapses_folder_move_chain() {
        let out = dedup_changes(vec![
            VaultWatchChange::FolderMoved {
                from: "notes/a".to_string(),
                to: "notes/b".to_string(),
            },
            VaultWatchChange::FolderMoved {
                from: "notes/b".to_string(),
                to: "notes/c".to_string(),
            },
        ])
        .expect("dedup");

        assert_eq!(
            out,
            vec![
                VaultWatchChange::FolderMoved {
                    from: "notes/a".to_string(),
                    to: "notes/c".to_string()
                },
                VaultWatchChange::FolderMoved {
                    from: "notes/b".to_string(),
                    to: "notes/c".to_string()
                }
            ]
        );
    }
}
