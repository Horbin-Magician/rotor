<template>
  <main class="container" 
        @mousedown="handleMouseDown"
        @mousemove="handleMouseMove"
        @mouseup="handleMouseUp">
    <div id="stage" ref="backImgRef"></div>
  </main>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, getAllWindows, PhysicalPosition } from '@tauri-apps/api/window';
import Konva from "konva";

enum State {
  Default,
  Moving
}

const appWindow = getCurrentWindow()

let state = State.Default
const backImg = ref()
const backImgRef = ref<HTMLImageElement | null>(null)
const backImgURL = ref()
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
async function handleMouseDown(_event: MouseEvent) {
  appWindow.startDragging();
  state = State.Moving;
}

// Mouse event handlers
function handleMouseUp(_event: MouseEvent) {
  state = State.Default;
}

// Mouse event handlers
function handleMouseMove(_event: MouseEvent) {

}

function handleKeyup(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    appWindow.close()
  }
}

{ // Mount something
  onMounted(async () => {
    window.addEventListener('keyup', handleKeyup);
  });

  onBeforeUnmount(() => {
    window.removeEventListener('keyup', handleKeyup);
  })
}
</script>

<style scoped>
.container {
  position: relative;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
}
</style>
