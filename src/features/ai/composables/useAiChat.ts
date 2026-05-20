import { nextTick, ref, type Ref } from 'vue'
import { startAiChatStream } from '../api'
import type { ChatMessage, StreamEvent } from '../types'

interface AiChatCallbacks {
  resetSearchState: () => void
  resizeWindow: () => void | Promise<void>
  scrollToBottom: () => void
}

export function useAiChat(searchQuery: Ref<string>, callbacks: AiChatCallbacks) {
  const isAiMode = ref(false)
  const chatMessages = ref<ChatMessage[]>([])
  const isLoading = ref(false)

  const resetAiChat = () => {
    chatMessages.value = []
    isAiMode.value = false
    isLoading.value = false
  }

  const toggleMode = () => {
    isAiMode.value = !isAiMode.value
    searchQuery.value = ''
    callbacks.resetSearchState()

    if (!isAiMode.value) {
      chatMessages.value = []
      isLoading.value = false
    }

    nextTick(callbacks.resizeWindow)
  }

  const sendAiMessage = async () => {
    const message = searchQuery.value.trim()
    if (!message || isLoading.value) return

    chatMessages.value.push({
      role: 'user',
      content: message,
    })

    searchQuery.value = ''
    isLoading.value = true

    await nextTick()
    await callbacks.resizeWindow()
    callbacks.scrollToBottom()

    try {
      const messages = chatMessages.value.map(msg => ({
        role: msg.role,
        content: msg.content,
      }))

      await startAiChatStream(messages)

      chatMessages.value.push({
        role: 'assistant',
        content: '',
      })

      await nextTick()
      await callbacks.resizeWindow()
      callbacks.scrollToBottom()
    } catch (error) {
      console.error('AI chat error:', error)
      chatMessages.value.push({
        role: 'assistant',
        content: `Error: ${error}`,
      })
      isLoading.value = false
      await nextTick()
      await callbacks.resizeWindow()
      callbacks.scrollToBottom()
    }
  }

  const handleStreamEvent = (event: StreamEvent) => {
    const lastMessage = chatMessages.value[chatMessages.value.length - 1]

    switch (event.type) {
      case 'Content':
        if (lastMessage && lastMessage.role === 'assistant') {
          lastMessage.content += event.content
          callbacks.scrollToBottom()
        }
        break
      case 'Done':
        isLoading.value = false
        callbacks.scrollToBottom()
        break
      case 'Error':
        if (lastMessage && lastMessage.role === 'assistant') {
          if (lastMessage.content === '') {
            lastMessage.content = `Error: ${event.message}`
          } else {
            lastMessage.content += `\n\nError: ${event.message}`
          }
        } else {
          chatMessages.value.push({
            role: 'assistant',
            content: `Error: ${event.message}`,
          })
        }
        isLoading.value = false
        callbacks.scrollToBottom()
        break
    }
  }

  return {
    isAiMode,
    chatMessages,
    isLoading,
    resetAiChat,
    toggleMode,
    sendAiMessage,
    handleStreamEvent,
  }
}
