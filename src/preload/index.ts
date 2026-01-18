import { contextBridge, ipcRenderer } from 'electron'
import type { XNoteAPI } from '@shared/api'
import type { NoteFile, VaultChangeEvent } from '@shared/types'
import type { AgentToolDefinition, AgentToolResult } from '@shared/agent'

const api: XNoteAPI = {
  async getVaultPath() {
    return (await ipcRenderer.invoke('settings:getVaultPath')) as string | null
  },
  async selectVault() {
    return (await ipcRenderer.invoke('vault:selectVault')) as string | null
  },
  async listNotes() {
    return (await ipcRenderer.invoke('vault:listNotes')) as NoteFile[]
  },
  async readNote(notePath) {
    return (await ipcRenderer.invoke('vault:readNote', notePath)) as string
  },
  async createNote(notePath, initialContent) {
    return (await ipcRenderer.invoke('vault:createNote', notePath, initialContent)) as string
  },
  async writeNote(notePath, content) {
    await ipcRenderer.invoke('vault:writeNote', notePath, content)
  },
  async renameNote(fromPath, toPath) {
    return (await ipcRenderer.invoke('vault:renameNote', fromPath, toPath)) as string
  },
  async deleteNote(notePath) {
    await ipcRenderer.invoke('vault:deleteNote', notePath)
  },
  async getBacklinks(notePath) {
    return (await ipcRenderer.invoke('vault:getBacklinks', notePath)) as string[]
  },
  async searchNotes(query) {
    return (await ipcRenderer.invoke('vault:searchNotes', query)) as import('@shared/search').NoteSearchResult[]
  },
  onVaultChanged(callback) {
    const listener = (_event: unknown, evt: VaultChangeEvent) => callback(evt)
    ipcRenderer.on('vault:changed', listener)
    return () => {
      ipcRenderer.removeListener('vault:changed', listener)
    }
  },
  async openExternal(url) {
    await ipcRenderer.invoke('shell:openExternal', url)
  },
  async windowMinimize() {
    await ipcRenderer.invoke('window:minimize')
  },
  async windowToggleMaximize() {
    await ipcRenderer.invoke('window:toggleMaximize')
  },
  async windowClose() {
    await ipcRenderer.invoke('window:close')
  },
  async windowIsMaximized() {
    return (await ipcRenderer.invoke('window:isMaximized')) as boolean
  },
  onWindowMaximizedChanged(callback) {
    const listener = (_event: unknown, isMaximized: boolean) => callback(isMaximized)
    ipcRenderer.on('window:maximizedChanged', listener)
    return () => {
      ipcRenderer.removeListener('window:maximizedChanged', listener)
    }
  },
  async agentListTools() {
    return (await ipcRenderer.invoke('agent:listTools')) as AgentToolDefinition[]
  },
  async agentRunTool(name, args) {
    return (await ipcRenderer.invoke('agent:runTool', name, args)) as AgentToolResult
  }
}

contextBridge.exposeInMainWorld('xnote', api)
