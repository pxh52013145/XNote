import { contextBridge, ipcRenderer } from 'electron'
import type { XNoteAPI } from '@shared/api'
import type { NoteFile, VaultChangeEvent, VaultLayout, VaultSettings } from '@shared/types'
import type { AgentToolDefinition, AgentToolResult } from '@shared/agent'

const api: XNoteAPI = {
  async getVaultPath() {
    return (await ipcRenderer.invoke('settings:getVaultPath')) as string | null
  },
  async listRecentVaults() {
    return (await ipcRenderer.invoke('vault:listRecentVaults')) as string[]
  },
  async getVaultLayout(vaultPath) {
    return (await ipcRenderer.invoke('vault:getLayout', vaultPath)) as VaultLayout | null
  },
  async saveVaultLayout(vaultPath, layout) {
    await ipcRenderer.invoke('vault:saveLayout', vaultPath, layout)
  },
  async getVaultSettings(vaultPath) {
    return (await ipcRenderer.invoke('vault:getSettings', vaultPath)) as VaultSettings | null
  },
  async saveVaultSettings(vaultPath, settings) {
    await ipcRenderer.invoke('vault:saveSettings', vaultPath, settings)
  },
  async getHotkeys() {
    return (await ipcRenderer.invoke('hotkeys:get')) as Record<string, string>
  },
  async setHotkey(commandId, hotkey) {
    await ipcRenderer.invoke('hotkeys:set', commandId, hotkey)
  },
  async clearHotkey(commandId) {
    await ipcRenderer.invoke('hotkeys:clear', commandId)
  },
  async selectVault() {
    return (await ipcRenderer.invoke('vault:selectVault')) as string | null
  },
  async openVaultPath(vaultPath) {
    return (await ipcRenderer.invoke('vault:openVault', vaultPath)) as string
  },
  async listNotes() {
    return (await ipcRenderer.invoke('vault:listNotes')) as NoteFile[]
  },
  async listFolders() {
    return (await ipcRenderer.invoke('vault:listFolders')) as string[]
  },
  async createFolder(folderPath) {
    return (await ipcRenderer.invoke('vault:createFolder', folderPath)) as string
  },
  async renameFolder(fromPath, toPath) {
    return (await ipcRenderer.invoke('vault:renameFolder', fromPath, toPath)) as string
  },
  async saveAttachment(args) {
    return (await ipcRenderer.invoke('vault:saveAttachment', args)) as string
  },
  async readVaultFile(filePath) {
    return (await ipcRenderer.invoke('vault:readVaultFile', filePath)) as { data: ArrayBuffer; mime: string | null }
  },
  async openVaultFile(filePath) {
    await ipcRenderer.invoke('vault:openVaultFile', filePath)
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
