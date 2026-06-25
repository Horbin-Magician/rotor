import {
  LogicalSize,
  PhysicalPosition,
  getCurrentWindow,
  primaryMonitor,
} from '@tauri-apps/api/window'
import type { Ref } from 'vue'

const WINDOW_CONFIG = {
  width: 500,
  itemHeight: 60,
  inputHeight: 50,
  maxVisibleItems: 7,
} as const

interface SearcherWindowState {
  searchQuery: Ref<string>
  resultCount: Ref<number>
}

export function useSearcherWindow(state: SearcherWindowState) {
  const appWindow = getCurrentWindow()

  const resizeWindow = async () => {
    let newHeight = WINDOW_CONFIG.inputHeight

    if (state.searchQuery.value.trim() && state.resultCount.value > 0) {
      const visibleItems = Math.min(state.resultCount.value, WINDOW_CONFIG.maxVisibleItems)
      newHeight += visibleItems * WINDOW_CONFIG.itemHeight
    }

    const monitor = await primaryMonitor()
    if (!monitor) return

    const scale = monitor.scaleFactor
    const centerX = monitor.position.x + (monitor.size.width - scale * WINDOW_CONFIG.width) / 2
    const centerY = Math.ceil(monitor.position.y + monitor.size.height * 0.3)

    await appWindow.setPosition(new PhysicalPosition(centerX, centerY))

    const currentSize = await appWindow.outerSize()
    const currentLogicalWidth = Math.round(currentSize.width / scale)
    const currentLogicalHeight = Math.round(currentSize.height / scale)

    if (currentLogicalWidth !== WINDOW_CONFIG.width || currentLogicalHeight !== newHeight) {
      await appWindow.setSize(new LogicalSize(WINDOW_CONFIG.width, newHeight))
    }
  }

  return {
    appWindow,
    resizeWindow,
  }
}
