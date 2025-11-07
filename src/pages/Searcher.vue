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

    <!-- Search Results -->
    <SearchResultList
      ref="searchResultListRef"
      v-model="selectedIndex"
      :items="searchResults"
      @item-click="clickItem"
      @action-click="handleActionClick"
      @load-more="handleLoadMore"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { getCurrentWindow, PhysicalPosition, LogicalSize, primaryMonitor } from '@tauri-apps/api/window'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import SearchInput from '../components/searcher/SearchInput.vue'
import SearchResultList from '../components/searcher/SearchResultList.vue'

// Types
interface Action {
  type: string
  title: string
}

interface SearchItem {
  title: string
  subtitle: string
  type: ItemType
  actions?: Action[]
  icon_data?: string // Base64 encoded PNG data
  alias?: string
}

type ItemType = 'app' | 'file'

// Constants
const WINDOW_CONFIG = {
  width: 500,
  itemHeight: 60,
  inputHeight: 50,
  maxVisibleItems: 7
} as const

// State
const appWindow = getCurrentWindow()
const searchInputRef = ref<InstanceType<typeof SearchInput>>()
const searchResultListRef = ref<InstanceType<typeof SearchResultList>>()
const searchQuery = ref('')
const selectedIndex = ref(0)

let unlistenBlur: (() => void) | null = null
let unlistenFocus: (() => void) | null = null

const searchResults = ref<SearchItem[]>([])

// Window management
const resizeWindow = async () => {
  const currentSize = await appWindow.outerSize()
  let newHeight = WINDOW_CONFIG.inputHeight

  if (searchQuery.value.trim() && searchResults.value.length > 0) {
    const visibleItems = Math.min(searchResults.value.length, WINDOW_CONFIG.maxVisibleItems)
    newHeight += visibleItems * WINDOW_CONFIG.itemHeight
  }

  if (currentSize.height !== newHeight) {
    await appWindow.setSize(new LogicalSize(WINDOW_CONFIG.width, newHeight))
  }

  // Center window
  const monitor = await primaryMonitor()
  if (!monitor) return
  
  const scale = await appWindow.scaleFactor()
  const centerX = monitor.position.x + (monitor.size.width - scale * WINDOW_CONFIG.width) / 2
  const centerY = Math.ceil(monitor.position.y + monitor.size.height * 0.3)
  
  await appWindow.setPosition(new PhysicalPosition(centerX, centerY))
}

// Event handlers
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

const clickItem = (item: SearchItem) => {
  invoke("open_file", {filePath: item.subtitle + item.title})
  hideWindow()
}

const handleActionClick = (action: Action, item: SearchItem) => {
  const filePath = item.subtitle + item.title
  
  switch (action.type) {
    case 'OpenAsAdmin':
      invoke("open_file_as_admin", { filePath })
        .catch(err => console.error('Failed to open as admin:', err))
      break
    case 'OpenFolder':
      invoke("open_file", { filePath: item.subtitle })
        .catch(err => console.error('Failed to open folder:', err))
      break
    default:
      console.warn('Unknown action type:', action.type)
  }
  
  hideWindow()
}

const hideWindow = async () => {
  searchQuery.value = ''
  selectedIndex.value = 0
  searchResults.value = []
  resizeWindow()
  await appWindow.hide()
  invoke("searcher_release")
}

const handleLoadMore = () => {
  invoke("searcher_find", { query: searchQuery.value });
}

watch(searchQuery, (newVal, _oldVal) => {
  if( newVal != "" ) {
    invoke("searcher_find", { query: newVal });
  }
});

let unlisten_update_result: UnlistenFn;
interface SearchResultItem {
  path: string;
  file_name: string;
  rank: number;
  icon_data?: string;
  alias?: string;
}
type UpdateResultPayload = [string, SearchResultItem[], boolean];

// Lifecycle
onMounted(async () => {
  unlistenBlur = await listen('tauri://blur', () => {
    // Delay checking to avoid instantaneous blur during window switching
    setTimeout(() => {
      // Check if the focus has really been lost at present
      getCurrentWindow().isFocused().then(focused => {
        if (!focused) { hideWindow() }
      })
    }, 100)
  })

  unlistenFocus = await listen('tauri://focus', () => {
    searchInputRef.value?.focus()
    resizeWindow()
  })

  // Listen for search result updates
  unlisten_update_result = await appWindow.listen<UpdateResultPayload>('update_result', async (event) => {
    const [filename, getSearchResults, if_increase] = event.payload;
    if (filename !== searchQuery.value) return;
    if (!if_increase) searchResults.value = [];
    searchResults.value = searchResults.value.concat(
      getSearchResults.map((item, _index) => {
        const lower_file_name = item.file_name.toLowerCase();
        const isApp = lower_file_name.endsWith('.app') || lower_file_name.endsWith('.exe') || lower_file_name.endsWith('.lnk');
        return {
          title: item.file_name,
          subtitle: item.path,
          type: (isApp ? 'app' : 'file') as ItemType,
          icon_data: item.icon_data,
          alias: item.alias,
          actions: [
            { type: 'OpenAsAdmin', title: 'message.openAsAdminTip' },
            { type: 'OpenFolder', title: 'message.openFolderTip' }
          ]
        };
      })
    );
    resizeWindow()
  });

  nextTick(resizeWindow)
})

onUnmounted(() => {
  // clean listeners
  if (unlistenBlur) { unlistenBlur() }
  if (unlistenFocus) { unlistenFocus() }
  if (unlisten_update_result) { unlisten_update_result() }
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
