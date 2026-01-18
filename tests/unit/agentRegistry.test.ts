import { describe, expect, it } from 'vitest'
import { AgentToolRegistry } from '../../src/main/agent/registry'

describe('AgentToolRegistry', () => {
  it('registers and lists tools', () => {
    const registry = new AgentToolRegistry()
    registry.register({ name: 't', description: 'test', inputSchema: {} }, async () => 123)
    expect(registry.listTools().map((t) => t.name)).toEqual(['t'])
  })

  it('rejects duplicate tool names', () => {
    const registry = new AgentToolRegistry()
    registry.register({ name: 't', description: 'test', inputSchema: {} }, async () => 1)
    expect(() => registry.register({ name: 't', description: 'test2', inputSchema: {} }, async () => 2)).toThrow(
      /Duplicate tool/
    )
  })

  it('returns error for unknown tool', async () => {
    const registry = new AgentToolRegistry()
    const res = await registry.runTool('missing', {})
    expect(res.ok).toBe(false)
  })

  it('catches handler errors', async () => {
    const registry = new AgentToolRegistry()
    registry.register({ name: 'boom', description: 'boom', inputSchema: {} }, async () => {
      throw new Error('nope')
    })
    const res = await registry.runTool('boom', {})
    expect(res).toEqual({ ok: false, error: 'nope' })
  })
})

