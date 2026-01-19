import { useEffect, useMemo, useState } from 'react'
import { xnote } from '../api'
import { isExternalUrl, resolveVaultRelativePath } from '../markdown/vaultPaths'

export function VaultImage(props: { src?: string; alt?: string; title?: string; notePath: string | null }) {
  const [objectUrl, setObjectUrl] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const src = props.src?.trim() ?? ''

  const resolvedPath = useMemo(() => {
    if (!src) return null
    if (isExternalUrl(src)) return null
    return resolveVaultRelativePath(src, props.notePath)
  }, [props.notePath, src])

  useEffect(() => {
    setError(null)
    if (!src) return
    if (isExternalUrl(src)) {
      setObjectUrl(null)
      return
    }
    if (!resolvedPath) {
      setObjectUrl(null)
      setError('Invalid image path')
      return
    }

    let cancelled = false
    let currentUrl: string | null = null

    void xnote
      .readVaultFile(resolvedPath)
      .then(({ data, mime }) => {
        if (cancelled) return
        const blob = new Blob([data], { type: mime ?? 'application/octet-stream' })
        currentUrl = URL.createObjectURL(blob)
        setObjectUrl(currentUrl)
      })
      .catch((e: unknown) => {
        if (cancelled) return
        setObjectUrl(null)
        setError(e instanceof Error ? e.message : String(e))
      })

    return () => {
      cancelled = true
      if (currentUrl) URL.revokeObjectURL(currentUrl)
    }
  }, [resolvedPath, src])

  if (!src) return null

  if (isExternalUrl(src)) {
    return <img src={src} alt={props.alt ?? ''} title={props.title} loading="lazy" />
  }

  if (!objectUrl) {
    return <span className="muted">{error ? `Image error: ${error}` : 'Loading image...'}</span>
  }

  return <img src={objectUrl} alt={props.alt ?? ''} title={props.title} loading="lazy" />
}
