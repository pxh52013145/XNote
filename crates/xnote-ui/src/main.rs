use gpui::{
  App, Application, Bounds, ClickEvent, Context, DragMoveEvent, ElementId, KeyDownEvent,
  MouseButton,
  SharedString, Task, Timer, Window, WindowBounds, WindowOptions, div,
  prelude::*, px, rgb, size, uniform_list,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use xnote_core::vault::{NoteEntry, Vault};

#[derive(Clone, Debug)]
enum ExplorerRow {
  Folder { folder: String, count: usize },
  Note { folder: String, path: String, file_name: String },
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
  Opening { path: PathBuf },
  Opened { vault: Vault, root_name: SharedString },
  Error { message: SharedString },
}

#[derive(Clone, Debug)]
enum ScanState {
  Idle,
  Scanning,
  Ready { note_count: usize, duration_ms: u128 },
  Error { message: SharedString },
}

struct XnoteWindow {
  vault_state: VaultState,
  scan_state: ScanState,
  explorer_rows: Vec<ExplorerRow>,
  folder_notes: HashMap<String, Vec<String>>,
  selected_note: Option<String>,
  drag_over: Option<DragOver>,
  next_order_nonce: u64,
  pending_order_nonce_by_folder: HashMap<String, u64>,
  open_note_path: Option<String>,
  open_note_loading: bool,
  open_note_dirty: bool,
  open_note_content: String,
  next_note_open_nonce: u64,
  current_note_open_nonce: u64,
  next_note_save_nonce: u64,
  pending_note_save_nonce: u64,
  status: SharedString,
}

impl XnoteWindow {
  fn new(cx: &mut Context<Self>) -> Self {
    let mut this = Self {
      vault_state: VaultState::NotConfigured,
      scan_state: ScanState::Idle,
      explorer_rows: Vec::new(),
      folder_notes: HashMap::new(),
      selected_note: None,
      drag_over: None,
      next_order_nonce: 0,
      pending_order_nonce_by_folder: HashMap::new(),
      open_note_path: None,
      open_note_loading: false,
      open_note_dirty: false,
      open_note_content: String::new(),
      next_note_open_nonce: 0,
      current_note_open_nonce: 0,
      next_note_save_nonce: 0,
      pending_note_save_nonce: 0,
      status: SharedString::from("Ready"),
    };

    if let Some(vault_path) = resolve_vault_path() {
      this.open_vault(vault_path, cx).detach();
    }

    this
  }

  fn open_vault(&mut self, vault_path: PathBuf, cx: &mut Context<Self>) -> Task<()> {
    self.vault_state = VaultState::Opening {
      path: vault_path.clone(),
    };
    self.scan_state = ScanState::Scanning;
    self.explorer_rows.clear();
    self.folder_notes.clear();
    self.selected_note = None;
    self.drag_over = None;
    self.pending_order_nonce_by_folder.clear();
    self.open_note_path = None;
    self.open_note_loading = false;
    self.open_note_dirty = false;
    self.open_note_content.clear();
    self.pending_note_save_nonce = 0;
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
            let (rows, folder_notes) = build_explorer_data(&vault, &entries)?;
            let duration_ms = started_at.elapsed().as_millis();

            Ok::<_, anyhow::Error>((
              vault,
              SharedString::from(root_name),
              rows,
              folder_notes,
              entries.len(),
              duration_ms,
            ))
          })
          .await;

        this
          .update(&mut cx, |this, cx| match result {
            Ok((vault, root_name, rows, folder_notes, note_count, duration_ms)) => {
              this.vault_state = VaultState::Opened { vault, root_name };
              this.scan_state = ScanState::Ready {
                note_count,
                duration_ms,
              };
              this.explorer_rows = rows;
              this.folder_notes = folder_notes;
              this.status = SharedString::from("Ready");
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
              this.folder_notes.clear();
              this.status = SharedString::from("Scan failed");
              cx.notify();
            }
          })
          .ok();
      }
    })
  }

  fn retry_open_vault(&mut self, cx: &mut Context<Self>) {
    if let Some(vault_path) = resolve_vault_path() {
      self.open_vault(vault_path, cx).detach();
    } else {
      self.vault_state = VaultState::NotConfigured;
      self.scan_state = ScanState::Idle;
      self.explorer_rows.clear();
      self.folder_notes.clear();
      self.selected_note = None;
      self.drag_over = None;
      self.pending_order_nonce_by_folder.clear();
      self.open_note_path = None;
      self.open_note_loading = false;
      self.open_note_dirty = false;
      self.open_note_content.clear();
      self.pending_note_save_nonce = 0;
      self.status = SharedString::from("No vault configured");
    }
  }

  fn vault(&self) -> Option<Vault> {
    match &self.vault_state {
      VaultState::Opened { vault, .. } => Some(vault.clone()),
      _ => None,
    }
  }

  fn open_note(&mut self, note_path: String, cx: &mut Context<Self>) {
    let Some(vault) = self.vault() else {
      return;
    };

    self.selected_note = Some(note_path.clone());
    self.open_note_path = Some(note_path.clone());
    self.open_note_loading = true;
    self.open_note_dirty = false;
    self.open_note_content.clear();
    self.pending_note_save_nonce = 0;

    self.next_note_open_nonce = self.next_note_open_nonce.wrapping_add(1);
    let open_nonce = self.next_note_open_nonce;
    self.current_note_open_nonce = open_nonce;

    self.status = SharedString::from(format!("Loading note: {note_path}"));
    cx.notify();

    cx.spawn(move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
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

        this
          .update(&mut cx, |this, cx| {
            if this.current_note_open_nonce != open_nonce || this.open_note_path.as_deref() != Some(note_path.as_str()) {
              return;
            }

            this.open_note_loading = false;
            match read_result {
              Ok(content) => {
                this.open_note_content = content;
                this.status = SharedString::from("Ready");
              }
              Err(err) => {
                this.open_note_content = format!("Failed to load note: {err}");
                this.status = SharedString::from("Failed to load note");
              }
            }

            cx.notify();
          })
          .ok();
      }
    })
    .detach();
  }

  fn on_editor_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
    if self.open_note_loading {
      return;
    }
    let Some(_note_path) = self.open_note_path.as_deref() else {
      return;
    };

    let is_save = (ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform)
      && ev.keystroke.key.eq_ignore_ascii_case("s");
    if is_save {
      self.force_save_note(cx);
      return;
    }

    let key = ev.keystroke.key.to_lowercase();
    match key.as_str() {
      "backspace" => {
        if self.open_note_content.pop().is_some() {
          self.open_note_dirty = true;
          self.status = SharedString::from("Editing...");
          self.schedule_save_note(Duration::from_millis(500), cx);
          cx.notify();
        }
      }
      "enter" | "return" => {
        self.open_note_content.push('\n');
        self.open_note_dirty = true;
        self.status = SharedString::from("Editing...");
        self.schedule_save_note(Duration::from_millis(500), cx);
        cx.notify();
      }
      "tab" => {
        self.open_note_content.push('\t');
        self.open_note_dirty = true;
        self.status = SharedString::from("Editing...");
        self.schedule_save_note(Duration::from_millis(500), cx);
        cx.notify();
      }
      _ => {
        if ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform {
          return;
        }
        let Some(text) = ev.keystroke.key_char.as_ref() else {
          return;
        };
        if text.is_empty() {
          return;
        }
        self.open_note_content.push_str(text);
        self.open_note_dirty = true;
        self.status = SharedString::from("Editing...");
        self.schedule_save_note(Duration::from_millis(500), cx);
        cx.notify();
      }
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
    if self.open_note_loading || !self.open_note_dirty {
      return;
    }

    self.next_note_save_nonce = self.next_note_save_nonce.wrapping_add(1);
    let save_nonce = self.next_note_save_nonce;
    self.pending_note_save_nonce = save_nonce;
    let open_nonce = self.current_note_open_nonce;

    cx.spawn(move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
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

        this
          .update(&mut cx, |this, cx| {
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
              Err(err) => this.status = SharedString::from(format!("Save failed: {err}")),
            }

            cx.notify();
          })
          .ok();
      }
    })
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

    self.replace_folder_rows(folder);
    true
  }

  fn replace_folder_rows(&mut self, folder: &str) {
    let Some(order) = self.folder_notes.get(folder) else {
      return;
    };

    let mut header_ix = None;
    for (ix, row) in self.explorer_rows.iter().enumerate() {
      if matches!(row, ExplorerRow::Folder { folder: f, .. } if f == folder) {
        header_ix = Some(ix);
        break;
      }
    }
    let Some(header_ix) = header_ix else {
      return;
    };

    let start_ix = header_ix + 1;
    let mut end_ix = start_ix;
    while end_ix < self.explorer_rows.len() {
      match &self.explorer_rows[end_ix] {
        ExplorerRow::Note { folder: f, .. } if f == folder => end_ix += 1,
        _ => break,
      }
    }

    if let Some(ExplorerRow::Folder { count, .. }) = self.explorer_rows.get_mut(header_ix) {
      *count = order.len();
    }

    let folder_owned = folder.to_string();
    let new_rows = order.iter().map(|path| ExplorerRow::Note {
      folder: folder_owned.clone(),
      path: path.clone(),
      file_name: file_name(path),
    });

    self.explorer_rows.splice(start_ix..end_ix, new_rows);
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
    self.pending_order_nonce_by_folder.insert(folder.clone(), nonce);

    self.status = SharedString::from(format!("Saving order: {folder}/"));
    cx.notify();

    cx.spawn(move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
      let mut cx = cx.clone();
      let vault = vault.clone();
      let folder = folder.clone();
      async move {
        Timer::after(Duration::from_millis(250)).await;

        let order_to_save = this
          .update(&mut cx, |this, _cx| match this.pending_order_nonce_by_folder.get(&folder) {
            Some(n) if *n == nonce => this.folder_notes.get(&folder).cloned(),
            _ => None,
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

        this
          .update(&mut cx, |this, cx| {
            match save_result {
              Ok(()) => this.status = SharedString::from(format!("Saved order: {folder}/")),
              Err(err) => this.status = SharedString::from(format!("Failed to save order: {err}")),
            }
            cx.notify();
          })
          .ok();
      }
    })
    .detach();
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
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let (vault_title, vault_hint) = match &self.vault_state {
      VaultState::Opened { root_name, .. } => (
        SharedString::from(format!("XNote - {root_name}")),
        SharedString::from(""),
      ),
      VaultState::Opening { path } => (
        SharedString::from("XNote - Opening vault..."),
        SharedString::from(path.display().to_string()),
      ),
      VaultState::Error { message } => (
        SharedString::from("XNote - Vault error"),
        SharedString::from(message.to_string()),
      ),
      VaultState::NotConfigured => (
        SharedString::from("XNote - No vault configured"),
        SharedString::from("Set env XNOTE_VAULT or run with --vault <path>"),
      ),
    };

    let scan_hint = match &self.scan_state {
      ScanState::Idle => SharedString::from(""),
      ScanState::Scanning => SharedString::from("Scanning..."),
      ScanState::Ready {
        note_count,
        duration_ms,
      } => SharedString::from(format!("{note_count} notes ({duration_ms} ms)")),
      ScanState::Error { message } => SharedString::from(message.to_string()),
    };

    let sidebar = div()
      .w(px(280.))
      .h_full()
      .bg(rgb(0xffffff))
      .border_r_1()
      .border_color(rgb(0xe0e0e0))
      .child(
        div()
          .h(px(36.))
          .px_3()
          .flex()
          .items_center()
          .justify_between()
          .border_b_1()
          .border_color(rgb(0xe0e0e0))
          .child("Explorer")
          .child(
            div()
              .text_sm()
              .text_color(rgb(0x6b7280))
              .child(scan_hint),
          )
          .child(
            div()
              .id("explorer.reload")
              .ml_2()
              .px_2()
              .py_1()
              .rounded_md()
              .bg(rgb(0xf3f4f6))
              .cursor_pointer()
              .hover(|this| this.bg(rgb(0xe5e7eb)))
              .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| this.retry_open_vault(cx)))
              .child("Reload"),
          ),
      )
      .child(
        div()
          .id("explorer.list")
          .h_full()
          .on_mouse_up(MouseButton::Left, cx.listener(|this, _ev, _window, cx| this.clear_drag_over(cx)))
          .child(
            uniform_list(
              "explorer",
              self.explorer_rows.len(),
              cx.processor(|this, range: std::ops::Range<usize>, _window, cx| {
                range
                  .map(|ix| match this.explorer_rows.get(ix) {
                    Some(ExplorerRow::Folder { folder, count }) => {
                      let label = if folder.is_empty() {
                        SharedString::from(format!("Root ({count})"))
                      } else {
                        SharedString::from(format!("{folder}/ ({count})"))
                      };
                      div()
                        .id(ElementId::Name(SharedString::from(format!(
                          "folder:{}",
                          if folder.is_empty() { "<root>" } else { folder.as_str() }
                        ))))
                        .px_3()
                        .py_2()
                        .text_sm()
                        .text_color(rgb(0x6b7280))
                        .bg(rgb(0xf9fafb))
                        .border_b_1()
                        .border_color(rgb(0xe5e7eb))
                        .child(label)
                    }
                    Some(ExplorerRow::Note {
                      folder,
                      path,
                      file_name,
                    }) => {
                      let is_selected = this.selected_note.as_deref() == Some(path.as_str());
                      let drag_over = this
                        .drag_over
                        .as_ref()
                        .filter(|d| d.folder == *folder && d.target_path == *path);
                      let insert_after = drag_over.map(|d| d.insert_after);

                      let row_id = ElementId::Name(SharedString::from(format!("note:{path}")));
                      let dragged_value = DraggedNote {
                        folder: folder.clone(),
                        path: path.clone(),
                      };

                      let target_folder = folder.clone();
                      let target_path = path.clone();
                      let selected_path = path.clone();
                      let display_name = file_name.clone();

                      div()
                        .id(row_id)
                        .px_3()
                        .py_2()
                        .cursor_pointer()
                        .when(is_selected, |this| this.bg(rgb(0xe0f2fe)))
                        .hover(|this| this.bg(rgb(0xf5f6f8)))
                        .when_some(insert_after, |this, insert_after| {
                          if insert_after {
                            this.border_b_1().border_color(rgb(0x2563eb))
                          } else {
                            this.border_t_1().border_color(rgb(0x2563eb))
                          }
                        })
                        .on_click(cx.listener(move |this, _ev: &ClickEvent, _window, cx| {
                          this.open_note(selected_path.clone(), cx);
                        }))
                        .on_drag(
                          dragged_value,
                          |dragged, _offset, _window, cx| {
                            cx.new(|_| DragPreview {
                              label: SharedString::from(dragged.path.clone()),
                            })
                          },
                        )
                        .can_drop({
                          let target_folder = target_folder.clone();
                          let target_path = target_path.clone();
                          move |dragged, _window, _cx| {
                            dragged
                              .downcast_ref::<DraggedNote>()
                              .is_some_and(|d| !target_folder.is_empty() && d.folder == target_folder && d.path != target_path)
                          }
                        })
                        .on_drag_move::<DraggedNote>(cx.listener({
                          let target_folder = target_folder.clone();
                          let target_path = target_path.clone();
                          move |this, ev: &DragMoveEvent<DraggedNote>, _window, cx| {
                            let Some(dragged) = ev.dragged_item().downcast_ref::<DraggedNote>() else {
                              return;
                            };
                            if dragged.folder != target_folder || target_folder.is_empty() {
                              return;
                            }
                            let mid_y = ev.bounds.origin.y + ev.bounds.size.height * 0.5;
                            let insert_after = ev.event.position.y >= mid_y;
                            this.set_drag_over(target_folder.clone(), target_path.clone(), insert_after, cx);
                          }
                        }))
                        .on_drop::<DraggedNote>(cx.listener({
                          let target_folder = target_folder.clone();
                          let target_path = target_path.clone();
                          move |this, dragged: &DraggedNote, _window, cx| {
                            this.handle_drop(dragged, &target_folder, &target_path, cx);
                          }
                        }))
                        .child(display_name)
                    }
                    None => div().id(ElementId::named_usize("explorer.missing", ix)).px_3().py_2().child(""),
                  })
                  .collect::<Vec<_>>()
              }),
            )
            .h_full(),
          ),
      );

    let editor_note_label = match self.open_note_path.as_deref() {
      Some(path) => {
        if self.open_note_dirty {
          format!("{path} *")
        } else {
          path.to_string()
        }
      }
      None => "No note selected".to_string(),
    };

    let editor_body_text = if self.open_note_path.is_none() {
      "Select a note in Explorer to open.".to_string()
    } else if self.open_note_loading {
      "Loading...".to_string()
    } else {
      self.open_note_content.clone()
    };

    let editor = div()
      .flex_1()
      .h_full()
      .bg(rgb(0xffffff))
      .border_r_1()
      .border_color(rgb(0xe0e0e0))
      .flex()
      .flex_col()
      .child(
        div()
          .h(px(36.))
          .px_3()
          .flex()
          .items_center()
          .justify_between()
          .border_b_1()
          .border_color(rgb(0xe0e0e0))
          .child("Editor")
          .child(
            div()
              .flex()
              .items_center()
              .gap_2()
              .child(
                div()
                  .text_sm()
                  .text_color(rgb(0x6b7280))
                  .child(editor_note_label),
              )
              .child(
                div()
                  .id("editor.save")
                  .px_2()
                  .py_1()
                  .rounded_md()
                  .bg(rgb(0xf3f4f6))
                  .cursor_pointer()
                  .hover(|this| this.bg(rgb(0xe5e7eb)))
                  .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                    this.force_save_note(cx);
                  }))
                  .child("Save (Ctrl+S)"),
              ),
          ),
      )
      .child(
        div()
          .id("editor.body")
          .flex_1()
          .overflow_y_scroll()
          .focusable()
          .p_3()
          .text_color(rgb(0x111827))
          .bg(rgb(0xffffff))
          .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
            this.on_editor_key(ev, cx);
          }))
          .child(editor_body_text),
      );

    let inspector = div()
      .w(px(320.))
      .h_full()
      .bg(rgb(0xffffff))
      .child(
        div()
          .h(px(36.))
          .px_3()
          .flex()
          .items_center()
          .justify_between()
          .border_b_1()
          .border_color(rgb(0xe0e0e0))
          .child("Inspector")
          .child(div().text_sm().text_color(rgb(0x6b7280)).child("Backlinks")),
      )
      .child(
        div()
          .p_4()
          .text_color(rgb(0x374151))
          .child("Outline / Backlinks / Links (placeholder)"),
      );

    let top = div()
      .h(px(33.))
      .w_full()
      .bg(rgb(0xeceff3))
      .border_b_1()
      .border_color(rgb(0xe0e0e0))
      .flex()
      .items_center()
      .justify_between()
      .px_3()
      .child(vault_title)
      .child(div().text_sm().text_color(rgb(0x6b7280)).child(vault_hint));

    let bottom = div()
      .h(px(28.))
      .w_full()
      .bg(rgb(0xeceff3))
      .border_t_1()
      .border_color(rgb(0xe0e0e0))
      .flex()
      .items_center()
      .justify_between()
      .px_3()
      .child("Module: Knowledge")
      .child(
        div()
          .text_sm()
          .text_color(rgb(0x6b7280))
          .child(self.status.clone()),
      );

    div()
      .size_full()
      .bg(rgb(0xf5f6f8))
      .flex()
      .flex_col()
      .child(top)
      .child(div().flex().flex_1().child(sidebar).child(editor).child(inspector))
      .child(bottom)
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

fn build_explorer_data(
  vault: &Vault,
  entries: &[NoteEntry],
) -> anyhow::Result<(Vec<ExplorerRow>, HashMap<String, Vec<String>>)> {
  let mut by_folder: BTreeMap<String, Vec<String>> = BTreeMap::new();
  for e in entries {
    let folder = match e.path.rsplit_once('/') {
      Some((folder, _)) => folder.to_string(),
      None => String::new(),
    };
    by_folder.entry(folder).or_default().push(e.path.clone());
  }

  let mut folder_notes = HashMap::with_capacity(by_folder.len());
  let mut rows = Vec::with_capacity(entries.len() + by_folder.len());
  for (folder, mut default_paths) in by_folder {
    default_paths.sort();

    let ordered_paths = if folder.is_empty() {
      default_paths
    } else {
      let order = vault.load_folder_order(&folder)?;
      apply_folder_order(&default_paths, &order)
    };

    folder_notes.insert(folder.clone(), ordered_paths.clone());
    rows.push(ExplorerRow::Folder {
      folder: folder.clone(),
      count: ordered_paths.len(),
    });
    for path in ordered_paths {
      rows.push(ExplorerRow::Note {
        folder: folder.clone(),
        file_name: file_name(&path),
        path,
      });
    }
  }

  Ok((rows, folder_notes))
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

fn main() {
  Application::new().run(|cx: &mut App| {
    let bounds = Bounds::centered(None, size(px(1200.0), px(760.0)), cx);
    cx.open_window(
      WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
      },
      |_, cx| cx.new(|cx| XnoteWindow::new(cx)),
    )
    .unwrap();
    cx.activate(true);
  });
}
