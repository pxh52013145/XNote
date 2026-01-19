import { describe, expect, it, vi } from 'vitest'
import { CommandRegistry } from '@shared/commands'

describe('CommandRegistry', () => {
  it('registers and lists commands', () => {
    const registry = new CommandRegistry()
    registry.register({ id: 'a', title: 'A' }, () => {})
    registry.register({ id: 'b', title: 'B' }, () => {})
    expect(registry.list().map((c) => c.id)).toEqual(['a', 'b'])
  })

  it('rejects duplicate ids', () => {
    const registry = new CommandRegistry()
    registry.register({ id: 'a', title: 'A' }, () => {})
    expect(() => registry.register({ id: 'a', title: 'A2' }, () => {})).toThrow(/Duplicate command/)
  })

  it('upserts definitions and handlers', async () => {
    const registry = new CommandRegistry()
    const first = vi.fn()
    const second = vi.fn()

    registry.upsert({ id: 'a', title: 'A' }, first)
    await registry.run('a')
    expect(first).toHaveBeenCalledTimes(1)

    registry.upsert({ id: 'a', title: 'A2' }, second)
    await registry.run('a')
    expect(second).toHaveBeenCalledTimes(1)
    expect(registry.get('a')?.title).toBe('A2')
  })

  it('notifies subscribers on changes', () => {
    const registry = new CommandRegistry()
    const listener = vi.fn()
    const unsubscribe = registry.subscribe(listener)

    registry.register({ id: 'a', title: 'A' }, () => {})
    expect(listener).toHaveBeenCalledTimes(1)

    registry.unregister('a')
    expect(listener).toHaveBeenCalledTimes(2)

    unsubscribe()
    registry.register({ id: 'b', title: 'B' }, () => {})
    expect(listener).toHaveBeenCalledTimes(2)
  })

  it('throws on unknown command', async () => {
    const registry = new CommandRegistry()
    await expect(registry.run('missing')).rejects.toThrow(/Unknown command/)
  })
})

