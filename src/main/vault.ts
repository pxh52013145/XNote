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

async function pathExists(p: string): Promise<boolean> {
  try {
    await fs.access(p)
    return true
  } catch {
    return false
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

export async function readNoteFile(vaultPath: string, notePath: string): Promise<string> {
  const fullPath = ensureInsideVault(vaultPath, notePath)
  return await fs.readFile(fullPath, 'utf8')
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
  await fs.unlink(fullPath)
}
