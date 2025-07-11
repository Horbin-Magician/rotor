<template>
  <main class="container" 
        @mousedown="handleMouseDown" 
        @mousemove="handleMouseMove" 
        @mouseup="handleMouseUp">
    <div id="stage" ref="backImgRef"></div>
    
    <!-- Selection rectangle -->
    <div v-if="isSelecting" class="selection-rect" :style="selectionStyle"></div>
    
    <!-- Magnifier -->
    <div class="magnifier" v-if="backImgURL" :style="magnifierStyle">
      <div class="magnifier-content" :style="magnifierContentStyle">
        <div class="magnifier-crosshair"></div>
      </div>
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
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from '@tauri-apps/api/window';
import Konva from "konva";
import { emit } from "@tauri-apps/api/event";

const appWindow = getCurrentWindow()
const backImg = ref()
const backImgRef = ref<HTMLImageElement | null>(null)
const backImgURL = ref()

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

let backImgLayer: Konva.Layer | null = null
// Load the screenshot
invoke("capture_screen").then(async (imgBuf: any) => {
  const width = window.screen.width * window.devicePixelRatio
  const height = window.screen.height * window.devicePixelRatio;
  const imgData = new ImageData(new Uint8ClampedArray(imgBuf), width, height);
  backImg.value = await createImageBitmap(imgData)

  backImgLayer = new Konva.Layer(); // then create layer
  const konvaImage = new Konva.Image({
    x: 0,
    y: 0,
    image: backImg.value,
    width: window.innerWidth,
    height: window.innerHeight,
  });
  backImgLayer.add(konvaImage);

  var stage = new Konva.Stage({
    container: 'stage', // id of container <div>
    width: window.innerWidth,
    height: window.innerHeight,
  });
  stage.add(backImgLayer); // add the layer to the stage

  backImgURL.value = stage.toDataURL({ mimeType:"image/png" })
})

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

// Computed styles for magnifier
const magnifierStyle = computed(() => {
  const height = magnifierSize + 36
  // Avoid obscuring the selection point
  const offset = 20
  
  // Get viewport dimensions
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

// Computed styles for magnifier content (the zoomed area)
const magnifierContentStyle = computed(() => {
  if (!backImgRef.value) return {}
  
  // Calculate the position to center the zoomed area on the cursor
  const imgRect = backImgRef.value.getBoundingClientRect()
  const x = currentX.value - imgRect.left
  const y = currentY.value - imgRect.top
  
  return { // TODO fix error when mouse move to edge
    backgroundImage: `url(${backImgURL.value})`,
    backgroundPosition: `-${(x * zoomFactor) - (magnifierSize / 2)}px -${(y * zoomFactor) - (magnifierSize / 2)}px`,
    backgroundSize: `${imgRect.width * zoomFactor}px ${imgRect.height * zoomFactor}px`,
    width: `${magnifierSize}px`,
    height: `${magnifierSize}px`
  }
})

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
  
  getPixelColor(event.clientX, event.clientY) // Get color at cursor position
}

// Function to get the color of the pixel at the cursor position
function getPixelColor(x: number, y: number) {
  if (!backImgLayer) return

  const canvas = backImgLayer.getCanvas()
  const ctx = canvas.getContext();

  let imgX = x * window.devicePixelRatio
  let imgY = y * window.devicePixelRatio
  const pixelData = ctx.getImageData(imgX, imgY, 1, 1).data

  const hexColor = "#" + 
    Array.from(pixelData).slice(0, 3)
      .map(x => {
        const hex = x.toString(16);
        return hex.length === 1 ? "0" + hex : hex;
      })
      .join("");

  pixelColor.value = hexColor
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

function handleKeydown(event: KeyboardEvent) {
  console.log('全局按下了键：', event.key); // TODO del
  if (event.key === 'Escape') {
    appWindow.close()
  }
}

onMounted(async () => {
  window.addEventListener('keydown', handleKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown);
});

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

.back_img {
  height: 100%;
  width: 100%;
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

.magnifier-crosshair {
  position: relative;
  width: 100%;
  height: 100%;
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

#stage {
  position: absolute;
  top: 0px;
  left: 0px;
  width: 100%;
  height: 100%;
}
</style>
