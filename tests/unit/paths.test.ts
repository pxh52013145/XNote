import { describe, expect, it } from 'vitest'
import { fileBaseName, noteTitleFromPath, stripMarkdownExtension } from '@shared/paths'

describe('paths', () => {
  it('fileBaseName returns last segment for both separators', () => {
    expect(fileBaseName('a/b/c.md')).toBe('c.md')
    expect(fileBaseName('a\\b\\c.md')).toBe('c.md')
  })

  it('stripMarkdownExtension removes .md (case-insensitive)', () => {
    expect(stripMarkdownExtension('note.md')).toBe('note')
    expect(stripMarkdownExtension('NOTE.MD')).toBe('NOTE')
    expect(stripMarkdownExtension('note.txt')).toBe('note.txt')
  })

  it('noteTitleFromPath strips extension and returns base', () => {
    expect(noteTitleFromPath('folder/a.md')).toBe('a')
    expect(noteTitleFromPath('folder\\b.md')).toBe('b')
    expect(noteTitleFromPath('c')).toBe('c')
  })
})

