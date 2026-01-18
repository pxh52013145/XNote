import { describe, expect, it } from 'vitest'
import { buildNoteTree } from '@shared/noteTree'
import type { NoteFile } from '@shared/types'

function nf(p: string): NoteFile {
  return { path: p, mtimeMs: 0, size: 0 }
}

describe('noteTree', () => {
  it('buildNoteTree groups by folders and sorts folders before files', () => {
    const files: NoteFile[] = [nf('z.md'), nf('folder/a.md'), nf('a.md'), nf('folder2/b.md')]
    const tree = buildNoteTree(files)

    expect(tree.children.map((c) => `${c.kind}:${c.name}`)).toEqual([
      'folder:folder',
      'folder:folder2',
      'file:a.md',
      'file:z.md'
    ])

    const folder = tree.children.find((c) => c.kind === 'folder' && c.name === 'folder')
    expect(folder && folder.kind === 'folder' ? folder.children.map((c) => `${c.kind}:${c.name}`) : []).toEqual([
      'file:a.md'
    ])
  })
})

