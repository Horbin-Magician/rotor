<template>
  <main class="container" 
        @mousedown="handleMouseDown"
        @mouseup="handleMouseUp">
    <div id="stage" ref="backImgRef"></div>
  </main>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from '@tauri-apps/api/window';
import Konva from "konva";

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

let backImgLayer: Konva.Layer | null = null

// // Load the screenshot
// invoke("capture_screen").then(async (imgBuf: any) => {
//   const width = window.screen.width * window.devicePixelRatio
//   const height = window.screen.height * window.devicePixelRatio;
//   const imgData = new ImageData(new Uint8ClampedArray(imgBuf), width, height);
//   backImg.value = await createImageBitmap(imgData)

//   backImgLayer = new Konva.Layer(); // then create layer
//   const konvaImage = new Konva.Image({
//     x: 0,
//     y: 0,
//     image: backImg.value,
//     width: window.innerWidth,
//     height: window.innerHeight,
//   });
//   backImgLayer.add(konvaImage);

//   var stage = new Konva.Stage({
//     container: 'stage', // id of container <div>
//     width: window.innerWidth,
//     height: window.innerHeight,
//   });
//   stage.add(backImgLayer); // add the layer to the stage

//   backImgURL.value = stage.toDataURL({ mimeType:"image/png" })
// })

// Mouse event handlers
function handleMouseDown(_event: MouseEvent) {
    const appWindow = getCurrentWindow()
    appWindow.startDragging()
}

// Mouse event handlers
function handleMouseUp(event: MouseEvent) {
    console.log('handleMouseUp'); // TODO del
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
.container {
  position: relative;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
}
</style>
