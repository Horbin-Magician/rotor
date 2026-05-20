<template>
  <div class="searcher-container">
    <!-- Search Input -->
    <SearchInput
      ref="searchInputRef"
      v-model="searchQuery"
      :placeholder="$t('message.searchPlaceholder')"
      :is-ai-mode="isAiMode"
      @input="handleSearch"
      @keydown="handleKeydown"
      @toggle-mode="toggleMode"
    />

    <!-- Search Results (Search Mode) -->
    <SearchResultList
      v-if="!isAiMode"
      v-model="selectedIndex"
      :items="searchResults"
      @item-click="clickItem"
      @action-click="handleActionClick"
      @load-more="handleLoadMore"
    />

    <!-- AI Chat Messages (AI Mode) -->
    <AiChatMessages
      v-else
      ref="aiChatMessagesRef"
      :messages="chatMessages"
      :is-loading="isLoading"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import SearchInput from '../components/searcher/SearchInput.vue'
import SearchResultList from '../components/searcher/SearchResultList.vue'
import AiChatMessages from '../components/searcher/AiChatMessages.vue'
import { useAiChat } from '../features/ai/composables/useAiChat'
import type { StreamEvent } from '../features/ai/types'
import { useFileSearch } from '../features/searcher/composables/useFileSearch'
import { useSearcherWindow } from '../features/searcher/composables/useSearcherWindow'
import type { UpdateResultPayload } from '../features/searcher/types'

// State
const searchInputRef = ref<InstanceType<typeof SearchInput>>()
const aiChatMessagesRef = ref<InstanceType<typeof AiChatMessages>>()
const searchQuery = ref('')

let unlistenBlur: UnlistenFn | null = null
let unlistenFocus: UnlistenFn | null = null
let unlistenAiStream: UnlistenFn | null = null
let unlistenUpdateResult: UnlistenFn | null = null
let resizeWindow: () => Promise<void> = async () => {}

const scrollChatToBottom = () => {
  aiChatMessagesRef.value?.scrollToBottom()
}

const {
  searchResults,
  selectedIndex,
  resetSearch,
  requestSearch,
  releaseSearch,
  handleLoadMore,
  handleUpdateResult,
  clickItem,
  handleActionClick,
} = useFileSearch(searchQuery, () => { void hideWindow() }, () => resizeWindow())

const {
  isAiMode,
  chatMessages,
  isLoading,
  resetAiChat,
  toggleMode,
  sendAiMessage,
  handleStreamEvent,
} = useAiChat(searchQuery, {
  resetSearchState: resetSearch,
  resizeWindow: () => resizeWindow(),
  scrollToBottom: scrollChatToBottom,
})

const searcherWindow = useSearcherWindow({
  isAiMode,
  chatCount: computed(() => chatMessages.value.length),
  isLoading,
  searchQuery,
  resultCount: computed(() => searchResults.value.length),
})
const appWindow = searcherWindow.appWindow
resizeWindow = searcherWindow.resizeWindow

const handleSearch = () => {
  if (!isAiMode.value) {
    selectedIndex.value = 0
    nextTick(resizeWindow)
  }
}

const handleKeydown = (event: KeyboardEvent) => {
  if (isAiMode.value) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      sendAiMessage()
    } else if (event.key === 'Escape') {
      event.preventDefault()
      hideWindow()
    }
    return
  }

  const maxIndex = searchResults.value.length - 1
  
  switch (event.key) {
    case 'ArrowDown':
      event.preventDefault()
      selectedIndex.value = Math.min(selectedIndex.value + 1, maxIndex)
      scrollToSelected()
      break
    case 'ArrowUp':
      event.preventDefault()
      selectedIndex.value = Math.max(selectedIndex.value - 1, 0)
      scrollToSelected()
      break
    case 'Enter':
      event.preventDefault()
      if (searchResults.value[selectedIndex.value]) {
        clickItem(searchResults.value[selectedIndex.value])
      }
      break
    case 'Escape':
      event.preventDefault()
      hideWindow()
      break
  }
}

const scrollToSelected = () => {
  nextTick(() => {
    const container = document.querySelector('.search-results')
    const selectedElement = container?.querySelector('.search-item.selected')
    if (selectedElement) {
      selectedElement.scrollIntoView({
        behavior: 'smooth',
        block: 'nearest'
      })
    }
  })
}

const hideWindow = async () => {
  searchQuery.value = ''
  resetSearch()
  resetAiChat()
  await resizeWindow()
  await appWindow.hide()
  releaseSearch()
}

watch(searchQuery, (newVal, _oldVal) => {
  if (!isAiMode.value && newVal != "") {
    requestSearch(newVal)
  }
});

onMounted(async () => {
  unlistenBlur = await listen('tauri://blur', () => {
    setTimeout(() => {
      appWindow.isFocused().then(focused => {
        if (!focused) { hideWindow() }
      })
    }, 100)
  })

  unlistenFocus = await listen('tauri://focus', () => {
    searchInputRef.value?.focus()
    resizeWindow()
  })

  unlistenAiStream = await listen<StreamEvent>('ai-stream', (event) => {
    handleStreamEvent(event.payload)
  })

  unlistenUpdateResult = await appWindow.listen<UpdateResultPayload>('update_result', async (event) => {
    await handleUpdateResult(event.payload)
  });

  nextTick(resizeWindow)
})

onUnmounted(() => {
  // clean listeners
  if (unlistenBlur) { unlistenBlur() }
  if (unlistenFocus) { unlistenFocus() }
  if (unlistenAiStream) { unlistenAiStream() }
  if (unlistenUpdateResult) { unlistenUpdateResult() }
})
</script>

<style scoped>
.searcher-container {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  overflow: hidden;
  background-color: var(--theme-background);
}
</style>
