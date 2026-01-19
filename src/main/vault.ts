import fs from 'node:fs/promises'
import path from 'node:path'
import type { NoteFile } from '@shared/types'

const DEFAULT_IGNORED_DIRS = new Set(['.git', 'node_modules', 'dist', '.vite'])

function toPosixPath(filePath: string): string {
  return filePath.split(path.sep).join(path.posix.sep)
}

function ensureInsideVault(vaultPath: string, notePath: string): string {
  const fullPath = path.resolve(vaultPath, notePath)
  const relative = path.relative(vaultPath, fullPath)

  if (relative === '' || relative.startsWith('..') || path.isAbsolute(relative)) {
    throw new Error('Invalid note path')
  }

  return fullPath
}

function normalizeMarkdownPath(notePath: string): string {
  const trimmed = notePath.trim()
  if (!trimmed) {
    throw new Error('Note path is required')
  }
  return trimmed.toLowerCase().endsWith('.md') ? trimmed : `${trimmed}.md`
}

function normalizeVaultRelativePath(filePath: string): string {
  const trimmed = filePath.trim().replace(/\\/g, '/').replace(/^\/+/, '')
  if (!trimmed) {
    throw new Error('Path is required')
  }
  return trimmed
}

function normalizeFolderPath(folderPath: string): string {
  const trimmed = folderPath.trim().replace(/\\/g, '/').replace(/^\.?\//, '').replace(/\/+$/, '')
  if (!trimmed) {
    throw new Error('Folder path is required')
  }
  const parts = trimmed.split('/').filter(Boolean)
  if (parts.length === 0) {
    throw new Error('Folder path is required')
  }
  if (parts.some((p) => p === '.' || p === '..')) {
    throw new Error('Invalid folder path')
  }
  return parts.join('/')
}

async function pathExists(p: string): Promise<boolean> {
  try {
    await fs.access(p)
    return true
  } catch {
    return false
  }
}

function extensionFromMime(mime: string): string | null {
  const lower = mime.trim().toLowerCase()
  if (lower === 'image/png') return 'png'
  if (lower === 'image/jpeg') return 'jpg'
  if (lower === 'image/jpg') return 'jpg'
  if (lower === 'image/gif') return 'gif'
  if (lower === 'image/webp') return 'webp'
  if (lower === 'image/svg+xml') return 'svg'
  return null
}

function mimeFromPath(p: string): string | null {
  const ext = path.extname(p).toLowerCase()
  if (ext === '.png') return 'image/png'
  if (ext === '.jpg' || ext === '.jpeg') return 'image/jpeg'
  if (ext === '.gif') return 'image/gif'
  if (ext === '.webp') return 'image/webp'
  if (ext === '.svg') return 'image/svg+xml'
  if (ext === '.pdf') return 'application/pdf'
  return null
}

function sanitizeFileName(name: string): string {
  const base = name.replace(/\\/g, '/').split('/').filter(Boolean).pop() ?? ''
  const cleaned = base
    .trim()
    .replace(/[<>:"/\\|?*\u0000-\u001F]/g, '-')
    .replace(/\s+/g, ' ')
    .replace(/[. ]+$/g, '')

  if (!cleaned) return 'file'
  return cleaned.length > 120 ? cleaned.slice(0, 120) : cleaned
}

async function walkFolders(
  vaultPath: string,
  dirPath: string,
  out: string[],
  options: { ignoredDirs?: Set<string> } = {}
): Promise<void> {
  const ignoredDirs = options.ignoredDirs ?? DEFAULT_IGNORED_DIRS

  const entries = await fs.readdir(dirPath, { withFileTypes: true })
  for (const entry of entries) {
    if (!entry.isDirectory()) continue
    if (entry.name.startsWith('.')) continue
    if (ignoredDirs.has(entry.name)) continue

    const fullPath = path.join(dirPath, entry.name)
    const relativePath = toPosixPath(path.relative(vaultPath, fullPath))
    out.push(relativePath)
    await walkFolders(vaultPath, fullPath, out, options)
  }
}

async function walkDir(
  vaultPath: string,
  dirPath: string,
  out: NoteFile[],
  options: { ignoredDirs?: Set<string> } = {}
): Promise<void> {
  const ignoredDirs = options.ignoredDirs ?? DEFAULT_IGNORED_DIRS

  const entries = await fs.readdir(dirPath, { withFileTypes: true })
  for (const entry of entries) {
    if (entry.name.startsWith('.')) {
      continue
    }

    const fullPath = path.join(dirPath, entry.name)
    if (entry.isDirectory()) {
      if (ignoredDirs.has(entry.name)) {
        continue
      }
      await walkDir(vaultPath, fullPath, out, options)
      continue
    }

    if (!entry.isFile()) {
      continue
    }

    if (!entry.name.toLowerCase().endsWith('.md')) {
      continue
    }

    const stat = await fs.stat(fullPath)
    const relativePath = toPosixPath(path.relative(vaultPath, fullPath))

    out.push({
      path: relativePath,
      mtimeMs: stat.mtimeMs,
      size: stat.size
    })
  }
}

export async function listMarkdownFiles(vaultPath: string): Promise<NoteFile[]> {
  const results: NoteFile[] = []
  await walkDir(vaultPath, vaultPath, results)
  results.sort((a, b) => a.path.localeCompare(b.path))
  return results
}

export async function listVaultFolders(vaultPath: string): Promise<string[]> {
  const results: string[] = []
  await walkFolders(vaultPath, vaultPath, results)
  results.sort((a, b) => a.localeCompare(b))
  return results
}

export async function readNoteFile(vaultPath: string, notePath: string): Promise<string> {
  const fullPath = ensureInsideVault(vaultPath, notePath)
  return await fs.readFile(fullPath, 'utf8')
}

export async function readVaultFileBinary(
  vaultPath: string,
  filePath: string
): Promise<{ data: Uint8Array; mime: string | null }> {
  const rel = normalizeVaultRelativePath(filePath)
  const fullPath = ensureInsideVault(vaultPath, rel)
  const data = await fs.readFile(fullPath)
  return { data, mime: mimeFromPath(rel) }
}

export async function saveAttachmentFile(
  vaultPath: string,
  options: { attachmentsFolder: string; fileName?: string; mime?: string; data: Uint8Array }
): Promise<string> {
  const attachmentsFolder = normalizeVaultRelativePath(options.attachmentsFolder)

  const desiredName = options.fileName ? sanitizeFileName(options.fileName) : ''
  const desiredExt = path.extname(desiredName).replace(/^\./, '')
  const mimeExt = options.mime ? extensionFromMime(options.mime) : null

  const ext = desiredExt || mimeExt || 'png'
  const stem = desiredName ? desiredName.replace(/\.[^/.]+$/, '') : ''

  const now = new Date()
  const stamp = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')} ${String(now.getHours()).padStart(2, '0')}${String(now.getMinutes()).padStart(2, '0')}${String(now.getSeconds()).padStart(2, '0')}`
  const baseStem = sanitizeFileName(stem || `Pasted image ${stamp}`)

  const dirFullPath = ensureInsideVault(vaultPath, attachmentsFolder)
  await fs.mkdir(dirFullPath, { recursive: true })

  let candidateName = `${baseStem}.${ext}`
  let candidateRel = toPosixPath(path.posix.join(attachmentsFolder, candidateName))
  let candidateFull = ensureInsideVault(vaultPath, candidateRel)
  let suffix = 1

  while (await pathExists(candidateFull)) {
    candidateName = `${baseStem}-${suffix}.${ext}`
    candidateRel = toPosixPath(path.posix.join(attachmentsFolder, candidateName))
    candidateFull = ensureInsideVault(vaultPath, candidateRel)
    suffix++
    if (suffix > 9999) {
      throw new Error('Unable to allocate unique attachment name')
    }
  }

  await fs.writeFile(candidateFull, options.data)
  return toPosixPath(path.relative(vaultPath, candidateFull))
}

export async function createFolder(vaultPath: string, folderPath: string): Promise<string> {
  const normalized = normalizeFolderPath(folderPath)
  const fullPath = ensureInsideVault(vaultPath, normalized)

  if (await pathExists(fullPath)) {
    throw new Error('Folder already exists')
  }

  await fs.mkdir(fullPath, { recursive: true })
  return toPosixPath(path.relative(vaultPath, fullPath))
}

export async function renameFolder(vaultPath: string, fromPath: string, toPath: string): Promise<string> {
  const fromRelative = normalizeFolderPath(fromPath)
  const toRelative = normalizeFolderPath(toPath)
  const fromFullPath = ensureInsideVault(vaultPath, fromRelative)
  const toFullPath = ensureInsideVault(vaultPath, toRelative)

  const fromStat = await fs.stat(fromFullPath).catch(() => null)
  if (!fromStat || !fromStat.isDirectory()) {
    throw new Error('Source folder does not exist')
  }

  if (await pathExists(toFullPath)) {
    throw new Error('Destination already exists')
  }

  await fs.mkdir(path.dirname(toFullPath), { recursive: true })
  await fs.rename(fromFullPath, toFullPath)
  return toPosixPath(path.relative(vaultPath, toFullPath))
}

export async function createNoteFile(
  vaultPath: string,
  notePath: string,
  initialContent = ''
): Promise<string> {
  const withExt = normalizeMarkdownPath(notePath)
  const fullPath = ensureInsideVault(vaultPath, withExt)

  if (await pathExists(fullPath)) {
    throw new Error('Note already exists')
  }

  await fs.mkdir(path.dirname(fullPath), { recursive: true })
  await fs.writeFile(fullPath, initialContent, 'utf8')
  return toPosixPath(path.relative(vaultPath, fullPath))
}

export async function writeNoteFile(
  vaultPath: string,
  notePath: string,
  content: string
): Promise<void> {
  const fullPath = ensureInsideVault(vaultPath, notePath)
  await fs.mkdir(path.dirname(fullPath), { recursive: true })
  await fs.writeFile(fullPath, content, 'utf8')
}

export async function renameNoteFile(
  vaultPath: string,
  fromPath: string,
  toPath: string
): Promise<string> {
  const fromFullPath = ensureInsideVault(vaultPath, normalizeMarkdownPath(fromPath))
  const toRelative = normalizeMarkdownPath(toPath)
  const toFullPath = ensureInsideVault(vaultPath, toRelative)

  if (!(await pathExists(fromFullPath))) {
    throw new Error('Source note does not exist')
  }
  if (await pathExists(toFullPath)) {
    throw new Error('Destination already exists')
  }

  await fs.mkdir(path.dirname(toFullPath), { recursive: true })
  await fs.rename(fromFullPath, toFullPath)
  return toPosixPath(path.relative(vaultPath, toFullPath))
}

export async function deleteNoteFile(vaultPath: string, notePath: string): Promise<void> {
  const fullPath = ensureInsideVault(vaultPath, normalizeMarkdownPath(notePath))

  if (process.versions.electron) {
    try {
      const { shell } = await import('electron')
      await shell.trashItem(fullPath)
      return
    } catch {
      // fall back to permanent delete
    }
  }

  await fs.unlink(fullPath)
}
