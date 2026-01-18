import { useEffect, useMemo, useState } from 'react'
import { Plus, Trash2 } from 'lucide-react'
import type { FrontmatterValue } from '@shared/frontmatter'
import {
  deleteFrontmatterKey,
  extractFrontmatter,
  listFrontmatterKeys,
  renameFrontmatterKey,
  setFrontmatterValue
} from '@shared/frontmatter'

function isValidKey(key: string): boolean {
  return /^[A-Za-z0-9_.-]+$/.test(key)
}

function toInputValue(value: FrontmatterValue): string {
  if (Array.isArray(value)) return value.map((v) => (v == null ? '' : String(v))).join(', ')
  if (value === null) return ''
  return String(value)
}

function parseScalarInput(raw: string): string | number | boolean | null {
  const trimmed = raw.trim()
  if (trimmed === 'null' || trimmed === '~') return null
  if (trimmed === 'true') return true
  if (trimmed === 'false') return false
  if (/^-?\d+(\.\d+)?$/.test(trimmed)) return Number(trimmed)
  return raw.trim().replace(/^['"]|['"]$/g, '')
}

function parseFreeformValue(raw: string, options: { preferArray: boolean }): FrontmatterValue {
  const trimmed = raw.trim()
  if (!trimmed) return options.preferArray ? [] : ''

  if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
    const inner = trimmed.slice(1, -1).trim()
    if (!inner) return []
    return inner
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
      .map((s) => parseScalarInput(s))
  }

  if (options.preferArray && trimmed.includes(',')) {
    return trimmed
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
      .map((s) => parseScalarInput(s))
  }

  return parseScalarInput(raw)
}

export function PropertiesPanel(props: {
  markdown: string
  onChangeMarkdown: (next: string) => void
  disabled?: boolean
}) {
  const extraction = useMemo(() => extractFrontmatter(props.markdown), [props.markdown])
  const data = extraction.data

  const orderedKeys = useMemo(() => {
    const inRaw = extraction.raw ? listFrontmatterKeys(extraction.raw) : []
    const known = Object.keys(data)
    const set = new Set(inRaw)
    const merged = [...inRaw.filter((k) => k in data), ...known.filter((k) => !set.has(k))].filter(Boolean)
    return merged
  }, [data, extraction.raw])

  const hiddenKeys = useMemo(() => {
    const inRaw = extraction.raw ? listFrontmatterKeys(extraction.raw) : []
    return inRaw.filter((k) => !(k in data))
  }, [data, extraction.raw])

  const [newKey, setNewKey] = useState('')
  const [newValue, setNewValue] = useState('')
  const [error, setError] = useState<string | null>(null)

  const apply = (next: string) => {
    setError(null)
    props.onChangeMarkdown(next)
  }

  return (
    <div className="properties">
      {hiddenKeys.length > 0 ? (
        <div className="properties-warning muted">
          Some YAML blocks are not editable yet: {hiddenKeys.join(', ')}
        </div>
      ) : null}

      {orderedKeys.length === 0 ? <div className="muted">No properties</div> : null}

      {orderedKeys.map((key) => {
        const value = data[key]
        if (value === undefined) return null

        return (
          <PropertyRow
            key={key}
            k={key}
            value={value}
            markdown={props.markdown}
            onApply={apply}
          />
        )
      })}

      <div className="prop-add">
        <div className="prop-key">
          <input
            className="prop-input"
            placeholder="Key"
            value={newKey}
            onChange={(e) => setNewKey(e.target.value)}
            disabled={props.disabled}
          />
        </div>
        <div className="prop-value">
          <input
            className="prop-input"
            placeholder="Value"
            value={newValue}
            onChange={(e) => setNewValue(e.target.value)}
            disabled={props.disabled}
          />
        </div>
        <div className="prop-actions">
          <button
            className="icon-btn"
            title="Add property"
            disabled={props.disabled}
            onClick={() => {
              const k = newKey.trim()
              if (!isValidKey(k)) {
                setError('Invalid key')
                return
              }
              const v = parseFreeformValue(newValue, { preferArray: k === 'tags' })
              try {
                const next = setFrontmatterValue(props.markdown, k, v)
                apply(next)
                setNewKey('')
                setNewValue('')
              } catch (e: unknown) {
                setError(e instanceof Error ? e.message : String(e))
              }
            }}
          >
            <Plus size={16} />
          </button>
        </div>
      </div>

      {error ? <div className="properties-error muted">{error}</div> : null}
    </div>
  )
}

function PropertyRow(props: { k: string; value: FrontmatterValue; markdown: string; onApply: (next: string) => void }) {
  const [keyDraft, setKeyDraft] = useState(props.k)
  const [valueDraft, setValueDraft] = useState(toInputValue(props.value))

  useEffect(() => {
    setKeyDraft(props.k)
    setValueDraft(toInputValue(props.value))
  }, [props.k, props.value])

  const commitValue = () => {
    const preferArray = Array.isArray(props.value) || props.k === 'tags'
    const nextValue = parseFreeformValue(valueDraft, { preferArray })
    const next = setFrontmatterValue(props.markdown, props.k, nextValue)
    props.onApply(next)
  }

  const commitKey = () => {
    const to = keyDraft.trim()
    if (!to || to === props.k) return
    if (!isValidKey(to)) return
    const next = renameFrontmatterKey(props.markdown, props.k, to)
    props.onApply(next)
  }

  return (
    <div className="prop-row">
      <div className="prop-key">
        <input
          className="prop-input prop-key-input"
          value={keyDraft}
          onChange={(e) => setKeyDraft(e.target.value)}
          onBlur={() => {
            try {
              commitKey()
            } catch {
              // ignore
            }
          }}
        />
      </div>
      <div className="prop-value">
        <input
          className="prop-input"
          value={valueDraft}
          onChange={(e) => setValueDraft(e.target.value)}
          onBlur={() => {
            try {
              commitValue()
            } catch {
              // ignore
            }
          }}
          onKeyDown={(e) => {
            if (e.key !== 'Enter') return
            e.preventDefault()
            ;(e.currentTarget as HTMLInputElement).blur()
          }}
        />
      </div>
      <div className="prop-actions">
        <button
          className="icon-btn"
          title="Delete property"
          onClick={() => {
            const next = deleteFrontmatterKey(props.markdown, props.k)
            props.onApply(next)
          }}
        >
          <Trash2 size={16} />
        </button>
      </div>
    </div>
  )
}
