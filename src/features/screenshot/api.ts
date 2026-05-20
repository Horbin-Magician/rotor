import { invoke } from '@tauri-apps/api/core'

export type ScreenRect = [number, number, number, number, number]

export interface NewPinInput {
  offsetX: number
  offsetY: number
  width: number
  height: number
}

export interface PinConfig {
  monitor_pos: [number, number]
  monitor_size: [number, number]
  rect: [number, number, number, number]
  offset: [number, number]
  zoom_factor: number
  mask_label: string
  minimized: boolean
}

export interface TextResult {
  left: number
  top: number
  width: number
  height: number
  text: string
}

export function changeCurrentMask() {
  return invoke<void>('change_current_mask')
}

export function newCachePin() {
  return invoke<void>('new_cache_pin')
}

export function closeCachePin() {
  return invoke<void>('close_cache_pin')
}

export function newPin(input: NewPinInput) {
  return invoke<void>('new_pin', {
    offsetX: input.offsetX.toString(),
    offsetY: input.offsetY.toString(),
    width: input.width.toString(),
    height: input.height.toString(),
  })
}

export function getScreenRects(label: string) {
  return invoke<ScreenRect[]>('get_screen_rects', { label })
}

export function getPinState(id: number) {
  return invoke<PinConfig | null>('get_pin_state', { id })
}

export function updatePinState(id: number, x: number, y: number, zoom: number, minimized: boolean) {
  return invoke<void>('update_pin_state', { id, x, y, zoom, minimized })
}

export function deletePinRecord(id: number) {
  return invoke<void>('delete_pin_record', { id })
}

export function saveImage(imgBuf: ArrayBuffer) {
  return invoke<boolean>('save_img', { imgBuf })
}

export function imageToText(imgBuf: ArrayBuffer) {
  return invoke<TextResult[]>('img2text', { imgBuf })
}
