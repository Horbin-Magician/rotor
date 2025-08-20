<template>
  <main class="container" 
        @mousedown="handleMouseDown" 
        @mousemove="handleMouseMove" 
        @mouseup="handleMouseUp">
    <canvas ref="canvasRef" id="main-canvas"
      :style="{ width: windowWidth + 'px', height: windowHeight + 'px' }"
      :width="bacImgWidth"
      :height="bacImgHeight"
    />
    
    <!-- Selection rectangle -->
    <div class="selection-rect" :style="selectionStyle"></div>
    
    <!-- Magnifier -->
    <div class="magnifier" :style="magnifierStyle">
      <canvas ref="magnifierCanvasRef" class="magnifier-canvas" :width="magnifierSize" :height="magnifierSize"></canvas>
      <div class="magnifier-crosshair" :style="{ width: magnifierSize + 'px', height: magnifierSize + 'px' }"></div>
      <div class="magnifier-info">
        <!-- Rect size info -->
        <div class="magnifier-info-item"> {{ selectionWidth }} × {{ selectionHeight }} </div>
        <!-- Point color info -->
        <div class="magnifier-info-item">
          <div class="color-preview" :style="{ backgroundColor: pixelColor }"></div>
          <span>{{ pixelColor }}</span>
        </div>
        <div class="magnifier-info-item">
          <span>{{ $t('message.copyColorHint') }}</span>
        </div>
      </div>
    </div>
  </main>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from '@tauri-apps/api/window';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { listen } from '@tauri-apps/api/event';
import { warn } from '@tauri-apps/plugin-log';

const appWindow = getCurrentWindow()

const canvasRef = ref<HTMLCanvasElement | null>(null)
const magnifierCanvasRef = ref<HTMLCanvasElement | null>(null)
const backImgBitmap = ref<ImageBitmap | null>(null)

const windowWidth = window.screen.width
const windowHeight = window.screen.height
const bacImgWidth = windowWidth * window.devicePixelRatio
const bacImgHeight = windowHeight * window.devicePixelRatio

let rects: [number, number, number, number, number][] = [];

// Selection state
const isSelecting = ref(false)
const startX = ref(0)
const startY = ref(0)
const endX = ref(0)
const endY = ref(0)
const currentX = ref(-999)
const currentY = ref(-999)
const autoSelectRect = ref<{x: number, y: number, width: number, height: number} | null>(null)

// Magnifier state
const magnifierSize = 100
const zoomFactor = 4
const pixelColor = ref('#ffffff')
const selectionWidth = ref(0)
const selectionHeight = ref(0)

let mainCanvas: HTMLCanvasElement | null = null
let mainCtx: CanvasRenderingContext2D | null = null
let magnifierCanvas: HTMLCanvasElement | null = null
let magnifierCtx: CanvasRenderingContext2D | null = null

// Computed styles for selection rectangle
const selectionStyle = computed(() => {
  let left = -2, top = -2, width = 0, height = 0
  if (isSelecting.value == true) {
    width = Math.abs(endX.value - startX.value)
    height = Math.abs(endY.value - startY.value)
    if (width > 5 && height > 5) {
      left = Math.min(startX.value, endX.value)
      top = Math.min(startY.value, endY.value)
    } else {
      width = 0
      height = 0
    }
  } else if (autoSelectRect.value) {
    left = autoSelectRect.value.x
    top = autoSelectRect.value.y
    width = autoSelectRect.value.width
    height = autoSelectRect.value.height
  }

  return {
    left: `${left}px`,
    top: `${top}px`,
    width: `${width}px`,
    height: `${height}px`
  }
})

// Initialize canvas
function initializeCanvas() {
  if (!canvasRef.value) return
  
  mainCanvas = canvasRef.value
  mainCtx = mainCanvas.getContext('2d', { alpha: false })
  
  if (!mainCtx) return
  
  const dpr = window.devicePixelRatio
  mainCtx.scale(dpr, dpr) // Scale context for high DPI
  mainCtx.imageSmoothingEnabled = false // Optimize rendering
}

// Initialize magnifier canvas
function initializeMagnifierCanvas() {
  if (!magnifierCanvasRef.value) return
  
  magnifierCanvas = magnifierCanvasRef.value
  magnifierCtx = magnifierCanvas.getContext('2d', { alpha: false })
  
  if (!magnifierCtx) return
  
  magnifierCtx.imageSmoothingEnabled = false
}

// Draw background image
function drawBackgroundImage() {
  if (!mainCtx || !backImgBitmap.value) return
  
  mainCtx.clearRect(0, 0, window.screen.width, window.screen.height)
  mainCtx.drawImage(
    backImgBitmap.value,
    0, 0,
    window.screen.width, window.screen.height
  )
}

// Computed styles for magnifier
const mgnfHeight = magnifierSize + 50
const mgnfOffset = 20
const viewportWidth = window.screen.width
const viewportHeight = window.screen.height
const magnifierStyle = computed(() => {
  let left = (currentX.value + magnifierSize > viewportWidth) ? 
    currentX.value - magnifierSize : currentX.value;
  let top = (currentY.value + mgnfOffset + mgnfHeight > viewportHeight) ? 
    currentY.value - mgnfOffset - mgnfHeight : (currentY.value + mgnfOffset);

  return {
    left: `${left}px`,
    top: `${top}px`,
    width: `${magnifierSize}px`,
    height: `${mgnfHeight}px`
  }
})

// Update magnifier
const srcSize = magnifierSize / zoomFactor
function updateMagnifier(x: number, y: number) {
  if (!magnifierCtx || !backImgBitmap.value) return
  
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
    const destX = ( srcSize / 2 - (x - left) ) * zoomFactor
    const destY = ( srcSize / 2 - (y - top) ) * zoomFactor
    const destWidth = (right - left) * zoomFactor
    const destHeight = (bottom - top) * zoomFactor

    srcX = left * window.devicePixelRatio
    srcY = top * window.devicePixelRatio
    const srcWidth = (right - left) * window.devicePixelRatio
    const srcHeight = (bottom - top) * window.devicePixelRatio

    // Draw the intersected area
    magnifierCtx.drawImage(
      backImgBitmap.value,
      srcX, srcY, srcWidth, srcHeight,
      destX, destY, destWidth, destHeight
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

    const hexColor = "#" + 
      Array.from(pixelData).slice(0, 3)
        .map(x => {
          const hex = x.toString(16);
          return hex.length === 1 ? "0" + hex : hex;
        })
        .join("");

    pixelColor.value = hexColor
  } catch (error) {
    warn(`Error sampling pixel color: ${error}`)
  }
}

// Auto-selection functionality
async function updateAutoSelection(x: number, y: number) {
  const minRect = rects.reduce((min: [number, number, number, number, number] | undefined, rect) => {
    const [left, top, zindex, width, height] = rect;
    if (x > left && x < left + width && y > top && y < top + height) {
      if (!min) return rect;
      if (zindex >= 0 && min[2] != zindex) {
        return min[2] > zindex ? min : rect;
      } else {
        const minArea = min[3] * min[4];
        const rectArea = width * height;
        return minArea < rectArea ? min : rect;
      }
    }
    return min;
  }, undefined);

  // Only update if we got valid window bounds
  if (minRect) {
    autoSelectRect.value = {
      x: minRect[0],
      y: minRect[1],
      width: minRect[3],
      height: minRect[4]
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
}

function handleMouseMove(event: MouseEvent) {
  currentX.value = event.clientX
  currentY.value = event.clientY
  
  if (isSelecting.value) {
    endX.value = event.clientX
    endY.value = event.clientY
    
    // Update selection dimensions
    selectionWidth.value = Math.abs(endX.value - startX.value)
    selectionHeight.value = Math.abs(endY.value - startY.value)
  }
  if(isSelecting.value == false) {
    updateAutoSelection(event.clientX, event.clientY)
  }
  updateMagnifier(event.clientX, event.clientY)
  getPixelColor(event.clientX, event.clientY)
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
    invoke("new_pin", { offsetX: x.toString(), offsetY: y.toString(), width: width.toString(), height: height.toString() })
    hideWindow()
  } else if (autoSelectRect.value) {
    const x = autoSelectRect.value.x * scale_factor
    const y = autoSelectRect.value.y * scale_factor
    const width = autoSelectRect.value.width * scale_factor
    const height = autoSelectRect.value.height * scale_factor
    invoke("new_pin", { offsetX: x.toString(), offsetY: y.toString(), width: width.toString(), height: height.toString() })
    hideWindow()
  }
  else {
    // Reset selection if it's too small
    isSelecting.value = false
    selectionWidth.value = 0
    selectionHeight.value = 0
  }
}

function handleKeyup(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    invoke("close_cache_pin")
    hideWindow()
  } else if (event.key.toLowerCase() === 'c') {
    writeText(pixelColor.value)
  }
}

onMounted(async () => {
  window.addEventListener('keyup', handleKeyup);
  initializeCanvas()
  initializeMagnifierCanvas()
});

onBeforeUnmount(() => {
  window.removeEventListener('keyup', handleKeyup);
});

// Load the screenshot
async function initializeScreenshot() {
  const imgBuf: any = await invoke("get_screen_img", {label: appWindow.label})
  // Create image data and bitmap asynchronously
  const imgData = new ImageData(new Uint8ClampedArray(imgBuf), bacImgWidth, bacImgHeight)
  backImgBitmap.value = await createImageBitmap(imgData)
  // Draw the background image
  drawBackgroundImage()
}

function hideWindow() {
  appWindow.hide()
  
  if (mainCtx) { mainCtx.clearRect(0, 0, windowWidth, windowHeight) }
  if (magnifierCtx) { magnifierCtx.clearRect(0, 0, magnifierSize, magnifierSize) }
  
  if (backImgBitmap.value) {
    backImgBitmap.value.close()
    backImgBitmap.value = null
  }

  isSelecting.value = false
  selectionWidth.value = 0
  selectionHeight.value = 0
  startX.value = 0
  startY.value = 0
  endX.value = 0
  endY.value = 0
  currentX.value = -999
  currentY.value = -999
  autoSelectRect.value = null
  pixelColor.value = '#ffffff'
  rects = []
}

// Load the rects
async function initializeAutoRects() {
  rects = await invoke("get_screen_rects", {label: appWindow.label})
}

{ // Mount something
  onMounted(async () => {
    listen('show-mask', async (_event) => {
      initializeAutoRects()
      await initializeScreenshot()
      // Show window
      appWindow.isVisible().then( (visible)=>{
        if(visible == false) {
          appWindow.show()
          appWindow.setFocus()
        }
      })
    });
  });
}
</script>

<style scoped>
html, body {
  background-color: transparent !important;
}

.container {
  position: relative;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
}

#main-canvas {
  position: absolute;
  top: 0;
  left: 0;
}

.selection-rect {
  position: absolute;
  border: 1px solid #2196F3;
  background-color: rgba(33, 150, 243, 0.1);
  pointer-events: none;
}

.magnifier {
  position: absolute;
  border: 1px solid #ffffff;
  overflow: hidden;
  pointer-events: none;
  z-index: 1000;
  background-color: black;
  display: flex;
  flex-direction: column;
}

.magnifier-crosshair {
  position: absolute;
  top: 0;
  left: 0;
  pointer-events: none;
  z-index: 1001;
  border-bottom: 1px solid #ffffff;
}

.magnifier-crosshair::before,
.magnifier-crosshair::after {
  content: '';
  position: absolute;
  background-color: #2196F3;
}

.magnifier-crosshair::before {
  top: 50%;
  left: 0;
  width: 100%;
  height: 1px;
}

.magnifier-crosshair::after {
  top: 0;
  left: 50%;
  width: 1px;
  height: 100%;
}

.magnifier-info {
  padding: 4px;
  color: white;
  font-size: 12px;
  display: flex;
  flex-direction: column;
}

.magnifier-info-item {
  width: 100%;
  height: 14px;
  align-items: center;
  text-align: center;
  display: flex;
  justify-content: center;
}

.color-preview {
  width: 10px;
  height: 10px;
  margin-right: 6px;
  display: inline-block;
}
</style>
