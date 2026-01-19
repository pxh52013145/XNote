import { describe, expect, it } from 'vitest'
import { NavigationHistory, type NavigationLocation } from '@shared/navigationHistory'

function locs(h: NavigationHistory): NavigationLocation[] {
  const result: NavigationLocation[] = []
  const cur = h.current()
  if (!cur) return result

  while (h.canGoBack()) h.back()
  let next: NavigationLocation | null = h.current()
  while (next) {
    result.push(next)
    next = h.forward()
  }

  return result
}

describe('NavigationHistory', () => {
  it('records locations and moves back/forward', () => {
    const h = new NavigationHistory()
    h.record({ type: 'note', path: 'a.md' })
    h.record({ type: 'note', path: 'b.md' })
    h.record({ type: 'graph' })

    expect(h.current()).toEqual({ type: 'graph' })
    expect(h.canGoBack()).toBe(true)
    expect(h.canGoForward()).toBe(false)

    expect(h.back()).toEqual({ type: 'note', path: 'b.md' })
    expect(h.back()).toEqual({ type: 'note', path: 'a.md' })
    expect(h.back()).toBeNull()
    expect(h.forward()).toEqual({ type: 'note', path: 'b.md' })
  })

  it('deduplicates consecutive records', () => {
    const h = new NavigationHistory()
    h.record({ type: 'note', path: 'a.md' })
    h.record({ type: 'note', path: 'a.md' })
    h.record({ type: 'graph' })
    h.record({ type: 'graph' })
    expect(locs(h)).toEqual([{ type: 'note', path: 'a.md' }, { type: 'graph' }])
  })

  it('clears forward history when recording after back', () => {
    const h = new NavigationHistory()
    h.record({ type: 'note', path: 'a.md' })
    h.record({ type: 'note', path: 'b.md' })
    h.record({ type: 'note', path: 'c.md' })

    expect(h.back()).toEqual({ type: 'note', path: 'b.md' })
    h.record({ type: 'note', path: 'd.md' })

    expect(locs(h)).toEqual([
      { type: 'note', path: 'a.md' },
      { type: 'note', path: 'b.md' },
      { type: 'note', path: 'd.md' }
    ])
    expect(h.canGoForward()).toBe(false)
  })

  it('resets', () => {
    const h = new NavigationHistory()
    h.record({ type: 'note', path: 'a.md' })
    h.reset()
    expect(h.current()).toBeNull()
    expect(h.canGoBack()).toBe(false)
    expect(h.canGoForward()).toBe(false)
  })
})

