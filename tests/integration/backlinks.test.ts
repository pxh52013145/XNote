import { describe, expect, it } from 'vitest'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { createNoteFile } from '../../src/main/vault'
import { listBacklinks } from '../../src/main/backlinks'

async function withTempDir<T>(fn: (dir: string) => Promise<T>): Promise<T> {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'xnote-'))
  try {
    return await fn(dir)
  } finally {
    await fs.rm(dir, { recursive: true, force: true })
  }
}

describe('backlinks (integration)', () => {
  it('lists notes that link to a target via wikilinks', async () => {
    await withTempDir(async (vault) => {
      await createNoteFile(vault, 'folder/b.md', 'Self [[b]] (ignored)')
      await createNoteFile(vault, 'a.md', 'See [[b]]')
      await createNoteFile(vault, 'c.md', 'See [[folder/b]]')
      await createNoteFile(vault, 'd.md', 'See [[other]]')

      expect(await listBacklinks(vault, 'folder/b.md')).toEqual(['a.md', 'c.md'])
    })
  })
})

