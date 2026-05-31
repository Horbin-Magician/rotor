import { invoke } from '@tauri-apps/api/core'

export type AppConfig = Record<string, string>

export interface ShortcutRegistrationNotice {
  key: string
  shortcut: string
  message: string
}

export function getAllConfig() {
  return invoke<AppConfig>('get_all_cfg')
}

export function getConfig(key: string) {
  return invoke<string>('get_cfg', { k: key })
}

export function setConfig(key: string, value: string) {
  return invoke<void>('set_cfg', { k: key, v: value })
}

export function takeShortcutRegistrationNotices() {
  return invoke<ShortcutRegistrationNotice[]>('take_shortcut_registration_notices')
}

export function getAppVersion() {
  return invoke<string>('get_app_version')
}

export function getWsPort() {
  return invoke<number>('get_ws_port')
}

export function openUrl(url: string) {
  return invoke<void>('open_url', { url })
}
