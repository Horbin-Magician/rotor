import { invoke } from '@tauri-apps/api/core'
import { debug as logDebug } from '@tauri-apps/plugin-log'

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
  image_rect?: [number, number, number, number] | null
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

export function isScreenshotSessionCurrent(sessionId: number) {
  return invoke<boolean>('is_screenshot_session_current', { sessionId })
}

export function finishScreenshotSession() {
  return invoke<void>('finish_screenshot_session')
}

export function cancelScreenshotSession() {
  return invoke<void>('cancel_screenshot_session')
}

export function newCachePin() {
  return invoke<void>('new_cache_pin')
}

export function closeCachePin() {
  return invoke<void>('close_cache_pin')
}

export function clearScreenshotCache() {
  return invoke<void>('clear_screenshot_cache')
}

export function newPin(input: NewPinInput) {
  return invoke<void>('new_pin', {
    offsetX: input.offsetX,
    offsetY: input.offsetY,
    width: input.width,
    height: input.height,
  })
}

export function getScreenRects(label: string) {
  return invoke<ScreenRect[]>('get_screen_rects', { label })
}

const SHARED_BUFFER_TIMEOUT_MS = 5000

let sharedBufferDisabled = false

function getScreenshotDataShared(_label: string): Promise<ArrayBuffer> {
  const webview = window.chrome?.webview
  if (!webview) {
    return Promise.reject(new Error('shared-buffer-unsupported'))
  }

  const requestId = crypto.randomUUID()

  return new Promise<ArrayBuffer>((resolve, reject) => {
    let settled = false

    const cleanup = () => {
      webview.removeEventListener('sharedbufferreceived', handler)
      clearTimeout(timer)
    }

    const handler = (event: WebView2SharedBufferReceivedEvent) => {
      const meta = event.additionalData as { requestId?: string; length?: number } | undefined
      if (!meta || meta.requestId !== requestId) {
        return
      }

      const buffer = event.getBuffer()
      const copy = new Uint8Array(buffer).slice().buffer
      webview.releaseBuffer(buffer)

      if (settled) return
      settled = true
      cleanup()
      resolve(copy)
    }

    const timer = setTimeout(() => {
      if (settled) return
      settled = true
      cleanup()
      reject(new Error('shared-buffer-timeout'))
    }, SHARED_BUFFER_TIMEOUT_MS)

    webview.addEventListener('sharedbufferreceived', handler)

    invoke<void>('get_screenshot_data_shared', { requestId }).catch((error) => {
      if (settled) return
      settled = true
      cleanup()
      reject(error instanceof Error ? error : new Error(String(error)))
    })
  })
}

export async function getScreenshotData(label: string): Promise<ArrayBuffer> {
  const start = performance.now()
  if (!sharedBufferDisabled) {
    try {
      const data = await getScreenshotDataShared(label)
      logDebug(
        `[screenshot] shared buffer transfer ${(performance.now() - start).toFixed(1)}ms (${data.byteLength} bytes)`,
      )
      return data
    } catch (error) {
      sharedBufferDisabled = true
      logDebug(`[screenshot] shared buffer unavailable, fallback to ipc: ${error}`)
    }
  }
  const fallbackStart = performance.now()
  const data = await invoke<ArrayBuffer | number[]>('get_screenshot_data', { label })
  const buffer = data instanceof ArrayBuffer ? data : Uint8Array.from(data).buffer
  logDebug(
    `[screenshot] ipc fallback transfer ${(performance.now() - fallbackStart).toFixed(1)}ms (${buffer.byteLength} bytes)`,
  )
  return buffer
}

export function getPinState(id: number) {
  return invoke<PinConfig | null>('get_pin_state', { id })
}

export function updatePinState(id: number, x: number, y: number, zoom: number, minimized: boolean) {
  return invoke<void>('update_pin_state', { id, x, y, zoom, minimized })
}

export function updatePinSelection(
  id: number,
  rectX: number,
  rectY: number,
  width: number,
  height: number,
  windowX: number,
  windowY: number,
  zoom: number,
  minimized: boolean,
) {
  return invoke<void>('update_pin_selection', {
    selection: {
      id,
      rectX,
      rectY,
      width,
      height,
      windowX,
      windowY,
      zoom,
      minimized,
    },
  })
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
