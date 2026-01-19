export type NavigationLocation = { type: 'note'; path: string } | { type: 'graph' }

function isSameLocation(a: NavigationLocation, b: NavigationLocation): boolean {
  if (a.type !== b.type) return false
  if (a.type === 'note') return a.path === (b as { type: 'note'; path: string }).path
  return true
}

export class NavigationHistory {
  private entries: NavigationLocation[] = []
  private index = -1
  private version = 0
  private readonly listeners = new Set<() => void>()

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  getVersion(): number {
    return this.version
  }

  private emitChange(): void {
    this.version += 1
    for (const listener of this.listeners) {
      listener()
    }
  }

  reset(): void {
    this.entries = []
    this.index = -1
    this.emitChange()
  }

  current(): NavigationLocation | null {
    return this.index >= 0 ? this.entries[this.index] ?? null : null
  }

  canGoBack(): boolean {
    return this.index > 0
  }

  canGoForward(): boolean {
    return this.index >= 0 && this.index < this.entries.length - 1
  }

  record(location: NavigationLocation): void {
    const cur = this.current()
    if (cur && isSameLocation(cur, location)) return

    const next = this.entries.slice(0, this.index + 1)
    next.push(location)
    this.entries = next
    this.index = next.length - 1
    this.emitChange()
  }

  back(): NavigationLocation | null {
    if (!this.canGoBack()) return null
    this.index -= 1
    this.emitChange()
    return this.entries[this.index] ?? null
  }

  forward(): NavigationLocation | null {
    if (!this.canGoForward()) return null
    this.index += 1
    this.emitChange()
    return this.entries[this.index] ?? null
  }
}
