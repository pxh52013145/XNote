import { useEffect, useState } from 'react'
import { Copy, Minus, Square, X } from 'lucide-react'
import { xnote } from '../api'

export function Titlebar(props: { title: string; subtitle?: string }) {
  const [isMaximized, setIsMaximized] = useState(false)

  useEffect(() => {
    void xnote.windowIsMaximized().then(setIsMaximized)
    const off = xnote.onWindowMaximizedChanged((v) => setIsMaximized(v))
    return () => off()
  }, [])

  return (
    <div className="titlebar" onDoubleClick={() => void xnote.windowToggleMaximize()}>
      <div className="titlebar-left">
        <div className="titlebar-appmark">X</div>
        <div className="titlebar-titles">
          <div className="titlebar-title">{props.title}</div>
          {props.subtitle ? <div className="titlebar-subtitle">{props.subtitle}</div> : null}
        </div>
      </div>

      <div className="titlebar-center" />

      <div className="titlebar-window-controls" onDoubleClick={(e) => e.stopPropagation()}>
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
    </div>
  )
}

