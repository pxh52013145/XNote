import { useMemo } from 'react'
import { Network } from 'lucide-react'

type Node = { id: string; x: number; y: number }
type Edge = { from: string; to: string }

function seededRandom(seed: number): () => number {
  let s = seed
  return () => {
    s = (s * 1664525 + 1013904223) % 4294967296
    return s / 4294967296
  }
}

export function GraphView(props: { seed?: number }) {
  const graph = useMemo(() => {
    const rand = seededRandom(props.seed ?? 42)
    const nodes: Node[] = Array.from({ length: 18 }, (_, i) => ({
      id: `n${i}`,
      x: 8 + rand() * 84,
      y: 10 + rand() * 80
    }))
    const edges: Edge[] = []
    for (let i = 0; i < nodes.length; i++) {
      const a = nodes[i]
      const b = nodes[Math.floor(rand() * nodes.length)]
      if (a.id !== b.id) edges.push({ from: a.id, to: b.id })
    }
    return { nodes, edges }
  }, [props.seed])

  const byId = new Map(graph.nodes.map((n) => [n.id, n]))

  return (
    <div className="graph-view">
      <div className="graph-hero">
        <Network size={20} />
        <div className="graph-hero-title">Graph view</div>
        <div className="graph-hero-subtitle muted">UI placeholder (renders a static graph).</div>
      </div>

      <div className="graph-canvas">
        <svg viewBox="0 0 100 100" className="graph-svg" aria-label="Graph">
          {graph.edges.map((e, idx) => {
            const a = byId.get(e.from)
            const b = byId.get(e.to)
            if (!a || !b) return null
            return <line key={idx} x1={a.x} y1={a.y} x2={b.x} y2={b.y} className="graph-edge" />
          })}
          {graph.nodes.map((n) => (
            <circle key={n.id} cx={n.x} cy={n.y} r={2.2} className="graph-node" />
          ))}
        </svg>
      </div>
    </div>
  )
}

