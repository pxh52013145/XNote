export type MarkdownHeading = {
  level: number
  text: string
  line: number
}

export function extractMarkdownHeadings(markdown: string): MarkdownHeading[] {
  const lines = markdown.split(/\r?\n/)
  const headings: MarkdownHeading[] = []
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const m = /^(#{1,6})\s+(.*)$/.exec(line)
    if (!m) continue
    headings.push({ level: m[1].length, text: m[2].trim(), line: i + 1 })
  }
  return headings
}

