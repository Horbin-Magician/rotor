<template>
  <div class="searcher-container">
    <!-- Search Input -->
    <div class="search-input-container">
      <div class="search-input-wrapper">
        <n-icon size="24" color="#666" class="search-icon">
          <SearchIcon />
        </n-icon>
        <input
          ref="searchInputRef"
          v-model="searchQuery"
          :placeholder="$t('message.search_placeholder')"
          autofocus
          @input="handleSearch"
          @keydown="handleKeydown"
          class="search-input"
          autocomplete="off"
          spellcheck="false"
        />
      </div>
    </div>

    <!-- Search Results -->
    <div v-if="searchResults.length > 0" class="search-results-container">
      <n-infinite-scroll
        class="search-results"
        @load="handleLoadMore"
        :scrollbar-props="{trigger: 'none'}"
      >
        <div
          v-for="(item, index) in searchResults"
          :key="index"
          :class="['search-item', { selected: selectedIndex === index }]"
          @click="clickItem(item)"
          @mouseenter="selectedIndex = index"
        >
          <div class="item-icon">
              <img :src="`data:image/png;base64,${item.icon_data}`" alt="File icon" />
          </div>
          <div class="item-content">
            <div class="item-title">{{ item.title }}</div>
            <div class="item-subtitle">{{ item.subtitle }}</div>
          </div>
          <div v-if="item.actions" class="item-actions">
            <div
              v-for="action in item.actions"
              :key="action.type"
              class="item-action-btn"
              :title="action.title"
              @click.stop="handleActionClick(action, item)"
            >
              <n-icon size="20">
                <component :is="getActionIcon(action.type)" />
              </n-icon>
            </div>
          </div>
        </div>
      </n-infinite-scroll>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { NIcon, NInfiniteScroll } from 'naive-ui'
import { getCurrentWindow, PhysicalPosition, LogicalSize, currentMonitor } from '@tauri-apps/api/window'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import {
  SearchRound as SearchIcon,
  AdminPanelSettingsFilled as OpenAsAdminIcon,
  FolderCopyRound as OpenFolderIcon,
  ErrorFilled as ErrorIcon,
} from '@vicons/material'
import { invoke } from '@tauri-apps/api/core'

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
}

type ItemType = 'app' | 'folder' | 'file' | 'settings'
type ActionType = 'OpenAsAdmin' | 'OpenFolder'

// Constants
const WINDOW_CONFIG = {
  width: 500,
  itemHeight: 60,
  inputHeight: 50,
  maxVisibleItems: 7
} as const

const ACTION_ICONS: Record<ActionType, any> = {
  OpenAsAdmin: OpenAsAdminIcon,
  OpenFolder: OpenFolderIcon
} as const

// State
const appWindow = getCurrentWindow()
const searchInputRef = ref<HTMLInputElement>()
const searchQuery = ref('')
const selectedIndex = ref(0)
let unlistenBlur: (() => void) | null = null

const searchResults = ref<SearchItem[]>([])

// Utils
const getActionIcon = (type: string) => ACTION_ICONS[type as ActionType] || ErrorIcon

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
  const monitor = await currentMonitor()
  if (!monitor) return
  
  const scale = await appWindow.scaleFactor()
  const centerX = monitor.position.x + (monitor.size.width - scale * WINDOW_CONFIG.width) / 2
  const centerY = Math.ceil(monitor.position.y + monitor.size.height * 0.4)
  
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
    if (!container) return
    
    const selectedElement = container.querySelector('.search-item.selected')
    if (selectedElement) {
      selectedElement.scrollIntoView({
        behavior: 'smooth',
        block: 'nearest'
      })
    }
  })
}

const clickItem = (item: SearchItem) => {
  invoke("open_file", {filePath: item.subtitle + '/' + item.title})
  hideWindow()
}

const handleActionClick = (action: Action, item: SearchItem) => {
  const filePath = item.subtitle + '/' + item.title
  
  switch (action.type) {
    case 'OpenAsAdmin':
      invoke("open_file_as_admin", { filePath })
        .catch(err => console.error('Failed to open as admin:', err))
      break
    case 'OpenFolder':
      invoke("open_file_as_admin", { filePath: item.subtitle })
        .catch(err => console.error('Failed to open folder:', err))
      break
    default:
      console.warn('Unknown action type:', action.type)
  }
  
  hideWindow()
}

const hideWindow = async () => {
  await appWindow.hide()
  searchQuery.value = ''
  selectedIndex.value = 0
  invoke("searcher_release")
  nextTick(resizeWindow)
}

const handleLoadMore = () => {
  invoke("searcher_find", { query: searchQuery.value });
}

watch(searchQuery, (newVal, _oldVal) => {
  invoke("searcher_find", { query: newVal });
});

let unlisten_update_result: UnlistenFn;
interface SearchResultItem {
  path: string;
  file_name: string;
  rank: number;
  icon_data?: string;
}
type UpdateResultPayload = [string, SearchResultItem[], boolean];

// Lifecycle
onMounted(async () => {
  nextTick(() => {
    searchInputRef.value?.focus()
    resizeWindow()
  })

  // 监听窗口失去焦点事件
  unlistenBlur = await listen('tauri://blur', () => {
    hideWindow()
  })

  unlistenBlur = await listen('tauri://focus', () => {
    searchInputRef.value?.focus()
  })

  unlisten_update_result = await appWindow.listen<UpdateResultPayload>('update_result', async (event) => {
    const [filename, getSearchResults, if_increase] = event.payload;
    if (filename !== searchQuery.value) return;
    if (!if_increase) searchResults.value = [];
    searchResults.value = searchResults.value.concat(
      getSearchResults.map((item, _index) => ({
        title: item.file_name,
        subtitle: item.path,
        type: 'file',
        icon_data: item.icon_data,
        actions: [
          { type: 'OpenAsAdmin', title: '管理员权限运行' },
          { type: 'OpenFolder', title: '打开路径' }
        ]
      }))
    );
    nextTick(resizeWindow)
  });
})

onUnmounted(() => {
  // 清理事件监听器
  if (unlistenBlur) {
    unlistenBlur()
    unlistenBlur = null
  }

  if(unlisten_update_result) { unlisten_update_result() }
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
}

/* Search Input Styles */
.search-input-container {
  position: relative;
  padding: 0 12px;
}

.search-input-container::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 10px;
  right: 10px;
  height: 2px;
  border-radius: 1px;
  background-color: #4b9df4;
}

.search-input-wrapper {
  display: flex;
  align-items: center;
  height: 50px;
  gap: 8px;
}

.search-icon {
  flex-shrink: 0;
}

.search-input {
  flex: 1;
  height: 100%;
  border: none;
  outline: none;
  background: transparent;
  font-size: 16px;
  color: var(--n-text-color);
  padding: 0;
}

/* Search Results Styles */
.search-results-container {
  flex: 1;
  overflow: hidden;
  position: relative;
}

.search-results {
  height: 100%;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: rgba(75, 157, 244, 0.6) rgba(255, 255, 255, 0.05);
  padding-right: 2px;
}

.search-item {
  width: 100%;
  display: flex;
  align-items: center;
  transition: background-color 0.1s ease;
  height: 60px;
  position: relative;
  overflow: hidden;
}

.search-item.selected {
  background-color: rgb(31, 31, 31);
}

.item-icon {
  padding: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.item-content {
  flex: 1;
  min-width: 0;
  padding-right: 10px;
  transition: padding-right 0.3s ease;
}

.item-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--n-text-color);
  margin-bottom: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.item-subtitle {
  font-size: 12px;
  color: var(--n-text-color-disabled);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.item-actions {
  height: 100%;
  display: flex;
  align-items: center;
  transform: translateX(100%);
  transition: transform 0.3s ease;
  position: absolute;
  right: 0;
  top: 0;
}

.search-item:hover .item-actions {
  transform: translateX(0);
}

.search-item:hover .item-content {
  padding-right: 80px;
}

.item-action-btn {
  width: 38px;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.2s ease;
  cursor: pointer;
}

.item-action-btn:hover {
  color: #4b9df4;
}

/* Empty State Styles */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  height: 100%;
  color: var(--n-text-color-disabled);
}
</style>
