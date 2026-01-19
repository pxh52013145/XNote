import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore, type PointerEvent, type ReactNode } from 'react'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import CodeMirror from '@uiw/react-codemirror'
import { markdown } from '@codemirror/lang-markdown'
import { languages } from '@codemirror/language-data'
import { oneDark } from '@codemirror/theme-one-dark'
import { EditorSelection } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { autocompletion, type CompletionContext } from '@codemirror/autocomplete'
import { ArrowLeft, ArrowRight, BookOpen, Command, FolderOpen, FolderPlus, PanelLeft, PanelRight, Pencil, Plus, Search } from 'lucide-react'
import type { NoteFile, VaultLayout, VaultSettings, VaultTab } from '@shared/types'
import { noteTitleFromPath } from '@shared/paths'
import { countWords } from '@shared/text'
import { parseSearchQuery, type NoteSearchResult, type SearchQuery } from '@shared/search'
import { fuzzyScore } from '@shared/fuzzy'
import { hotkeyFromEvent, normalizeHotkey } from '@shared/hotkeys'
import { NavigationHistory, type NavigationLocation } from '@shared/navigationHistory'
import { xnote } from './api'
import { commandRegistry } from './commands'
import { remarkWikiLinks } from './markdown/remarkWikiLinks'
import { resolveVaultRelativePath } from './markdown/vaultPaths'
import { CommandPalette, type PaletteItem } from './components/CommandPalette'
import { ContextMenu, type ContextMenuItem } from './components/ContextMenu'
import { CalloutBlockquote } from './components/CalloutBlockquote'
import { FileExplorer } from './components/FileExplorer'
import { FindReplaceModal } from './components/FindReplaceModal'
import { GraphView } from './components/GraphView'
import { Ribbon, type LeftSidebarView } from './components/Ribbon'
import { RightSidebar, type RightSidebarTab } from './components/RightSidebar'
import { SettingsModal } from './components/SettingsModal'
import { Topbar } from './components/Topbar'
import { VaultImage } from './components/VaultImage'

type SaveState = 'idle' | 'saving' | 'saved' | 'error'

type WorkspaceTab =
  | { id: 'welcome'; type: 'welcome'; title: 'Welcome' }
  | { id: `note:${string}`; type: 'note'; title: string; path: string }
  | { id: 'graph'; type: 'graph'; title: 'Graph view' }

function getPastedImageFile(event: ClipboardEvent): File | null {
  const dt = event.clipboardData
  if (!dt) return null

  for (const item of Array.from(dt.items)) {
    if (item.kind === 'file' && item.type.startsWith('image/')) {
      return item.getAsFile()
    }
  }

  for (const file of Array.from(dt.files)) {
    if (file.type.startsWith('image/')) return file
  }

  return null
}

function countOccurrences(haystack: string, needle: string): number {
  if (!needle) return 0
  let count = 0
  let offset = 0

  while (offset <= haystack.length) {
    const idx = haystack.indexOf(needle, offset)
    if (idx === -1) break
    count++
    offset = idx + needle.length
  }

  return count
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function highlightText(text: string, query: SearchQuery): ReactNode {
  const terms = query.tokens.length > 0 ? query.tokens : query.lower ? [query.lower] : []
  const uniqueTerms = Array.from(new Set(terms.map((t) => t.trim()).filter(Boolean)))
  if (uniqueTerms.length === 0) return text

  uniqueTerms.sort((a, b) => b.length - a.length)
  const pattern = uniqueTerms.map(escapeRegExp).join('|')
  if (!pattern) return text

  const matcher = new RegExp(`(${pattern})`, 'ig')
  const parts = text.split(matcher)
  const lowerTerms = new Set(uniqueTerms.map((t) => t.toLowerCase()))

  return parts.map((part, idx) => {
    const lower = part.toLowerCase()
    if (lowerTerms.has(lower)) {
      return (
        <mark key={idx} className="search-highlight">
          {part}
        </mark>
      )
    }
    return <span key={idx}>{part}</span>
  })
}

function clamp(n: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, n))
}

function vaultNameFromFsPath(p: string): string {
  return p.split(/[/\\]/).filter(Boolean).pop() ?? p
}

function normalizeWikiTarget(target: string): string {
  return target.trim().replace(/\\/g, '/').replace(/^\.?\//, '').replace(/\.md$/i, '')
}

export default function App() {
  const [vaultPath, setVaultPath] = useState<string | null>(null)
  const [recentVaults, setRecentVaults] = useState<string[]>([])
  const [notes, setNotes] = useState<NoteFile[]>([])
  const [folders, setFolders] = useState<string[]>([])
  const [vaultSettings, setVaultSettings] = useState<VaultSettings>({})

  const [tabs, setTabs] = useState<WorkspaceTab[]>([{ id: 'welcome', type: 'welcome', title: 'Welcome' }])
  const [activeTabId, setActiveTabId] = useState<WorkspaceTab['id']>('welcome')
  const activeTab = useMemo(
    () => tabs.find((t) => t.id === activeTabId) ?? tabs[0] ?? { id: 'welcome', type: 'welcome', title: 'Welcome' },
    [activeTabId, tabs]
  )

  const activeNotePath = activeTab.type === 'note' ? activeTab.path : null

  const [viewMode, setViewMode] = useState<'edit' | 'read'>('edit')
  const [content, setContent] = useState<string>('')
  const [isDirty, setIsDirty] = useState(false)
  const [saveState, setSaveState] = useState<SaveState>('idle')
  const [error, setError] = useState<string | null>(null)

  const [theme, setTheme] = useState<'dark' | 'light'>('dark')
  const [leftView, setLeftView] = useState<LeftSidebarView>('explorer')
  const [rightTab, setRightTab] = useState<RightSidebarTab>('outline')

  const [isLeftSidebarOpen, setIsLeftSidebarOpen] = useState(true)
  const [isRightSidebarOpen, setIsRightSidebarOpen] = useState(true)
  const [leftWidth, setLeftWidth] = useState(320)
  const [rightWidth, setRightWidth] = useState(320)
  const resizingRef = useRef<null | { side: 'left' | 'right'; startX: number; startWidth: number }>(null)
  const editorViewRef = useRef<EditorView | null>(null)

  const [paletteOpen, setPaletteOpen] = useState(false)
  const [paletteMode, setPaletteMode] = useState<'commands' | 'files'>('commands')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [customHotkeys, setCustomHotkeys] = useState<Record<string, string>>({})
  const [contextMenu, setContextMenu] = useState<null | { x: number; y: number; items: ContextMenuItem[] }>(null)
  const [findReplaceOpen, setFindReplaceOpen] = useState(false)
  const [findText, setFindText] = useState('')
  const [replaceText, setReplaceText] = useState('')
  const [findReplaceFocus, setFindReplaceFocus] = useState<'find' | 'replace'>('find')

  const closeContextMenu = useCallback(() => setContextMenu(null), [])
  const openContextMenu = useCallback((x: number, y: number, items: ContextMenuItem[]) => setContextMenu({ x, y, items }), [])

  const navHistory = useMemo(() => new NavigationHistory(), [])
  const navHistoryVersion = useSyncExternalStore(
    (listener) => navHistory.subscribe(listener),
    () => navHistory.getVersion(),
    () => navHistory.getVersion()
  )

  const commands = useSyncExternalStore(
    (listener) => commandRegistry.subscribe(listener),
    () => commandRegistry.list(),
    () => commandRegistry.list()
  )

  const [searchQuery, setSearchQuery] = useState('')
  const parsedSearchQuery = useMemo(() => parseSearchQuery(searchQuery), [searchQuery])
  const [searchResults, setSearchResults] = useState<NoteSearchResult[]>([])
  const [searchLoading, setSearchLoading] = useState(false)
  const [searchError, setSearchError] = useState<string | null>(null)
  const [backlinks, setBacklinks] = useState<string[]>([])
  const [backlinksLoading, setBacklinksLoading] = useState(false)
  const [backlinksError, setBacklinksError] = useState<string | null>(null)

  const markdownExtension = useMemo(() => markdown({ codeLanguages: languages }), [])
  const pasteImageExtension = useMemo(() => {
    return EditorView.domEventHandlers({
      paste(event, view) {
        if (!vaultPath || !activeNotePath) return false
        const file = getPastedImageFile(event)
        if (!file) return false

        event.preventDefault()
        void (async () => {
          try {
            const savedPath = await xnote.saveAttachment({
              data: await file.arrayBuffer(),
              mime: file.type,
              notePath: activeNotePath
            })

            if (!view.dom.isConnected) return

            const insert = `![](</${savedPath}>)`
            const selection = view.state.selection.main
            view.dispatch({ changes: { from: selection.from, to: selection.to, insert } })
            view.focus()
          } catch (err) {
            console.error('Failed to paste image', err)
          }
        })()

        return true
      }
    })
  }, [activeNotePath, vaultPath])

  const wikilinkAutocompleteExtension = useMemo(() => {
    const source = (context: CompletionContext) => {
      const match = context.matchBefore(/\[\[[^[\]]*/)
      if (!match) return null

      const typed = match.text.slice(2)
      if (/[|#]/.test(typed)) return null

      const scored = notes
        .map((n) => {
          const target = normalizeWikiTarget(n.path)
          if (!target) return null
          const haystack = `${noteTitleFromPath(n.path)} ${target}`
          const score = fuzzyScore(typed, haystack)
          if (score == null) return null
          return { score, target, path: n.path, title: noteTitleFromPath(n.path) }
        })
        .filter((v): v is { score: number; target: string; path: string; title: string } => Boolean(v))
        .sort((a, b) => b.score - a.score || a.path.localeCompare(b.path))
        .slice(0, 50)

      return {
        from: match.from + 2,
        to: match.to,
        options: scored.map((item) => ({
          label: item.title,
          detail: item.target,
          info: item.path,
          apply(view: EditorView, _completion: unknown, from: number, to: number) {
            const after = view.state.doc.sliceString(to, to + 2)
            const insert = after === ']]' ? item.target : `${item.target}]]`
            view.dispatch({
              changes: { from, to, insert },
              selection: EditorSelection.cursor(from + item.target.length),
              scrollIntoView: true
            })
          }
        }))
      }
    }

    return autocompletion({ override: [source] })
  }, [notes])

  const cmExtensions = useMemo(
    () => [markdownExtension, wikilinkAutocompleteExtension, pasteImageExtension],
    [markdownExtension, pasteImageExtension, wikilinkAutocompleteExtension]
  )

  const vaultName = vaultPath ? vaultPath.split(/[/\\]/).filter(Boolean).pop() ?? 'Vault' : 'No vault'
  const newNotesFolder = useMemo(() => {
    const raw = vaultSettings.newNotesFolder?.trim()
    if (!raw) return ''
    return raw.replace(/\\/g, '/').replace(/^\/+/, '').replace(/\/+$/, '')
  }, [vaultSettings.newNotesFolder])

  const stats = useMemo(() => {
    return {
      words: countWords(content),
      chars: content.length
    }
  }, [content])

  const findMatchCount = useMemo(() => countOccurrences(content, findText), [content, findText])

  const refreshNotes = useCallback(async (): Promise<void> => {
    const [noteList, folderList] = await Promise.all([
      xnote.listNotes().catch(() => [] as NoteFile[]),
      xnote.listFolders().catch(() => [] as string[])
    ])
    setNotes(noteList)
    setFolders(folderList)
  }, [])

  const refreshRecentVaults = useCallback(async (): Promise<void> => {
    try {
      setRecentVaults(await xnote.listRecentVaults())
    } catch {
      setRecentVaults([])
    }
  }, [])

  useEffect(() => {
    void xnote
      .getHotkeys()
      .then((stored) => {
        const next: Record<string, string> = {}
        for (const [commandId, hotkey] of Object.entries(stored)) {
          const normalized = normalizeHotkey(hotkey)
          if (!normalized) continue
          next[commandId] = normalized
        }
        setCustomHotkeys(next)
      })
      .catch(() => setCustomHotkeys({}))
  }, [])

  const setCommandHotkey = useCallback(async (commandId: string, hotkey: string): Promise<void> => {
    const normalized = normalizeHotkey(hotkey)
    if (!normalized) return

    try {
      await xnote.setHotkey(commandId, normalized)
      setCustomHotkeys((prev) => ({ ...prev, [commandId]: normalized }))
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  const clearCommandHotkey = useCallback(async (commandId: string): Promise<void> => {
    try {
      await xnote.clearHotkey(commandId)
      setCustomHotkeys((prev) => {
        if (!(commandId in prev)) return prev
        const next = { ...prev }
        delete next[commandId]
        return next
      })
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  const saveVaultSettings = useCallback(
    async (next: VaultSettings): Promise<void> => {
      if (!vaultPath) return
      try {
        await xnote.saveVaultSettings(vaultPath, next)
        const stored = await xnote.getVaultSettings(vaultPath)
        setVaultSettings(stored ?? {})
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e))
      }
    },
    [vaultPath]
  )

  const applyVaultSelection = useCallback(
    async (selected: string): Promise<void> => {
      navHistory.reset()
      setVaultPath(selected)
      setTabs([{ id: 'welcome', type: 'welcome', title: 'Welcome' }])
      setActiveTabId('welcome')
      setContent('')
      setIsDirty(false)
      setSaveState('idle')
      setLeftView('explorer')
      setIsLeftSidebarOpen(true)
      setIsRightSidebarOpen(true)
      setLeftWidth(320)
      setRightWidth(320)
      setRightTab('outline')
      setVaultSettings({})
      await refreshNotes()
      await refreshRecentVaults()

      try {
        const layout = await xnote.getVaultLayout(selected)
        if (layout) {
          setIsLeftSidebarOpen(layout.isLeftSidebarOpen)
          setIsRightSidebarOpen(layout.isRightSidebarOpen)
          setLeftWidth(clamp(layout.leftWidth, 240, 520))
          setRightWidth(clamp(layout.rightWidth, 260, 520))
          setLeftView(layout.leftView)
          setRightTab(layout.rightTab)

          if (layout.tabs && layout.tabs.length > 0) {
            const restored: WorkspaceTab[] = []
            const seen = new Set<string>()

            for (const t of layout.tabs) {
              if (t.type === 'note') {
                const id = `note:${t.path}` as const
                if (seen.has(id)) continue
                seen.add(id)
                restored.push({ id, type: 'note', title: noteTitleFromPath(t.path), path: t.path })
              } else if (t.type === 'graph') {
                if (seen.has('graph')) continue
                seen.add('graph')
                restored.push({ id: 'graph', type: 'graph', title: 'Graph view' })
              }
            }

            if (restored.length > 0) {
              setTabs(restored)

              const activeFromLayout = layout.activeTab
              const desiredId =
                activeFromLayout?.type === 'note'
                  ? (`note:${activeFromLayout.path}` as const)
                  : activeFromLayout?.type === 'graph'
                    ? ('graph' as const)
                    : null

              const fallbackId = restored[restored.length - 1]?.id ?? 'welcome'
              const chosenId = desiredId && restored.some((t) => t.id === desiredId) ? desiredId : fallbackId
              setActiveTabId(chosenId)
              setViewMode(chosenId === 'graph' ? 'read' : 'edit')
            }
          }
        }
      } catch {
        // ignore
      }

      try {
        const stored = await xnote.getVaultSettings(selected)
        setVaultSettings(stored ?? {})
      } catch {
        setVaultSettings({})
      }
    },
    [navHistory, refreshNotes, refreshRecentVaults]
  )

  useEffect(() => {
    if (!vaultPath) return

    const openTabs: VaultTab[] = tabs
      .filter((t) => t.type !== 'welcome')
      .map((t) => (t.type === 'note' ? { type: 'note', path: t.path } : { type: 'graph' }))

    const active: VaultTab | null = activeTab.type === 'note' ? { type: 'note', path: activeTab.path } : activeTab.type === 'graph' ? { type: 'graph' } : null

    const layout: VaultLayout = {
      isLeftSidebarOpen,
      isRightSidebarOpen,
      leftWidth,
      rightWidth,
      leftView,
      rightTab,
      tabs: openTabs.length > 0 ? openTabs : undefined,
      activeTab: active
    }

    const t = window.setTimeout(() => {
      void xnote.saveVaultLayout(vaultPath, layout).catch(() => {})
    }, 250)

    return () => window.clearTimeout(t)
  }, [activeTab.type, activeTab.type === 'note' ? activeTab.path : null, isLeftSidebarOpen, isRightSidebarOpen, leftView, leftWidth, rightTab, rightWidth, tabs, vaultPath])

  useEffect(() => {
    if (activeTab.type === 'note') {
      navHistory.record({ type: 'note', path: activeTab.path })
    } else if (activeTab.type === 'graph') {
      navHistory.record({ type: 'graph' })
    }
  }, [activeTab, navHistory])

  useEffect(() => {
    let t: number | null = null
    const off = xnote.onVaultChanged(() => {
      if (t) window.clearTimeout(t)
      t = window.setTimeout(() => {
        void refreshNotes()
      }, 250)
    })
    return () => {
      if (t) window.clearTimeout(t)
      off()
    }
  }, [refreshNotes])

  const flushSave = useCallback(async (): Promise<void> => {
    if (!activeNotePath) return
    if (!isDirty) return
    try {
      setSaveState('saving')
      await xnote.writeNote(activeNotePath, content)
      setSaveState('saved')
      setIsDirty(false)
      await refreshNotes()
    } catch (e: unknown) {
      setSaveState('error')
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [activeNotePath, content, isDirty, refreshNotes])

  const openVault = useCallback(async (): Promise<void> => {
    setError(null)
    const selected = await xnote.selectVault()
    if (!selected) return
    await applyVaultSelection(selected)
  }, [applyVaultSelection])

  const openRecentVault = useCallback(
    async (selected: string): Promise<void> => {
      setError(null)
      try {
        const opened = await xnote.openVaultPath(selected)
        await applyVaultSelection(opened)
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e))
      }
    },
    [applyVaultSelection]
  )

  const openNoteTab = useCallback(
    async (notePath: string): Promise<void> => {
      await flushSave()
      setError(null)
      const tabId = `note:${notePath}` as const
      setTabs((prev) => {
        const withoutWelcome = prev.filter((t) => t.type !== 'welcome')
        const existing = withoutWelcome.find((t) => t.type === 'note' && t.path === notePath)
        if (existing) return withoutWelcome
        return [
          ...withoutWelcome,
          {
            id: tabId,
            type: 'note',
            title: noteTitleFromPath(notePath),
            path: notePath
          }
        ]
      })
      setActiveTabId(tabId)
      setViewMode('edit')
    },
    [flushSave]
  )

  const openGraphTab = useCallback(async (): Promise<void> => {
    await flushSave()
    setTabs((prev) => {
      const withoutWelcome = prev.filter((t) => t.type !== 'welcome')
      const hasGraph = withoutWelcome.some((t) => t.type === 'graph')
      return hasGraph ? withoutWelcome : [...withoutWelcome, { id: 'graph', type: 'graph', title: 'Graph view' }]
    })
    setActiveTabId('graph')
    setViewMode('read')
  }, [flushSave])

  const navigateToLocation = useCallback(
    async (location: NavigationLocation): Promise<void> => {
      if (location.type === 'note') {
        await openNoteTab(location.path)
        return
      }
      if (location.type === 'graph') {
        await openGraphTab()
      }
    },
    [openGraphTab, openNoteTab]
  )

  const goBack = useCallback(async (): Promise<void> => {
    const loc = navHistory.back()
    if (!loc) return
    await navigateToLocation(loc)
  }, [navigateToLocation, navHistory])

  const goForward = useCallback(async (): Promise<void> => {
    const loc = navHistory.forward()
    if (!loc) return
    await navigateToLocation(loc)
  }, [navigateToLocation, navHistory])

  const createNote = useCallback(async (): Promise<void> => {
    setError(null)
    if (!vaultPath) {
      await openVault()
      return
    }
    const suggested = newNotesFolder ? `${newNotesFolder}/Untitled.md` : 'Untitled.md'
    const input = window.prompt('New note path (relative to vault):', suggested)
    if (!input) return

    const createdPath = await xnote.createNote(input, '')
    await refreshNotes()
    await openNoteTab(createdPath)
  }, [newNotesFolder, openNoteTab, openVault, refreshNotes, vaultPath])

  const createFolder = useCallback(async (): Promise<void> => {
    setError(null)
    if (!vaultPath) {
      await openVault()
      return
    }

    const baseFolder = activeNotePath ? activeNotePath.split('/').slice(0, -1).join('/') : newNotesFolder
    const suggested = baseFolder ? `${baseFolder}/New folder` : 'New folder'
    const input = window.prompt('New folder path (relative to vault):', suggested)
    if (!input) return

    try {
      await xnote.createFolder(input)
      await refreshNotes()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [activeNotePath, newNotesFolder, openVault, refreshNotes, vaultPath])

  const resolveNotePathForWikiTarget = useCallback(
    (rawTarget: string): string | null => {
      const target = normalizeWikiTarget(rawTarget)
      if (!target) return null

      if (target.includes('/')) {
        const normalizedLower = target.toLowerCase()
        const found = notes.find((n) => normalizeWikiTarget(n.path).toLowerCase() === normalizedLower)
        return found?.path ?? null
      }

      const targetLower = target.toLowerCase()
      const candidates = notes.filter((n) => noteTitleFromPath(n.path).toLowerCase() === targetLower)
      if (candidates.length === 0) return null
      if (candidates.length === 1) return candidates[0]?.path ?? null

      if (activeNotePath) {
        const folder = activeNotePath.split('/').slice(0, -1).join('/')
        const sameFolder = candidates.find((n) => n.path.split('/').slice(0, -1).join('/') === folder)
        if (sameFolder) return sameFolder.path
      }

      return candidates[0]?.path ?? null
    },
    [activeNotePath, notes]
  )

  const createPathForWikiTarget = useCallback(
    (rawTarget: string): string => {
      const target = normalizeWikiTarget(rawTarget)
      if (!target) return 'Untitled.md'
      if (target.includes('/')) return `${target}.md`
      const folder = activeNotePath ? activeNotePath.split('/').slice(0, -1).join('/') : newNotesFolder
      return folder ? `${folder}/${target}.md` : `${target}.md`
    },
    [activeNotePath, newNotesFolder]
  )

  const openWikiLinkTarget = useCallback(
    async (rawTarget: string): Promise<void> => {
      if (!vaultPath) return
      const target = normalizeWikiTarget(rawTarget)
      if (!target) return

      try {
        const existing = resolveNotePathForWikiTarget(target)
        if (existing) {
          await openNoteTab(existing)
          return
        }

        const createdPath = await xnote.createNote(createPathForWikiTarget(target), '')
        await refreshNotes()
        await openNoteTab(createdPath)
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e))
      }
    },
    [createPathForWikiTarget, openNoteTab, refreshNotes, resolveNotePathForWikiTarget, vaultPath]
  )

  const openMarkdownLinkTarget = useCallback(
    async (rawHref: string): Promise<void> => {
      if (!vaultPath) return
      const resolved = resolveVaultRelativePath(rawHref, activeNotePath)
      if (!resolved) return

      const lower = resolved.toLowerCase()
      const looksLikeNote = lower.endsWith('.md') || !resolved.includes('.')

      if (!looksLikeNote) {
        await xnote.openVaultFile(resolved)
        return
      }

      const notePath = lower.endsWith('.md') ? resolved : `${resolved}.md`
      const existing = notes.find((n) => n.path.toLowerCase() === notePath.toLowerCase())
      if (existing) {
        await openNoteTab(existing.path)
        return
      }

      const createdPath = await xnote.createNote(notePath, '')
      await refreshNotes()
      await openNoteTab(createdPath)
    },
    [activeNotePath, notes, openNoteTab, refreshNotes, vaultPath]
  )

  const openHref = useCallback(
    (href: string): void => {
      if (!href) return

      if (href.startsWith('xnote://wikilink')) {
        try {
          const url = new URL(href)
          const target = url.searchParams.get('target')
          if (target) void openWikiLinkTarget(target)
        } catch {
          // ignore
        }
        return
      }

      if (href.startsWith('http://') || href.startsWith('https://')) {
        void xnote.openExternal(href)
        return
      }

      if (href.startsWith('#')) return

      if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(href)) {
        void xnote.openExternal(href)
        return
      }

      void openMarkdownLinkTarget(href).catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
    },
    [openMarkdownLinkTarget, openWikiLinkTarget]
  )

  const closeTab = useCallback(
    async (tabId: WorkspaceTab['id']): Promise<void> => {
      if (tabId === 'welcome') return
      if (tabId === activeTabId) {
        await flushSave()
      }

      setTabs((prev) => {
        const next = prev.filter((t) => t.id !== tabId)
        return next.length === 0 ? [{ id: 'welcome', type: 'welcome', title: 'Welcome' }] : next
      })

      setActiveTabId((prevId) => {
        if (prevId !== tabId) return prevId
        const remaining = tabs.filter((t) => t.id !== tabId)
        const fallback = remaining[remaining.length - 1]?.id ?? 'welcome'
        return fallback
      })
    },
    [activeTabId, flushSave, tabs]
  )

  const renameNoteAtPath = useCallback(
    async (fromPath: string): Promise<void> => {
      const wasActive = activeNotePath === fromPath
      if (wasActive) {
        await flushSave()
      }

      setError(null)

      const input = window.prompt('Rename note (new path):', fromPath)
      if (!input) return
      if (input === fromPath) return

      try {
        const renamedPath = await xnote.renameNote(fromPath, input)
        const nextTabId = `note:${renamedPath}` as const

        setTabs((prev) =>
          prev.map((t) =>
            t.type === 'note' && t.path === fromPath
              ? { ...t, id: nextTabId, title: noteTitleFromPath(renamedPath), path: renamedPath }
              : t
          )
        )
        if (wasActive) setActiveTabId(nextTabId)
        await refreshNotes()
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e))
      }
    },
    [activeNotePath, flushSave, refreshNotes]
  )

  const renameNoteSilent = useCallback(
    async (fromPath: string, toPath: string): Promise<string | null> => {
      try {
        const renamedPath = await xnote.renameNote(fromPath, toPath)
        const nextTabId = `note:${renamedPath}` as const

        setTabs((prev) =>
          prev.map((t) =>
            t.type === 'note' && t.path === fromPath
              ? { ...t, id: nextTabId, title: noteTitleFromPath(renamedPath), path: renamedPath }
              : t
          )
        )
        setActiveTabId((prevId) => {
          return prevId === (`note:${fromPath}` as const) ? nextTabId : prevId
        })

        return renamedPath
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e))
        return null
      }
    },
    []
  )

  const moveNoteToFolder = useCallback(
    async (notePath: string, folderPath: string): Promise<void> => {
      const base = notePath.split('/').filter(Boolean).pop() ?? notePath
      const destination = folderPath ? `${folderPath}/${base}` : base
      if (destination === notePath) return

      if (activeNotePath === notePath) {
        await flushSave()
      }

      setError(null)
      const renamed = await renameNoteSilent(notePath, destination)
      if (!renamed) return
      await refreshNotes()
    },
    [activeNotePath, flushSave, refreshNotes, renameNoteSilent]
  )

  const moveFolderToFolder = useCallback(
    async (fromFolder: string, toFolder: string): Promise<void> => {
      const source = fromFolder.replace(/\/+$/, '')
      const target = toFolder.replace(/\/+$/, '')
      if (!source) return

      if (target === source || target.startsWith(`${source}/`)) {
        return
      }

      const name = source.split('/').filter(Boolean).pop()
      if (!name) return

      const nextFolder = target ? `${target}/${name}` : name
      if (nextFolder === source) return

      const prefix = `${source}/`
      const noteCount = notes.filter((n) => n.path.startsWith(prefix)).length

      const ok = window.confirm(`Move folder?\n\n${source}\n→ ${target || '(root)'}\n\nNotes: ${noteCount}`)
      if (!ok) return

      if (activeNotePath && activeNotePath.startsWith(prefix)) {
        await flushSave()
      }

      setError(null)

      try {
        await xnote.renameFolder(source, nextFolder)
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e))
        await refreshNotes()
        return
      }

      setTabs((prev) =>
        prev.map((t) => {
          if (t.type !== 'note') return t
          if (!t.path.startsWith(prefix)) return t
          const nextPath = `${nextFolder}${t.path.slice(source.length)}`
          return { ...t, id: `note:${nextPath}` as const, title: noteTitleFromPath(nextPath), path: nextPath }
        })
      )
      setActiveTabId((prevId) => {
        if (!prevId.startsWith('note:')) return prevId
        const prevPath = prevId.slice('note:'.length)
        if (!prevPath.startsWith(prefix)) return prevId
        const nextPath = `${nextFolder}${prevPath.slice(source.length)}`
        return `note:${nextPath}` as const
      })

      await refreshNotes()
    },
    [activeNotePath, flushSave, notes, refreshNotes]
  )

  const deleteNoteAtPath = useCallback(
    async (notePath: string): Promise<void> => {
      const ok = window.confirm(`Delete note?\n\n${notePath}`)
      if (!ok) return

      const wasActive = activeNotePath === notePath
      if (wasActive) {
        await flushSave()
      }

      setError(null)

      try {
        await xnote.deleteNote(notePath)
        await refreshNotes()
        await closeTab(`note:${notePath}` as const)
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e))
      }
    },
    [activeNotePath, closeTab, flushSave, refreshNotes]
  )

  const renameActiveNote = useCallback(async (): Promise<void> => {
    if (!activeNotePath) return
    await renameNoteAtPath(activeNotePath)
  }, [activeNotePath, renameNoteAtPath])

  const deleteActiveNote = useCallback(async (): Promise<void> => {
    if (!activeNotePath) return
    await deleteNoteAtPath(activeNotePath)
  }, [activeNotePath, deleteNoteAtPath])

  const copyToClipboard = useCallback(async (text: string): Promise<void> => {
    try {
      await navigator.clipboard.writeText(text)
    } catch {
      // ignore
    }
  }, [])

  const openFileContextMenu = useCallback(
    (notePath: string, x: number, y: number): void => {
      openContextMenu(x, y, [
        {
          id: 'open',
          label: 'Open',
          onClick: () => void openNoteTab(notePath)
        },
        {
          id: 'rename',
          label: 'Rename…',
          onClick: () => void renameNoteAtPath(notePath)
        },
        {
          id: 'delete',
          label: 'Delete…',
          onClick: () => void deleteNoteAtPath(notePath)
        },
        { kind: 'separator', id: 'sep' },
        {
          id: 'copy-path',
          label: 'Copy path',
          onClick: () => void copyToClipboard(notePath)
        }
      ])
    },
    [copyToClipboard, deleteNoteAtPath, openContextMenu, openNoteTab, renameNoteAtPath]
  )

  const openEditorContextMenu = useCallback(
    (x: number, y: number): void => {
      const canEdit = activeTab.type === 'note' && viewMode === 'edit'
      openContextMenu(x, y, [
        {
          id: 'undo',
          label: 'Undo',
          disabled: !canEdit,
          onClick: () => {
            document.execCommand('undo')
          }
        },
        {
          id: 'redo',
          label: 'Redo',
          disabled: !canEdit,
          onClick: () => {
            document.execCommand('redo')
          }
        },
        { kind: 'separator', id: 'sep-1' },
        {
          id: 'cut',
          label: 'Cut',
          disabled: !canEdit,
          onClick: () => {
            document.execCommand('cut')
          }
        },
        {
          id: 'copy',
          label: 'Copy',
          onClick: () => {
            document.execCommand('copy')
          }
        },
        {
          id: 'paste',
          label: 'Paste',
          disabled: !canEdit,
          onClick: () => {
            document.execCommand('paste')
          }
        },
        { kind: 'separator', id: 'sep-2' },
        {
          id: 'select-all',
          label: 'Select all',
          onClick: () => {
            document.execCommand('selectAll')
          }
        }
      ])
    },
    [activeTab.type, openContextMenu, viewMode]
  )

  const openLinkContextMenu = useCallback(
    (href: string, x: number, y: number): void => {
      const items: ContextMenuItem[] = [
        {
          id: 'open-link',
          label: 'Open link',
          onClick: () => openHref(href)
        },
        {
          id: 'copy-link',
          label: 'Copy link address',
          onClick: () => void copyToClipboard(href)
        }
      ]

      if (href.startsWith('xnote://wikilink')) {
        try {
          const url = new URL(href)
          const target = url.searchParams.get('target')
          if (target) {
            items.push({
              id: 'copy-wikilink',
              label: 'Copy wikilink',
              onClick: () => void copyToClipboard(`[[${target}]]`)
            })
          }
        } catch {
          // ignore
        }
      } else if (!href.startsWith('http://') && !href.startsWith('https://') && !/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(href)) {
        const pathPart = href.split('#', 2)[0] ?? ''
        const looksLikeNote = pathPart.toLowerCase().endsWith('.md') || !pathPart.includes('.')
        if (looksLikeNote && pathPart) {
          items.push({
            id: 'copy-note-path',
            label: 'Copy note path',
            onClick: () => void copyToClipboard(pathPart)
          })
        }
      }

      openContextMenu(x, y, items)
    },
    [copyToClipboard, openContextMenu, openHref]
  )

  const beginResize = useCallback(
    (side: 'left' | 'right', e: PointerEvent<HTMLDivElement>) => {
      const startWidth = side === 'left' ? leftWidth : rightWidth
      resizingRef.current = { side, startX: e.clientX, startWidth }
      ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    },
    [leftWidth, rightWidth]
  )

  const onResizeMove = useCallback((e: PointerEvent<HTMLDivElement>) => {
    const data = resizingRef.current
    if (!data) return
    const dx = e.clientX - data.startX
    if (data.side === 'left') {
      setLeftWidth(clamp(data.startWidth + dx, 240, 520))
    } else {
      setRightWidth(clamp(data.startWidth - dx, 260, 520))
    }
  }, [])

  const endResize = useCallback((e: PointerEvent<HTMLDivElement>) => {
    resizingRef.current = null
    try {
      ;(e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId)
    } catch {
      // ignore
    }
  }, [])

  const openCommandPalette = useCallback(() => {
    setPaletteMode('commands')
    setPaletteOpen(true)
  }, [])

  const openQuickSwitcher = useCallback(() => {
    setPaletteMode('files')
    setPaletteOpen(true)
  }, [])

  const toggleAssistant = useCallback(() => {
    setIsRightSidebarOpen(true)
    setRightTab('assistant')
  }, [])

  const updateMarkdownFromPanel = useCallback((next: string) => {
    setSaveState('idle')
    setIsDirty(true)
    setContent(next)
  }, [])

  const openFindReplace = useCallback(
    (focus: 'find' | 'replace' = 'find') => {
      if (activeTab.type !== 'note') return
      if (viewMode !== 'edit') setViewMode('edit')
      setFindReplaceFocus(focus)
      setFindReplaceOpen(true)
    },
    [activeTab.type, viewMode]
  )

  const closeFindReplace = useCallback(() => {
    setFindReplaceOpen(false)
    editorViewRef.current?.focus()
  }, [])

  const findNextInFile = useCallback(() => {
    const query = findText
    if (!query) return
    const view = editorViewRef.current
    if (!view) return

    const docText = view.state.doc.toString()
    const start = view.state.selection.main.to

    let index = docText.indexOf(query, start)
    if (index === -1 && start > 0) {
      index = docText.indexOf(query, 0)
    }
    if (index === -1) return

    view.dispatch({
      selection: EditorSelection.range(index, index + query.length),
      scrollIntoView: true
    })
    view.focus()
  }, [findText])

  const findPrevInFile = useCallback(() => {
    const query = findText
    if (!query) return
    const view = editorViewRef.current
    if (!view) return

    const docText = view.state.doc.toString()
    const start = view.state.selection.main.from - 1

    let index = docText.lastIndexOf(query, start)
    if (index === -1) {
      index = docText.lastIndexOf(query)
    }
    if (index === -1) return

    view.dispatch({
      selection: EditorSelection.range(index, index + query.length),
      scrollIntoView: true
    })
    view.focus()
  }, [findText])

  const replaceOneInFile = useCallback(() => {
    const query = findText
    if (!query) return
    const view = editorViewRef.current
    if (!view) return

    const selection = view.state.selection.main
    const selectedText = view.state.doc.sliceString(selection.from, selection.to)

    if (selectedText !== query) {
      findNextInFile()
      return
    }

    view.dispatch({
      changes: { from: selection.from, to: selection.to, insert: replaceText },
      selection: EditorSelection.cursor(selection.from + replaceText.length),
      scrollIntoView: true
    })
    view.focus()
    findNextInFile()
  }, [findNextInFile, findText, replaceText])

  const replaceAllInFile = useCallback(() => {
    const query = findText
    if (!query) return
    const view = editorViewRef.current
    if (!view) return

    const docText = view.state.doc.toString()
    if (!docText.includes(query)) return

    const nextText = docText.split(query).join(replaceText)
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: nextText },
      scrollIntoView: true
    })
    view.focus()
  }, [findText, replaceText])

  useEffect(() => {
    if (!findReplaceOpen) return
    if (activeTab.type !== 'note' || viewMode !== 'edit') {
      setFindReplaceOpen(false)
    }
  }, [activeTab.type, findReplaceOpen, viewMode])

  const isMac = useMemo(() => navigator.platform.toLowerCase().includes('mac'), [])
  const modKey = useMemo(() => (isMac ? 'Cmd' : 'Ctrl'), [isMac])

  useEffect(() => {
    commandRegistry.upsert(
      {
        id: 'cmd.commandPalette',
        title: 'Command palette',
        description: 'Search and run commands',
        group: 'App',
        keywords: 'command palette',
        defaultHotkey: `${modKey}+P`
      },
      async () => openCommandPalette()
    )
    commandRegistry.upsert(
      {
        id: 'cmd.openVault',
        title: 'Open vault',
        description: 'Select a folder to open as vault',
        group: 'Vault',
        keywords: 'vault folder open',
        defaultHotkey: `${modKey}+Shift+O`
      },
      async () => openVault()
    )
    commandRegistry.upsert(
      {
        id: 'cmd.newNote',
        title: 'New note',
        description: 'Create a new note in the vault',
        group: 'Note',
        keywords: 'new create note',
        defaultHotkey: `${modKey}+N`
      },
      async () => createNote()
    )
    commandRegistry.upsert(
      {
        id: 'cmd.newFolder',
        title: 'New folder',
        description: 'Create a new folder in the vault',
        group: 'Vault',
        keywords: 'new create folder',
        defaultHotkey: `${modKey}+Shift+N`
      },
      async () => createFolder()
    )
    commandRegistry.upsert(
      {
        id: 'cmd.renameNote',
        title: 'Rename note',
        description: 'Rename/move the current note',
        group: 'Note',
        keywords: 'rename move note',
        defaultHotkey: `${modKey}+R`
      },
      async () => renameActiveNote()
    )
    commandRegistry.upsert(
      {
        id: 'cmd.deleteNote',
        title: 'Delete note',
        description: 'Delete the current note',
        group: 'Note',
        keywords: 'delete remove note'
      },
      async () => deleteActiveNote()
    )
    commandRegistry.upsert(
      {
        id: 'cmd.quickSwitcher',
        title: 'Quick switcher',
        description: 'Search notes by title or path',
        group: 'Navigation',
        keywords: 'search switcher open note',
        defaultHotkey: `${modKey}+O`
      },
      async () => openQuickSwitcher()
    )
    commandRegistry.upsert(
      {
        id: 'cmd.graph',
        title: 'Open graph view',
        description: 'Open graph view tab',
        group: 'View',
        keywords: 'graph view',
        defaultHotkey: `${modKey}+G`
      },
      async () => openGraphTab()
    )
    commandRegistry.upsert(
      {
        id: 'cmd.toggleLeftSidebar',
        title: 'Toggle left sidebar',
        description: 'Show/hide the left sidebar',
        group: 'Layout',
        keywords: 'toggle left sidebar explorer search',
        defaultHotkey: `${modKey}+\\`
      },
      async () => setIsLeftSidebarOpen((v) => !v)
    )
    commandRegistry.upsert(
      {
        id: 'cmd.toggleRightSidebar',
        title: 'Toggle right sidebar',
        description: 'Show/hide the right sidebar',
        group: 'Layout',
        keywords: 'toggle right sidebar outline backlinks properties assistant',
        defaultHotkey: `${modKey}+Shift+\\`
      },
      async () => setIsRightSidebarOpen((v) => !v)
    )
    commandRegistry.upsert(
      {
        id: 'cmd.toggleMode',
        title: 'Toggle reading/editing mode',
        description: 'Switch between edit and read mode',
        group: 'View',
        keywords: 'toggle mode edit read preview',
        defaultHotkey: `${modKey}+E`
      },
      async () => setViewMode((m) => (m === 'edit' ? 'read' : 'edit'))
    )
    commandRegistry.upsert(
      {
        id: 'cmd.findInFile',
        title: 'Find in file',
        description: 'Find text in the current file',
        group: 'Editor',
        keywords: 'find search in file',
        defaultHotkey: `${modKey}+F`
      },
      async () => openFindReplace('find')
    )
    commandRegistry.upsert(
      {
        id: 'cmd.replaceInFile',
        title: 'Replace in file',
        description: 'Find and replace text in the current file',
        group: 'Editor',
        keywords: 'find replace in file',
        defaultHotkey: `${modKey}+H`
      },
      async () => openFindReplace('replace')
    )
    commandRegistry.upsert(
      {
        id: 'cmd.settings',
        title: 'Settings',
        description: 'Open settings',
        group: 'App',
        keywords: 'settings preferences',
        defaultHotkey: `${modKey}+,`
      },
      async () => setSettingsOpen(true)
    )
    commandRegistry.upsert(
      {
        id: 'cmd.navBack',
        title: 'Back',
        description: 'Go back in navigation history',
        group: 'Navigation',
        keywords: 'back previous history',
        defaultHotkey: isMac ? `${modKey}+[`: 'Alt+Left'
      },
      async () => goBack()
    )
    commandRegistry.upsert(
      {
        id: 'cmd.navForward',
        title: 'Forward',
        description: 'Go forward in navigation history',
        group: 'Navigation',
        keywords: 'forward next history',
        defaultHotkey: isMac ? `${modKey}+]` : 'Alt+Right'
      },
      async () => goForward()
    )
  }, [
    createFolder,
    createNote,
    deleteActiveNote,
    goBack,
    goForward,
    isMac,
    modKey,
    openCommandPalette,
    openFindReplace,
    openGraphTab,
    openQuickSwitcher,
    openVault,
    renameActiveNote
  ])

  useEffect(() => {
    const prefix = 'cmd.openRecentVault:'
    const wanted = new Set(recentVaults.map((p) => `${prefix}${p}`))

    for (const cmd of commandRegistry.list()) {
      if (!cmd.id.startsWith(prefix)) continue
      if (!wanted.has(cmd.id)) commandRegistry.unregister(cmd.id)
    }

    for (const p of recentVaults) {
      commandRegistry.upsert(
        {
          id: `${prefix}${p}`,
          title: `Open vault: ${vaultNameFromFsPath(p)}`,
          description: p,
          group: 'Vault',
          keywords: 'vault recent switch'
        },
        async () => openRecentVault(p)
      )
    }
  }, [openRecentVault, recentVaults])

  const commandItems = useMemo<PaletteItem[]>(() => {
    const iconById: Record<string, JSX.Element | undefined> = {
      'cmd.commandPalette': <Command size={16} />,
      'cmd.openVault': <FolderOpen size={16} />,
      'cmd.newNote': <Plus size={16} />,
      'cmd.newFolder': <FolderPlus size={16} />,
      'cmd.quickSwitcher': <Search size={16} />,
      'cmd.graph': <Search size={16} />,
      'cmd.findInFile': <Search size={16} />,
      'cmd.replaceInFile': <Search size={16} />,
      'cmd.toggleLeftSidebar': <PanelLeft size={16} />,
      'cmd.toggleRightSidebar': <PanelRight size={16} />,
      'cmd.toggleMode': viewMode === 'edit' ? <BookOpen size={16} /> : <Pencil size={16} />,
      'cmd.settings': <Command size={16} />,
      'cmd.navBack': <ArrowLeft size={16} />,
      'cmd.navForward': <ArrowRight size={16} />
    }

    return commands.map((cmd) => ({
      id: cmd.id,
      title: cmd.title,
      description: cmd.description,
      keywords: cmd.keywords,
      group: cmd.group,
      shortcut: customHotkeys[cmd.id] ?? cmd.defaultHotkey,
      icon: iconById[cmd.id] ?? (cmd.id.startsWith('cmd.openRecentVault:') ? <FolderOpen size={16} /> : undefined),
      onSelect: () => {
        void commandRegistry.run(cmd.id).catch((e: unknown) => {
          setError(e instanceof Error ? e.message : String(e))
        })
      }
    }))
  }, [commandRegistry, commands, customHotkeys, viewMode])

  const fileItems = useMemo<PaletteItem[]>(() => {
    return notes.map((n) => ({
      id: `file:${n.path}`,
      title: noteTitleFromPath(n.path),
      description: n.path,
      keywords: n.path,
      onSelect: () => void openNoteTab(n.path)
    }))
  }, [notes, openNoteTab])

  const paletteItems = paletteMode === 'files' ? fileItems : commandItems
  const paletteTitle = paletteMode === 'files' ? 'Quick switcher' : 'Command palette'
  const palettePlaceholder = paletteMode === 'files' ? 'Type to search notes...' : 'Type a command...'

  const commandIdsByHotkey = useMemo(() => {
    const map = new Map<string, string[]>()
    for (const cmd of commands) {
      const raw = customHotkeys[cmd.id] ?? cmd.defaultHotkey
      if (!raw) continue
      const normalized = normalizeHotkey(raw)
      if (!normalized) continue
      const existing = map.get(normalized)
      if (existing) existing.push(cmd.id)
      else map.set(normalized, [cmd.id])
    }
    return map
  }, [commands, customHotkeys])

  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  useEffect(() => {
    ;(async () => {
      await refreshRecentVaults()
      const restored = await xnote.getVaultPath()
      if (!restored) return
      await applyVaultSelection(restored)
    })().catch((e: unknown) => {
      setError(e instanceof Error ? e.message : String(e))
    })
  }, [applyVaultSelection, refreshRecentVaults])

  useEffect(() => {
    if (!activeNotePath) {
      setContent('')
      setIsDirty(false)
      setSaveState('idle')
      return
    }

    setError(null)
    void xnote
      .readNote(activeNotePath)
      .then((text) => {
        setContent(text)
        setIsDirty(false)
        setSaveState('idle')
      })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }, [activeNotePath])

  useEffect(() => {
    if (!activeNotePath) return
    if (!isDirty) return
    const timeout = window.setTimeout(async () => {
      try {
        setSaveState('saving')
        await xnote.writeNote(activeNotePath, content)
        setSaveState('saved')
        setIsDirty(false)
        await refreshNotes()
      } catch (e: unknown) {
        setSaveState('error')
        setError(e instanceof Error ? e.message : String(e))
      }
    }, 500)

    return () => window.clearTimeout(timeout)
  }, [activeNotePath, content, isDirty, refreshNotes])

  useEffect(() => {
    if (!vaultPath || !activeNotePath || rightTab !== 'backlinks') {
      setBacklinks([])
      setBacklinksLoading(false)
      setBacklinksError(null)
      return
    }

    let cancelled = false
    setBacklinksLoading(true)
    setBacklinksError(null)

    void xnote
      .getBacklinks(activeNotePath)
      .then((list) => {
        if (cancelled) return
        setBacklinks(list)
      })
      .catch((e: unknown) => {
        if (cancelled) return
        setBacklinksError(e instanceof Error ? e.message : String(e))
      })
      .finally(() => {
        if (cancelled) return
        setBacklinksLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [activeNotePath, notes, rightTab, vaultPath])

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return
      if (e.repeat) return

      const hotkey = hotkeyFromEvent(e)
      if (!hotkey) return

      const matches = commandIdsByHotkey.get(hotkey)
      if (!matches || matches.length === 0) return

      e.preventDefault()
      e.stopPropagation()
      void commandRegistry.run(matches[0]).catch((err: unknown) => {
        setError(err instanceof Error ? err.message : String(err))
      })
    }

    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [commandIdsByHotkey])

  useEffect(() => {
    if (!vaultPath || leftView !== 'search') {
      setSearchResults([])
      setSearchLoading(false)
      setSearchError(null)
      return
    }

    const q = searchQuery.trim()
    if (!q) {
      setSearchResults([])
      setSearchLoading(false)
      setSearchError(null)
      return
    }

    let cancelled = false
    setSearchLoading(true)
    setSearchError(null)

    const t = window.setTimeout(() => {
      void xnote
        .searchNotes(q)
        .then((results) => {
          if (cancelled) return
          setSearchResults(results)
        })
        .catch((e: unknown) => {
          if (cancelled) return
          setSearchError(e instanceof Error ? e.message : String(e))
        })
        .finally(() => {
          if (cancelled) return
          setSearchLoading(false)
        })
    }, 150)

    return () => {
      cancelled = true
      window.clearTimeout(t)
    }
  }, [leftView, notes, searchQuery, vaultPath])

  const subtitle = activeTab.type === 'note' ? activeTab.path : vaultName
  const canGoBack = navHistoryVersion >= 0 && navHistory.canGoBack()
  const canGoForward = navHistoryVersion >= 0 && navHistory.canGoForward()

  return (
    <div className="app">
      <Topbar
        title="XNote"
        subtitle={subtitle}
        tabs={tabs.map((t) => ({
          id: t.id,
          title: t.title,
          tooltip: t.type === 'note' ? t.path : t.title,
          isDirty: t.id === activeTab.id && isDirty && t.type === 'note',
          closable: t.id !== 'welcome'
        }))}
        activeTabId={activeTab.id}
        onSelectTab={(id) => setActiveTabId(id as WorkspaceTab['id'])}
        onCloseTab={(id) => void closeTab(id as WorkspaceTab['id'])}
        canGoBack={canGoBack}
        canGoForward={canGoForward}
        onGoBack={() => void goBack()}
        onGoForward={() => void goForward()}
        isLeftSidebarOpen={isLeftSidebarOpen}
        onToggleLeftSidebar={() => setIsLeftSidebarOpen((v) => !v)}
        isRightSidebarOpen={isRightSidebarOpen}
        onToggleRightSidebar={() => setIsRightSidebarOpen((v) => !v)}
        canToggleMode={activeTab.type === 'note'}
        viewMode={viewMode}
        onToggleViewMode={() => setViewMode((m) => (m === 'edit' ? 'read' : 'edit'))}
        onOpenCommandPalette={openCommandPalette}
      />

      {error ? (
        <div className="notice error">
          <div className="notice-title">Error</div>
          <div className="notice-body">{error}</div>
        </div>
      ) : null}

      <div className="shell">
        <Ribbon
          leftView={leftView}
          onSelectLeftView={setLeftView}
          vaultLoaded={Boolean(vaultPath)}
          assistantActive={isRightSidebarOpen && rightTab === 'assistant'}
          graphActive={activeTab.type === 'graph'}
          onOpenCommandPalette={openCommandPalette}
          onOpenGraph={() => void openGraphTab()}
          onToggleAssistant={toggleAssistant}
          onOpenSettings={() => setSettingsOpen(true)}
        />

        {isLeftSidebarOpen ? (
          <>
            <aside className="left-sidebar" style={{ width: leftWidth }}>
              <div className="panel-header">
                <div className="panel-title">{leftView === 'search' ? 'Search' : 'File explorer'}</div>
                <div className="panel-actions">
                  <button className="icon-btn" title="Open vault" onClick={() => void openVault()}>
                    <FolderOpen size={16} />
                  </button>
                  <button className="icon-btn" title="New note" onClick={() => void createNote()} disabled={!vaultPath}>
                    <Plus size={16} />
                  </button>
                </div>
              </div>

              <div className="panel-body">
                {leftView === 'search' ? (
                  <div className="search-panel">
                    <input
                      className="panel-filter"
                      placeholder={vaultPath ? 'Search...' : 'Open a vault to search'}
                      value={searchQuery}
                      onChange={(e) => setSearchQuery(e.target.value)}
                      disabled={!vaultPath}
                    />
                    <div className="search-results">
                      {searchQuery.trim() ? (
                        searchLoading ? (
                          <div className="muted">Searching...</div>
                        ) : searchError ? (
                          <div className="muted">{searchError}</div>
                        ) : searchResults.length === 0 ? (
                          <div className="muted">No results</div>
                        ) : (
                          searchResults.map((r) => (
                            <button
                              key={r.path}
                              className="search-result"
                              onClick={() => void openNoteTab(r.path)}
                              title={r.path}
                            >
                              <div className="search-result-title">
                                {highlightText(r.title || noteTitleFromPath(r.path), parsedSearchQuery)}
                              </div>
                              <div className="search-result-path muted">{highlightText(r.path, parsedSearchQuery)}</div>
                              {r.snippet ? (
                                <div className="search-result-snippet muted">{highlightText(r.snippet, parsedSearchQuery)}</div>
                              ) : null}
                            </button>
                          ))
                        )
                      ) : (
                        <div className="muted">Type to search note content, titles and paths.</div>
                      )}
                    </div>
                  </div>
                ) : (
                  <FileExplorer
                    files={notes}
                    folders={folders}
                    activePath={activeNotePath}
                    onOpenFile={(p) => void openNoteTab(p)}
                    onContextMenuFile={(p, x, y) => openFileContextMenu(p, x, y)}
                    onMoveFile={(fromPath, toFolder) => void moveNoteToFolder(fromPath, toFolder)}
                    onMoveFolder={(fromFolder, toFolder) => void moveFolderToFolder(fromFolder, toFolder)}
                    vaultLoaded={Boolean(vaultPath)}
                    vaultLabel={vaultName}
                  />
                )}
              </div>
            </aside>

            <div
              className="resize-handle"
              title="Resize sidebar"
              onPointerDown={(e) => beginResize('left', e)}
              onPointerMove={onResizeMove}
              onPointerUp={endResize}
            />
          </>
        ) : null}

        <main className="workspace">
          <div className="workspace-body">
            {activeTab.type === 'welcome' ? (
              <div className="welcome">
                <div className="welcome-title">Welcome to XNote</div>
                <div className="welcome-body">
                  This is an Obsidian-like UI shell. Open a vault and start writing Markdown.
                </div>
                <div className="welcome-actions">
                  <button className="primary-btn" onClick={() => void openVault()}>
                    <FolderOpen size={16} />
                    Open vault
                  </button>
                  <button className="secondary-btn" onClick={() => openCommandPalette()}>
                    <Command size={16} />
                    Command palette
                  </button>
                </div>
                {recentVaults.length > 0 ? (
                  <div className="welcome-section">
                    <div className="welcome-section-title">Recent vaults</div>
                    <div className="welcome-recent-list">
                      {recentVaults.map((p) => (
                        <button key={p} className="search-result" onClick={() => void openRecentVault(p)} title={p}>
                          <div className="search-result-title">{vaultNameFromFsPath(p)}</div>
                          <div className="search-result-path muted">{p}</div>
                        </button>
                      ))}
                    </div>
                  </div>
                ) : null}
              </div>
            ) : activeTab.type === 'graph' ? (
              <GraphView />
            ) : viewMode === 'edit' ? (
              <div
                className="editor-surface"
                onContextMenu={(e) => {
                  e.preventDefault()
                  openEditorContextMenu(e.clientX, e.clientY)
                }}
              >
                <CodeMirror
                  value={content}
                  height="100%"
                  extensions={cmExtensions}
                  theme={oneDark}
                  onCreateEditor={(view) => {
                    editorViewRef.current = view
                  }}
                  onChange={(v) => {
                    setSaveState('idle')
                    setIsDirty(true)
                    setContent(v)
                  }}
                />
              </div>
            ) : (
              <div className="reading markdown">
                <Markdown
                  remarkPlugins={[remarkGfm, remarkWikiLinks]}
                  components={{
                    a({ href, children, ...props }) {
                      return (
                        <a
                          {...props}
                          href={href}
                          onContextMenu={(e) => {
                            if (!href) return
                            e.preventDefault()
                            openLinkContextMenu(href, e.clientX, e.clientY)
                          }}
                          onClick={(e) => {
                            if (!href) return
                            if (href.startsWith('#')) return
                            e.preventDefault()
                            openHref(href)
                          }}
                        >
                          {children}
                        </a>
                      )
                    },
                    img({ src, alt, title }) {
                      return <VaultImage src={src} alt={alt} title={title} notePath={activeNotePath} />
                    },
                    blockquote({ node, children }) {
                      return <CalloutBlockquote node={node} children={children} />
                    }
                  }}
                >
                  {content}
                </Markdown>
              </div>
            )}
          </div>
        </main>

        {isRightSidebarOpen ? (
          <>
            <div
              className="resize-handle"
              title="Resize sidebar"
              onPointerDown={(e) => beginResize('right', e)}
              onPointerMove={onResizeMove}
              onPointerUp={endResize}
            />
            <RightSidebar
              open={isRightSidebarOpen}
              width={rightWidth}
              activeTab={rightTab}
              onSelectTab={setRightTab}
              markdown={content}
              activeNotePath={activeNotePath}
              backlinks={backlinks}
              backlinksLoading={backlinksLoading}
              backlinksError={backlinksError}
              onOpenBacklink={(p) => void openNoteTab(p)}
              onUpdateMarkdown={updateMarkdownFromPanel}
              onClose={() => setIsRightSidebarOpen(false)}
            />
          </>
        ) : null}
      </div>

      <footer className="statusbar">
        <div className="statusbar-left">
          {vaultPath ? vaultName : 'No vault'}
          {activeTab.type === 'note' ? <span className="status-sep">|</span> : null}
          {activeTab.type === 'note' ? <span className="status-item">{activeTab.path}</span> : null}
        </div>
        <div className="statusbar-right">
          {activeTab.type === 'note' ? (
            <>
              <span className="status-item">
                {viewMode === 'edit' ? 'Editing' : 'Reading'} | {saveState}
              </span>
              <span className="status-sep">|</span>
              <span className="status-item">{stats.words} words</span>
              <span className="status-sep">|</span>
              <span className="status-item">{stats.chars} chars</span>
            </>
          ) : (
            <span className="status-item">Ready</span>
          )}
        </div>
      </footer>

      <CommandPalette
        open={paletteOpen}
        title={paletteTitle}
        placeholder={palettePlaceholder}
        items={paletteItems}
        onClose={() => setPaletteOpen(false)}
      />

      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        theme={theme}
        onChangeTheme={setTheme}
        vaultPath={vaultPath}
        vaultSettings={vaultPath ? vaultSettings : null}
        onSaveVaultSettings={(next) => void saveVaultSettings(next)}
        commands={commands}
        hotkeys={customHotkeys}
        onSetHotkey={setCommandHotkey}
        onClearHotkey={clearCommandHotkey}
      />

      <FindReplaceModal
        open={findReplaceOpen}
        focus={findReplaceFocus}
        find={findText}
        replace={replaceText}
        matchCount={findMatchCount}
        onChangeFind={setFindText}
        onChangeReplace={setReplaceText}
        onFindNext={findNextInFile}
        onFindPrev={findPrevInFile}
        onReplaceOne={replaceOneInFile}
        onReplaceAll={replaceAllInFile}
        onClose={closeFindReplace}
      />

      <ContextMenu
        open={Boolean(contextMenu)}
        x={contextMenu?.x ?? 0}
        y={contextMenu?.y ?? 0}
        items={contextMenu?.items ?? []}
        onClose={closeContextMenu}
      />
    </div>
  )
}
