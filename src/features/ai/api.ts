import { invoke } from '@tauri-apps/api/core'
import type { AiRequestMessage } from './types'

export function startAiChatStream(messages: AiRequestMessage[]) {
  return invoke<void>('ai_chat_stream', { messages })
}
