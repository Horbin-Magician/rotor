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
  <div class="toolbar" :class="{ 'toolbar-hidden': !toolbarVisible }">
    <div class="toolbar-item" @click="minimizeWindow">
      <n-icon size="20" color="#007bff">
        <MinusFilled />
      </n-icon>
    </div>
    <div class="toolbar-item" @click="saveImage">
      <n-icon size="20" color="#007bff">
        <SaveAltFilled />
      </n-icon>
    </div>
    <div class="toolbar-item" @click="closeWindow">
      <n-icon size="20" color="#007bff">
        <CloseFilled />
      </n-icon>
    </div>
    <div class="toolbar-item">
      <n-icon size="20" color="#007bff">
        <ContentCopyRound />
      </n-icon>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { CloseFilled, SaveAltFilled, ContentCopyRound, MinusFilled } from '@vicons/material';
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from '@tauri-apps/api/window';
import Konva from "konva";

enum State {
  Default,
  Moving,
  Drawing
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
const toolbarVisible = ref(true);
let hideToolbarTimer: number | null = null;

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

function minimizeWindow() {
  appWindow.minimize();
}

function saveImage() {
  // TODO: Implement save functionality
}

function closeWindow() {
  appWindow.close();
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

    appWindow.onFocusChanged((event) => {
      if(event.payload) {
        toolbarVisible.value = true;
      } else {
        toolbarVisible.value = false;
      }
    });
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
  color: white;
  border-radius: 4px;
  font-size: 14px;
  z-index: 1000;
}

.toolbar {
  position: fixed;
  bottom: 0px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  background-color: rgba(0, 0, 0, 0.7);
  border-radius: 8px 8px 0px 0px;
  padding: 4px;
  gap: 4px;
  z-index: 1000;
  transition: transform 0.5s ease;
  opacity: 0.5;
}

.toolbar-hidden {
  transform: translate(-50%, 100%);
}

.toolbar-item {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  border-radius: 4px;
  transition: background-color 0.2s;
}

.toolbar-item:hover {
  background-color: rgba(255, 255, 255, 0.2);
}
</style>
