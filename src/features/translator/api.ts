import { Channel, invoke } from '@tauri-apps/api/core'
import type { TranslateResult, TranslateStreamEvent } from './types'

export function translatorTranslate(
  text: string,
  onStreamEvent: (event: TranslateStreamEvent) => void,
) {
  const onEvent = new Channel<TranslateStreamEvent>()
  onEvent.onmessage = onStreamEvent
  return invoke<TranslateResult>('translator_translate', { text, onEvent })
}
