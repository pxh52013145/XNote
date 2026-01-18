/**
 * Best-effort basename for vault-relative paths.
 * Accepts both POSIX ("a/b.md") and Windows ("a\\b.md") separators.
 */
export function fileBaseName(p: string): string {
  const normalized = p.replace(/\\/g, '/')
  const parts = normalized.split('/').filter(Boolean)
  return parts[parts.length - 1] ?? p
}

export function stripMarkdownExtension(name: string): string {
  return name.toLowerCase().endsWith('.md') ? name.slice(0, -3) : name
}

export function noteTitleFromPath(p: string): string {
  return stripMarkdownExtension(fileBaseName(p))
}
