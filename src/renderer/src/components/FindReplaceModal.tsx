import { useEffect, useRef } from 'react'
import { Modal } from './Modal'

export function FindReplaceModal(props: {
  open: boolean
  focus?: 'find' | 'replace'
  find: string
  replace: string
  matchCount: number
  onChangeFind: (next: string) => void
  onChangeReplace: (next: string) => void
  onFindNext: () => void
  onFindPrev: () => void
  onReplaceOne: () => void
  onReplaceAll: () => void
  onClose: () => void
}) {
  const findRef = useRef<HTMLInputElement>(null)
  const replaceRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!props.open) return
    const el = props.focus === 'replace' ? replaceRef.current : findRef.current
    if (!el) return
    el.focus()
    el.select()
  }, [props.focus, props.open])

  const canSearch = props.find.trim().length > 0

  return (
    <Modal open={props.open} title="Find & Replace" onClose={props.onClose} width={560}>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
        <label className="settings-field">
          <div className="settings-field-label">Find</div>
          <input
            ref={findRef}
            className="panel-filter"
            placeholder="Search text…"
            value={props.find}
            onChange={(e) => props.onChangeFind(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                if (e.shiftKey) props.onFindPrev()
                else props.onFindNext()
              }
            }}
          />
          <div className="settings-field-desc muted">
            {canSearch ? `${props.matchCount} matches` : 'Type to search in the current file.'}
          </div>
        </label>

        <label className="settings-field">
          <div className="settings-field-label">Replace</div>
          <input
            ref={replaceRef}
            className="panel-filter"
            placeholder="Replace with…"
            value={props.replace}
            onChange={(e) => props.onChangeReplace(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                props.onReplaceOne()
              }
            }}
          />
        </label>

        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', justifyContent: 'flex-end' }}>
          <button className="secondary-btn" onClick={props.onFindPrev} disabled={!canSearch}>
            Prev
          </button>
          <button className="secondary-btn" onClick={props.onFindNext} disabled={!canSearch}>
            Next
          </button>
          <button className="secondary-btn" onClick={props.onReplaceOne} disabled={!canSearch}>
            Replace
          </button>
          <button className="secondary-btn" onClick={props.onReplaceAll} disabled={!canSearch}>
            Replace all
          </button>
        </div>
      </div>
    </Modal>
  )
}

