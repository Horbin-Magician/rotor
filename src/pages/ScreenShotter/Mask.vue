<template>
  <main class="container" 
        @mousedown="handleMouseDown" 
        @mousemove="handleMouseMove" 
        @mouseup="handleMouseUp">
    <canvas ref="canvasRef" id="main-canvas"></canvas>
    
    <!-- Selection rectangle -->
    <div v-if="isSelecting" class="selection-rect" :style="selectionStyle"></div>
    
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
      </div>
    </div>
  </main>
</template>

<script setup lang="ts">

console.log("js begin: ", Date.now())

import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from '@tauri-apps/api/window';

const appWindow = getCurrentWindow()

const canvasRef = ref<HTMLCanvasElement | null>(null)
const magnifierCanvasRef = ref<HTMLCanvasElement | null>(null)
const backImgBitmap = ref<ImageBitmap | null>(null)

// Selection state
const isSelecting = ref(false)
const startX = ref(0)
const startY = ref(0)
const endX = ref(0)
const endY = ref(0)
const currentX = ref(0)
const currentY = ref(0)

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
  const left = Math.min(startX.value, endX.value)
  const top = Math.min(startY.value, endY.value)
  const width = Math.abs(endX.value - startX.value)
  const height = Math.abs(endY.value - startY.value)
  
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
  mainCtx = mainCanvas.getContext('2d', { 
    alpha: false,
    desynchronized: true // Better performance for frequent updates
  })
  
  if (!mainCtx) return
  
  // Set canvas size
  const dpr = window.devicePixelRatio || 1
  const width = window.innerWidth
  const height = window.innerHeight
  
  mainCanvas.width = width * dpr
  mainCanvas.height = height * dpr
  mainCanvas.style.width = `${width}px`
  mainCanvas.style.height = `${height}px`
  
  // Scale context for high DPI
  mainCtx.scale(dpr, dpr)
  
  // Optimize rendering
  mainCtx.imageSmoothingEnabled = false
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
  
  mainCtx.clearRect(0, 0, window.innerWidth, window.innerHeight)
  mainCtx.drawImage(
    backImgBitmap.value,
    0, 0,
    window.innerWidth, window.innerHeight
  )
}

// Computed styles for magnifier
const magnifierStyle = computed(() => {
  const height = magnifierSize + 36
  const offset = 20
  
  const viewportWidth = window.innerWidth
  const viewportHeight = window.innerHeight
  
  let left = (currentX.value + magnifierSize > viewportWidth) ? 
    Math.min(currentX.value, viewportWidth) - magnifierSize : Math.max(currentX.value, 0);
  let top  = ((currentY.value) + offset + height > viewportHeight) ? 
    Math.min((currentY.value) - offset, viewportHeight) - height : Math.max(currentY.value + offset, 0);
  
  return {
    left: `${left}px`,
    top: `${top}px`,
    width: `${magnifierSize}px`,
    height: `${height}px`
  }
})

// Update magnifier
function updateMagnifier(x: number, y: number) {
  if (!magnifierCtx || !backImgBitmap.value) return
  
  // Clear magnifier canvas
  magnifierCtx.clearRect(0, 0, magnifierSize, magnifierSize)
  
  // Calculate source area to magnify
  const sourceSize = magnifierSize / zoomFactor
  const sourceX = Math.max(0, Math.min(x - sourceSize / 2, window.innerWidth - sourceSize))
  const sourceY = Math.max(0, Math.min(y - sourceSize / 2, window.innerHeight - sourceSize))
  
  // Draw magnified area
  magnifierCtx.drawImage(
    backImgBitmap.value,
    sourceX * (backImgBitmap.value.width / window.innerWidth),
    sourceY * (backImgBitmap.value.height / window.innerHeight),
    sourceSize * (backImgBitmap.value.width / window.innerWidth),
    sourceSize * (backImgBitmap.value.height / window.innerHeight),
    0, 0,
    magnifierSize, magnifierSize
  )
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
    console.warn('Error sampling pixel color:', error)
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
  
  updateMagnifier(event.clientX, event.clientY)
  getPixelColor(event.clientX, event.clientY)
}

function handleMouseUp() {
  // Complete selection if it has a minimum size
  const width = Math.abs(endX.value - startX.value)
  const height = Math.abs(endY.value - startY.value)
  
  if (width > 5 && height > 5) {
    isSelecting.value = false
    const x = Math.min(startX.value, endX.value)
    const y = Math.min(startY.value, endY.value)
    const width = Math.abs(endX.value - startX.value)
    const height = Math.abs(endY.value - startY.value)
    invoke("new_pin", { x: x.toString(), y: y.toString(), width: width.toString(), height: height.toString() })
  } else {
    // Reset selection if it's too small
    isSelecting.value = false
    selectionWidth.value = 0
    selectionHeight.value = 0
  }
}

function handleKeyup(event: KeyboardEvent) {
  console.log('全局按下了键：', event.key); // TODO del
  if (event.key === 'Escape') {
    appWindow.close()
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
const width = window.screen.width * window.devicePixelRatio
const height = window.screen.height * window.devicePixelRatio

async function initializeScreenshot() {
  try {
    const imgBuf: any = await invoke("capture_screen")

    console.log("get imgBuf: ", Date.now())

    // Create image data and bitmap asynchronously
    const imgData = new ImageData(new Uint8ClampedArray(imgBuf), width, height)
    backImgBitmap.value = await createImageBitmap(imgData)

    // Draw the background image
    drawBackgroundImage()

    console.log("Show window: ", Date.now())

    // Show window
    const visible = await appWindow.isVisible()
    if(!visible) {
      appWindow.show()
      appWindow.setFocus()
    }
  } catch (err) {
    console.error("Failed to capture_screen", err)
    appWindow.close()
  }
}

// Initialize screenshot loading
initializeScreenshot()
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
  cursor: crosshair;
}

#main-canvas {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
}

.selection-rect {
  position: absolute;
  border: 1px solid #2196F3;
  background-color: rgba(33, 150, 243, 0.2);
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

/* TODO */
.magnifier-canvas {
  display: block;
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
