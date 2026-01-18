import { useEffect, useMemo, useState } from 'react'
import { ChevronDown, ChevronRight, FileText, Folder, FolderOpen } from 'lucide-react'
import type { NoteFile } from '@shared/types'
import { stripMarkdownExtension } from '@shared/paths'
import { buildNoteTree, type NoteTreeNode } from '@shared/noteTree'

export function FileExplorer(props: {
  files: NoteFile[]
  activePath: string | null
  onOpenFile: (path: string) => void
  vaultLoaded: boolean
  vaultLabel?: string
}) {
  const tree = useMemo(() => buildNoteTree(props.files), [props.files])
  const [filter, setFilter] = useState('')
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(['']))

  useEffect(() => {
    if (!props.vaultLoaded) {
      setExpanded(new Set(['']))
      setFilter('')
    }
  }, [props.vaultLoaded])

  const filterNorm = filter.trim().toLowerCase()

  const toggleFolder = (folderPath: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(folderPath)) next.delete(folderPath)
      else next.add(folderPath)
      next.add('')
      return next
    })
  }

  const shouldShowNode = (node: NoteTreeNode): boolean => {
    if (!filterNorm) return true
    if (node.kind === 'file') return node.path.toLowerCase().includes(filterNorm)
    return node.children.some((c) => shouldShowNode(c))
  }

  const renderNode = (node: NoteTreeNode, depth: number) => {
    if (!shouldShowNode(node)) return null

    if (node.kind === 'folder') {
      const isRoot = node.path === ''
      const isOpen = filterNorm ? true : expanded.has(node.path)
      const label = isRoot ? props.vaultLabel ?? 'Vault' : node.name

      return (
        <div key={node.path || 'root'}>
          <button
            className="tree-row folder"
            style={{ paddingLeft: `${10 + depth * 14}px` }}
            onClick={() => (isRoot ? toggleFolder('') : toggleFolder(node.path))}
            title={node.path || label}
          >
            <span className="tree-icon">
              {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
            </span>
            <span className="tree-icon">{isOpen ? <FolderOpen size={14} /> : <Folder size={14} />}</span>
            <span className="tree-label">{label}</span>
          </button>

          {isOpen ? (
            <div>{node.children.map((child) => renderNode(child, isRoot ? depth + 1 : depth + 1))}</div>
          ) : null}
        </div>
      )
    }

    const isActive = node.path === props.activePath
    return (
      <button
        key={node.path}
        className={isActive ? 'tree-row file active' : 'tree-row file'}
        style={{ paddingLeft: `${10 + depth * 14}px` }}
        onClick={() => props.onOpenFile(node.path)}
        title={node.path}
      >
        <span className="tree-icon spacer" />
        <span className="tree-icon">
          <FileText size={14} />
        </span>
        <span className="tree-label">{stripMarkdownExtension(node.name)}</span>
      </button>
    )
  }

  return (
    <div className="file-explorer">
      <input
        className="panel-filter"
        placeholder={props.vaultLoaded ? 'Filter files...' : 'Open a vault to browse'}
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
        disabled={!props.vaultLoaded}
      />
      <div className="tree">{renderNode(tree, 0)}</div>
    </div>
  )
}
