type MdastNode = {
  type: string
  value?: string
  url?: string
  children?: MdastNode[]
}

function parseInner(inner: string): { target: string; display: string; heading?: string } | null {
  const trimmed = inner.trim()
  if (!trimmed) return null

  const [rawTargetPart, rawDisplayPart] = trimmed.split('|', 2)
  const targetPart = (rawTargetPart ?? '').trim()
  if (!targetPart) return null

  const [beforeHeading, afterHeading] = targetPart.split('#', 2)
  const target = beforeHeading.trim()
  if (!target) return null

  const heading = afterHeading?.trim()
  const display = (rawDisplayPart ?? '').trim()
  return { target, display: display || target, heading: heading || undefined }
}

function createLinkNode(target: string, display: string, heading?: string): MdastNode {
  const url = new URL('xnote://wikilink')
  url.searchParams.set('target', target)
  if (heading) url.searchParams.set('heading', heading)
  return { type: 'link', url: url.toString(), children: [{ type: 'text', value: display }] }
}

function splitTextByWikiLinks(value: string): MdastNode[] {
  const nodes: MdastNode[] = []
  const re = /!?\[\[([^[\]]+?)\]\]/g
  let lastIndex = 0

  for (const match of value.matchAll(re)) {
    const index = match.index ?? 0
    if (index > lastIndex) {
      nodes.push({ type: 'text', value: value.slice(lastIndex, index) })
    }

    const parsed = parseInner(match[1] ?? '')
    if (parsed) {
      nodes.push(createLinkNode(parsed.target, parsed.display, parsed.heading))
    } else {
      nodes.push({ type: 'text', value: match[0] ?? '' })
    }

    lastIndex = index + (match[0]?.length ?? 0)
  }

  if (lastIndex < value.length) {
    nodes.push({ type: 'text', value: value.slice(lastIndex) })
  }

  return nodes.length === 0 ? [{ type: 'text', value }] : nodes
}

/**
 * remark plugin: converts Obsidian-style wiki links into mdast `link` nodes
 * so `react-markdown` can render them as anchors.
 */
export function remarkWikiLinks() {
  return (tree: MdastNode) => {
    const visit = (node: MdastNode) => {
      const children = node.children
      if (!children || children.length === 0) return

      for (let i = 0; i < children.length; i++) {
        const child = children[i]
        if (child.type === 'text' && typeof child.value === 'string') {
          const next = splitTextByWikiLinks(child.value)
          if (next.length === 1 && next[0]?.type === 'text') {
            continue
          }
          children.splice(i, 1, ...next)
          i += next.length - 1
          continue
        }

        visit(child)
      }
    }

    visit(tree)
  }
}

