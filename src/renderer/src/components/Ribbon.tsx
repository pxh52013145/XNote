import { Bot, Command, FileText, Network, Search, Settings } from 'lucide-react'

export type LeftSidebarView = 'explorer' | 'search'

export function Ribbon(props: {
  leftView: LeftSidebarView
  onSelectLeftView: (v: LeftSidebarView) => void
  vaultLoaded: boolean
  assistantActive: boolean
  graphActive: boolean
  onOpenCommandPalette: () => void
  onOpenGraph: () => void
  onToggleAssistant: () => void
  onOpenSettings: () => void
}) {
  return (
    <div className="ribbon">
      <div className="ribbon-group">
        <button
          className={props.leftView === 'explorer' ? 'ribbon-btn active' : 'ribbon-btn'}
          title="File explorer"
          onClick={() => props.onSelectLeftView('explorer')}
        >
          <FileText size={18} />
        </button>
        <button
          className={props.leftView === 'search' ? 'ribbon-btn active' : 'ribbon-btn'}
          title="Search"
          onClick={() => props.onSelectLeftView('search')}
          disabled={!props.vaultLoaded}
        >
          <Search size={18} />
        </button>
      </div>

      <div className="ribbon-divider" />

      <div className="ribbon-group">
        <button className="ribbon-btn" title="Command palette" onClick={props.onOpenCommandPalette}>
          <Command size={18} />
        </button>
        <button
          className={props.graphActive ? 'ribbon-btn active' : 'ribbon-btn'}
          title="Graph view"
          onClick={props.onOpenGraph}
          disabled={!props.vaultLoaded}
        >
          <Network size={18} />
        </button>
      </div>

      <div className="ribbon-spacer" />

      <div className="ribbon-group">
        <button
          className={props.assistantActive ? 'ribbon-btn active' : 'ribbon-btn'}
          title="AI assistant"
          onClick={props.onToggleAssistant}
        >
          <Bot size={18} />
        </button>
        <button className="ribbon-btn" title="Settings" onClick={props.onOpenSettings}>
          <Settings size={18} />
        </button>
      </div>
    </div>
  )
}
