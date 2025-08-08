<template>
  <div class="searcher-container">
    <!-- Search Input -->
    <div class="search-input-container">
      <div class="search-input-wrapper">
        <n-icon size="20" color="#666" class="search-icon">
          <SearchIcon />
        </n-icon>
        <input
          ref="searchInputRef"
          v-model="searchQuery"
          :placeholder="$t('message.search')"
          autofocus
          @input="handleSearch"
          @keydown="handleKeydown"
          @focus="handleFocus"
          @blur="handleBlur"
          class="search-input"
          autocomplete="off"
          autocorrect="off"
          autocapitalize="off"
          spellcheck="false"
        />
        <button
          v-if="searchQuery"
          @click="clearSearch"
          class="clear-button"
          type="button"
        >
          <n-icon size="16">
            <component :is="ClearIcon" />
          </n-icon>
        </button>
      </div>
    </div>

    <!-- Search Results -->
    <div class="search-results" v-if="displayItems.length > 0">
      <div
        v-for="(item, index) in displayItems"
        :key="item.id"
        :class="['search-item', { 'selected': selectedIndex === index }]"
        @click="selectItem(item)"
        @mouseenter="selectedIndex = index"
      >
        <div class="item-icon">
          <n-icon size="30">
            <component :is="getIcon(item.type)" />
          </n-icon>
        </div>
        <div class="item-content">
          <div class="item-title">{{ item.title }}</div>
          <div class="item-subtitle">{{ item.subtitle }}</div>
        </div>
        <div class="item-actions" v-if="item.actions">
          <div class="item-action-btn" v-for="action in item.actions" :key="action.type">
            <n-icon size="20" class="action-icon" :title="action.title">
              <component :is="getActionIcon(action.type)" />
            </n-icon>
          </div>
        </div>
      </div>
    </div>

    <!-- Empty State -->
    <div class="empty-state" v-else-if="searchQuery.trim()">
      <n-icon size="48" color="#ccc">
        <SearchIcon />
      </n-icon>
      <div>{{ $t('message.search') }} "{{ searchQuery }}" 无结果</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue'
import { NIcon } from 'naive-ui'
import { getCurrentWindow, PhysicalPosition } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/window'
import { currentMonitor } from '@tauri-apps/api/window'
import {
  SearchRound as SearchIcon,
  FolderRound as FolderIcon,
  InsertDriveFileRound as FileIcon,
  AppsRound as AppIcon,
  SettingsRound as SettingsIcon,
  LaunchRound as LaunchIcon,
  ContentCopyRound as CopyIcon,
  CloseRound as ClearIcon
} from '@vicons/material'

// 类型定义
interface Action {
  type: string
  title: string
}

interface SearchItem {
  id: number
  title: string
  subtitle: string
  type: string
  actions?: Action[]
}

type ItemType = 'app' | 'folder' | 'file' | 'settings'
type ActionType = 'launch' | 'copy'

// 常量配置
const WINDOW_CONFIG = {
  width: 500,
  itemHeight: 50,
  padding: 12,
  inputHeight: 50
}

const appWindow = getCurrentWindow()
const searchInputRef = ref()
const searchQuery = ref('')
const selectedIndex = ref(0)
const isFocused = ref(false)

const searchResults = ref<SearchItem[]>([
  { id: 1, title: 'Zotero.lnk', subtitle: 'C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\asdasdsdadsadsdadadsdsdadsadsdad', type: 'app', actions: [{ type: 'launch', title: '启动' }, { type: 'copy', title: '复制路径' }] },
  { id: 2, title: 'zotero.exe', subtitle: 'D:\\Zotero\\', type: 'app', actions: [{ type: 'launch', title: '启动' }, { type: 'copy', title: '复制路径' }] },
  { id: 3, title: 'Zotero', subtitle: 'C:\\Users\\22400\\AppData\\Local\\Zotero\\sdasdsdadsadsdadadsdasdsdadsadsdadadsdasdsdadsadsdadadsdasdsdadsadsdadadsdasdsdadsadsdadadsdasdsdadsadsdadad', type: 'folder', actions: [{ type: 'launch', title: '打开' }, { type: 'copy', title: '复制路径' }] },
  { id: 4, title: 'project-report.pdf', subtitle: 'C:\\Users\\Documents\\', type: 'file', actions: [{ type: 'launch', title: '打开' }, { type: 'copy', title: '复制路径' }] },
  { id: 5, title: 'Visual Studio Code', subtitle: 'Microsoft Corporation', type: 'app', actions: [{ type: 'launch', title: '启动' }] },
  { id: 6, title: 'Chrome', subtitle: 'Google LLC', type: 'app', actions: [{ type: 'launch', title: '启动' }] }
])

const displayItems = computed(() => {
  if (!searchQuery.value.trim()) { return [] }
  
  const query = searchQuery.value.toLowerCase()
  return searchResults.value
    .filter(item => 
      item.title.toLowerCase().includes(query) || 
      item.subtitle.toLowerCase().includes(query)
    )
    .slice(0, 8)
})

const ICON_MAP: Record<ItemType, { icon: any; color: string }> = {
  app: { icon: AppIcon, color: '#54a4db' },
  folder: { icon: FolderIcon, color: '#ffa726' },
  file: { icon: FileIcon, color: '#66bb6a' },
  settings: { icon: SettingsIcon, color: '#ab47bc' }
}
const ACTION_ICONS: Record<ActionType, any> = {
  launch: LaunchIcon,
  copy: CopyIcon
}
const getIcon = (type: string) => ICON_MAP[type as ItemType]?.icon || FileIcon
const getActionIcon = (type: string) => ACTION_ICONS[type as ActionType] || LaunchIcon

const resizeWindow = async () => {
  const currentSize = await appWindow.outerSize()
  let newHeight = WINDOW_CONFIG.inputHeight

  if (searchQuery.value.trim()) {
    newHeight += displayItems.value.length > 0 ? displayItems.value.length * WINDOW_CONFIG.itemHeight : 120
  }

  if (currentSize.height != newHeight) {
    await appWindow.setSize(new LogicalSize(WINDOW_CONFIG.width, newHeight))
  }

  const monitor = await currentMonitor()
  if (!monitor) return
  const scale = await appWindow.scaleFactor()

  const centerX = monitor.position.x + (monitor.size.width - scale * WINDOW_CONFIG.width) / 2
  const centerY = Math.ceil(monitor.position.y + monitor.size.height * 0.4)
  
  await appWindow.setPosition(new PhysicalPosition(centerX, centerY))
}

const handleSearch = () => {
  selectedIndex.value = 0
  nextTick(resizeWindow)
}

const handleKeydown = (event: KeyboardEvent) => {
  const maxIndex = displayItems.value.length - 1
  
  switch (event.key) {
    case 'ArrowDown':
      event.preventDefault()
      selectedIndex.value = Math.min(selectedIndex.value + 1, maxIndex)
      break
    case 'ArrowUp':
      event.preventDefault()
      selectedIndex.value = Math.max(selectedIndex.value - 1, 0)
      break
    case 'Enter':
      event.preventDefault()
      if (displayItems.value[selectedIndex.value]) {
        selectItem(displayItems.value[selectedIndex.value])
      }
      break
    case 'Escape':
      event.preventDefault()
      hideWindow()
      break
  }
}

// TODO
const selectItem = (item: SearchItem) => {
  console.log('Selected item:', item)
  hideWindow()
}

const handleFocus = () => {
  isFocused.value = true
}

const handleBlur = () => {
  isFocused.value = false
}

const clearSearch = () => {
  searchQuery.value = ''
  selectedIndex.value = 0
  nextTick(resizeWindow)
}

const hideWindow = async () => {
  await appWindow.hide()
  searchQuery.value = ''
  selectedIndex.value = 0
  nextTick(resizeWindow)
}

onMounted(async () => {
  const visible = await appWindow.isVisible()
  if (!visible) {
    await appWindow.show()
    await appWindow.setFocus()
  }
  
  nextTick(() => {
    searchInputRef.value?.focus()
    resizeWindow()
  })
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

.search-input-container {
  position: relative;
  padding: 0 12px;
}

.search-input-container::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 12px;
  right: 12px;
  height: 2px;
  background-color: #4b9df4;
  transform: scaleX(0);
  transition: transform 0.3s ease;
  transform-origin: center;
}

.search-input-container:focus-within::after {
  transform: scaleX(1);
}

.search-input-wrapper {
  display: flex;
  align-items: center;
  height: 50px;
  position: relative;
}

.search-icon {
  margin-right: 8px;
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

.search-input::placeholder {
  color: var(--n-text-color-disabled);
}

.clear-button {
  background: none;
  border: none;
  cursor: pointer;
  padding: 4px;
  margin-left: 8px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--n-text-color-disabled);
  transition: all 0.2s ease;
  flex-shrink: 0;
}

.clear-button:hover {
  background-color: var(--n-fill-color-hover);
  color: var(--n-text-color);
}

.search-item {
  width: 100%;
  display: flex;
  align-items: center;
  transition: all 0.1s ease;
  height: 50px;
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
  transform: translateX(100%);
  transition: all 0.3s ease;
  position: absolute;
  right: 0;
  top: 0;
  align-items: center;
}

.search-item:hover .item-actions {
  transform: translateX(0);
}

.search-item:hover .item-content {
  padding-right: 80px;
}

.item-action-btn {
  width: 38px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.2s ease;
  cursor: pointer;
}

.item-action-btn:hover {
  color: #4b9df4;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  height: 100%;
}
</style>
