import { extractWikiLinks } from '@shared/links'
import { noteTitleFromPath } from '@shared/paths'
import { listMarkdownFiles, readNoteFile } from './vault'

function stripMarkdownExt(p: string): string {
  return p.toLowerCase().endsWith('.md') ? p.slice(0, -3) : p
}

function normalizeWikiTarget(target: string): string {
  return target.trim().replace(/\\/g, '/').replace(/^\.?\//, '').replace(/\.md$/i, '')
}

export async function listBacklinks(vaultPath: string, targetNotePath: string): Promise<string[]> {
  const targetPathNoExt = stripMarkdownExt(targetNotePath).toLowerCase()
  const targetTitle = noteTitleFromPath(targetNotePath).toLowerCase()

  const files = await listMarkdownFiles(vaultPath)
  const backlinks: string[] = []

  for (const file of files) {
    if (file.path === targetNotePath) continue
    const content = await readNoteFile(vaultPath, file.path)
    const links = extractWikiLinks(content)

    const matches = links.some((l) => {
      const normalized = normalizeWikiTarget(l.target)
      if (!normalized) return false
      if (normalized.includes('/')) {
        return normalized.toLowerCase() === targetPathNoExt
      }
      return normalized.toLowerCase() === targetTitle
    })

    if (matches) backlinks.push(file.path)
  }

  backlinks.sort((a, b) => a.localeCompare(b))
  return backlinks
}

