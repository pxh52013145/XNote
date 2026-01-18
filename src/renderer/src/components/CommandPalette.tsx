import { useEffect, useMemo, useRef, useState } from 'react'
import { fuzzyScore } from '@shared/fuzzy'

export type PaletteItem = {
  id: string
  title: string
  description?: string
  shortcut?: string
  keywords?: string
  icon?: JSX.Element
  onSelect: () => void
}

function normalize(s: string): string {
  return s.trim().toLowerCase()
}

function bestItemScore(item: PaletteItem, query: string): number | null {
  if (!query) return 0

  const titleScore = fuzzyScore(query, item.title)
  const keywordScore = item.keywords ? fuzzyScore(query, item.keywords) : null
  const descScore = item.description ? fuzzyScore(query, item.description) : null
  const shortcutScore = item.shortcut ? fuzzyScore(query, item.shortcut) : null

  const weighted: Array<number | null> = [
    titleScore == null ? null : titleScore * 3,
    keywordScore == null ? null : keywordScore * 2,
    descScore == null ? null : descScore * 1,
    shortcutScore == null ? null : shortcutScore * 0.5
  ]

  const best = weighted.reduce<number | null>((acc, v) => {
    if (v == null) return acc
    if (acc == null) return v
    return Math.max(acc, v)
  }, null)

  return best
}

export function CommandPalette(props: {
  open: boolean
  title: string
  placeholder: string
  items: PaletteItem[]
  onClose: () => void
}) {
  const inputRef = useRef<HTMLInputElement | null>(null)
  const [query, setQuery] = useState('')
  const [activeIndex, setActiveIndex] = useState(0)

  const filtered = useMemo(() => {
    const q = normalize(query)
    const ranked = props.items
      .map((item, index) => ({ item, index, score: bestItemScore(item, q) }))
      .filter((r) => r.score != null)
      .sort((a, b) => (b.score ?? 0) - (a.score ?? 0) || a.index - b.index)

    return ranked.map((r) => r.item)
  }, [props.items, query])

  useEffect(() => {
    if (!props.open) return
    setQuery('')
    setActiveIndex(0)
    const t = window.setTimeout(() => inputRef.current?.focus(), 0)
    return () => window.clearTimeout(t)
  }, [props.open])

  useEffect(() => {
    if (!props.open) return
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        props.onClose()
        return
      }
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setActiveIndex((i) => Math.min(i + 1, Math.max(0, filtered.length - 1)))
        return
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setActiveIndex((i) => Math.max(0, i - 1))
        return
      }
      if (e.key === 'Enter') {
        const item = filtered[activeIndex]
        if (!item) return
        e.preventDefault()
        props.onClose()
        item.onSelect()
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [activeIndex, filtered, props])

  useEffect(() => {
    setActiveIndex((i) => Math.min(i, Math.max(0, filtered.length - 1)))
  }, [filtered.length])

  if (!props.open) return null

  return (
    <div className="overlay" role="presentation" onMouseDown={props.onClose}>
      <div className="palette" role="dialog" aria-modal="true" onMouseDown={(e) => e.stopPropagation()}>
        <div className="palette-header">
          <div className="palette-title">{props.title}</div>
          <input
            ref={inputRef}
            className="palette-input"
            placeholder={props.placeholder}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>

        <div className="palette-list" role="listbox">
          {filtered.length === 0 ? <div className="palette-empty">No results</div> : null}

          {filtered.map((item, idx) => (
            <button
              key={item.id}
              className={idx === activeIndex ? 'palette-item active' : 'palette-item'}
              onMouseEnter={() => setActiveIndex(idx)}
              onClick={() => {
                props.onClose()
                item.onSelect()
              }}
              role="option"
              aria-selected={idx === activeIndex}
            >
              <div className="palette-item-left">
                {item.icon ? <div className="palette-icon">{item.icon}</div> : null}
                <div className="palette-text">
                  <div className="palette-item-title">{item.title}</div>
                  {item.description ? <div className="palette-item-desc">{item.description}</div> : null}
                </div>
              </div>
              {item.shortcut ? <div className="palette-shortcut">{item.shortcut}</div> : null}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
