import { invoke } from '@tauri-apps/api/core'
import type { SearchIndexStatus } from './types'

export function searcherFind(query: string) {
  return invoke<void>('searcher_find', { query })
}

export function searcherRelease() {
  return invoke<void>('searcher_release')
}

export function searcherIndexStatus() {
  return invoke<SearchIndexStatus>('searcher_index_status')
}

export function openFile(filePath: string) {
  return invoke<void>('open_file', { filePath })
}

export function openFileAsAdmin(filePath: string) {
  return invoke<void>('open_file_as_admin', { filePath })
}
