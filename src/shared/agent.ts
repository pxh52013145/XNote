export interface AgentToolDefinition {
  name: string
  description: string
  /**
   * JSON Schema (draft-07 style) describing tool input.
   * Kept as `unknown` to avoid binding to a specific validator dependency in MVP.
   */
  inputSchema: unknown
}

export type AgentToolResult =
  | {
      ok: true
      result: unknown
    }
  | {
      ok: false
      error: string
    }

