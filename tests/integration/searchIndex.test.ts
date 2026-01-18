import { describe, expect, it } from 'vitest'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { createNoteFile } from '../../src/main/vault'
import { VaultIndex } from '../../src/main/vaultIndex'

async function withTempDir<T>(fn: (dir: string) => Promise<T>): Promise<T> {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'xnote-'))
  try {
    return await fn(dir)
  } finally {
    await fs.rm(dir, { recursive: true, force: true })
  }
}

describe('vault index (integration)', () => {
  it('builds index and supports full-text search + backlinks', async () => {
    await withTempDir(async (vault) => {
      await createNoteFile(vault, 'a.md', 'Hello world')
      await createNoteFile(vault, 'b.md', 'Something else')
      await createNoteFile(vault, 'folder/c.md', 'Links to [[a]] and says hello again')

      const index = new VaultIndex(vault)
      try {
        await index.ready()

        const results = await index.search('hello')
        expect(results.map((r) => r.path)).toEqual(['a.md', 'folder/c.md'])

        const backlinks = index.listBacklinks('a.md')
        expect(backlinks).toEqual(['folder/c.md'])
      } finally {
        index.dispose()
      }
    })
  })
})

