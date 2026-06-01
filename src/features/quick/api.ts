import { invoke } from '@tauri-apps/api/core'
import type { QuickAction } from './types'

export function getQuickActions() {
  return invoke<QuickAction[]>('get_quick_actions')
}

export function setQuickActions(actions: QuickAction[]) {
  return invoke<void>('set_quick_actions', { actions })
}

export function runQuickAction(id: string) {
  return invoke<void>('run_quick_action', { id })
}
