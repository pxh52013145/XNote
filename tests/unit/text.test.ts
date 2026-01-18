import { describe, expect, it } from 'vitest'
import { fileBaseName } from '@shared/paths'
import { countWords } from '@shared/text'

describe('text utils', () => {
  it('countWords handles empty', () => {
    expect(countWords('')).toBe(0)
    expect(countWords('   ')).toBe(0)
  })

  it('countWords counts whitespace-separated tokens', () => {
    expect(countWords('hello world')).toBe(2)
    expect(countWords(' hello   world \n test ')).toBe(3)
  })

  it('fileBaseName returns last path segment', () => {
    expect(fileBaseName('a/b/c.md')).toBe('c.md')
    expect(fileBaseName('c.md')).toBe('c.md')
    expect(fileBaseName('a\\b\\c.md')).toBe('c.md')
    expect(fileBaseName('')).toBe('')
  })
})

