import { describe, expect, it } from 'vitest'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import {
  createNoteFile,
  deleteNoteFile,
  listMarkdownFiles,
  readNoteFile,
  renameNoteFile,
  writeNoteFile
} from '../../src/main/vault'

async function withTempDir<T>(fn: (dir: string) => Promise<T>): Promise<T> {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'xnote-'))
  try {
    return await fn(dir)
  } finally {
    await fs.rm(dir, { recursive: true, force: true })
  }
}

describe('vault filesystem (integration)', () => {
  it('creates, reads, writes, lists, renames, deletes', async () => {
    await withTempDir(async (vault) => {
      const created = await createNoteFile(vault, 'note', 'hello')
      expect(created).toBe('note.md')

      expect(await readNoteFile(vault, created)).toBe('hello')

      await writeNoteFile(vault, created, 'updated')
      expect(await readNoteFile(vault, created)).toBe('updated')

      expect((await listMarkdownFiles(vault)).map((f) => f.path)).toEqual(['note.md'])

      const renamed = await renameNoteFile(vault, 'note.md', 'folder/renamed')
      expect(renamed).toBe('folder/renamed.md')

      expect((await listMarkdownFiles(vault)).map((f) => f.path)).toEqual(['folder/renamed.md'])
      expect(await readNoteFile(vault, renamed)).toBe('updated')

      await deleteNoteFile(vault, renamed)
      expect(await listMarkdownFiles(vault)).toEqual([])
    })
  })

  it('prevents path traversal', async () => {
    await withTempDir(async (vault) => {
      await expect(writeNoteFile(vault, '../evil.md', 'x')).rejects.toThrow(/Invalid note path/)
      await expect(deleteNoteFile(vault, '../evil')).rejects.toThrow(/Invalid note path/)
      await expect(renameNoteFile(vault, 'a.md', '../evil')).rejects.toThrow(/Invalid note path/)
    })
  })

  it('prevents overwriting on create/rename', async () => {
    await withTempDir(async (vault) => {
      await createNoteFile(vault, 'a.md', '')
      await createNoteFile(vault, 'b.md', '')
      await expect(createNoteFile(vault, 'a.md', '')).rejects.toThrow(/already exists/i)
      await expect(renameNoteFile(vault, 'a.md', 'b.md')).rejects.toThrow(/already exists/i)
    })
  })
})

