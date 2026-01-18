export interface NoteFile {
  /**
   * Vault-relative path using POSIX separators, e.g. "folder/note.md".
   */
  path: string
  mtimeMs: number
  size: number
}

export interface XNoteSettings {
  vaultPath?: string
}

export type VaultChangeEvent =
  | { type: 'upsert'; path: string }
  | { type: 'delete'; path: string }
  | { type: 'rebuild' }
