import { app, BrowserWindow, dialog, ipcMain, shell } from 'electron'
import path from 'node:path'
import fs from 'node:fs/promises'
import { loadSettings, saveSettings } from './settings'
import {
  createNoteFile,
  deleteNoteFile,
  listMarkdownFiles,
  readNoteFile,
  renameNoteFile,
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
    await saveSettings({ ...settings, vaultPath: selected })

    return selected
  })

  ipcMain.handle('vault:listNotes', async () => {
    if (!vaultPath) {
      throw new Error('No vault selected')
    }
    return await listMarkdownFiles(vaultPath)
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
