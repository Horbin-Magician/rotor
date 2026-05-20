<template>
  <div class="searcher-container">
    <!-- Search Input -->
    <SearchInput
      ref="searchInputRef"
      v-model="searchQuery"
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
import { useFileSearch } from '../features/searcher/composables/useFileSearch'
import { useSearcherWindow } from '../features/searcher/composables/useSearcherWindow'
import type { UpdateResultPayload } from '../features/searcher/types'

// State
const searchInputRef = ref<InstanceType<typeof SearchInput>>()
const searchQuery = ref('')

let unlistenBlur: UnlistenFn | null = null
let unlistenFocus: UnlistenFn | null = null
let unlistenUpdateResult: UnlistenFn | null = null
let resizeWindow: () => Promise<void> = async () => {}

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

const hideWindow = async () => {
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

  unlistenUpdateResult = await appWindow.listen<UpdateResultPayload>('update_result', async (event) => {
    await handleUpdateResult(event.payload)
  });

  nextTick(resizeWindow)
})

onUnmounted(() => {
  // clean listeners
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
