export type CommandId = string

export interface CommandDefinition {
  id: CommandId
  title: string
  description?: string
  group?: string
  keywords?: string
  defaultHotkey?: string
}

export type CommandHandler = () => void | Promise<void>

type RegisteredCommand = {
  definition: CommandDefinition
  handler: CommandHandler
}

export class CommandRegistry {
  private readonly commands = new Map<CommandId, RegisteredCommand>()
  private readonly listeners = new Set<() => void>()
  private snapshot: readonly CommandDefinition[] = Object.freeze([] as CommandDefinition[])

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  private emitChange(): void {
    this.snapshot = Object.freeze([...this.commands.values()].map((c) => c.definition)) as readonly CommandDefinition[]
    for (const listener of this.listeners) {
      listener()
    }
  }

  register(definition: CommandDefinition, handler: CommandHandler): void {
    if (this.commands.has(definition.id)) {
      throw new Error(`Duplicate command: ${definition.id}`)
    }
    this.commands.set(definition.id, { definition, handler })
    this.emitChange()
  }

  upsert(definition: CommandDefinition, handler: CommandHandler): void {
    this.commands.set(definition.id, { definition, handler })
    this.emitChange()
  }

  unregister(id: CommandId): void {
    const deleted = this.commands.delete(id)
    if (deleted) this.emitChange()
  }

  list(): readonly CommandDefinition[] {
    return this.snapshot
  }

  get(id: CommandId): CommandDefinition | undefined {
    return this.commands.get(id)?.definition
  }

  async run(id: CommandId): Promise<void> {
    const cmd = this.commands.get(id)
    if (!cmd) throw new Error(`Unknown command: ${id}`)
    await cmd.handler()
  }
}
