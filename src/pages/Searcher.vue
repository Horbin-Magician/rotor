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
import { ref, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { getCurrentWindow, PhysicalPosition, LogicalSize, primaryMonitor } from '@tauri-apps/api/window'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import SearchInput from '../components/searcher/SearchInput.vue'
import SearchResultList from '../components/searcher/SearchResultList.vue'
import AiChatMessages, { type ChatMessage } from '../components/searcher/AiChatMessages.vue'

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

// Stream event types from backend
interface StreamEventContent {
  type: 'Content'
  content: string
}

interface StreamEventDone {
  type: 'Done'
}

interface StreamEventError {
  type: 'Error'
  message: string
}

type StreamEvent = StreamEventContent | StreamEventDone | StreamEventError

// Constants
const WINDOW_CONFIG = {
  width: 500,
  itemHeight: 60,
  inputHeight: 50,
  maxVisibleItems: 7,
  aiHeight: 420,
} as const

// State
const appWindow = getCurrentWindow()
const searchInputRef = ref<InstanceType<typeof SearchInput>>()
const aiChatMessagesRef = ref<InstanceType<typeof AiChatMessages>>()
const searchQuery = ref('')
const selectedIndex = ref(0)

// AI Mode state
const isAiMode = ref(false)
const chatMessages = ref<ChatMessage[]>([])
const isLoading = ref(false)

let unlistenBlur: (() => void) | null = null
let unlistenFocus: (() => void) | null = null
let unlistenAiStream: UnlistenFn | null = null

const searchResults = ref<SearchItem[]>([])

// Window management
const resizeWindow = async () => {
  const currentSize = await appWindow.outerSize()
  let newHeight = WINDOW_CONFIG.inputHeight

  if (isAiMode.value) {
    // AI mode: show chat area
    if (chatMessages.value.length > 0 || isLoading.value) {
      newHeight += WINDOW_CONFIG.aiHeight
    }
  } else {
    // Search mode
    if (searchQuery.value.trim() && searchResults.value.length > 0) {
      const visibleItems = Math.min(searchResults.value.length, WINDOW_CONFIG.maxVisibleItems)
      newHeight += visibleItems * WINDOW_CONFIG.itemHeight
    }
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

// Toggle between search and AI mode
const toggleMode = () => {
  isAiMode.value = !isAiMode.value
  searchQuery.value = ''
  selectedIndex.value = 0
  
  if (!isAiMode.value) {
    // Switching to search mode
    searchResults.value = []
  }
  
  nextTick(resizeWindow)
}

// Event handlers
const handleSearch = () => {
  if (!isAiMode.value) {
    selectedIndex.value = 0
    nextTick(resizeWindow)
  }
}

const handleKeydown = (event: KeyboardEvent) => {
  if (isAiMode.value) {
    // AI mode key handling
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      sendAiMessage()
    } else if (event.key === 'Escape') {
      event.preventDefault()
      hideWindow()
    }
    return
  }

  // Search mode key handling
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

// AI Chat functions with streaming
const sendAiMessage = async () => {
  const message = searchQuery.value.trim()
  if (!message || isLoading.value) return

  // Add user message
  chatMessages.value.push({
    role: 'user',
    content: message
  })
  
  searchQuery.value = ''
  isLoading.value = true
  
  await nextTick()
  resizeWindow()
  scrollChatToBottom()

  try {
    // Prepare messages for API (include conversation history)
    const messages = chatMessages.value.map(msg => ({
      role: msg.role,
      content: msg.content
    }))

    // Start streaming chat
    await invoke('ai_chat_stream', { messages })
    
    // Add empty assistant message that will be filled by stream events
    chatMessages.value.push({
      role: 'assistant',
      content: ''
    })
    
    await nextTick()
    resizeWindow()
    scrollChatToBottom()
  } catch (error) {
    console.error('AI chat error:', error)
    chatMessages.value.push({
      role: 'assistant',
      content: `Error: ${error}`
    })
    isLoading.value = false
    await nextTick()
    resizeWindow()
    scrollChatToBottom()
  }
}

// Handle stream events from backend
const handleStreamEvent = (event: StreamEvent) => {
  const lastMessage = chatMessages.value[chatMessages.value.length - 1]
  
  switch (event.type) {
    case 'Content':
      if (lastMessage && lastMessage.role === 'assistant') {
        lastMessage.content += event.content
        scrollChatToBottom()
      }
      break
    case 'Done':
      isLoading.value = false
      scrollChatToBottom()
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
          content: `Error: ${event.message}`
        })
      }
      isLoading.value = false
      scrollChatToBottom()
      break
  }
}

const scrollChatToBottom = () => {
  aiChatMessagesRef.value?.scrollToBottom()
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
  // Keep chat history for AI mode, but clear on hide
  chatMessages.value = []
  isAiMode.value = false
  isLoading.value = false
  resizeWindow()
  await appWindow.hide()
  invoke("searcher_release")
}

const handleLoadMore = () => {
  invoke("searcher_find", { query: searchQuery.value });
}

watch(searchQuery, (newVal, _oldVal) => {
  if (!isAiMode.value && newVal != "") {
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

  // Listen for AI stream events
  unlistenAiStream = await listen<StreamEvent>('ai-stream', (event) => {
    handleStreamEvent(event.payload)
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
  if (unlistenAiStream) { unlistenAiStream() }
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
