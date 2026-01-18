import { describe, expect, it } from 'vitest'
import { extractInlineTags } from '@shared/tags'

describe('tags', () => {
  it('extractInlineTags finds hashtags and ignores code', () => {
    const md = `---
tags: [a]
---
Hello #tag and #tag2.

\`\`\`
#notatag
\`\`\`

Inline \`#nope\`.
`

    expect(extractInlineTags(md)).toEqual(['tag', 'tag2'])
  })
})

