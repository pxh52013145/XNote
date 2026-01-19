export function isExternalUrl(href: string): boolean {
  return /^(https?:|data:|blob:)/i.test(href)
}

function safeDecodeURIComponent(value: string): string {
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

function normalizePosixPath(p: string): string | null {
  const rawParts = p.split('/').filter(Boolean)
  if (rawParts.length === 0) return null

  const out: string[] = []
  for (const part of rawParts) {
    const decoded = safeDecodeURIComponent(part)
    if (!decoded || decoded.includes('/') || decoded.includes('\\') || decoded.includes('\0')) return null

    if (decoded === '.') continue
    if (decoded === '..') {
      if (out.length === 0) return null
      out.pop()
      continue
    }

    out.push(decoded)
  }

  return out.length > 0 ? out.join('/') : null
}

export function resolveVaultRelativePath(rawHref: string, notePath: string | null): string | null {
  const cleaned = rawHref.trim().replace(/\\/g, '/')
  if (!cleaned || cleaned.startsWith('#')) return null

  const withoutBrackets =
    cleaned.startsWith('<') && cleaned.endsWith('>') ? cleaned.slice(1, -1).trim() : cleaned

  const beforeFragment = withoutBrackets.split('#', 2)[0] ?? ''
  const beforeQuery = beforeFragment.split('?', 2)[0] ?? ''
  const raw = beforeQuery.trim()
  if (!raw) return null

  if (raw.startsWith('/')) {
    return normalizePosixPath(raw.slice(1))
  }

  const baseFolder = notePath ? notePath.split('/').slice(0, -1).join('/') : ''
  const joined = baseFolder ? `${baseFolder}/${raw}` : raw
  return normalizePosixPath(joined)
}

