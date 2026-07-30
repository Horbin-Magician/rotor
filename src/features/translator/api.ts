import { invoke } from '@tauri-apps/api/core'
import type { TranslateResult } from './types'

export function translatorTranslate(text: string) {
  return invoke<TranslateResult>('translator_translate', { text })
}
