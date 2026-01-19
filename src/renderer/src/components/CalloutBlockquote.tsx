import { Children, type ReactNode } from 'react'

type CalloutInfo = {
  kind: string
  title: string
  fold?: '+' | '-' | null
}

function extractText(node: unknown): string {
  if (!node || typeof node !== 'object') return ''
  const anyNode = node as { type?: unknown; value?: unknown; children?: unknown }

  if (anyNode.type === 'text') {
    return typeof anyNode.value === 'string' ? anyNode.value : ''
  }

  const children = Array.isArray(anyNode.children) ? anyNode.children : []
  return children.map(extractText).join('')
}

function sanitizeCalloutKind(kind: string): string {
  return kind
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '')
}

function getFirstParagraphText(blockquoteNode: unknown): string | null {
  if (!blockquoteNode || typeof blockquoteNode !== 'object') return null
  const anyNode = blockquoteNode as { type?: unknown; tagName?: unknown; children?: unknown }
  if (anyNode.type !== 'element' || anyNode.tagName !== 'blockquote') return null

  const children = Array.isArray(anyNode.children) ? anyNode.children : []
  const firstP = children.find((c) => {
    if (!c || typeof c !== 'object') return false
    const el = c as { type?: unknown; tagName?: unknown }
    return el.type === 'element' && el.tagName === 'p'
  })
  if (!firstP) return null
  return extractText(firstP).trim()
}

export function parseCalloutFromBlockquoteNode(blockquoteNode: unknown): CalloutInfo | null {
  const firstLine = getFirstParagraphText(blockquoteNode)
  if (!firstLine) return null

  const match = /^\[!([^\]]+)\]([+-])?\s*(.*)$/.exec(firstLine)
  if (!match) return null

  const kind = sanitizeCalloutKind(match[1] ?? '')
  if (!kind) return null

  const fold = (match[2] as '+' | '-' | undefined) ?? null
  const title = (match[3] ?? '').trim() || kind
  return { kind, title, fold }
}

export function CalloutBlockquote(props: { node?: unknown; children?: ReactNode }) {
  const info = parseCalloutFromBlockquoteNode(props.node)
  if (!info) {
    return <blockquote>{props.children}</blockquote>
  }

  const renderedChildren = Children.toArray(props.children)
  const body = renderedChildren.slice(1)

  const kindClass = `callout-${info.kind}`
  const foldClass = info.fold ? `callout-fold-${info.fold === '+' ? 'open' : 'closed'}` : ''

  return (
    <div className={`callout ${kindClass} ${foldClass}`}>
      <div className="callout-title">
        <span className="callout-icon" aria-hidden="true" />
        <span className="callout-title-text">{info.title}</span>
      </div>
      {info.fold === '-' ? null : <div className="callout-content">{body}</div>}
    </div>
  )
}

