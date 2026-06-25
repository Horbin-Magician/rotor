<template>
  <main
    class="container"
    @mousedown="handleMouseDown"
    @mousemove="handleMouseMove"
    @mouseout="handleMouseOut"
    @mouseup="handleMouseUp"
  >
    <ScreenCanvas
      :window-width="windowWidth"
      :window-height="windowHeight"
      :bac-img-width="bacImgWidth"
      :bac-img-height="bacImgHeight"
      @canvas-ready="handleCanvasReady"
    />

    <SelectionRect
      :is-selecting="isSelecting"
      :start-x="startX"
      :start-y="startY"
      :end-x="endX"
      :end-y="endY"
      :auto-select-rect="autoSelectRect"
      :is-window-focused="isWindowFocused"
    />

    <Magnifier
      :current-x="currentX"
      :current-y="currentY"
      :magnifier-size="magnifierSize"
      :selection-width="selectionWidth"
      :selection-height="selectionHeight"
      :pixel-color="pixelColor"
      :is-window-focused="isWindowFocused"
      @canvas-ready="handleMagnifierCanvasReady"
    />
  </main>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { cursorPosition, getCurrentWindow } from '@tauri-apps/api/window'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { info, warn } from '@tauri-apps/plugin-log'
import { connectDataSocket, requestDataSocketBytes } from '../shared/api/dataSocket'
import {
  cancelScreenshotSession,
  changeCurrentMask as focusCurrentMask,
  finishScreenshotSession,
  getScreenRects,
  isScreenshotSessionCurrent,
  newCachePin,
  newPin,
  type ScreenRect,
} from '../features/screenshot/api'

import ScreenCanvas from '../components/screenShotter/mask/ScreenCanvas.vue'
import SelectionRect from '../components/screenShotter/mask/SelectionRect.vue'
import Magnifier from '../components/screenShotter/mask/Magnifier.vue'

const appWindow = getCurrentWindow()

let ws: WebSocket | null = null
let backImgBitmap: ImageBitmap | null = null
let unlistenShowMask: UnlistenFn | null = null
let unlistenHideMask: UnlistenFn | null = null
let screenshotRequestId = 0

type HideWindowOptions = {
  requestId?: number
}

async function initWebSocket() {
  try {
    ws?.close()
    ws = null
    ws = await connectDataSocket()
    info('WebSocket connection established')
  } catch (error) {
    warn(`WebSocket connection error: ${error}`)
    throw error
  }
}

const windowWidth = window.screen.width
const windowHeight = window.screen.height
const bacImgWidth = windowWidth * window.devicePixelRatio
const bacImgHeight = windowHeight * window.devicePixelRatio
const hiddenPointerPosition = -9999

let rects: ScreenRect[] = []

// Selection state
const isSelecting = ref(false)
const startX = ref(0)
const startY = ref(0)
const endX = ref(0)
const endY = ref(0)
const currentX = ref(hiddenPointerPosition)
const currentY = ref(hiddenPointerPosition)
const autoSelectRect = ref<{ x: number; y: number; width: number; height: number } | null>(null)

// Magnifier state
const magnifierSize = 100
const zoomFactor = 4
const pixelColor = ref('#ffffff')
const selectionWidth = ref(0)
const selectionHeight = ref(0)
const isWindowFocused = ref(true)

let mainCanvas: HTMLCanvasElement | null = null
let mainCtx: CanvasRenderingContext2D | null = null
let magnifierCtx: CanvasRenderingContext2D | null = null

// Handle canvas ready events
function handleCanvasReady(canvas: HTMLCanvasElement, ctx: CanvasRenderingContext2D) {
  mainCanvas = canvas
  mainCtx = ctx
}

function handleMagnifierCanvasReady(_canvas: HTMLCanvasElement, ctx: CanvasRenderingContext2D) {
  // magnifierCanvas = canvas
  magnifierCtx = ctx
}

// Draw background image
function drawBackgroundImage() {
  if (!mainCtx || !backImgBitmap) return

  mainCtx.clearRect(0, 0, window.screen.width, window.screen.height)
  mainCtx.drawImage(backImgBitmap, 0, 0, window.screen.width, window.screen.height)
}

// Update magnifier
const srcSize = magnifierSize / zoomFactor
function updateMagnifier(x: number, y: number) {
  if (!magnifierCtx || !backImgBitmap) return

  // Clear magnifier canvas
  magnifierCtx.clearRect(0, 0, magnifierSize, magnifierSize)

  // Calculate source area to magnify (centered on mouse position)
  let srcX = x - srcSize / 2
  let srcY = y - srcSize / 2

  // Find the intersection of source area with viewport
  const left = Math.max(srcX, 0)
  const top = Math.max(srcY, 0)
  const right = Math.min(srcX + srcSize, window.screen.width)
  const bottom = Math.min(srcY + srcSize, window.screen.height)

  // Only draw if there's a valid intersection
  if (left < right && top < bottom) {
    // Calculate destination coordinates in magnifier canvas
    const destX = (srcSize / 2 - (x - left)) * zoomFactor
    const destY = (srcSize / 2 - (y - top)) * zoomFactor
    const destWidth = (right - left) * zoomFactor
    const destHeight = (bottom - top) * zoomFactor

    srcX = left * window.devicePixelRatio
    srcY = top * window.devicePixelRatio
    const srcWidth = (right - left) * window.devicePixelRatio
    const srcHeight = (bottom - top) * window.devicePixelRatio

    // Draw the intersected area
    magnifierCtx.drawImage(
      backImgBitmap,
      srcX,
      srcY,
      srcWidth,
      srcHeight,
      destX,
      destY,
      destWidth,
      destHeight,
    )
  }
}

// Get pixel color at position
function getPixelColor(x: number, y: number) {
  if (!mainCtx) return

  try {
    const dpr = window.devicePixelRatio || 1
    const imgX = Math.floor(x * dpr)
    const imgY = Math.floor(y * dpr)

    // Bounds checking
    if (imgX < 0 || imgY < 0 || imgX >= mainCanvas!.width || imgY >= mainCanvas!.height) {
      return
    }

    const pixelData = mainCtx.getImageData(imgX, imgY, 1, 1).data

    const hexColor =
      '#' +
      Array.from(pixelData)
        .slice(0, 3)
        .map((x) => {
          const hex = x.toString(16)
          return hex.length === 1 ? '0' + hex : hex
        })
        .join('')

    pixelColor.value = hexColor
  } catch (error) {
    warn(`Error sampling pixel color: ${error}`)
  }
}

function updateAutoSelection(x: number, y: number) {
  const minRect = rects.reduce((min: ScreenRect | undefined, rect) => {
    const [left, top, zindex, width, height] = rect
    if (x > left && x < left + width && y > top && y < top + height) {
      if (!min) return rect
      if (zindex >= 0 && min[2] != zindex) {
        return min[2] > zindex ? min : rect
      } else {
        const minArea = min[3] * min[4]
        const rectArea = width * height
        return minArea < rectArea ? min : rect
      }
    }
    return min
  }, undefined)

  // Only update if we got valid window bounds
  if (minRect) {
    autoSelectRect.value = {
      x: minRect[0],
      y: minRect[1],
      width: minRect[3],
      height: minRect[4],
    }
    // Update selection dimensions for display
    selectionWidth.value = minRect[3]
    selectionHeight.value = minRect[4]
  } else {
    autoSelectRect.value = null
    selectionWidth.value = 0
    selectionHeight.value = 0
  }
}

function applyPointerPosition(x: number, y: number) {
  currentX.value = x
  currentY.value = y

  if (isSelecting.value) {
    endX.value = x
    endY.value = y

    // Update selection dimensions
    selectionWidth.value = Math.abs(endX.value - startX.value)
    selectionHeight.value = Math.abs(endY.value - startY.value)
  }
  if (!isSelecting.value) {
    updateAutoSelection(x, y)
  }
  updateMagnifier(x, y)
  getPixelColor(x, y)
}

async function syncCursorPosition() {
  try {
    const [cursor, windowPosition, scaleFactor] = await Promise.all([
      cursorPosition(),
      appWindow.outerPosition(),
      appWindow.scaleFactor(),
    ])
    const x = (cursor.x - windowPosition.x) / scaleFactor
    const y = (cursor.y - windowPosition.y) / scaleFactor

    if (x < 0 || y < 0 || x > windowWidth || y > windowHeight) {
      return false
    }

    applyPointerPosition(x, y)
    return true
  } catch (error) {
    warn(`Failed to sync cursor position: ${error}`)
    return false
  }
}

// Mouse event handlers
function handleMouseDown(event: MouseEvent) {
  // Start selection
  isSelecting.value = true
  startX.value = event.clientX
  startY.value = event.clientY
  endX.value = event.clientX
  endY.value = event.clientY

  // Initialize selection dimensions
  selectionWidth.value = 0
  selectionHeight.value = 0

  void newCachePin()
}

function handleMouseMove(event: MouseEvent) {
  applyPointerPosition(event.clientX, event.clientY)
}

function handleMouseOut(_event: MouseEvent) {
  void focusCurrentMask()
}

async function handleMouseUp() {
  // Complete selection if it has a minimum size
  const width = Math.abs(endX.value - startX.value)
  const height = Math.abs(endY.value - startY.value)
  const scale_factor = await appWindow.scaleFactor()

  if (width > 5 && height > 5) {
    isSelecting.value = false
    const x = Math.min(startX.value, endX.value) * scale_factor
    const y = Math.min(startY.value, endY.value) * scale_factor
    const width = Math.abs(endX.value - startX.value) * scale_factor
    const height = Math.abs(endY.value - startY.value) * scale_factor
    void newPin({ offsetX: x, offsetY: y, width, height })
    finishScreenshotMask()
  } else if (autoSelectRect.value) {
    const x = autoSelectRect.value.x * scale_factor
    const y = autoSelectRect.value.y * scale_factor
    const width = autoSelectRect.value.width * scale_factor
    const height = autoSelectRect.value.height * scale_factor
    void newPin({ offsetX: x, offsetY: y, width, height })
    finishScreenshotMask()
  } else {
    // Reset selection if it's too small
    isSelecting.value = false
    selectionWidth.value = 0
    selectionHeight.value = 0
  }
}

function handleKeyup(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    cancelScreenshotMask()
  } else if (event.key.toLowerCase() === 'c') {
    void writeText(pixelColor.value)
  }
}

function handleWindowFocus() {
  isWindowFocused.value = true
}

function handleWindowBlur() {
  isWindowFocused.value = false
}

onMounted(async () => {
  window.addEventListener('keyup', handleKeyup)
  window.addEventListener('focus', handleWindowFocus)
  window.addEventListener('blur', handleWindowBlur)
})

onBeforeUnmount(() => {
  window.removeEventListener('keyup', handleKeyup)
  window.removeEventListener('focus', handleWindowFocus)
  window.removeEventListener('blur', handleWindowBlur)
  if (unlistenShowMask) {
    unlistenShowMask()
    unlistenShowMask = null
  }
  if (unlistenHideMask) {
    unlistenHideMask()
    unlistenHideMask = null
  }
  ws?.close()
  ws = null
})

// Load the screenshot
function isCurrentScreenshotRequest(requestId: number) {
  return requestId === screenshotRequestId
}

function getEventSessionId(payload: unknown) {
  return typeof payload === 'number' && Number.isFinite(payload) ? payload : null
}

async function isActiveScreenshotRequest(requestId: number) {
  if (!isCurrentScreenshotRequest(requestId)) return false

  try {
    return await isScreenshotSessionCurrent(requestId)
  } catch (error) {
    warn(`Failed to verify screenshot session ${requestId}: ${error}`)
    return false
  }
}

async function ensureActiveScreenshotRequest(requestId: number) {
  if (await isActiveScreenshotRequest(requestId)) return true

  if (!isCurrentScreenshotRequest(requestId)) return false

  try {
    await appWindow.hide()
  } catch (error) {
    warn(`Failed to hide stale mask window ${appWindow.label}: ${error}`)
  }
  return false
}

async function initializeScreenshot(requestId: number, imgBuf: ArrayBuffer) {
  if (imgBuf.byteLength === 0) {
    throw new Error(`No image data returned for mask ${appWindow.label}`)
  }

  const imgData = new ImageData(new Uint8ClampedArray(imgBuf), bacImgWidth, bacImgHeight)
  const bitmap = await createImageBitmap(imgData)
  if (!(await ensureActiveScreenshotRequest(requestId))) {
    bitmap.close()
    return false
  }

  backImgBitmap = bitmap

  // Draw the background image
  requestAnimationFrame(() => {
    if (isCurrentScreenshotRequest(requestId)) {
      drawBackgroundImage()
    }
  })

  if (!(await ensureActiveScreenshotRequest(requestId))) return false
  await appWindow.show()
  if (!(await ensureActiveScreenshotRequest(requestId))) return false

  const isCursorInCurrentWindow = await syncCursorPosition()
  if (!(await ensureActiveScreenshotRequest(requestId))) return false

  if (isCursorInCurrentWindow) {
    await appWindow.setFocus()
  }
  if (!(await ensureActiveScreenshotRequest(requestId))) return false

  await focusCurrentMask() // Focus the current mask window
  return ensureActiveScreenshotRequest(requestId)
}

function hideWindow({ requestId }: HideWindowOptions = {}) {
  if (requestId) {
    screenshotRequestId = Math.max(screenshotRequestId, requestId)
  } else {
    screenshotRequestId += 1
  }
  ws?.close()
  ws = null

  void appWindow.hide()

  if (mainCtx) {
    mainCtx.clearRect(0, 0, windowWidth, windowHeight)
  }
  if (magnifierCtx) {
    magnifierCtx.clearRect(0, 0, magnifierSize, magnifierSize)
  }

  if (backImgBitmap) {
    backImgBitmap.close()
    backImgBitmap = null
  }

  isSelecting.value = false
  selectionWidth.value = 0
  selectionHeight.value = 0
  startX.value = 0
  startY.value = 0
  endX.value = 0
  endY.value = 0
  currentX.value = hiddenPointerPosition
  currentY.value = hiddenPointerPosition
  autoSelectRect.value = null
  pixelColor.value = '#ffffff'
  rects = []
}

function finishScreenshotMask() {
  endScreenshotMask(finishScreenshotSession)
}

function cancelScreenshotMask() {
  endScreenshotMask(cancelScreenshotSession)
}

function endScreenshotMask(endSession: () => Promise<void>) {
  hideWindow({ requestId: screenshotRequestId + 1 })
  endSession().catch((error) => {
    warn(`Failed to end screenshot session: ${error}`)
  })
}

// Load the rects
async function initializeAutoRects(requestId: number) {
  const nextRects = await getScreenRects(appWindow.label)
  if (await isActiveScreenshotRequest(requestId)) {
    rects = nextRects
  }
}

onMounted(async () => {
  appWindow.setSimpleFullscreen(true) // Enable simple fullscreen mode

  unlistenShowMask = await listen<number>('show-mask', async (event) => {
    const requestId = getEventSessionId(event.payload) ?? screenshotRequestId + 1
    if (requestId < screenshotRequestId) return
    screenshotRequestId = requestId

    try {
      await initWebSocket() // Initialize WebSocket connection
      const rectsPromise = initializeAutoRects(requestId).catch((error) => {
        if (requestId === screenshotRequestId) {
          warn(`Failed to initialize screenshot rects: ${error}`)
        }
      })
      if (!ws) {
        throw new Error('WebSocket did not initialize')
      }
      const imgBuf = await requestDataSocketBytes(ws, appWindow.label)
      if (!(await ensureActiveScreenshotRequest(requestId))) return
      const initialized = await initializeScreenshot(requestId, imgBuf)
      if (!initialized) return
      await rectsPromise
    } catch (error) {
      if (await isActiveScreenshotRequest(requestId)) {
        warn(`Failed to initialize screenshot websocket: ${error}`)
        cancelScreenshotMask()
      }
    }
  })

  unlistenHideMask = await listen<number>('hide-mask', async (event) => {
    const requestId = getEventSessionId(event.payload)
    if (requestId && requestId <= screenshotRequestId) return
    hideWindow({ requestId: requestId ?? undefined })
  })
})
</script>

<style>
/* Global styles for body element */
body {
  background-color: transparent !important;
}
</style>

<style scoped>
.container {
  position: relative;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
}
</style>
