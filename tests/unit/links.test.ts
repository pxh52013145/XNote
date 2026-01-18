import { describe, expect, it } from 'vitest'
import { extractWikiLinks } from '@shared/links'

describe('links', () => {
  it('extractWikiLinks finds targets and aliases', () => {
    const md = `
Hello [[World]] and [[Folder/Note|Alias]].
Also [[Note#Heading|With heading]] and ![[Embed]].
`

    expect(extractWikiLinks(md)).toEqual([
      { target: 'World', embed: false },
      { target: 'Folder/Note', display: 'Alias', embed: false },
      { target: 'Note', heading: 'Heading', display: 'With heading', embed: false },
      { target: 'Embed', embed: true }
    ])
  })

  it('extractWikiLinks ignores empty links', () => {
    expect(extractWikiLinks('[[  ]] [[|x]]')).toEqual([])
  })
})

