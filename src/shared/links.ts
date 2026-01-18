export type WikiLink = {
  /**
   * Target as typed inside [[...]] (without alias/heading), trimmed.
   * May include folders, e.g. "folder/note".
   */
  target: string
  /**
   * Optional heading fragment (after "#") trimmed.
   */
  heading?: string
  /**
   * Optional display text (after "|") trimmed.
   */
  display?: string
  /**
   * Whether this link was an embed (![[...]]). Kept for future.
   */
  embed: boolean
}

function normalizeTarget(raw: string): { target: string; heading?: string } {
  const trimmed = raw.trim()
  const [beforeHeading, afterHeading] = trimmed.split('#', 2)
  const target = beforeHeading.trim()
  const heading = afterHeading?.trim()
  return heading ? { target, heading } : { target }
}

/**
 * Extract Obsidian-style wiki links:
 * - [[Target]]
 * - [[Target|Alias]]
 * - [[Target#Heading|Alias]]
 * - ![[Target]] (embed)
 */
export function extractWikiLinks(markdown: string): WikiLink[] {
  const results: WikiLink[] = []
  const re = /(!?)\[\[([^[\]]+?)\]\]/g

  for (const match of markdown.matchAll(re)) {
    const embed = match[1] === '!'
    const inner = (match[2] ?? '').trim()
    if (!inner) continue

    const [rawTargetPart, rawDisplayPart] = inner.split('|', 2)
    const { target, heading } = normalizeTarget(rawTargetPart ?? '')
    if (!target) continue

    const display = rawDisplayPart?.trim()
    results.push({ target, heading, display: display || undefined, embed })
  }

  return results
}

