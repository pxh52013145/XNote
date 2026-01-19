export interface NoteFile {
  /**
   * Vault-relative path using POSIX separators, e.g. "folder/note.md".
   */
  path: string
  mtimeMs: number
  size: number
}

export type VaultTab = { type: 'note'; path: string } | { type: 'graph' }

export interface VaultLayout {
  isLeftSidebarOpen: boolean
  isRightSidebarOpen: boolean
  leftWidth: number
  rightWidth: number
  leftView: 'explorer' | 'search'
  rightTab: 'outline' | 'backlinks' | 'properties' | 'assistant'
  tabs?: VaultTab[]
  activeTab?: VaultTab | null
}

export interface VaultSettings {
  newNotesFolder?: string
  attachmentsFolder?: string
}

export interface XNoteSettings {
  vaultPath?: string
  recentVaults?: string[]
  vaultLayouts?: Record<string, VaultLayout>
  vaultSettings?: Record<string, VaultSettings>
  hotkeys?: Record<string, string>
}

export type VaultChangeEvent =
  | { type: 'upsert'; path: string }
  | { type: 'delete'; path: string }
  | { type: 'rebuild' }
