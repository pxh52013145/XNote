import { noteTitleFromPath } from './paths'

export type SearchQuery = {
  raw: string
  lower: string
  /**
   * ASCII word tokens (len>=2) extracted from the query, lowercased.
   */
  tokens: string[]
}

export type NoteSearchResult = {
  path: string
  title: string
  score: number
  snippet: string
}

export function parseSearchQuery(input: string): SearchQuery {
  const raw = input.trim()
  const lower = raw.toLowerCase()
  const tokens = Array.from(new Set((lower.match(/[a-z0-9]{2,}/g) ?? []).filter(Boolean)))
  return { raw, lower, tokens }
}

export function tokenizeForIndex(text: string): string[] {
  const lower = text.toLowerCase()
  return Array.from(new Set((lower.match(/[a-z0-9]{2,}/g) ?? []).filter(Boolean)))
}

export function countOccurrences(haystackLower: string, needleLower: string): number {
  if (!needleLower) return 0
  let count = 0
  let idx = 0
  while (true) {
    const found = haystackLower.indexOf(needleLower, idx)
    if (found === -1) break
    count++
    idx = found + needleLower.length
  }
  return count
}

function normalizeSnippetText(s: string): string {
  return s.replace(/\s+/g, ' ').trim()
}

export function buildSnippet(text: string, matchIndex: number, matchLength: number, maxLen = 160): string {
  if (matchIndex < 0) return normalizeSnippetText(text).slice(0, maxLen)
  const start = Math.max(0, matchIndex - 60)
  const end = Math.min(text.length, matchIndex + matchLength + 60)
  const prefix = start > 0 ? '…' : ''
  const suffix = end < text.length ? '…' : ''
  return prefix + normalizeSnippetText(text.slice(start, end)) + suffix
}

export function noteTitle(path: string, frontmatterTitle?: unknown): string {
  if (typeof frontmatterTitle === 'string' && frontmatterTitle.trim()) return frontmatterTitle.trim()
  return noteTitleFromPath(path)
}

