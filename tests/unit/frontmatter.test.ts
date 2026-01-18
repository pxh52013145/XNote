import { describe, expect, it } from 'vitest'
import { deleteFrontmatterKey, extractFrontmatter, renameFrontmatterKey, setFrontmatterValue } from '@shared/frontmatter'

describe('frontmatter', () => {
  it('extracts YAML frontmatter and body', () => {
    const md = `---
title: Hello
tags: [a, "b"]
flag: true
count: 3
list:
  - one
  - 2
---
# Heading
Body
`

    const res = extractFrontmatter(md)
    expect(res.data).toEqual({
      title: 'Hello',
      tags: ['a', 'b'],
      flag: true,
      count: 3,
      list: ['one', 2]
    })
    expect(res.body.startsWith('# Heading')).toBe(true)
    expect(res.raw).toContain('title:')
  })

  it('returns empty when no frontmatter', () => {
    const res = extractFrontmatter('# hi')
    expect(res.data).toEqual({})
    expect(res.body).toBe('# hi')
    expect(res.raw).toBeUndefined()
  })

  it('setFrontmatterValue adds/updates while preserving unknown blocks', () => {
    const md = `---
obj:
  nested: 1
tags:
  - a
---
body
`

    const updated = setFrontmatterValue(md, 'title', 'Hello')
    expect(updated).toContain('obj:\n  nested: 1')
    expect(updated).toContain('title: Hello')
    expect(updated).toContain('tags:\n  - a')

    const updated2 = setFrontmatterValue(updated, 'tags', ['a', 'b'])
    expect(updated2).toContain('tags:\n  - a\n  - b')
    expect(updated2).toContain('obj:\n  nested: 1')
  })

  it('deleteFrontmatterKey removes key and removes frontmatter when empty', () => {
    const md = `---
title: "Hello"
---
body
`
    const removed = deleteFrontmatterKey(md, 'title')
    expect(removed.startsWith('---')).toBe(false)
    expect(removed.trim()).toBe('body')
  })

  it('renameFrontmatterKey renames key in-place', () => {
    const md = `---
old: 1
---
body
`
    const renamed = renameFrontmatterKey(md, 'old', 'new')
    expect(renamed).toContain('new: 1')
    expect(renamed).not.toContain('\nold:')
  })
})
