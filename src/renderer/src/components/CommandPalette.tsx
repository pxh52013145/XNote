import { useEffect, useMemo, useRef, useState } from 'react'
import { fuzzyScore } from '@shared/fuzzy'

export type PaletteItem = {
  id: string
  title: string
  description?: string
  shortcut?: string
  keywords?: string
  group?: string
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

  const { groups, flatItems } = useMemo(() => {
    const q = normalize(query)
    const ranked = props.items
      .map((item, index) => ({ item, index, score: bestItemScore(item, q) }))
      .filter((r) => r.score != null)

    const hasGroups = ranked.some((r) => Boolean(r.item.group?.trim()))
    if (!hasGroups) {
      const items = ranked.sort((a, b) => (b.score ?? 0) - (a.score ?? 0) || a.index - b.index).map((r) => r.item)
      return { groups: [{ name: null as string | null, items }], flatItems: items }
    }

    type RankedItem = (typeof ranked)[number]
    type GroupState = { name: string; firstIndex: number; bestScore: number; items: RankedItem[] }

    const byGroup = new Map<string, GroupState>()
    for (const r of ranked) {
      const name = r.item.group?.trim() || 'Other'
      const existing = byGroup.get(name)
      if (!existing) {
        byGroup.set(name, { name, firstIndex: r.index, bestScore: r.score ?? 0, items: [r] })
        continue
      }
      existing.firstIndex = Math.min(existing.firstIndex, r.index)
      existing.bestScore = Math.max(existing.bestScore, r.score ?? 0)
      existing.items.push(r)
    }

    const queryActive = q.length > 0
    const sortedGroups = [...byGroup.values()].sort((a, b) => {
      if (queryActive) {
        return b.bestScore - a.bestScore || a.firstIndex - b.firstIndex
      }
      return a.firstIndex - b.firstIndex
    })

    const groups = sortedGroups.map((g) => {
      const items = g.items
        .sort((a, b) => {
          if (queryActive) {
            return (b.score ?? 0) - (a.score ?? 0) || a.index - b.index
          }
          return a.index - b.index
        })
        .map((r) => r.item)
      return { name: g.name, items }
    })

    const flatItems = groups.flatMap((g) => g.items)
    return { groups, flatItems }
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
        setActiveIndex((i) => Math.min(i + 1, Math.max(0, flatItems.length - 1)))
        return
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setActiveIndex((i) => Math.max(0, i - 1))
        return
      }
      if (e.key === 'Enter') {
        const item = flatItems[activeIndex]
        if (!item) return
        e.preventDefault()
        props.onClose()
        item.onSelect()
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [activeIndex, flatItems, props])

  useEffect(() => {
    setActiveIndex((i) => Math.min(i, Math.max(0, flatItems.length - 1)))
  }, [flatItems.length])

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
          {flatItems.length === 0 ? <div className="palette-empty">No results</div> : null}

          {(() => {
            let offset = 0
            return groups.map((group) => {
              const start = offset
              offset += group.items.length
              return (
                <div key={group.name ?? 'all'} className="palette-group-block">
                  {group.name ? <div className="palette-group">{group.name}</div> : null}
                  {group.items.map((item, idx) => {
                    const globalIndex = start + idx
                    return (
                      <button
                        key={item.id}
                        className={globalIndex === activeIndex ? 'palette-item active' : 'palette-item'}
                        onMouseEnter={() => setActiveIndex(globalIndex)}
                        onClick={() => {
                          props.onClose()
                          item.onSelect()
                        }}
                        role="option"
                        aria-selected={globalIndex === activeIndex}
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
                    )
                  })}
                </div>
              )
            })
          })()}
        </div>
      </div>
    </div>
  )
}
