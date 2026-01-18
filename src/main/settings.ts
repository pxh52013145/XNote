import fs from 'node:fs/promises'
import path from 'node:path'
import { app } from 'electron'
import type { XNoteSettings } from '@shared/types'

function settingsFilePath(): string {
  return path.join(app.getPath('userData'), 'settings.json')
}

export async function loadSettings(): Promise<XNoteSettings> {
  try {
    const raw = await fs.readFile(settingsFilePath(), 'utf8')
    const data = JSON.parse(raw) as XNoteSettings
    return data ?? {}
  } catch {
    return {}
  }
}

export async function saveSettings(next: XNoteSettings): Promise<void> {
  await fs.mkdir(app.getPath('userData'), { recursive: true })
  await fs.writeFile(settingsFilePath(), JSON.stringify(next, null, 2), 'utf8')
}

