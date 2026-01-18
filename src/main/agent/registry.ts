import type { AgentToolDefinition, AgentToolResult } from '@shared/agent'

export type AgentToolHandler = (args: unknown) => Promise<unknown>

type RegisteredTool = {
  definition: AgentToolDefinition
  handler: AgentToolHandler
}

export class AgentToolRegistry {
  private readonly tools = new Map<string, RegisteredTool>()

  register(definition: AgentToolDefinition, handler: AgentToolHandler): void {
    if (this.tools.has(definition.name)) {
      throw new Error(`Duplicate tool: ${definition.name}`)
    }
    this.tools.set(definition.name, { definition, handler })
  }

  listTools(): AgentToolDefinition[] {
    return [...this.tools.values()].map((t) => t.definition).sort((a, b) => a.name.localeCompare(b.name))
  }

  async runTool(name: string, args: unknown): Promise<AgentToolResult> {
    const tool = this.tools.get(name)
    if (!tool) {
      return { ok: false, error: `Unknown tool: ${name}` }
    }

    try {
      const result = await tool.handler(args)
      return { ok: true, result }
    } catch (e: unknown) {
      return { ok: false, error: e instanceof Error ? e.message : String(e) }
    }
  }
}

