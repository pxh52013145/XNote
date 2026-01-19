import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'

export type ContextMenuItem =
  | {
      kind?: 'item'
      id: string
      label: string
      shortcut?: string
      disabled?: boolean
      onClick: () => void
    }
  | { kind: 'separator'; id: string }

export function ContextMenu(props: {
  open: boolean
  x: number
  y: number
  items: ContextMenuItem[]
  onClose: () => void
}) {
  const menuRef = useRef<HTMLDivElement | null>(null)
  const [pos, setPos] = useState(() => ({ x: props.x, y: props.y }))

  useEffect(() => {
    if (!props.open) return
    setPos({ x: props.x, y: props.y })
  }, [props.open, props.x, props.y])

  const focusableItemCount = useMemo(() => props.items.filter((i) => i.kind !== 'separator' && !i.disabled).length, [props.items])

  useEffect(() => {
    if (!props.open) return
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.preventDefault()
      props.onClose()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [props])

  useLayoutEffect(() => {
    if (!props.open) return
    const el = menuRef.current
    if (!el) return

    const padding = 8
    const rect = el.getBoundingClientRect()
    const maxX = Math.max(padding, window.innerWidth - rect.width - padding)
    const maxY = Math.max(padding, window.innerHeight - rect.height - padding)

    const nextX = Math.min(pos.x, maxX)
    const nextY = Math.min(pos.y, maxY)

    if (nextX === pos.x && nextY === pos.y) return
    setPos({ x: nextX, y: nextY })
  }, [focusableItemCount, pos.x, pos.y, props.open])

  if (!props.open) return null

  return (
    <div
      className="context-menu-overlay"
      role="presentation"
      onMouseDown={() => props.onClose()}
      onContextMenu={(e) => {
        e.preventDefault()
        props.onClose()
      }}
    >
      <div
        ref={menuRef}
        className="context-menu"
        role="menu"
        style={{ left: `${pos.x}px`, top: `${pos.y}px` }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {props.items.map((item) => {
          if (item.kind === 'separator') {
            return <div key={item.id} className="context-menu-sep" role="separator" />
          }

          return (
            <button
              key={item.id}
              className="context-menu-item"
              role="menuitem"
              disabled={item.disabled}
              onClick={() => {
                props.onClose()
                item.onClick()
              }}
            >
              <span className="context-menu-label">{item.label}</span>
              {item.shortcut ? <span className="context-menu-shortcut">{item.shortcut}</span> : null}
            </button>
          )
        })}
      </div>
    </div>
  )
}

