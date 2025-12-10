<template>
  <div class="ai-chat-container" ref="chatContainerRef">
    <div class="ai-messages">
      <div
        v-for="(msg, index) in messages"
        :key="index"
        class="ai-message"
        :class="msg.role"
      >
        <div class="message-content">
          {{ msg.content }}
          <span
            v-if="isLoading && index === messages.length - 1 && msg.role === 'assistant'"
            class="cursor-blink"
          >|</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick } from 'vue'

// Types
export interface ChatMessage {
  role: 'user' | 'assistant'
  content: string
}

// Props
interface Props {
  messages: ChatMessage[]
  isLoading?: boolean
}

withDefaults(defineProps<Props>(), {
  isLoading: false
})

// Refs
const chatContainerRef = ref<HTMLDivElement>()

// Methods
const scrollToBottom = () => {
  nextTick(() => {
    if (chatContainerRef.value) {
      chatContainerRef.value.scrollTop = chatContainerRef.value.scrollHeight
    }
  })
}

// Expose methods
defineExpose({
  scrollToBottom
})
</script>

<style scoped>
.ai-chat-container {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ai-messages {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ai-message {
  max-width: 85%;
  padding: 8px 12px;
  border-radius: 12px;
  font-size: 14px;
  line-height: 1.5;
  word-wrap: break-word;
}

.ai-message.user {
  align-self: flex-end;
  background-color: var(--theme-primary);
  color: var(--theme-text-primary);
  border-bottom-right-radius: 4px;
}

.ai-message.assistant {
  align-self: flex-start;
  background-color: var(--theme-primary);
  color: var(--theme-text-primary);
  border-bottom-left-radius: 4px;
}

.ai-message.loading .message-content {
  display: flex;
  align-items: center;
}

.cursor-blink {
  animation: blink 1s step-end infinite;
}

@keyframes blink {
  0%, 50% { opacity: 1; }
  51%, 100% { opacity: 0; }
}

.message-content {
  white-space: pre-wrap;
}
</style>
