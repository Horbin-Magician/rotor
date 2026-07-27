<template>
  <div class="translator-container">
    <div class="translator-input-wrapper" :class="{ 'is-loading': translating }">
      <n-icon size="22" class="translator-icon">
        <TranslateIcon />
      </n-icon>
      <input
        ref="inputRef"
        v-model="sourceText"
        :placeholder="$t('message.translatorPlaceholder')"
        autofocus
        class="translator-input"
        autocomplete="off"
        spellcheck="false"
        @keydown="handleKeydown"
      />
    </div>

    <div v-if="result || translating || errorMessage" class="translator-result">
      <div v-if="translating" class="translator-status">
        {{ $t('message.translating') }}
      </div>
      <div v-else-if="errorMessage" class="translator-status is-error">
        {{ errorMessage }}
      </div>
      <template v-else-if="result">
        <div class="translator-meta">{{ result.from }} → {{ result.to }}</div>
        <div class="translator-text">{{ result.translated }}</div>
        <div class="translator-actions">
          <button class="translator-copy" @click="copyResult">
            {{ copied ? $t('message.translatorCopied') : $t('message.translatorCopy') }}
          </button>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { LogicalSize, getCurrentWindow } from '@tauri-apps/api/window'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { NIcon } from 'naive-ui'
import { TranslateRound as TranslateIcon } from '@vicons/material'
import { translatorTranslate } from '../features/translator/api'
import type { TranslateResult } from '../features/translator/types'

const WINDOW_WIDTH = 560
const INPUT_HEIGHT = 50
const RESULT_MIN_HEIGHT = 96
const RESULT_MAX_HEIGHT = 320

const appWindow = getCurrentWindow()

const inputRef = ref<HTMLInputElement>()
const sourceText = ref('')
const result = ref<TranslateResult | null>(null)
const translating = ref(false)
const errorMessage = ref('')
const copied = ref(false)

let unlistenBlur: UnlistenFn | null = null
let unlistenFocus: UnlistenFn | null = null
let unlistenSelect: UnlistenFn | null = null
let unlistenInput: UnlistenFn | null = null
let translateSeq = 0

const resizeWindow = async () => {
  let height = INPUT_HEIGHT
  if (result.value || translating.value || errorMessage.value) {
    height += RESULT_MIN_HEIGHT
    await nextTick()
    const resultEl = document.querySelector('.translator-result')
    if (resultEl) {
      height = INPUT_HEIGHT + Math.min(resultEl.scrollHeight, RESULT_MAX_HEIGHT)
    }
  }
  await appWindow.setSize(new LogicalSize(WINDOW_WIDTH, height))
}

const doTranslate = async (text: string) => {
  const query = text.trim()
  if (!query) {
    result.value = null
    errorMessage.value = ''
    await resizeWindow()
    return
  }

  const seq = ++translateSeq
  translating.value = true
  errorMessage.value = ''
  copied.value = false
  await resizeWindow()

  try {
    const translated = await translatorTranslate(query)
    if (seq !== translateSeq) return
    result.value = translated
  } catch (error) {
    if (seq !== translateSeq) return
    result.value = null
    errorMessage.value = typeof error === 'string' ? error : String(error)
  } finally {
    if (seq === translateSeq) {
      translating.value = false
      await resizeWindow()
    }
  }
}

const copyResult = async () => {
  if (!result.value) return
  try {
    await writeText(result.value.translated)
    copied.value = true
  } catch (error) {
    console.warn('Failed to copy translate result:', error)
  }
}

const handleKeydown = (event: KeyboardEvent) => {
  switch (event.key) {
    case 'Enter':
      event.preventDefault()
      void doTranslate(sourceText.value)
      break
    case 'Escape':
      event.preventDefault()
      hideWindow()
      break
  }
}

const resetState = () => {
  translateSeq++
  sourceText.value = ''
  result.value = null
  translating.value = false
  errorMessage.value = ''
  copied.value = false
}

const hideWindow = async () => {
  resetState()
  await resizeWindow()
  await appWindow.hide()
}

onMounted(async () => {
  unlistenSelect = await appWindow.listen<string>('translate-select', async (event) => {
    sourceText.value = event.payload
    await nextTick()
    void doTranslate(event.payload)
  })

  unlistenInput = await appWindow.listen('translate-input', async () => {
    resetState()
    await resizeWindow()
    await nextTick()
    inputRef.value?.focus()
  })

  unlistenBlur = await listen('tauri://blur', () => {
    setTimeout(() => {
      appWindow.isFocused().then((focused) => {
        if (!focused) {
          hideWindow()
        }
      })
    }, 100)
  })

  unlistenFocus = await listen('tauri://focus', () => {
    inputRef.value?.focus()
  })

  await resizeWindow()
})

onUnmounted(() => {
  if (unlistenBlur) unlistenBlur()
  if (unlistenFocus) unlistenFocus()
  if (unlistenSelect) unlistenSelect()
  if (unlistenInput) unlistenInput()
})
</script>

<style scoped>
.translator-container {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  overflow: hidden;
  background-color: var(--theme-background);
}

.translator-input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  height: 50px;
  gap: 8px;
  padding: 0 12px;
  flex-shrink: 0;
}

.translator-input-wrapper::after {
  content: '';
  position: absolute;
  right: 12px;
  bottom: 0;
  left: 12px;
  height: 2px;
  border-radius: 999px;
  background-color: var(--theme-primary);
}

.translator-input-wrapper.is-loading::after {
  animation: translator-line-breathe 1.15s ease-in-out infinite;
}

.translator-icon {
  flex-shrink: 0;
  color: var(--theme-text-secondary);
}

.translator-input {
  flex: 1;
  height: 100%;
  border: none;
  outline: none;
  background: transparent;
  font-size: 15px;
  color: var(--theme-text-primary);
  padding: 0;
}

.translator-input::placeholder {
  color: var(--theme-text-disabled);
}

.translator-result {
  flex: 1;
  overflow-y: auto;
  padding: 10px 14px 12px;
  box-sizing: border-box;
}

.translator-status {
  font-size: 13px;
  color: var(--theme-text-secondary);
}

.translator-status.is-error {
  color: var(--theme-error);
}

.translator-meta {
  font-size: 12px;
  color: var(--theme-text-disabled);
  margin-bottom: 6px;
}

.translator-text {
  font-size: 15px;
  line-height: 1.6;
  color: var(--theme-text-primary);
  white-space: pre-wrap;
  word-break: break-word;
  user-select: text;
}

.translator-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 8px;
}

.translator-copy {
  border: none;
  outline: none;
  cursor: pointer;
  font-size: 12px;
  padding: 4px 12px;
  border-radius: 4px;
  color: var(--theme-text-secondary);
  background-color: transparent;
}

.translator-copy:hover {
  color: var(--theme-primary);
  background-color: color-mix(in srgb, var(--theme-primary) 12%, transparent);
}

@keyframes translator-line-breathe {
  0%,
  100% {
    opacity: 0.3;
  }

  50% {
    opacity: 1;
  }
}
</style>
