export type HotkeyString = string

export type HotkeyEventLike = {
  key: string
  ctrlKey?: boolean
  metaKey?: boolean
  altKey?: boolean
  shiftKey?: boolean
  repeat?: boolean
  isComposing?: boolean
}

const modifierAliases: Record<string, 'Cmd' | 'Ctrl' | 'Alt' | 'Shift'> = {
  cmd: 'Cmd',
  command: 'Cmd',
  meta: 'Cmd',
  ctrl: 'Ctrl',
  control: 'Ctrl',
  alt: 'Alt',
  option: 'Alt',
  shift: 'Shift'
}

const modifierOrder: Array<'Cmd' | 'Ctrl' | 'Alt' | 'Shift'> = ['Cmd', 'Ctrl', 'Alt', 'Shift']

function normalizeModifier(part: string): 'Cmd' | 'Ctrl' | 'Alt' | 'Shift' | null {
  const key = part.trim().toLowerCase()
  return modifierAliases[key] ?? null
}

function normalizeKey(key: string): string | null {
  const trimmed = key.trim()
  if (!trimmed) return null

  const lower = trimmed.toLowerCase()
  if (lower === 'esc' || lower === 'escape') return 'Esc'
  if (lower === 'space' || trimmed === ' ') return 'Space'
  if (lower === 'enter' || lower === 'return') return 'Enter'
  if (lower === 'tab') return 'Tab'
  if (lower === 'backspace') return 'Backspace'
  if (lower === 'delete' || lower === 'del') return 'Delete'

  if (lower === 'arrowleft' || lower === 'left') return 'Left'
  if (lower === 'arrowright' || lower === 'right') return 'Right'
  if (lower === 'arrowup' || lower === 'up') return 'Up'
  if (lower === 'arrowdown' || lower === 'down') return 'Down'

  if (/^f([1-9]|1[0-2])$/i.test(trimmed)) return trimmed.toUpperCase()

  if (trimmed.length === 1) {
    const ch = trimmed
    if (/[a-z]/i.test(ch)) return ch.toUpperCase()
    return ch
  }

  return trimmed
}

function isModifierKey(key: string): boolean {
  const lower = key.toLowerCase()
  return lower === 'shift' || lower === 'control' || lower === 'ctrl' || lower === 'alt' || lower === 'meta' || lower === 'command'
}

export function normalizeHotkey(hotkey: string): HotkeyString | null {
  const raw = hotkey.trim()
  if (!raw) return null

  const parts = raw.split('+').map((p) => p.trim()).filter(Boolean)
  if (parts.length === 0) return null

  const modifiers = new Set<'Cmd' | 'Ctrl' | 'Alt' | 'Shift'>()
  const keys: string[] = []

  for (const part of parts) {
    const mod = normalizeModifier(part)
    if (mod) {
      modifiers.add(mod)
      continue
    }
    keys.push(part)
  }

  if (keys.length !== 1) return null
  const normalizedKey = normalizeKey(keys[0] ?? '')
  if (!normalizedKey) return null

  const ordered = modifierOrder.filter((m) => modifiers.has(m))
  const out = [...ordered, normalizedKey].join('+')
  return out
}

export function hotkeyFromEvent(event: HotkeyEventLike): HotkeyString | null {
  if (event.isComposing) return null

  const key = event.key
  if (!key || key === 'Unidentified' || key === 'Dead') return null
  if (isModifierKey(key)) return null

  const normalizedKey = normalizeKey(key)
  if (!normalizedKey) return null

  const modifiers: Array<'Cmd' | 'Ctrl' | 'Alt' | 'Shift'> = []
  if (event.metaKey) modifiers.push('Cmd')
  if (event.ctrlKey) modifiers.push('Ctrl')
  if (event.altKey) modifiers.push('Alt')
  if (event.shiftKey) modifiers.push('Shift')

  const out = [...modifiers, normalizedKey].join('+')
  return out
}

