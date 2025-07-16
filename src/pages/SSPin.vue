<template>
  <main class="container" 
        @mousedown="handleMouseDown"
        @mouseup="handleMouseUp"
        @mousemove="handleMouseMove"
        @wheel="handleWheel">
    <div id="stage" ref="backImgRef"></div>
  </main>
  <div class="tips" v-if="show_tips">
    {{tips}}
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from '@tauri-apps/api/window';
import Konva from "konva";

enum State {
  Default,
  Moving
}

const appWindow = getCurrentWindow()
appWindow.isVisible().then( (visible)=>{
  if(visible == false) {
    appWindow.show()
    appWindow.setFocus()
  }
})

let state = State.Default

const backImg = ref()
const backImgRef = ref<HTMLImageElement | null>(null)
const backImgURL = ref()
let backImgLayer: Konva.Layer | null = null

const tips = ref("")
const show_tips = ref(false)

let zoom_scale = 100;

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
  state = State.Moving
}

// Mouse event handlers
function handleMouseUp(_event: MouseEvent) {
  state = State.Default;
}

// Mouse event handlers
function handleMouseMove(_event: MouseEvent) {

}

function handleWheel(event: WheelEvent){
  zoomWindow(event.deltaY)
}

function handleKeyup(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    appWindow.close()
  }
}

async function zoomWindow(wheel_delta: number) {
  let delta = wheel_delta > 0 ? -2 : 2 // TODO use setting
  zoom_scale += delta
  zoom_scale = Math.max(5, Math.min(zoom_scale, 500))
  tips.value = zoom_scale + "%"
  show_tips.value = true
  // TODO scale window
}

{ // Mount something
  onMounted(async () => {
    window.addEventListener('keyup', handleKeyup);
  });

  onBeforeUnmount(async () => {
    window.removeEventListener('keyup', handleKeyup)
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

.tips {
  position: fixed;
  left: 50%;
  top: 50%;
  padding: 2px 8px 2px 8px;
  transform: translate(-50%, -50%);
  background-color: black;
}
</style>
