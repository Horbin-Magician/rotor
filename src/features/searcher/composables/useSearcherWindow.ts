import { LogicalSize, PhysicalPosition, getCurrentWindow, primaryMonitor } from '@tauri-apps/api/window'
import type { Ref } from 'vue'

const WINDOW_CONFIG = {
  width: 500,
  itemHeight: 60,
  inputHeight: 50,
  maxVisibleItems: 7,
  aiHeight: 420,
} as const

interface SearcherWindowState {
  isAiMode: Ref<boolean>
  chatCount: Ref<number>
  isLoading: Ref<boolean>
  searchQuery: Ref<string>
  resultCount: Ref<number>
}

export function useSearcherWindow(state: SearcherWindowState) {
  const appWindow = getCurrentWindow()

  const resizeWindow = async () => {
    const currentSize = await appWindow.outerSize()
    let newHeight = WINDOW_CONFIG.inputHeight

    if (state.isAiMode.value) {
      if (state.chatCount.value > 0 || state.isLoading.value) {
        newHeight += WINDOW_CONFIG.aiHeight
      }
    } else if (state.searchQuery.value.trim() && state.resultCount.value > 0) {
      const visibleItems = Math.min(state.resultCount.value, WINDOW_CONFIG.maxVisibleItems)
      newHeight += visibleItems * WINDOW_CONFIG.itemHeight
    }

    if (currentSize.height !== newHeight) {
      await appWindow.setSize(new LogicalSize(WINDOW_CONFIG.width, newHeight))
    }

    const monitor = await primaryMonitor()
    if (!monitor) return

    const scale = await appWindow.scaleFactor()
    const centerX = monitor.position.x + (monitor.size.width - scale * WINDOW_CONFIG.width) / 2
    const centerY = Math.ceil(monitor.position.y + monitor.size.height * 0.3)

    await appWindow.setPosition(new PhysicalPosition(centerX, centerY))
  }

  return {
    appWindow,
    resizeWindow,
  }
}
