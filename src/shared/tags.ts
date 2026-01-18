import { extractFrontmatter } from './frontmatter'

function stripCodeBlocks(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/`[^`]*`/g, ' ')
}

export function extractInlineTags(markdown: string): string[] {
  const body = extractFrontmatter(markdown).body
  const cleaned = stripCodeBlocks(body)
  const re = /(^|\s)#([A-Za-z0-9/_-]+(?:[A-Za-z0-9/_-]+)*)/g
  const tags = new Set<string>()
  for (const match of cleaned.matchAll(re)) {
    const tag = (match[2] ?? '').trim()
    if (!tag) continue
    tags.add(tag)
  }
  return [...tags].sort((a, b) => a.localeCompare(b))
}

