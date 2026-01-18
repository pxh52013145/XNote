import { useEffect, useId, type ReactNode } from 'react'
import { X } from 'lucide-react'

export function Modal(props: {
  open: boolean
  title: string
  onClose: () => void
  children: ReactNode
  width?: number
  height?: number
}) {
  const titleId = useId()

  useEffect(() => {
    if (!props.open) return
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        props.onClose()
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [props])

  if (!props.open) return null

  return (
    <div className="overlay" role="presentation" onMouseDown={() => props.onClose()}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        style={{
          width: props.width ? `${props.width}px` : undefined,
          height: props.height ? `${props.height}px` : undefined
        }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <div className="modal-title" id={titleId}>
            {props.title}
          </div>
          <button className="icon-btn" title="Close" onClick={props.onClose}>
            <X size={16} />
          </button>
        </div>
        <div className="modal-body">{props.children}</div>
      </div>
    </div>
  )
}
