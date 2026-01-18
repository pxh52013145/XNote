import fs from 'node:fs'
import path from 'node:path'
import { extractFrontmatter, type FrontmatterData } from '@shared/frontmatter'
import { extractWikiLinks } from '@shared/links'
import { noteTitleFromPath } from '@shared/paths'
import { buildSnippet, countOccurrences, noteTitle, parseSearchQuery, tokenizeForIndex } from '@shared/search'
import type { NoteSearchResult } from '@shared/search'
import { extractInlineTags } from '@shared/tags'
import type { VaultChangeEvent } from '@shared/types'
import { listMarkdownFiles, readNoteFile } from './vault'

type IndexedNote = {
  path: string
  title: string
  titleLower: string
  pathLower: string
  content: string
  contentLower: string
  body: string
  bodyLower: string
  frontmatter: FrontmatterData
  tokens: Set<string>
  linkKeys: Set<string>
  tags: Set<string>
}

const DEFAULT_IGNORED_DIRS = new Set(['.git', 'node_modules', 'dist', '.vite'])

function toPosixPath(p: string): string {
  return p.replace(/\\/g, '/')
}

function stripMarkdownExt(p: string): string {
  return p.toLowerCase().endsWith('.md') ? p.slice(0, -3) : p
}

function normalizeWikiTarget(target: string): string {
  return target.trim().replace(/\\/g, '/').replace(/^\.?\//, '').replace(/\.md$/i, '')
}

function isIgnoredRelativePath(rel: string): boolean {
  const normalized = rel.replace(/\\/g, '/')
  const parts = normalized.split('/').filter(Boolean)
  if (parts.some((p) => p.startsWith('.'))) return true
  if (parts.some((p) => DEFAULT_IGNORED_DIRS.has(p))) return true
  return false
}

function intersectSets(a: Set<string> | null, b: Set<string>): Set<string> {
  if (!a) return new Set(b)
  const next = new Set<string>()
  for (const item of a) {
    if (b.has(item)) next.add(item)
  }
  return next
}

function ensureInsideVault(vaultPath: string, notePath: string): string {
  const fullPath = path.resolve(vaultPath, notePath)
  const relative = path.relative(vaultPath, fullPath)
  if (relative === '' || relative.startsWith('..') || path.isAbsolute(relative)) {
    throw new Error('Invalid note path')
  }
  return fullPath
}

export class VaultIndex {
  private readonly vaultPath: string
  private readonly onChange?: (event: VaultChangeEvent) => void
  private readonly notes = new Map<string, IndexedNote>()
  private readonly tokenIndex = new Map<string, Set<string>>()
  private readonly backlinkIndex = new Map<string, Set<string>>()
  private readonly tagIndex = new Map<string, Set<string>>()
  private watcher: fs.FSWatcher | null = null
  private readonly pending = new Map<string, NodeJS.Timeout>()
  private readonly readyPromise: Promise<void>
  private suppressEvents = true

  constructor(vaultPath: string, onChange?: (event: VaultChangeEvent) => void) {
    this.vaultPath = vaultPath
    this.onChange = onChange
    this.readyPromise = this.buildInitialIndex().finally(() => {
      this.suppressEvents = false
      this.emit({ type: 'rebuild' })
    })
    this.startWatcher()
  }

  async ready(): Promise<void> {
    await this.readyPromise
  }

  dispose(): void {
    for (const t of this.pending.values()) {
      clearTimeout(t)
    }
    this.pending.clear()
    this.watcher?.close()
    this.watcher = null
  }

  private async buildInitialIndex(): Promise<void> {
    const files = await listMarkdownFiles(this.vaultPath)
    const concurrency = 16
    let idx = 0

    const workers = Array.from({ length: concurrency }, async () => {
      while (idx < files.length) {
        const cur = files[idx]
        idx++
        if (!cur) continue
        const content = await readNoteFile(this.vaultPath, cur.path)
        this.upsertNote(cur.path, content)
      }
    })

    await Promise.all(workers)
  }

  private startWatcher(): void {
    try {
      this.watcher = fs.watch(this.vaultPath, { recursive: true }, (_eventType, filename) => {
        if (!filename) {
          this.scheduleFullRebuild()
          return
        }
        const rel = filename.toString()
        if (!rel) return
        this.scheduleRescan(rel)
      })
    } catch {
      // ignore watcher failures (still functional via explicit writes)
    }
  }

  private scheduleFullRebuild(): void {
    const key = '__FULL__'
    const existing = this.pending.get(key)
    if (existing) clearTimeout(existing)
    this.pending.set(
      key,
      setTimeout(() => {
        this.pending.delete(key)
        void this.rebuildAll().catch(() => {
          // ignore
        })
      }, 300)
    )
  }

  private scheduleRescan(relativePath: string): void {
    const relPosix = toPosixPath(relativePath)
    if (isIgnoredRelativePath(relPosix)) return
    if (!relPosix.toLowerCase().endsWith('.md')) return

    const existing = this.pending.get(relPosix)
    if (existing) clearTimeout(existing)

    this.pending.set(
      relPosix,
      setTimeout(() => {
        this.pending.delete(relPosix)
        void this.rescanFromDisk(relPosix).catch(() => {
          // ignore
        })
      }, 200)
    )
  }

  private async rebuildAll(): Promise<void> {
    this.suppressEvents = true
    this.notes.clear()
    this.tokenIndex.clear()
    this.backlinkIndex.clear()
    this.tagIndex.clear()
    await this.buildInitialIndex()
    this.suppressEvents = false
    this.emit({ type: 'rebuild' })
  }

  private async rescanFromDisk(notePath: string): Promise<void> {
    const fullPath = ensureInsideVault(this.vaultPath, notePath)
    try {
      await fs.promises.access(fullPath, fs.constants.F_OK)
    } catch {
      this.deleteNote(notePath)
      return
    }

    const content = await readNoteFile(this.vaultPath, notePath)
    this.upsertNote(notePath, content)
  }

  private emit(event: VaultChangeEvent): void {
    if (this.suppressEvents) return
    this.onChange?.(event)
  }

  private removeNoteInternal(notePath: string): IndexedNote | null {
    const normalizedPath = toPosixPath(notePath)
    const existing = this.notes.get(normalizedPath)
    if (!existing) return null

    this.notes.delete(normalizedPath)

    for (const token of existing.tokens) {
      const set = this.tokenIndex.get(token)
      if (!set) continue
      set.delete(normalizedPath)
      if (set.size === 0) this.tokenIndex.delete(token)
    }

    for (const key of existing.linkKeys) {
      const set = this.backlinkIndex.get(key)
      if (!set) continue
      set.delete(normalizedPath)
      if (set.size === 0) this.backlinkIndex.delete(key)
    }

    for (const tag of existing.tags) {
      const set = this.tagIndex.get(tag)
      if (!set) continue
      set.delete(normalizedPath)
      if (set.size === 0) this.tagIndex.delete(tag)
    }

    return existing
  }

  upsertNote(notePath: string, content: string): void {
    const normalizedPath = toPosixPath(notePath)
    this.removeNoteInternal(normalizedPath)

    const extraction = extractFrontmatter(content)
    const title = noteTitle(normalizedPath, extraction.data.title)

    const contentLower = content.toLowerCase()
    const body = extraction.body
    const bodyLower = body.toLowerCase()

    const tokens = new Set<string>([
      ...tokenizeForIndex(normalizedPath),
      ...tokenizeForIndex(title),
      ...tokenizeForIndex(content),
      ...tokenizeForIndex(extraction.raw ?? '')
    ])

    const links = extractWikiLinks(content)
    const linkKeys = new Set<string>()
    for (const link of links) {
      const normalized = normalizeWikiTarget(link.target)
      if (!normalized) continue
      const key = normalized.includes('/') ? `path:${normalized.toLowerCase()}` : `title:${normalized.toLowerCase()}`
      linkKeys.add(key)
    }

    const tags = new Set<string>()
    const fmTags = extraction.data.tags
    if (typeof fmTags === 'string') {
      for (const part of fmTags.split(/[, ]+/)) {
        const tag = part.trim().replace(/^#/, '')
        if (tag) tags.add(tag)
      }
    } else if (Array.isArray(fmTags)) {
      for (const item of fmTags) {
        if (typeof item !== 'string') continue
        const tag = item.trim().replace(/^#/, '')
        if (tag) tags.add(tag)
      }
    }

    for (const tag of extractInlineTags(content)) {
      if (tag) tags.add(tag)
    }

    const indexed: IndexedNote = {
      path: normalizedPath,
      title,
      titleLower: title.toLowerCase(),
      pathLower: normalizedPath.toLowerCase(),
      content,
      contentLower,
      body,
      bodyLower,
      frontmatter: extraction.data,
      tokens,
      linkKeys,
      tags
    }

    this.notes.set(normalizedPath, indexed)

    for (const token of tokens) {
      let set = this.tokenIndex.get(token)
      if (!set) {
        set = new Set()
        this.tokenIndex.set(token, set)
      }
      set.add(normalizedPath)
    }

    for (const key of linkKeys) {
      let set = this.backlinkIndex.get(key)
      if (!set) {
        set = new Set()
        this.backlinkIndex.set(key, set)
      }
      set.add(normalizedPath)
    }

    for (const tag of tags) {
      let set = this.tagIndex.get(tag)
      if (!set) {
        set = new Set()
        this.tagIndex.set(tag, set)
      }
      set.add(normalizedPath)
    }

    this.emit({ type: 'upsert', path: normalizedPath })
  }

  deleteNote(notePath: string): void {
    const normalizedPath = toPosixPath(notePath)
    if (!this.removeNoteInternal(normalizedPath)) return
    this.emit({ type: 'delete', path: normalizedPath })
  }

  renameNote(fromPath: string, toPath: string): void {
    const from = toPosixPath(fromPath)
    const to = toPosixPath(toPath)
    const existing = this.notes.get(from)
    if (!existing) return
    this.deleteNote(from)
    this.upsertNote(to, existing.content)
  }

  listBacklinks(targetNotePath: string): string[] {
    const target = toPosixPath(targetNotePath)
    const keys = new Set<string>()
    keys.add(`path:${stripMarkdownExt(target).toLowerCase()}`)
    keys.add(`title:${noteTitleFromPath(target).toLowerCase()}`)

    const union = new Set<string>()
    for (const key of keys) {
      const set = this.backlinkIndex.get(key)
      if (!set) continue
      for (const p of set) union.add(p)
    }
    union.delete(target)
    return [...union].sort((a, b) => a.localeCompare(b))
  }

  async search(query: string, limit = 50): Promise<NoteSearchResult[]> {
    await this.ready()
    const q = parseSearchQuery(query)
    if (!q.raw) return []

    let candidates: Set<string> | null = null
    if (q.tokens.length > 0) {
      for (const token of q.tokens) {
        const set = this.tokenIndex.get(token)
        if (!set) return []
        candidates = intersectSets(candidates, set)
        if (candidates.size === 0) return []
      }
    }

    const results: NoteSearchResult[] = []
    const iter = candidates ? candidates.values() : this.notes.keys()

    for (const notePath of iter) {
      const note = this.notes.get(notePath)
      if (!note) continue

      const haystackLower = `${note.titleLower}\n${note.pathLower}\n${note.contentLower}`

      if (q.tokens.length === 0) {
        if (!haystackLower.includes(q.lower)) continue
      } else {
        let ok = true
        for (const token of q.tokens) {
          if (!haystackLower.includes(token)) {
            ok = false
            break
          }
        }
        if (!ok) continue
      }

      let score = 0
      if (q.tokens.length === 0) {
        const occ = countOccurrences(haystackLower, q.lower)
        score += occ * 25
        if (note.titleLower.includes(q.lower)) score += 40
        if (note.pathLower.includes(q.lower)) score += 10
      } else {
        for (const token of q.tokens) {
          const occ = countOccurrences(haystackLower, token)
          score += occ * 10
          if (note.titleLower.includes(token)) score += 12
        }
        if (haystackLower.includes(q.lower)) score += 25
      }

      let matchIndex = -1
      let matchLen = Math.max(1, q.lower.length)
      if (q.tokens.length === 0) {
        matchIndex = note.bodyLower.indexOf(q.lower)
      } else if (q.lower && note.bodyLower.includes(q.lower)) {
        matchIndex = note.bodyLower.indexOf(q.lower)
      } else {
        let best = Number.POSITIVE_INFINITY
        let bestLen = 1
        for (const token of q.tokens) {
          const idx = note.bodyLower.indexOf(token)
          if (idx !== -1 && idx < best) {
            best = idx
            bestLen = token.length
          }
        }
        if (best !== Number.POSITIVE_INFINITY) {
          matchIndex = best
          matchLen = bestLen
        }
      }

      results.push({
        path: note.path,
        title: note.title,
        score,
        snippet: buildSnippet(note.body, matchIndex, matchLen, 180)
      })
    }

    results.sort((a, b) => b.score - a.score || a.path.localeCompare(b.path))
    return results.slice(0, limit)
  }
}
