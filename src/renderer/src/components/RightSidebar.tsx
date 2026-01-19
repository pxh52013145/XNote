import { useCallback, useEffect, useMemo, useState } from 'react'
import { Bot, Link2, ListTree, SlidersHorizontal, X } from 'lucide-react'
import type { AgentToolDefinition, AgentToolResult } from '@shared/agent'
import { extractMarkdownHeadings } from '@shared/markdown'
import { noteTitleFromPath } from '@shared/paths'
import { xnote } from '../api'
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

  const [tools, setTools] = useState<AgentToolDefinition[]>([])
  const [toolsLoading, setToolsLoading] = useState(false)
  const [toolsError, setToolsError] = useState<string | null>(null)
  const [selectedToolName, setSelectedToolName] = useState<string>('')
  const [toolArgs, setToolArgs] = useState<string>('{}')
  const [runResult, setRunResult] = useState<AgentToolResult | null>(null)
  const [running, setRunning] = useState(false)

  const selectedTool = useMemo(() => tools.find((t) => t.name === selectedToolName) ?? null, [selectedToolName, tools])

  useEffect(() => {
    if (props.activeTab !== 'assistant') return
    let cancelled = false

    setToolsLoading(true)
    setToolsError(null)

    void xnote
      .agentListTools()
      .then((list) => {
        if (cancelled) return
        setTools(list)
        setSelectedToolName((prev) => prev || list[0]?.name || '')
      })
      .catch((e: unknown) => {
        if (cancelled) return
        setToolsError(e instanceof Error ? e.message : String(e))
      })
      .finally(() => {
        if (cancelled) return
        setToolsLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [props.activeTab])

  const runTool = useCallback(async () => {
    if (!selectedToolName) return
    let args: unknown = {}
    const raw = toolArgs.trim()
    if (raw) {
      try {
        args = JSON.parse(raw) as unknown
      } catch {
        setRunResult({ ok: false, error: 'Invalid JSON args' })
        return
      }
    }

    setRunning(true)
    setRunResult(null)
    try {
      const result = await xnote.agentRunTool(selectedToolName, args)
      setRunResult(result)
    } finally {
      setRunning(false)
    }
  }, [selectedToolName, toolArgs])

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
            <div className="assistant-toolbar">
              <select
                className="panel-filter"
                value={selectedToolName}
                onChange={(e) => {
                  setSelectedToolName(e.target.value)
                  setRunResult(null)
                }}
                disabled={toolsLoading || tools.length === 0}
              >
                {tools.length === 0 ? <option value="">No tools</option> : null}
                {tools.map((t) => (
                  <option key={t.name} value={t.name}>
                    {t.name}
                  </option>
                ))}
              </select>
              <button className="assistant-send" onClick={() => void runTool()} disabled={!selectedToolName || running}>
                {running ? 'Running…' : 'Run'}
              </button>
            </div>

            <textarea
              className="panel-filter assistant-args"
              value={toolArgs}
              placeholder='Tool args (JSON), e.g. {"path":"Note.md"}'
              onChange={(e) => setToolArgs(e.target.value)}
            />

            <div className="assistant-messages">
              {toolsLoading ? <div className="assistant-message system">Loading tools…</div> : null}
              {toolsError ? <div className="assistant-message system">{toolsError}</div> : null}

              {selectedTool ? (
                <div className="assistant-message system">
                  <div style={{ fontWeight: 800 }}>{selectedTool.name}</div>
                  <div style={{ marginTop: 6 }}>{selectedTool.description}</div>
                  <div style={{ marginTop: 10, fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace' }}>
                    {JSON.stringify(selectedTool.inputSchema, null, 2)}
                  </div>
                </div>
              ) : null}

              {runResult ? (
                <pre className="assistant-message assistant-output">
                  {runResult.ok ? JSON.stringify(runResult.result, null, 2) : `Error: ${runResult.error}`}
                </pre>
              ) : null}
            </div>
          </div>
        ) : null}
      </div>
    </aside>
  )
}
