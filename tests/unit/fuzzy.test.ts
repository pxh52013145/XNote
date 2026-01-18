import { describe, expect, it } from 'vitest'
import { fuzzyScore } from '@shared/fuzzy'

describe('fuzzy', () => {
  it('matches sequential characters and rejects non-matching', () => {
    expect(fuzzyScore('abc', 'a_b_c')).toBeTypeOf('number')
    expect(fuzzyScore('abc', 'acb')).toBeNull()
  })

  it('prefers earlier and more contiguous matches', () => {
    const exact = fuzzyScore('note', 'note.md')
    const gapped = fuzzyScore('note', 'n---o---t---e.md')
    expect(exact).not.toBeNull()
    expect(gapped).not.toBeNull()
    expect((exact ?? 0) > (gapped ?? 0)).toBe(true)
  })

  it('supports multi-token queries (all tokens must match)', () => {
    expect(fuzzyScore('foo bar', 'foo/bar.md')).not.toBeNull()
    expect(fuzzyScore('foo bar', 'foo.md')).toBeNull()
  })
})

