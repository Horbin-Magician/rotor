import { ref, type Ref } from 'vue'
import { warn } from '@tauri-apps/plugin-log'
import { openFile, openFileAsAdmin, searcherFind, searcherRelease } from '../api'
import type { SearchAction, SearchItem, UpdateResultPayload } from '../types'

const MAX_RETAINED_RESULTS = 100

function mapSearchItem(item: UpdateResultPayload[1][number]): SearchItem {
  const lowerFileName = item.file_name.toLowerCase()
  const isApp =
    lowerFileName.endsWith('.app') ||
    lowerFileName.endsWith('.exe') ||
    lowerFileName.endsWith('.lnk')

  return {
    title: item.file_name,
    subtitle: item.path,
    file_path: item.file_path,
    type: isApp ? 'app' : 'file',
    icon_data: item.icon_data,
    alias: item.alias,
    actions: [
      { type: 'OpenAsAdmin', title: 'message.openAsAdminTip' },
      { type: 'OpenFolder', title: 'message.openFolderTip' },
    ],
  }
}

export function useFileSearch(
  searchQuery: Ref<string>,
  onHide: () => void,
  onResize: () => void | Promise<void>,
) {
  const searchResults = ref<SearchItem[]>([])
  const selectedIndex = ref(0)

  const resetSearch = () => {
    selectedIndex.value = 0
    searchResults.value = []
  }

  const requestSearch = (query = searchQuery.value) => {
    if (query) {
      searcherFind(query).catch((err) => warn(`Search request failed: ${err}`))
    }
  }

  const releaseSearch = () => {
    searcherRelease().catch((err) => warn(`Search release failed: ${err}`))
  }

  const handleLoadMore = () => {
    if (searchResults.value.length >= MAX_RETAINED_RESULTS) return
    requestSearch()
  }

  const handleUpdateResult = async (payload: UpdateResultPayload) => {
    const [filename, getSearchResults, ifIncrease] = payload
    if (filename !== searchQuery.value) return

    if (!ifIncrease) {
      searchResults.value = []
    }

    const availableSlots = MAX_RETAINED_RESULTS - searchResults.value.length
    if (availableSlots <= 0) return

    searchResults.value = searchResults.value.concat(
      getSearchResults.slice(0, availableSlots).map(mapSearchItem),
    )
    await onResize()
  }

  const clickItem = (item: SearchItem) => {
    openFile(item.file_path).catch((err) => warn(`Failed to open file: ${err}`))
    onHide()
  }

  const handleActionClick = (action: SearchAction, item: SearchItem) => {
    switch (action.type) {
      case 'OpenAsAdmin':
        openFileAsAdmin(item.file_path).catch((err) => warn(`Failed to open as admin: ${err}`))
        break
      case 'OpenFolder':
        openFile(item.subtitle).catch((err) => warn(`Failed to open folder: ${err}`))
        break
      default:
        warn(`Unknown action type: ${action.type}`)
    }

    onHide()
  }

  return {
    searchResults,
    selectedIndex,
    resetSearch,
    requestSearch,
    releaseSearch,
    handleLoadMore,
    handleUpdateResult,
    clickItem,
    handleActionClick,
  }
}
