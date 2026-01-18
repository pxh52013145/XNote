export type FrontmatterScalar = string | number | boolean | null
export type FrontmatterValue = FrontmatterScalar | FrontmatterScalar[]
export type FrontmatterData = Record<string, FrontmatterValue>

export type FrontmatterExtraction = {
  data: FrontmatterData
  /**
   * Markdown body without the frontmatter block (if any).
   */
  body: string
  /**
   * Raw YAML (without the --- lines). Present only when frontmatter exists.
   */
  raw?: string
}

function isDelimiterLine(line: string): boolean {
  const trimmed = line.trim()
  return trimmed === '---' || trimmed === '...'
}

function isTopLevelKeyLine(line: string): boolean {
  return /^[A-Za-z0-9_.-]+\s*:\s*.*$/.test(line) && !/^\s/.test(line)
}

function stripQuotes(value: string): string {
  const v = value.trim()
  if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) {
    return v.slice(1, -1)
  }
  return v
}

function parseScalar(raw: string): FrontmatterScalar {
  const v = raw.trim()
  if (!v) return ''
  if (v === 'null' || v === '~') return null
  if (v === 'true') return true
  if (v === 'false') return false
  if (/^-?\d+(\.\d+)?$/.test(v)) return Number(v)
  return stripQuotes(v)
}

function parseInlineArray(raw: string): FrontmatterScalar[] | null {
  const v = raw.trim()
  if (!v.startsWith('[') || !v.endsWith(']')) return null
  const inner = v.slice(1, -1).trim()
  if (!inner) return []
  return inner
    .split(',')
    .map((s) => stripQuotes(s.trim()))
    .filter((s) => s.length > 0)
    .map((s) => parseScalar(s))
}

function parseYamlFrontmatter(yaml: string): FrontmatterData {
  const data: FrontmatterData = {}
  const lines = yaml.replace(/\r\n/g, '\n').split('\n')

  let i = 0
  while (i < lines.length) {
    const line = lines[i] ?? ''
    const trimmed = line.trim()
    i++

    if (!trimmed) continue
    if (trimmed.startsWith('#')) continue

    const match = /^([A-Za-z0-9_.-]+)\s*:\s*(.*)$/.exec(line)
    if (!match) continue

    const key = match[1]!
    const rest = (match[2] ?? '').trim()

    if (!rest) {
      // Null scalar when there is no indented block following.
      let j = i
      while (j < lines.length && !(lines[j] ?? '').trim()) j++
      const peek = j < lines.length ? (lines[j] ?? '') : ''
      const hasIndentedBlock = /^\s+/.test(peek)

      if (!hasIndentedBlock) {
        data[key] = null
        continue
      }

      // Possible block list
      const items: FrontmatterScalar[] = []
      while (i < lines.length) {
        const nextLine = lines[i] ?? ''
        const nextTrimmed = nextLine.trim()
        if (!nextTrimmed) {
          i++
          continue
        }

        const isIndented = /^\s+/.test(nextLine)
        if (!isIndented) break

        const itemMatch = /^\s*-\s*(.*)$/.exec(nextLine)
        if (!itemMatch) break

        items.push(parseScalar(itemMatch[1] ?? ''))
        i++
      }

      if (items.length > 0) {
        data[key] = items
      }
      continue
    }

    const inlineArr = parseInlineArray(rest)
    if (inlineArr) {
      data[key] = inlineArr
      continue
    }

    data[key] = parseScalar(rest)
  }

  return data
}

function yamlFormatScalar(value: FrontmatterScalar): string {
  if (value === null) return 'null'
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    return Number.isFinite(value) ? String(value) : JSON.stringify(String(value))
  }
  const s = value

  const safe = /^[A-Za-z0-9/_-]+$/.test(s) && !/^(true|false|null)$/.test(s) && !/^-?\d+(\.\d+)?$/.test(s)
  return safe ? s : JSON.stringify(s)
}

function yamlBlockForValue(key: string, value: FrontmatterValue): string[] {
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return [`${key}: []`]
    }
    return [`${key}:`, ...value.map((v) => `  - ${yamlFormatScalar(v)}`)]
  }

  return [`${key}: ${yamlFormatScalar(value)}`]
}

type YamlBlock =
  | { kind: 'preamble'; lines: string[] }
  | { kind: 'entry'; key: string; lines: string[] }

function splitYamlIntoBlocks(yaml: string): YamlBlock[] {
  const lines = yaml.replace(/\r\n/g, '\n').split('\n')
  const blocks: YamlBlock[] = []

  let i = 0
  let preamble: string[] = []

  while (i < lines.length) {
    const line = lines[i] ?? ''
    if (!isTopLevelKeyLine(line)) {
      preamble.push(line)
      i++
      continue
    }

    if (preamble.length > 0) {
      blocks.push({ kind: 'preamble', lines: preamble })
      preamble = []
    }

    const key = (line.split(':', 1)[0] ?? '').trim()
    const entryLines: string[] = [line]
    i++

    while (i < lines.length) {
      const next = lines[i] ?? ''
      if (isTopLevelKeyLine(next)) break
      entryLines.push(next)
      i++
    }

    blocks.push({ kind: 'entry', key, lines: entryLines })
  }

  if (preamble.length > 0) {
    blocks.push({ kind: 'preamble', lines: preamble })
  }

  return blocks
}

function joinYamlBlocks(blocks: YamlBlock[]): string {
  const lines: string[] = []
  for (const block of blocks) {
    lines.push(...block.lines)
  }

  // Trim leading/trailing empty lines for a clean block.
  while (lines.length > 0 && !(lines[0] ?? '').trim()) lines.shift()
  while (lines.length > 0 && !(lines[lines.length - 1] ?? '').trim()) lines.pop()

  return lines.join('\n')
}

function buildMarkdownWithFrontmatter(rawYaml: string | null, body: string): string {
  const trimmedYaml = rawYaml?.trim() ?? ''
  if (!trimmedYaml) {
    return body
  }

  const bodyNormalized = body.replace(/^\n+/, '')
  return `---\n${trimmedYaml}\n---\n${bodyNormalized}`
}

export function listFrontmatterKeys(rawYaml: string): string[] {
  const keys: string[] = []
  const seen = new Set<string>()
  for (const block of splitYamlIntoBlocks(rawYaml)) {
    if (block.kind !== 'entry') continue
    if (seen.has(block.key)) continue
    seen.add(block.key)
    keys.push(block.key)
  }
  return keys
}

function isValidFrontmatterKey(key: string): boolean {
  return /^[A-Za-z0-9_.-]+$/.test(key)
}

export function setFrontmatterValue(markdown: string, key: string, value: FrontmatterValue): string {
  const k = key.trim()
  if (!isValidFrontmatterKey(k)) throw new Error('Invalid property key')

  const extraction = extractFrontmatter(markdown)
  const rawYaml = extraction.raw ?? ''

  const blocks = splitYamlIntoBlocks(rawYaml)
  const updated: YamlBlock[] = []
  let replaced = false

  for (const block of blocks) {
    if (block.kind !== 'entry') {
      updated.push(block)
      continue
    }
    if (block.key !== k) {
      updated.push(block)
      continue
    }
    if (!replaced) {
      updated.push({ kind: 'entry', key: k, lines: yamlBlockForValue(k, value) })
      replaced = true
    }
    // Remove duplicates of the same key.
  }

  if (!replaced) {
    updated.push({ kind: 'entry', key: k, lines: yamlBlockForValue(k, value) })
  }

  const nextYaml = joinYamlBlocks(updated)
  return buildMarkdownWithFrontmatter(nextYaml ? nextYaml : null, extraction.body)
}

export function deleteFrontmatterKey(markdown: string, key: string): string {
  const k = key.trim()
  if (!k) return markdown

  const extraction = extractFrontmatter(markdown)
  if (!extraction.raw) return markdown

  const blocks = splitYamlIntoBlocks(extraction.raw)
  const updated = blocks.filter((b) => b.kind !== 'entry' || b.key !== k)
  const nextYaml = joinYamlBlocks(updated)
  return buildMarkdownWithFrontmatter(nextYaml ? nextYaml : null, extraction.body)
}

export function renameFrontmatterKey(markdown: string, fromKey: string, toKey: string): string {
  const from = fromKey.trim()
  const to = toKey.trim()
  if (!from || !to) return markdown
  if (!isValidFrontmatterKey(to)) throw new Error('Invalid property key')

  const extraction = extractFrontmatter(markdown)
  if (!extraction.raw) return markdown

  const blocks = splitYamlIntoBlocks(extraction.raw)
  const updated: YamlBlock[] = []

  let renamed = false
  let targetExists = false
  for (const block of blocks) {
    if (block.kind === 'entry' && block.key === to) {
      targetExists = true
      break
    }
  }
  if (targetExists) throw new Error('Property already exists')

  for (const block of blocks) {
    if (block.kind !== 'entry') {
      updated.push(block)
      continue
    }
    if (block.key !== from || renamed) {
      updated.push(block)
      continue
    }

    const first = block.lines[0] ?? ''
    const replaced = first.replace(/^([A-Za-z0-9_.-]+)\s*:/, `${to}:`)
    const lines = [replaced, ...block.lines.slice(1)]
    updated.push({ kind: 'entry', key: to, lines })
    renamed = true
  }

  const nextYaml = joinYamlBlocks(updated)
  return buildMarkdownWithFrontmatter(nextYaml ? nextYaml : null, extraction.body)
}

export function extractFrontmatter(markdown: string): FrontmatterExtraction {
  const normalized = markdown.replace(/\r\n/g, '\n')
  const lines = normalized.split('\n')
  if (lines.length === 0) return { data: {}, body: markdown }

  if (!isDelimiterLine(lines[0] ?? '')) {
    return { data: {}, body: markdown }
  }

  let end = -1
  for (let i = 1; i < lines.length; i++) {
    if (isDelimiterLine(lines[i] ?? '')) {
      end = i
      break
    }
  }

  if (end === -1) return { data: {}, body: markdown }

  const raw = lines.slice(1, end).join('\n')
  const body = lines.slice(end + 1).join('\n')
  const data = parseYamlFrontmatter(raw)
  return { data, body, raw }
}
