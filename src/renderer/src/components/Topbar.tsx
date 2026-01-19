import { useEffect, useState } from 'react'
import { ArrowLeft, ArrowRight, BookOpen, Command, Copy, Minus, PanelLeft, PanelRight, Pencil, Square, X } from 'lucide-react'
import { xnote } from '../api'

export type TopbarTab = {
  id: string
  title: string
  tooltip?: string
  isDirty?: boolean
  closable?: boolean
}

export function Topbar(props: {
  title: string
  subtitle?: string
  tabs: TopbarTab[]
  activeTabId: string
  onSelectTab: (id: string) => void
  onCloseTab: (id: string) => void

  canGoBack: boolean
  canGoForward: boolean
  onGoBack: () => void
  onGoForward: () => void

  isLeftSidebarOpen: boolean
  onToggleLeftSidebar: () => void
  isRightSidebarOpen: boolean
  onToggleRightSidebar: () => void

  canToggleMode: boolean
  viewMode: 'edit' | 'read'
  onToggleViewMode: () => void

  onOpenCommandPalette: () => void
}) {
  const [isMaximized, setIsMaximized] = useState(false)

  useEffect(() => {
    void xnote.windowIsMaximized().then(setIsMaximized)
    const off = xnote.onWindowMaximizedChanged((v) => setIsMaximized(v))
    return () => off()
  }, [])

  return (
    <header className="topbar" onDoubleClick={() => void xnote.windowToggleMaximize()}>
      <div className="topbar-left">
        <div className="titlebar-appmark">X</div>
        <div className="topbar-titles">
          <div className="topbar-title">{props.title}</div>
          {props.subtitle ? <div className="topbar-subtitle">{props.subtitle}</div> : null}
        </div>
      </div>

      <div className="topbar-tabs">
        <div className="tab-strip">
          {props.tabs.map((t) => {
            const isActive = t.id === props.activeTabId
            return (
              <button
                key={t.id}
                className={isActive ? 'tab active' : 'tab'}
                onClick={() => props.onSelectTab(t.id)}
                title={t.tooltip ?? t.title}
              >
                <span className="tab-title">
                  {t.title}
                  {isActive && t.isDirty ? <span className="tab-dirty">*</span> : null}
                </span>
                {t.closable ? (
                  <span
                    className="tab-close"
                    onClick={(e) => {
                      e.preventDefault()
                      e.stopPropagation()
                      props.onCloseTab(t.id)
                    }}
                    title="Close"
                  >
                    <X size={14} />
                  </span>
                ) : null}
              </button>
            )
          })}
        </div>
      </div>

      <div className="topbar-actions">
        <button className="icon-btn" title="Back" onClick={props.onGoBack} disabled={!props.canGoBack}>
          <ArrowLeft size={16} />
        </button>
        <button className="icon-btn" title="Forward" onClick={props.onGoForward} disabled={!props.canGoForward}>
          <ArrowRight size={16} />
        </button>
        <button
          className={props.isLeftSidebarOpen ? 'icon-btn' : 'icon-btn active'}
          title={props.isLeftSidebarOpen ? 'Hide left sidebar' : 'Show left sidebar'}
          onClick={props.onToggleLeftSidebar}
        >
          <PanelLeft size={16} />
        </button>
        <button
          className={props.isRightSidebarOpen ? 'icon-btn' : 'icon-btn active'}
          title={props.isRightSidebarOpen ? 'Hide right sidebar' : 'Show right sidebar'}
          onClick={props.onToggleRightSidebar}
        >
          <PanelRight size={16} />
        </button>
        {props.canToggleMode ? (
          <button
            className="icon-btn"
            title={props.viewMode === 'edit' ? 'Switch to reading mode' : 'Switch to editing mode'}
            onClick={props.onToggleViewMode}
          >
            {props.viewMode === 'edit' ? <BookOpen size={16} /> : <Pencil size={16} />}
          </button>
        ) : null}
        <button className="icon-btn" title="Command palette" onClick={props.onOpenCommandPalette}>
          <Command size={16} />
        </button>
      </div>

      <div className="topbar-window-controls" onDoubleClick={(e) => e.stopPropagation()}>
        <button className="win-btn" title="Minimize" onClick={() => void xnote.windowMinimize()}>
          <Minus size={16} />
        </button>
        <button
          className="win-btn"
          title={isMaximized ? 'Restore' : 'Maximize'}
          onClick={() => void xnote.windowToggleMaximize()}
        >
          {isMaximized ? <Copy size={16} /> : <Square size={16} />}
        </button>
        <button className="win-btn close" title="Close" onClick={() => void xnote.windowClose()}>
          <X size={16} />
        </button>
      </div>
    </header>
  )
}
