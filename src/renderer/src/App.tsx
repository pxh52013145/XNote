import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent } from 'react'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import CodeMirror from '@uiw/react-codemirror'
import { markdown } from '@codemirror/lang-markdown'
import { languages } from '@codemirror/language-data'
import { oneDark } from '@codemirror/theme-one-dark'
import { BookOpen, Command, FolderOpen, PanelLeft, PanelRight, Pencil, Plus, Search } from 'lucide-react'
import type { NoteFile } from '@shared/types'
import { noteTitleFromPath } from '@shared/paths'
import { countWords } from '@shared/text'
import type { NoteSearchResult } from '@shared/search'
import { xnote } from './api'
import { remarkWikiLinks } from './markdown/remarkWikiLinks'
import { CommandPalette, type PaletteItem } from './components/CommandPalette'
import { FileExplorer } from './components/FileExplorer'
import { GraphView } from './components/GraphView'
import { Ribbon, type LeftSidebarView } from './components/Ribbon'
import { RightSidebar, type RightSidebarTab } from './components/RightSidebar'
import { SettingsModal } from './components/SettingsModal'
import { Topbar } from './components/Topbar'

type SaveState = 'idle' | 'saving' | 'saved' | 'error'

type WorkspaceTab =
  | { id: 'welcome'; type: 'welcome'; title: 'Welcome' }
  | { id: `note:${string}`; type: 'note'; title: string; path: string }
  | { id: 'graph'; type: 'graph'; title: 'Graph view' }

function clamp(n: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, n))
}

function normalizeWikiTarget(target: string): string {
  return target.trim().replace(/\\/g, '/').replace(/^\.?\//, '').replace(/\.md$/i, '')
}

export default function App() {
  const [vaultPath, setVaultPath] = useState<string | null>(null)
  const [notes, setNotes] = useState<NoteFile[]>([])

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

  const [paletteOpen, setPaletteOpen] = useState(false)
  const [paletteMode, setPaletteMode] = useState<'commands' | 'files'>('commands')
  const [settingsOpen, setSettingsOpen] = useState(false)

  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<NoteSearchResult[]>([])
  const [searchLoading, setSearchLoading] = useState(false)
  const [searchError, setSearchError] = useState<string | null>(null)
  const [backlinks, setBacklinks] = useState<string[]>([])
  const [backlinksLoading, setBacklinksLoading] = useState(false)
  const [backlinksError, setBacklinksError] = useState<string | null>(null)

  const cmExtensions = useMemo(() => [markdown({ codeLanguages: languages })], [])

  const vaultName = vaultPath ? vaultPath.split(/[/\\]/).filter(Boolean).pop() ?? 'Vault' : 'No vault'

  const stats = useMemo(() => {
    return {
      words: countWords(content),
      chars: content.length
    }
  }, [content])

  const refreshNotes = useCallback(async (): Promise<void> => {
    const list = await xnote.listNotes()
    setNotes(list)
  }, [])

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
    setVaultPath(selected)
    setTabs([{ id: 'welcome', type: 'welcome', title: 'Welcome' }])
    setActiveTabId('welcome')
    setContent('')
    setIsDirty(false)
    setSaveState('idle')
    setLeftView('explorer')
    setIsLeftSidebarOpen(true)
    await refreshNotes()
  }, [refreshNotes])

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

  const createNote = useCallback(async (): Promise<void> => {
    setError(null)
    if (!vaultPath) {
      await openVault()
      return
    }
    const input = window.prompt('New note path (relative to vault):', 'Untitled.md')
    if (!input) return

    const createdPath = await xnote.createNote(input, '')
    await refreshNotes()
    await openNoteTab(createdPath)
  }, [openNoteTab, openVault, refreshNotes, vaultPath])

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
      const folder = activeNotePath ? activeNotePath.split('/').slice(0, -1).join('/') : ''
      return folder ? `${folder}/${target}.md` : `${target}.md`
    },
    [activeNotePath]
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

  const renameActiveNote = useCallback(async (): Promise<void> => {
    if (!activeNotePath) return

    await flushSave()
    setError(null)

    const input = window.prompt('Rename note (new path):', activeNotePath)
    if (!input) return

    try {
      const renamedPath = await xnote.renameNote(activeNotePath, input)
      const nextTabId = `note:${renamedPath}` as const

      setTabs((prev) =>
        prev.map((t) =>
          t.type === 'note' && t.path === activeNotePath
            ? { ...t, id: nextTabId, title: noteTitleFromPath(renamedPath), path: renamedPath }
            : t
        )
      )
      setActiveTabId(nextTabId)
      await refreshNotes()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [activeNotePath, flushSave, refreshNotes])

  const deleteActiveNote = useCallback(async (): Promise<void> => {
    if (!activeNotePath) return

    const ok = window.confirm(`Delete note?\n\n${activeNotePath}`)
    if (!ok) return

    await flushSave()
    setError(null)

    try {
      await xnote.deleteNote(activeNotePath)
      await refreshNotes()
      await closeTab(`note:${activeNotePath}` as const)
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [activeNotePath, closeTab, flushSave, refreshNotes])

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

  const commandItems = useMemo<PaletteItem[]>(() => {
    const mod = navigator.platform.toLowerCase().includes('mac') ? 'Cmd' : 'Ctrl'
    return [
      {
        id: 'cmd.openVault',
        title: 'Open vault',
        shortcut: `${mod}+Shift+O`,
        icon: <FolderOpen size={16} />,
        onSelect: () => void openVault()
      },
      {
        id: 'cmd.newNote',
        title: 'New note',
        shortcut: `${mod}+N`,
        icon: <Plus size={16} />,
        onSelect: () => void createNote()
      },
      {
        id: 'cmd.renameNote',
        title: 'Rename note',
        shortcut: `${mod}+R`,
        onSelect: () => void renameActiveNote()
      },
      {
        id: 'cmd.deleteNote',
        title: 'Delete note',
        onSelect: () => void deleteActiveNote()
      },
      {
        id: 'cmd.quickSwitcher',
        title: 'Quick switcher',
        shortcut: `${mod}+O`,
        icon: <Search size={16} />,
        onSelect: () => openQuickSwitcher()
      },
      {
        id: 'cmd.graph',
        title: 'Open graph view',
        shortcut: `${mod}+G`,
        icon: <Search size={16} />,
        onSelect: () => void openGraphTab()
      },
      {
        id: 'cmd.toggleLeftSidebar',
        title: 'Toggle left sidebar',
        shortcut: `${mod}+\\`,
        icon: <PanelLeft size={16} />,
        onSelect: () => setIsLeftSidebarOpen((v) => !v)
      },
      {
        id: 'cmd.toggleRightSidebar',
        title: 'Toggle right sidebar',
        shortcut: `${mod}+Shift+\\`,
        icon: <PanelRight size={16} />,
        onSelect: () => setIsRightSidebarOpen((v) => !v)
      },
      {
        id: 'cmd.toggleMode',
        title: 'Toggle reading/editing mode',
        shortcut: `${mod}+E`,
        icon: viewMode === 'edit' ? <BookOpen size={16} /> : <Pencil size={16} />,
        onSelect: () => setViewMode((m) => (m === 'edit' ? 'read' : 'edit'))
      },
      {
        id: 'cmd.settings',
        title: 'Settings',
        shortcut: `${mod}+,`,
        icon: <Command size={16} />,
        onSelect: () => setSettingsOpen(true)
      }
    ]
  }, [createNote, openGraphTab, openQuickSwitcher, openVault, viewMode])

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

  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  useEffect(() => {
    ;(async () => {
      const restored = await xnote.getVaultPath()
      if (!restored) return
      setVaultPath(restored)
      await refreshNotes()
    })().catch((e: unknown) => {
      setError(e instanceof Error ? e.message : String(e))
    })
  }, [refreshNotes])

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
      const isMod = e.ctrlKey || e.metaKey
      if (!isMod) return

      if (e.key.toLowerCase() === 'p') {
        e.preventDefault()
        openCommandPalette()
        return
      }

      if (e.key.toLowerCase() === 'o') {
        e.preventDefault()
        openQuickSwitcher()
        return
      }

      if (e.key === ',') {
        e.preventDefault()
        setSettingsOpen(true)
        return
      }
    }

    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [openCommandPalette, openQuickSwitcher])

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
                              <div className="search-result-title">{r.title || noteTitleFromPath(r.path)}</div>
                              <div className="search-result-path muted">{r.path}</div>
                              {r.snippet ? <div className="search-result-snippet muted">{r.snippet}</div> : null}
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
                    activePath={activeNotePath}
                    onOpenFile={(p) => void openNoteTab(p)}
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
              </div>
            ) : activeTab.type === 'graph' ? (
              <GraphView />
            ) : viewMode === 'edit' ? (
              <CodeMirror
                value={content}
                height="100%"
                extensions={cmExtensions}
                theme={oneDark}
                onChange={(v) => {
                  setSaveState('idle')
                  setIsDirty(true)
                  setContent(v)
                }}
              />
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
                          onClick={(e) => {
                            if (!href) return
                            if (href.startsWith('xnote://wikilink')) {
                              e.preventDefault()
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
                              e.preventDefault()
                              void xnote.openExternal(href)
                              return
                            }

                            if (href.startsWith('#')) return

                            if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(href)) {
                              e.preventDefault()
                              void xnote.openExternal(href)
                              return
                            }

                            const pathPart = href.split('#', 2)[0] ?? ''
                            const looksLikeNote = pathPart.toLowerCase().endsWith('.md') || !pathPart.includes('.')
                            if (looksLikeNote) {
                              e.preventDefault()
                              void openWikiLinkTarget(pathPart)
                            }
                          }}
                        >
                          {children}
                        </a>
                      )
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
      />
    </div>
  )
}
