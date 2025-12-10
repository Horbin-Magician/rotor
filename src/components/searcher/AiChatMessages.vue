<template>
  <div class="ai-chat-container" ref="chatContainerRef">
    <div class="ai-messages">
      <div
        v-for="(msg, index) in messages"
        :key="index"
        class="ai-message"
        :class="msg.role"
      >
        <div 
          class="message-content markdown-body"
          v-html="parseMarkdown(msg.content)"
        ></div>
        <span
          v-if="isLoading && index === messages.length - 1 && msg.role === 'assistant'"
          class="cursor-blink"
        >|</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick } from 'vue'
import { marked } from 'marked'

// Configure marked options
marked.setOptions({
  breaks: true,
  gfm: true
})

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
const parseMarkdown = (content: string): string => {
  if (!content) return ''
  return marked.parse(content, { async: false }) as string
}

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
  margin-left: 2px;
}

@keyframes blink {
  0%, 50% { opacity: 1; }
  51%, 100% { opacity: 0; }
}

/* Markdown styles */
.message-content.markdown-body {
  word-wrap: break-word;
  overflow-wrap: break-word;
}

.message-content.markdown-body :deep(p) {
  margin: 0 0 8px 0;
}

.message-content.markdown-body :deep(p:last-child) {
  margin-bottom: 0;
}

.message-content.markdown-body :deep(h1),
.message-content.markdown-body :deep(h2),
.message-content.markdown-body :deep(h3),
.message-content.markdown-body :deep(h4),
.message-content.markdown-body :deep(h5),
.message-content.markdown-body :deep(h6) {
  margin: 12px 0 8px 0;
  font-weight: 600;
  line-height: 1.25;
}

.message-content.markdown-body :deep(h1) {
  font-size: 1.5em;
}

.message-content.markdown-body :deep(h2) {
  font-size: 1.3em;
}

.message-content.markdown-body :deep(h3) {
  font-size: 1.1em;
}

.message-content.markdown-body :deep(ul),
.message-content.markdown-body :deep(ol) {
  margin: 8px 0;
  padding-left: 20px;
}

.message-content.markdown-body :deep(li) {
  margin: 4px 0;
}

.message-content.markdown-body :deep(code) {
  background-color: rgba(0, 0, 0, 0.15);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 0.9em;
}

.message-content.markdown-body :deep(pre) {
  background-color: rgba(0, 0, 0, 0.2);
  padding: 12px;
  border-radius: 8px;
  overflow-x: auto;
  margin: 8px 0;
}

.message-content.markdown-body :deep(pre code) {
  background-color: transparent;
  padding: 0;
  border-radius: 0;
  font-size: 0.85em;
  line-height: 1.4;
}

.message-content.markdown-body :deep(blockquote) {
  border-left: 3px solid rgba(255, 255, 255, 0.4);
  margin: 8px 0;
  padding-left: 12px;
  color: inherit;
  opacity: 0.85;
}

.message-content.markdown-body :deep(a) {
  color: #58a6ff;
  text-decoration: none;
}

.message-content.markdown-body :deep(a:hover) {
  text-decoration: underline;
}

.message-content.markdown-body :deep(table) {
  border-collapse: collapse;
  margin: 8px 0;
  width: 100%;
}

.message-content.markdown-body :deep(th),
.message-content.markdown-body :deep(td) {
  border: 1px solid rgba(255, 255, 255, 0.2);
  padding: 6px 12px;
  text-align: left;
}

.message-content.markdown-body :deep(th) {
  background-color: rgba(0, 0, 0, 0.1);
  font-weight: 600;
}

.message-content.markdown-body :deep(hr) {
  border: none;
  border-top: 1px solid rgba(255, 255, 255, 0.2);
  margin: 12px 0;
}

.message-content.markdown-body :deep(img) {
  max-width: 100%;
  border-radius: 4px;
}

.message-content.markdown-body :deep(strong) {
  font-weight: 600;
}

.message-content.markdown-body :deep(em) {
  font-style: italic;
}
</style>
