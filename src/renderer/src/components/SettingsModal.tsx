import { useEffect, useMemo, useState } from 'react'
import { Brush, FileCog, Keyboard, SlidersHorizontal, User } from 'lucide-react'
import type { CommandDefinition } from '@shared/commands'
import { hotkeyFromEvent, normalizeHotkey } from '@shared/hotkeys'
import type { VaultSettings } from '@shared/types'
import { Modal } from './Modal'

type SettingsSection = 'about' | 'appearance' | 'editor' | 'files' | 'hotkeys' | 'advanced'

function Toggle(props: { label: string; description?: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="toggle-row">
      <div className="toggle-text">
        <div className="toggle-label">{props.label}</div>
        {props.description ? <div className="toggle-desc">{props.description}</div> : null}
      </div>
      <input
        className="toggle-input"
        type="checkbox"
        checked={props.checked}
        onChange={(e) => props.onChange(e.target.checked)}
      />
    </label>
  )
}

export function SettingsModal(props: {
  open: boolean
  onClose: () => void
  theme: 'dark' | 'light'
  onChangeTheme: (t: 'dark' | 'light') => void
  vaultPath: string | null
  vaultSettings: VaultSettings | null
  onSaveVaultSettings: (next: VaultSettings) => void
  commands: readonly CommandDefinition[]
  hotkeys: Record<string, string>
  onSetHotkey: (commandId: string, hotkey: string) => void
  onClearHotkey: (commandId: string) => void
}) {
  const [section, setSection] = useState<SettingsSection>('appearance')
  const [hotkeyFilter, setHotkeyFilter] = useState('')
  const [editingCommandId, setEditingCommandId] = useState<string | null>(null)
  const [newNotesFolder, setNewNotesFolder] = useState('')
  const [attachmentsFolder, setAttachmentsFolder] = useState('')

  useEffect(() => {
    setNewNotesFolder(props.vaultSettings?.newNotesFolder ?? '')
    setAttachmentsFolder(props.vaultSettings?.attachmentsFolder ?? '')
  }, [props.vaultSettings, props.vaultPath])

  const commitVaultSettings = () => {
    if (!props.vaultPath) return

    const next: VaultSettings = {
      newNotesFolder: newNotesFolder.trim() || undefined,
      attachmentsFolder: attachmentsFolder.trim() || undefined
    }

    const currentNew = props.vaultSettings?.newNotesFolder ?? ''
    const currentAttachments = props.vaultSettings?.attachmentsFolder ?? ''

    if ((next.newNotesFolder ?? '') === currentNew && (next.attachmentsFolder ?? '') === currentAttachments) {
      return
    }

    props.onSaveVaultSettings(next)
  }

  useEffect(() => {
    if (!props.open) setEditingCommandId(null)
  }, [props.open])

  useEffect(() => {
    if (section !== 'hotkeys') setEditingCommandId(null)
  }, [section])

  const eligibleCommands = useMemo(() => {
    return [...props.commands]
      .filter((cmd) => !cmd.id.startsWith('cmd.openRecentVault:'))
      .sort((a, b) => (a.group || '').localeCompare(b.group || '') || a.title.localeCompare(b.title) || a.id.localeCompare(b.id))
  }, [props.commands])

  const visibleCommands = useMemo(() => {
    const filter = hotkeyFilter.trim().toLowerCase()
    if (!filter) return eligibleCommands

    return eligibleCommands.filter((cmd) => {
      const hay = `${cmd.title} ${cmd.description ?? ''} ${cmd.group ?? ''} ${cmd.keywords ?? ''} ${cmd.id}`.toLowerCase()
      return hay.includes(filter)
    })
  }, [eligibleCommands, hotkeyFilter])

  const effectiveHotkeyById = useMemo(() => {
    const byId = new Map<string, string>()
    for (const cmd of eligibleCommands) {
      const custom = props.hotkeys[cmd.id]
      if (custom) {
        byId.set(cmd.id, custom)
        continue
      }
      const normalizedDefault = cmd.defaultHotkey ? normalizeHotkey(cmd.defaultHotkey) : null
      if (normalizedDefault) byId.set(cmd.id, normalizedDefault)
    }
    return byId
  }, [eligibleCommands, props.hotkeys])

  const commandById = useMemo(() => {
    const map = new Map<string, CommandDefinition>()
    for (const cmd of eligibleCommands) {
      map.set(cmd.id, cmd)
    }
    return map
  }, [eligibleCommands])

  const idsByHotkey = useMemo(() => {
    const map = new Map<string, string[]>()
    for (const [id, hk] of effectiveHotkeyById.entries()) {
      const existing = map.get(hk)
      if (existing) existing.push(id)
      else map.set(hk, [id])
    }
    return map
  }, [effectiveHotkeyById])

  return (
    <Modal open={props.open} title="Settings" onClose={props.onClose} width={980} height={640}>
      <div className="settings">
        <nav className="settings-nav">
          <button
            className={section === 'about' ? 'settings-nav-item active' : 'settings-nav-item'}
            onClick={() => setSection('about')}
          >
            <User size={16} />
            About
          </button>
          <button
            className={section === 'appearance' ? 'settings-nav-item active' : 'settings-nav-item'}
            onClick={() => setSection('appearance')}
          >
            <Brush size={16} />
            Appearance
          </button>
          <button
            className={section === 'editor' ? 'settings-nav-item active' : 'settings-nav-item'}
            onClick={() => setSection('editor')}
          >
            <SlidersHorizontal size={16} />
            Editor
          </button>
          <button
            className={section === 'files' ? 'settings-nav-item active' : 'settings-nav-item'}
            onClick={() => setSection('files')}
          >
            <FileCog size={16} />
            Files & Links
          </button>
          <button
            className={section === 'hotkeys' ? 'settings-nav-item active' : 'settings-nav-item'}
            onClick={() => setSection('hotkeys')}
          >
            <Keyboard size={16} />
            Hotkeys
          </button>
          <button
            className={section === 'advanced' ? 'settings-nav-item active' : 'settings-nav-item'}
            onClick={() => setSection('advanced')}
          >
            <SlidersHorizontal size={16} />
            Advanced
          </button>
        </nav>

        <div className="settings-content">
          {section === 'about' ? (
            <div className="settings-page">
              <div className="settings-title">About</div>
              <div className="settings-subtitle muted">XNote (Obsidian-like UI MVP)</div>
              <div className="kv">
                <div className="k">Vault</div>
                <div className="v">{props.vaultPath ?? 'Not selected'}</div>
              </div>
            </div>
          ) : null}

          {section === 'appearance' ? (
            <div className="settings-page">
              <div className="settings-title">Appearance</div>
              <div className="settings-section">
                <div className="settings-section-title">Theme</div>
                <div className="segmented">
                  <button
                    className={props.theme === 'dark' ? 'segmented-btn active' : 'segmented-btn'}
                    onClick={() => props.onChangeTheme('dark')}
                  >
                    Dark
                  </button>
                  <button
                    className={props.theme === 'light' ? 'segmented-btn active' : 'segmented-btn'}
                    onClick={() => props.onChangeTheme('light')}
                  >
                    Light
                  </button>
                </div>
              </div>
            </div>
          ) : null}

          {section === 'editor' ? (
            <div className="settings-page">
              <div className="settings-title">Editor</div>
              <Toggle label="Show line numbers" checked={false} onChange={() => {}} />
              <Toggle label="Spellcheck" checked={false} onChange={() => {}} />
              <Toggle label="Live preview" checked={true} onChange={() => {}} />
            </div>
          ) : null}

          {section === 'files' ? (
            <div className="settings-page">
              <div className="settings-title">Files & Links</div>
              <Toggle label="Use [[Wikilinks]]" checked={true} onChange={() => {}} />
              <Toggle label="Detect all file extensions" checked={false} onChange={() => {}} />
              <Toggle label="New notes in root" checked={false} onChange={() => {}} />

              <div className="settings-section">
                <div className="settings-section-title">Vault</div>
                {!props.vaultPath ? (
                  <div className="muted">Open a vault to configure per-vault settings.</div>
                ) : (
                  <div className="settings-fields">
                    <label className="settings-field">
                      <div className="settings-field-label">New notes folder</div>
                      <div className="settings-field-desc muted">Vault-relative folder used as the default for new notes.</div>
                      <input
                        className="panel-filter"
                        placeholder="(root)"
                        value={newNotesFolder}
                        onChange={(e) => setNewNotesFolder(e.target.value)}
                        onBlur={commitVaultSettings}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') {
                            e.preventDefault()
                            ;(e.currentTarget as HTMLInputElement).blur()
                          }
                          if (e.key === 'Escape') {
                            e.preventDefault()
                            setNewNotesFolder(props.vaultSettings?.newNotesFolder ?? '')
                            ;(e.currentTarget as HTMLInputElement).blur()
                          }
                        }}
                      />
                    </label>

                    <label className="settings-field">
                      <div className="settings-field-label">Attachments folder</div>
                      <div className="settings-field-desc muted">Vault-relative folder for pasted images and other attachments.</div>
                      <input
                        className="panel-filter"
                        placeholder="attachments"
                        value={attachmentsFolder}
                        onChange={(e) => setAttachmentsFolder(e.target.value)}
                        onBlur={commitVaultSettings}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') {
                            e.preventDefault()
                            ;(e.currentTarget as HTMLInputElement).blur()
                          }
                          if (e.key === 'Escape') {
                            e.preventDefault()
                            setAttachmentsFolder(props.vaultSettings?.attachmentsFolder ?? '')
                            ;(e.currentTarget as HTMLInputElement).blur()
                          }
                        }}
                      />
                    </label>
                  </div>
                )}
              </div>
            </div>
          ) : null}

          {section === 'hotkeys' ? (
            <div className="settings-page">
              <div className="settings-title">Hotkeys</div>
              <div className="hotkeys">
                <div className="hotkey-toolbar">
                  <input
                    className="panel-filter"
                    placeholder="Filter commands..."
                    value={hotkeyFilter}
                    onChange={(e) => setHotkeyFilter(e.target.value)}
                  />
                </div>

                {visibleCommands.map((cmd) => {
                  const custom = props.hotkeys[cmd.id] ?? null
                  const defaultHotkey = cmd.defaultHotkey ? normalizeHotkey(cmd.defaultHotkey) : null
                  const effectiveHotkey = effectiveHotkeyById.get(cmd.id) ?? null

                  const conflictIds = effectiveHotkey ? (idsByHotkey.get(effectiveHotkey) ?? []) : []
                  const conflictOthers = conflictIds.filter((id) => id !== cmd.id)
                  const conflictLabel =
                    conflictOthers.length > 0
                      ? `Conflicts with: ${conflictOthers
                          .slice(0, 2)
                          .map((id) => commandById.get(id)?.title ?? id)
                          .join(', ')}${conflictOthers.length > 2 ? '…' : ''}`
                      : null

                  const isEditing = editingCommandId === cmd.id

                  return (
                    <div key={cmd.id} className="hotkey-row">
                      <div className="hotkey-action">
                        <div className="hotkey-action-title">{cmd.title}</div>
                        <div className="hotkey-action-meta muted">
                          {(cmd.group ?? 'Other').trim() || 'Other'} · {cmd.id}
                        </div>
                        {defaultHotkey && custom ? (
                          <div className="hotkey-action-meta muted">Default: {defaultHotkey}</div>
                        ) : null}
                        {conflictLabel ? <div className="hotkey-conflict">{conflictLabel}</div> : null}
                      </div>

                      <div className="hotkey-controls">
                        {isEditing ? (
                          <input
                            className="hotkey-capture"
                            value={effectiveHotkey ?? ''}
                            placeholder="Press keys..."
                            autoFocus
                            readOnly
                            onBlur={() => setEditingCommandId(null)}
                            onKeyDown={(e) => {
                              e.preventDefault()
                              e.stopPropagation()

                              if (e.key === 'Escape') {
                                setEditingCommandId(null)
                                return
                              }

                              const next = hotkeyFromEvent(e)
                              if (!next) return

                              if (next === 'Backspace' || next === 'Delete') {
                                props.onClearHotkey(cmd.id)
                                setEditingCommandId(null)
                                return
                              }

                              const existing = idsByHotkey.get(next) ?? []
                              const others = existing.filter((id) => id !== cmd.id)
                              if (others.length > 0) {
                                const label = others.map((id) => commandById.get(id)?.title ?? id).join(', ')
                                const ok = window.confirm(`Hotkey "${next}" is already used by: ${label}\n\nAssign anyway?`)
                                if (!ok) return
                              }

                              props.onSetHotkey(cmd.id, next)
                              setEditingCommandId(null)
                            }}
                          />
                        ) : (
                          <button className="hotkey-btn" onClick={() => setEditingCommandId(cmd.id)}>
                            {effectiveHotkey ?? 'Unassigned'}
                          </button>
                        )}
                        {custom ? (
                          <button
                            className="hotkey-reset"
                            onClick={() => {
                              props.onClearHotkey(cmd.id)
                              if (editingCommandId === cmd.id) setEditingCommandId(null)
                            }}
                            title="Reset to default"
                          >
                            Reset
                          </button>
                        ) : (
                          <button className="hotkey-reset" disabled title="Using default">
                            Reset
                          </button>
                        )}
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          ) : null}

          {section === 'advanced' ? (
            <div className="settings-page">
              <div className="settings-title">Advanced</div>
              <div className="muted">Placeholder for plugin system, AI endpoint config, vault indexing options, etc.</div>
            </div>
          ) : null}
        </div>
      </div>
    </Modal>
  )
}
