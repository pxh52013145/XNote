import type { NoteFile, VaultChangeEvent, VaultLayout, VaultSettings } from './types'
import type { AgentToolDefinition, AgentToolResult } from './agent'
import type { NoteSearchResult } from './search'

export interface XNoteAPI {
  getVaultPath(): Promise<string | null>
  listRecentVaults(): Promise<string[]>
  getVaultLayout(vaultPath: string): Promise<VaultLayout | null>
  saveVaultLayout(vaultPath: string, layout: VaultLayout): Promise<void>
  getVaultSettings(vaultPath: string): Promise<VaultSettings | null>
  saveVaultSettings(vaultPath: string, settings: VaultSettings): Promise<void>
  getHotkeys(): Promise<Record<string, string>>
  setHotkey(commandId: string, hotkey: string): Promise<void>
  clearHotkey(commandId: string): Promise<void>
  selectVault(): Promise<string | null>
  openVaultPath(vaultPath: string): Promise<string>
  listNotes(): Promise<NoteFile[]>
  listFolders(): Promise<string[]>
  createFolder(folderPath: string): Promise<string>
  renameFolder(fromPath: string, toPath: string): Promise<string>
  saveAttachment(args: { data: ArrayBuffer; fileName?: string; mime?: string; notePath?: string }): Promise<string>
  readVaultFile(filePath: string): Promise<{ data: ArrayBuffer; mime: string | null }>
  openVaultFile(filePath: string): Promise<void>
  readNote(notePath: string): Promise<string>
  createNote(notePath: string, initialContent?: string): Promise<string>
  writeNote(notePath: string, content: string): Promise<void>
  renameNote(fromPath: string, toPath: string): Promise<string>
  deleteNote(notePath: string): Promise<void>
  getBacklinks(notePath: string): Promise<string[]>
  searchNotes(query: string): Promise<NoteSearchResult[]>
  onVaultChanged(callback: (event: VaultChangeEvent) => void): () => void
  openExternal(url: string): Promise<void>

  // Window chrome (frameless window controls)
  windowMinimize(): Promise<void>
  windowToggleMaximize(): Promise<void>
  windowClose(): Promise<void>
  windowIsMaximized(): Promise<boolean>
  onWindowMaximizedChanged(callback: (isMaximized: boolean) => void): () => void

  // Agent framework (remote AI service can call tools via the app)
  agentListTools(): Promise<AgentToolDefinition[]>
  agentRunTool(name: string, args: unknown): Promise<AgentToolResult>
}
