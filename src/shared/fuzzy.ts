function normalize(s: string): string {
  return s.toLowerCase()
}

function isBoundaryChar(c: string): boolean {
  return c === '/' || c === '\\' || c === '-' || c === '_' || c === ' ' || c === '.' || c === ':' || c === '#'
}

function scoreToken(token: string, haystack: string): number | null {
  const pattern = normalize(token.trim())
  if (!pattern) return 0

  const text = normalize(haystack)
  if (!text) return null

  let score = 0
  let lastIdx = -1
  let firstIdx = -1
  let searchFrom = 0

  for (let pi = 0; pi < pattern.length; pi++) {
    const ch = pattern[pi]!
    const idx = text.indexOf(ch, searchFrom)
    if (idx === -1) return null

    if (firstIdx === -1) firstIdx = idx

    score += 10

    if (idx === 0 || isBoundaryChar(text[idx - 1]!)) {
      score += 8
    }

    if (lastIdx !== -1) {
      const gap = idx - lastIdx - 1
      if (gap === 0) score += 15
      else score -= Math.min(8, gap)
    }

    lastIdx = idx
    searchFrom = idx + 1
  }

  if (firstIdx >= 0) {
    score += Math.max(0, 30 - firstIdx)
  }

  score += Math.max(0, 20 - (text.length - pattern.length))

  return score
}

/**
 * Returns a score for a fuzzy match, or null if no match.
 * Higher scores are better.
 *
 * Supports multi-token queries separated by whitespace: all tokens must match.
 */
export function fuzzyScore(query: string, text: string): number | null {
  const q = query.trim()
  if (!q) return 0
  const tokens = q.split(/\s+/).filter(Boolean)
  if (tokens.length === 0) return 0

  let total = 0
  for (const token of tokens) {
    const score = scoreToken(token, text)
    if (score == null) return null
    total += score
  }

  return total
}

