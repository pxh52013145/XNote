import type { NoteFile } from './types'

export type NoteTreeNode =
  | { kind: 'folder'; name: string; path: string; children: NoteTreeNode[] }
  | { kind: 'file'; name: string; path: string }

export type NoteTreeFolder = Extract<NoteTreeNode, { kind: 'folder' }>

function splitPosixPath(p: string): string[] {
  return p.split('/').filter(Boolean)
}

export function buildNoteTree(files: NoteFile[]): NoteTreeFolder {
  const root: NoteTreeFolder = { kind: 'folder', name: '', path: '', children: [] }
  const folders = new Map<string, NoteTreeFolder>()
  folders.set('', root)

  const ensureFolder = (folderPath: string, folderName: string): NoteTreeFolder => {
    const existing = folders.get(folderPath)
    if (existing) return existing
    const parentPath = folderPath.split('/').slice(0, -1).join('/')
    const parent = ensureFolder(parentPath, parentPath.split('/').pop() ?? '')
    const node: NoteTreeFolder = { kind: 'folder', name: folderName, path: folderPath, children: [] }
    parent.children.push(node)
    folders.set(folderPath, node)
    return node
  }

  for (const file of files) {
    const parts = splitPosixPath(file.path)
    if (parts.length === 0) continue
    const fileName = parts[parts.length - 1]
    const folderParts = parts.slice(0, -1)
    const folderPath = folderParts.join('/')
    const folder = ensureFolder(folderPath, folderParts[folderParts.length - 1] ?? '')
    folder.children.push({ kind: 'file', name: fileName, path: file.path })
  }

  const sortNode = (node: NoteTreeFolder) => {
    node.children.sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === 'folder' ? -1 : 1
      return a.name.localeCompare(b.name)
    })
    for (const child of node.children) {
      if (child.kind === 'folder') sortNode(child)
    }
  }

  sortNode(root)
  return root
}

