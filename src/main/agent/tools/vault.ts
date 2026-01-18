import { AgentToolRegistry } from '../registry'
import {
  createNoteFile,
  deleteNoteFile,
  listMarkdownFiles,
  readNoteFile,
  renameNoteFile,
  writeNoteFile
} from '../../vault'
import { listBacklinks } from '../../backlinks'

function requireObject(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('Invalid tool args')
  }
  return value as Record<string, unknown>
}

function requireString(value: unknown, fieldName: string): string {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new Error(`Invalid "${fieldName}"`)
  }
  return value
}

export function registerVaultTools(registry: AgentToolRegistry, getVaultPath: () => string | null): void {
  registry.register(
    {
      name: 'vault.list_notes',
      description: 'List all Markdown notes in the current vault.',
      inputSchema: { type: 'object', additionalProperties: false, properties: {} }
    },
    async () => {
      const vaultPath = getVaultPath()
      if (!vaultPath) throw new Error('No vault selected')
      return await listMarkdownFiles(vaultPath)
    }
  )

  registry.register(
    {
      name: 'vault.read_note',
      description: 'Read a Markdown note by vault-relative path.',
      inputSchema: {
        type: 'object',
        additionalProperties: false,
        required: ['path'],
        properties: { path: { type: 'string' } }
      }
    },
    async (args) => {
      const vaultPath = getVaultPath()
      if (!vaultPath) throw new Error('No vault selected')
      const obj = requireObject(args)
      const notePath = requireString(obj.path, 'path')
      const content = await readNoteFile(vaultPath, notePath)
      return { path: notePath, content }
    }
  )

  registry.register(
    {
      name: 'vault.create_note',
      description: 'Create a new Markdown note by vault-relative path.',
      inputSchema: {
        type: 'object',
        additionalProperties: false,
        required: ['path'],
        properties: { path: { type: 'string' }, content: { type: 'string' } }
      }
    },
    async (args) => {
      const vaultPath = getVaultPath()
      if (!vaultPath) throw new Error('No vault selected')
      const obj = requireObject(args)
      const notePath = requireString(obj.path, 'path')
      const content = typeof obj.content === 'string' ? obj.content : ''
      const createdPath = await createNoteFile(vaultPath, notePath, content)
      return { path: createdPath }
    }
  )

  registry.register(
    {
      name: 'vault.write_note',
      description: 'Overwrite a Markdown note by vault-relative path.',
      inputSchema: {
        type: 'object',
        additionalProperties: false,
        required: ['path', 'content'],
        properties: { path: { type: 'string' }, content: { type: 'string' } }
      }
    },
    async (args) => {
      const vaultPath = getVaultPath()
      if (!vaultPath) throw new Error('No vault selected')
      const obj = requireObject(args)
      const notePath = requireString(obj.path, 'path')
      const content = requireString(obj.content, 'content')
      await writeNoteFile(vaultPath, notePath, content)
      return { path: notePath }
    }
  )

  registry.register(
    {
      name: 'vault.rename_note',
      description: 'Rename (move) a Markdown note by vault-relative path.',
      inputSchema: {
        type: 'object',
        additionalProperties: false,
        required: ['from', 'to'],
        properties: { from: { type: 'string' }, to: { type: 'string' } }
      }
    },
    async (args) => {
      const vaultPath = getVaultPath()
      if (!vaultPath) throw new Error('No vault selected')
      const obj = requireObject(args)
      const fromPath = requireString(obj.from, 'from')
      const toPath = requireString(obj.to, 'to')
      const renamedPath = await renameNoteFile(vaultPath, fromPath, toPath)
      return { from: fromPath, to: renamedPath }
    }
  )

  registry.register(
    {
      name: 'vault.delete_note',
      description: 'Delete a Markdown note by vault-relative path.',
      inputSchema: {
        type: 'object',
        additionalProperties: false,
        required: ['path'],
        properties: { path: { type: 'string' } }
      }
    },
    async (args) => {
      const vaultPath = getVaultPath()
      if (!vaultPath) throw new Error('No vault selected')
      const obj = requireObject(args)
      const notePath = requireString(obj.path, 'path')
      await deleteNoteFile(vaultPath, notePath)
      return { path: notePath }
    }
  )

  registry.register(
    {
      name: 'vault.list_backlinks',
      description: 'List notes that link to a target note (wikilinks).',
      inputSchema: {
        type: 'object',
        additionalProperties: false,
        required: ['path'],
        properties: { path: { type: 'string' } }
      }
    },
    async (args) => {
      const vaultPath = getVaultPath()
      if (!vaultPath) throw new Error('No vault selected')
      const obj = requireObject(args)
      const notePath = requireString(obj.path, 'path')
      return await listBacklinks(vaultPath, notePath)
    }
  )
}
