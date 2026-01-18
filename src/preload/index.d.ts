import type { XNoteAPI } from '@shared/api'

declare global {
  interface Window {
    xnote: XNoteAPI
  }
}

export {}

