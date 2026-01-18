import { useState } from 'react'
import { Brush, FileCog, Keyboard, SlidersHorizontal, User } from 'lucide-react'
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
}) {
  const [section, setSection] = useState<SettingsSection>('appearance')

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
            </div>
          ) : null}

          {section === 'hotkeys' ? (
            <div className="settings-page">
              <div className="settings-title">Hotkeys</div>
              <div className="hotkeys">
                <div className="hotkey-row">
                  <div className="hotkey-action">Command palette</div>
                  <div className="hotkey-keys">Ctrl+P</div>
                </div>
                <div className="hotkey-row">
                  <div className="hotkey-action">Quick switcher</div>
                  <div className="hotkey-keys">Ctrl+O</div>
                </div>
                <div className="hotkey-row">
                  <div className="hotkey-action">Settings</div>
                  <div className="hotkey-keys">Ctrl+,</div>
                </div>
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

