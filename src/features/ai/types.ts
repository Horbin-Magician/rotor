export interface ChatMessage {
  role: 'user' | 'assistant'
  content: string
}

export interface AiRequestMessage {
  role: 'system' | 'user' | 'assistant'
  content: string
}

export interface StreamEventContent {
  type: 'Content'
  content: string
}

export interface StreamEventDone {
  type: 'Done'
}

export interface StreamEventError {
  type: 'Error'
  message: string
}

export type StreamEvent = StreamEventContent | StreamEventDone | StreamEventError
