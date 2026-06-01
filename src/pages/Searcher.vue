<template>
  <div class="searcher-container">
    <!-- Search Input -->
    <SearchInput
      ref="searchInputRef"
      v-model="searchQuery"
      :index-state="searchIndexState"
      :placeholder="$t('message.searchPlaceholder')"
      @input="handleSearch"
      @keydown="handleKeydown"
    />

    <SearchResultList
      v-model="selectedIndex"
      :items="searchResults"
      @item-click="clickItem"
      @action-click="handleActionClick"
      @load-more="handleLoadMore"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import SearchInput from '../components/searcher/SearchInput.vue'
import SearchResultList from '../components/searcher/SearchResultList.vue'
import { searcherIndexStatus } from '../features/searcher/api'
import { useFileSearch } from '../features/searcher/composables/useFileSearch'
import { useSearcherWindow } from '../features/searcher/composables/useSearcherWindow'
import type { SearchIndexState, UpdateResultPayload } from '../features/searcher/types'

// State
const searchInputRef = ref<InstanceType<typeof SearchInput>>()
const searchQuery = ref('')
const searchIndexState = ref<SearchIndexState>('loading')

let unlistenBlur: UnlistenFn | null = null
let unlistenFocus: UnlistenFn | null = null
let unlistenUpdateResult: UnlistenFn | null = null
let resizeWindow: () => Promise<void> = async () => {}
let indexStatusTimer: number | null = null
let isRefreshingIndexStatus = false

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

const searcherWindow = useSearcherWindow({
  searchQuery,
  resultCount: computed(() => searchResults.value.length),
})
const appWindow = searcherWindow.appWindow
resizeWindow = searcherWindow.resizeWindow

const handleSearch = () => {
  selectedIndex.value = 0
  nextTick(resizeWindow)
}

const handleKeydown = (event: KeyboardEvent) => {
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

const isSettledIndexState = (state: SearchIndexState) => {
  return state === 'ready' || state === 'error' || state === 'unavailable'
}

const stopIndexStatusPolling = () => {
  if (indexStatusTimer != null) {
    window.clearInterval(indexStatusTimer)
    indexStatusTimer = null
  }
}

const refreshSearchIndexStatus = async () => {
  if (isRefreshingIndexStatus) return

  isRefreshingIndexStatus = true
  try {
    const status = await searcherIndexStatus()
    searchIndexState.value = status.state
    if (isSettledIndexState(status.state)) {
      stopIndexStatusPolling()
    }
  } catch (error) {
    console.warn('Failed to load search index status:', error)
    searchIndexState.value = 'unavailable'
    stopIndexStatusPolling()
  } finally {
    isRefreshingIndexStatus = false
  }
}

const startIndexStatusPolling = () => {
  stopIndexStatusPolling()
  searchIndexState.value = 'loading'
  void refreshSearchIndexStatus()
  indexStatusTimer = window.setInterval(() => {
    void refreshSearchIndexStatus()
  }, 1000)
}

const hideWindow = async () => {
  stopIndexStatusPolling()
  searchIndexState.value = 'loading'
  searchQuery.value = ''
  resetSearch()
  await resizeWindow()
  await appWindow.hide()
  releaseSearch()
}

watch(searchQuery, (newVal, _oldVal) => {
  if (newVal != "") {
    requestSearch(newVal)
  }
});

onMounted(async () => {
  void refreshSearchIndexStatus()

  unlistenBlur = await listen('tauri://blur', () => {
    setTimeout(() => {
      appWindow.isFocused().then(focused => {
        if (!focused) { hideWindow() }
      })
    }, 100)
  })

  unlistenFocus = await listen('tauri://focus', () => {
    searchInputRef.value?.focus()
    startIndexStatusPolling()
    resizeWindow()
  })

  unlistenUpdateResult = await appWindow.listen<UpdateResultPayload>('update_result', async (event) => {
    await handleUpdateResult(event.payload)
  });

  nextTick(resizeWindow)
})

onUnmounted(() => {
  // clean listeners
  stopIndexStatusPolling()
  if (unlistenBlur) { unlistenBlur() }
  if (unlistenFocus) { unlistenFocus() }
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
