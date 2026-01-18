import { useMemo } from 'react'
import { Bot, Link2, ListTree, SlidersHorizontal, X } from 'lucide-react'
import { extractMarkdownHeadings } from '@shared/markdown'
import { noteTitleFromPath } from '@shared/paths'
import { PropertiesPanel } from './PropertiesPanel'

export type RightSidebarTab = 'outline' | 'backlinks' | 'properties' | 'assistant'

export function RightSidebar(props: {
  open: boolean
  width: number
  activeTab: RightSidebarTab
  onSelectTab: (tab: RightSidebarTab) => void
  markdown: string
  activeNotePath: string | null
  backlinks: string[]
  backlinksLoading: boolean
  backlinksError: string | null
  onOpenBacklink: (path: string) => void
  onUpdateMarkdown: (next: string) => void
  onClose: () => void
}) {
  const headings = useMemo(() => extractMarkdownHeadings(props.markdown), [props.markdown])

  if (!props.open) return null

  return (
    <aside className="right-sidebar" style={{ width: props.width }}>
      <div className="right-tabs">
        <button
          className={props.activeTab === 'outline' ? 'right-tab active' : 'right-tab'}
          title="Outline"
          onClick={() => props.onSelectTab('outline')}
        >
          <ListTree size={16} />
        </button>
        <button
          className={props.activeTab === 'backlinks' ? 'right-tab active' : 'right-tab'}
          title="Backlinks"
          onClick={() => props.onSelectTab('backlinks')}
        >
          <Link2 size={16} />
        </button>
        <button
          className={props.activeTab === 'properties' ? 'right-tab active' : 'right-tab'}
          title="Properties"
          onClick={() => props.onSelectTab('properties')}
          disabled={!props.activeNotePath}
        >
          <SlidersHorizontal size={16} />
        </button>
        <button
          className={props.activeTab === 'assistant' ? 'right-tab active' : 'right-tab'}
          title="Assistant"
          onClick={() => props.onSelectTab('assistant')}
        >
          <Bot size={16} />
        </button>

        <div className="right-tabs-spacer" />
        <button className="right-tab" title="Close sidebar" onClick={props.onClose}>
          <X size={16} />
        </button>
      </div>

      <div className="panel-body">
        {props.activeTab === 'outline' ? (
          <div className="outline">
            {headings.length === 0 ? (
              <div className="muted">No headings</div>
            ) : (
              headings.map((h) => (
                <div
                  key={`${h.line}-${h.text}`}
                  className="outline-item"
                  style={{ paddingLeft: `${(h.level - 1) * 12}px` }}
                >
                  {h.text}
                </div>
              ))
            )}
          </div>
        ) : null}

        {props.activeTab === 'backlinks' ? (
          !props.activeNotePath ? (
            <div className="muted">Open a note to see backlinks</div>
          ) : props.backlinksLoading ? (
            <div className="muted">Loading backlinks...</div>
          ) : props.backlinksError ? (
            <div className="muted">{props.backlinksError}</div>
          ) : props.backlinks.length === 0 ? (
            <div className="muted">No backlinks</div>
          ) : (
            <div className="search-results">
              {props.backlinks.map((p) => (
                <button
                  key={p}
                  className="search-result"
                  onClick={() => props.onOpenBacklink(p)}
                  title={p}
                >
                  <div className="search-result-title">{noteTitleFromPath(p)}</div>
                  <div className="search-result-path muted">{p}</div>
                </button>
              ))}
            </div>
          )
        ) : null}

        {props.activeTab === 'properties' ? (
          !props.activeNotePath ? (
            <div className="muted">Open a note to edit properties</div>
          ) : (
            <PropertiesPanel markdown={props.markdown} onChangeMarkdown={props.onUpdateMarkdown} />
          )
        ) : null}

        {props.activeTab === 'assistant' ? (
          <div className="assistant">
            <div className="assistant-messages">
              <div className="assistant-message system">
                Assistant UI stub. Later this will connect to your remote AI service and call app tools.
              </div>
              <div className="assistant-message user">Help me set up a vault and create a note</div>
              <div className="assistant-message assistant">
                I can open a vault, create notes, and insert links once the remote service is connected.
              </div>
            </div>
            <div className="assistant-composer">
              <input className="assistant-input" placeholder="Ask the assistant..." disabled />
              <button className="assistant-send" disabled>
                Send
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </aside>
  )
}
