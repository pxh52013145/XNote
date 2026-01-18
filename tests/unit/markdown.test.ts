import { describe, expect, it } from 'vitest'
import { extractMarkdownHeadings } from '@shared/markdown'

describe('markdown', () => {
  it('extractMarkdownHeadings extracts headings with line numbers', () => {
    const md = ['# Title', '', '## Section', 'Text', '### Sub'].join('\n')
    expect(extractMarkdownHeadings(md)).toEqual([
      { level: 1, text: 'Title', line: 1 },
      { level: 2, text: 'Section', line: 3 },
      { level: 3, text: 'Sub', line: 5 }
    ])
  })

  it('extractMarkdownHeadings ignores invalid headings', () => {
    const md = ['#NoSpace', '####### TooMany', '##  OK'].join('\n')
    expect(extractMarkdownHeadings(md)).toEqual([{ level: 2, text: 'OK', line: 3 }])
  })
})

