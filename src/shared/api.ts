import type { NoteFile, VaultChangeEvent } from './types'
import type { AgentToolDefinition, AgentToolResult } from './agent'
import type { NoteSearchResult } from './search'

export interface XNoteAPI {
  getVaultPath(): Promise<string | null>
  selectVault(): Promise<string | null>
  listNotes(): Promise<NoteFile[]>
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
