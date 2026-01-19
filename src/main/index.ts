import { app, BrowserWindow, dialog, ipcMain, shell } from 'electron'
import path from 'node:path'
import fs from 'node:fs/promises'
import { loadSettings, saveSettings } from './settings'
import type { VaultLayout, VaultSettings } from '@shared/types'
import {
  createFolder,
  createNoteFile,
  deleteNoteFile,
  listVaultFolders,
  listMarkdownFiles,
  readNoteFile,
  readVaultFileBinary,
  renameFolder,
  renameNoteFile,
  saveAttachmentFile,
  writeNoteFile
} from './vault'
import { listBacklinks } from './backlinks'
import { VaultIndex } from './vaultIndex'
import { AgentToolRegistry } from './agent/registry'
import { registerVaultTools } from './agent/tools/vault'

let mainWindow: BrowserWindow | null = null
let vaultPath: string | null = null
let vaultIndex: VaultIndex | null = null
const agentTools = new AgentToolRegistry()

registerVaultTools(agentTools, () => vaultPath)

const MAX_RECENT_VAULTS = 10

function setVault(next: string | null): void {
  vaultPath = next
  vaultIndex?.dispose()
  vaultIndex = next
    ? new VaultIndex(next, (evt) => {
        mainWindow?.webContents.send('vault:changed', evt)
      })
    : null
}

async function isDirectory(p: string): Promise<boolean> {
  try {
    return (await fs.stat(p)).isDirectory()
  } catch {
    return false
  }
}

function normalizeVaultPath(p: string): string {
  return path.normalize(p)
}

function vaultPathKey(p: string): string {
  const normalized = normalizeVaultPath(p)
  return process.platform === 'win32' ? normalized.toLowerCase() : normalized
}

function dedupeRecentVaults(list: string[]): string[] {
  const out: string[] = []
  const seen = new Set<string>()
  for (const p of list) {
    const normalized = normalizeVaultPath(p)
    const key = vaultPathKey(normalized)
    if (seen.has(key)) continue
    seen.add(key)
    out.push(normalized)
    if (out.length >= MAX_RECENT_VAULTS) break
  }
  return out
}

function resolvePreloadPath(): string {
  return path.join(__dirname, '../preload/index.js')
}

function resolveIndexHtmlPath(): string {
  return path.join(__dirname, '../renderer/index.html')
}

async function createWindow(): Promise<void> {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    minWidth: 960,
    minHeight: 600,
    backgroundColor: '#1e1e1e',
    frame: false,
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'hidden',
    autoHideMenuBar: true,
    webPreferences: {
      preload: resolvePreloadPath(),
      contextIsolation: true,
      nodeIntegration: false
    }
  })

  mainWindow.setMenuBarVisibility(false)

  mainWindow.webContents.on('did-finish-load', () => {
    console.log('[renderer] did-finish-load')
  })

  mainWindow.webContents.on('did-fail-load', (_event, errorCode, errorDescription, validatedURL, isMainFrame) => {
    if (!isMainFrame) return
    console.error('[renderer] did-fail-load', { errorCode, errorDescription, validatedURL })
  })

  mainWindow.webContents.on('render-process-gone', (_event, details) => {
    console.error('[renderer] render-process-gone', details)
  })

  mainWindow.webContents.on('console-message', (_event, level, message, line, sourceId) => {
    if (level < 1) return
    const tag = level === 1 ? 'warn' : level === 2 ? 'error' : 'log'
    console.error(`[renderer] console.${tag}`, message, sourceId ? `(${sourceId}:${line})` : '')
  })

  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    shell.openExternal(url)
    return { action: 'deny' }
  })

  const emitMaximizedChanged = () => {
    if (!mainWindow) return
    mainWindow.webContents.send('window:maximizedChanged', mainWindow.isMaximized())
  }
  mainWindow.on('maximize', emitMaximizedChanged)
  mainWindow.on('unmaximize', emitMaximizedChanged)

  const devUrl = process.env.VITE_DEV_SERVER_URL || process.env.ELECTRON_RENDERER_URL

  if (devUrl) {
    await mainWindow.loadURL(devUrl)
  } else {
    await mainWindow.loadFile(resolveIndexHtmlPath())
  }

  emitMaximizedChanged()

  mainWindow.on('closed', () => {
    mainWindow = null
  })
}

async function restoreVaultFromSettings(): Promise<void> {
  const settings = await loadSettings()
  if (!settings.vaultPath) return
  if (!(await isDirectory(settings.vaultPath))) return
  setVault(settings.vaultPath)
}

function registerIpc(): void {
  ipcMain.handle('settings:getVaultPath', async () => {
    return vaultPath
  })

  ipcMain.handle('hotkeys:get', async () => {
    const settings = await loadSettings()
    return settings.hotkeys ?? {}
  })

  ipcMain.handle('hotkeys:set', async (_event, commandId: string, hotkey: string) => {
    const trimmedId = typeof commandId === 'string' ? commandId.trim() : ''
    const trimmedHotkey = typeof hotkey === 'string' ? hotkey.trim() : ''

    if (!trimmedId) throw new Error('Invalid commandId')
    if (!trimmedHotkey) throw new Error('Invalid hotkey')

    const settings = await loadSettings()
    const nextHotkeys = { ...(settings.hotkeys ?? {}) }
    nextHotkeys[trimmedId] = trimmedHotkey
    await saveSettings({ ...settings, hotkeys: nextHotkeys })
  })

  ipcMain.handle('hotkeys:clear', async (_event, commandId: string) => {
    const trimmedId = typeof commandId === 'string' ? commandId.trim() : ''
    if (!trimmedId) throw new Error('Invalid commandId')

    const settings = await loadSettings()
    if (!settings.hotkeys || !(trimmedId in settings.hotkeys)) return

    const nextHotkeys = { ...settings.hotkeys }
    delete nextHotkeys[trimmedId]
    await saveSettings({ ...settings, hotkeys: Object.keys(nextHotkeys).length > 0 ? nextHotkeys : undefined })
  })

  ipcMain.handle('vault:getLayout', async (_event, selected: string) => {
    const settings = await loadSettings()
    const key = vaultPathKey(selected)
    return settings.vaultLayouts?.[key] ?? null
  })

  ipcMain.handle('vault:saveLayout', async (_event, selected: string, layout: VaultLayout) => {
    const settings = await loadSettings()
    const key = vaultPathKey(selected)
    const nextLayouts = { ...(settings.vaultLayouts ?? {}) }
    nextLayouts[key] = layout
    await saveSettings({ ...settings, vaultLayouts: nextLayouts })
  })

  ipcMain.handle('vault:getSettings', async (_event, selected: string) => {
    const settings = await loadSettings()
    const key = vaultPathKey(selected)
    return settings.vaultSettings?.[key] ?? null
  })

  function sanitizeVaultRelativeFolder(value: unknown): string | undefined {
    if (typeof value !== 'string') return undefined
    const trimmed = value.trim()
    if (!trimmed) return undefined
    const normalized = trimmed.replace(/\\/g, '/').replace(/^\/+/, '').replace(/\/+$/, '')
    if (!normalized) return undefined
    const parts = normalized.split('/').filter(Boolean)
    if (parts.length === 0) return undefined
    if (parts.some((p) => p === '.' || p === '..')) return undefined
    return parts.join('/')
  }

  ipcMain.handle('vault:saveSettings', async (_event, selected: string, next: VaultSettings) => {
    const settings = await loadSettings()
    const key = vaultPathKey(selected)

    const sanitized: VaultSettings = {
      newNotesFolder: sanitizeVaultRelativeFolder(next?.newNotesFolder),
      attachmentsFolder: sanitizeVaultRelativeFolder(next?.attachmentsFolder)
    }

    const hasValues = Boolean(sanitized.newNotesFolder || sanitized.attachmentsFolder)
    const current = { ...(settings.vaultSettings ?? {}) }

    if (!hasValues) {
      if (key in current) delete current[key]
      await saveSettings({ ...settings, vaultSettings: Object.keys(current).length > 0 ? current : undefined })
      return
    }

    current[key] = sanitized
    await saveSettings({ ...settings, vaultSettings: current })
  })

  ipcMain.handle('vault:listRecentVaults', async () => {
    const settings = await loadSettings()
    const seed = settings.recentVaults && settings.recentVaults.length > 0 ? settings.recentVaults : settings.vaultPath ? [settings.vaultPath] : []
    const deduped = dedupeRecentVaults(seed)

    const existing: string[] = []
    for (const p of deduped) {
      if (await isDirectory(p)) existing.push(p)
    }
    return existing
  })

  ipcMain.handle('vault:openVault', async (_event, selected: string) => {
    if (!(await isDirectory(selected))) {
      throw new Error('Selected vault is not a directory')
    }

    setVault(selected)

    const settings = await loadSettings()
    const nextRecent = dedupeRecentVaults([selected, ...(settings.recentVaults ?? [])])
    await saveSettings({ ...settings, vaultPath: selected, recentVaults: nextRecent })

    return selected
  })

  ipcMain.handle('vault:selectVault', async () => {
    const result = await dialog.showOpenDialog({
      properties: ['openDirectory']
    })

    if (result.canceled || result.filePaths.length === 0) {
      return null
    }

    const selected = result.filePaths[0]
    setVault(selected)

    const settings = await loadSettings()
    const nextRecent = dedupeRecentVaults([selected, ...(settings.recentVaults ?? [])])
    await saveSettings({ ...settings, vaultPath: selected, recentVaults: nextRecent })

    return selected
  })

  ipcMain.handle('vault:listNotes', async () => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    return await listMarkdownFiles(vaultPath)
  })

  ipcMain.handle('vault:listFolders', async () => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    return await listVaultFolders(vaultPath)
  })

  ipcMain.handle('vault:createFolder', async (_event, folderPath: string) => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    return await createFolder(vaultPath, folderPath)
  })

  ipcMain.handle('vault:renameFolder', async (_event, fromPath: string, toPath: string) => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }

    const renamedPath = await renameFolder(vaultPath, fromPath, toPath)
    setVault(vaultPath)
    return renamedPath
  })

  ipcMain.handle(
    'vault:saveAttachment',
    async (_event, args: { data: ArrayBuffer | Uint8Array; fileName?: string; mime?: string; notePath?: string }) => {
      if (!vaultPath) {
        throw new Error('No vault selected')
      }

      const settings = await loadSettings()
      const key = vaultPathKey(vaultPath)
      const attachmentsFolder = settings.vaultSettings?.[key]?.attachmentsFolder ?? 'attachments'

      let data: Uint8Array
      if (args.data instanceof Uint8Array) {
        data = args.data
      } else if (args.data instanceof ArrayBuffer) {
        data = new Uint8Array(args.data)
      } else {
        throw new Error('Invalid attachment data')
      }

      return await saveAttachmentFile(vaultPath, {
        attachmentsFolder,
        fileName: args.fileName,
        mime: args.mime,
        data
      })
    }
  )

  ipcMain.handle('vault:readVaultFile', async (_event, filePath: string) => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    const { data, mime } = await readVaultFileBinary(vaultPath, filePath)
    const buf = data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength)
    return { data: buf, mime }
  })

  ipcMain.handle('vault:openVaultFile', async (_event, filePath: string) => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }

    const rel = typeof filePath === 'string' ? filePath.trim().replace(/\\/g, '/').replace(/^\/+/, '') : ''
    if (!rel) throw new Error('Invalid file path')

    const fullPath = path.resolve(vaultPath, rel)
    const relative = path.relative(vaultPath, fullPath)
    if (relative === '' || relative.startsWith('..') || path.isAbsolute(relative)) {
      throw new Error('Invalid file path')
    }

    const error = await shell.openPath(fullPath)
    if (error) throw new Error(error)
  })

  ipcMain.handle('vault:readNote', async (_event, notePath: string) => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    return await readNoteFile(vaultPath, notePath)
  })

  ipcMain.handle(
    'vault:createNote',
    async (_event, notePath: string, initialContent?: string) => {
      if (!vaultPath) {
        throw new Error('No vault selected')
      }
      const createdPath = await createNoteFile(vaultPath, notePath, initialContent ?? '')
      vaultIndex?.upsertNote(createdPath, initialContent ?? '')
      return createdPath
    }
  )

  ipcMain.handle('vault:writeNote', async (_event, notePath: string, content: string) => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    await writeNoteFile(vaultPath, notePath, content)
    vaultIndex?.upsertNote(notePath, content)
  })

  ipcMain.handle('vault:renameNote', async (_event, fromPath: string, toPath: string) => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    const renamedPath = await renameNoteFile(vaultPath, fromPath, toPath)
    vaultIndex?.renameNote(fromPath, renamedPath)
    return renamedPath
  })

  ipcMain.handle('vault:deleteNote', async (_event, notePath: string) => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    await deleteNoteFile(vaultPath, notePath)
    vaultIndex?.deleteNote(notePath)
  })

  ipcMain.handle('vault:getBacklinks', async (_event, notePath: string) => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    if (vaultIndex) {
      await vaultIndex.ready()
      return vaultIndex.listBacklinks(notePath)
    }
    return await listBacklinks(vaultPath, notePath)
  })

  ipcMain.handle('vault:searchNotes', async (_event, query: string) => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    if (!vaultIndex) {
      vaultIndex = new VaultIndex(vaultPath, (evt) => {
        mainWindow?.webContents.send('vault:changed', evt)
      })
    }
    return await vaultIndex.search(query, 50)
  })

  ipcMain.handle('shell:openExternal', async (_event, url: string) => {
    await shell.openExternal(url)
  })

  ipcMain.handle('window:minimize', async () => {
    mainWindow?.minimize()
  })

  ipcMain.handle('window:toggleMaximize', async () => {
    if (!mainWindow) return
    if (mainWindow.isMaximized()) {
      mainWindow.unmaximize()
    } else {
      mainWindow.maximize()
    }
  })

  ipcMain.handle('window:isMaximized', async () => {
    return mainWindow?.isMaximized() ?? false
  })

  ipcMain.handle('window:close', async () => {
    mainWindow?.close()
  })

  ipcMain.handle('agent:listTools', async () => {
    return agentTools.listTools()
  })

  ipcMain.handle('agent:runTool', async (_event, name: string, args: unknown) => {
    return await agentTools.runTool(name, args)
  })
}

app.whenReady().then(async () => {
  await restoreVaultFromSettings()
  registerIpc()
  await createWindow()

  app.on('activate', async () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      await createWindow()
    }
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit()
  }
})
