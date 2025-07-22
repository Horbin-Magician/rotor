<template>
  <main class="container" 
        @mousedown="handleMouseDown"
        @mouseup="handleMouseUp"
        @mousemove="handleMouseMove"
        @wheel="handleWheel">
    <div id="stage" ref="backImgRef"></div>
    <div class="tips" v-if="show_tips">
      {{tips}}
    </div>
  </main>
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
    <div class="toolbar-item" @click="copyImage">
      <n-icon size="20" color="#007bff">
        <ContentCopyRound />
      </n-icon>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";
import { CloseFilled, SaveAltFilled, ContentCopyRound, MinusFilled } from '@vicons/material';
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalPosition, LogicalSize } from '@tauri-apps/api/window';
import { Menu } from '@tauri-apps/api/menu';
import { writeImage } from '@tauri-apps/plugin-clipboard-manager';
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
const backImgRef = ref<ImageBitmap | null>(null)
let backImgLayer: Konva.Layer | null = null
let stage: Konva.Stage | null = null
let pixelRatio = 1

const tips = ref("")
const show_tips = ref(false)

let zoom_scale = 100;
const toolbarVisible = ref(true);
let hideTipTimeout: number | null = null;

// Minimum window dimensions to show toolbar
const MIN_WIDTH_FOR_TOOLBAR = 120;
const MIN_HEIGHT_FOR_TOOLBAR = 80;

// Load the screenshot
async function loadScreenShot() {
  const pos = await appWindow.outerPosition();
  const size = await appWindow.outerSize();
  const imgBuf: ArrayBuffer = await invoke("get_screen_img_rect", {
    x: pos.x.toString(),
    y: pos.y.toString(),
    width: size.width.toString(),
    height: size.height.toString(),
  });
  const imgData = new ImageData(new Uint8ClampedArray(imgBuf), size.width, size.height);
  backImg.value = await createImageBitmap(imgData)
  
  pixelRatio = size.width / window.innerWidth

  backImgLayer = new Konva.Layer(); // then create layer
  const konvaImage = new Konva.Image({
    x: 0,
    y: 0,
    image: backImg.value,
    width: window.innerWidth,
    height: window.innerHeight,
  });
  backImgLayer.add(konvaImage);

  stage = new Konva.Stage({
    container: 'stage', // id of container <div>
    width: window.innerWidth,
    height: window.innerHeight,
  });
  stage.add(backImgLayer); // add the layer to the stage
}
loadScreenShot();

// Mouse event handlers
async function handleMouseDown(event: MouseEvent) {
  if (event.button == 0) { // left button
    appWindow.startDragging();
    state = State.Moving
  }
}

// Mouse event handlers
function handleMouseUp(_event: MouseEvent) {
  state = State.Default;
}

// Mouse event handlers
function handleMouseMove(_event: MouseEvent) {

}

function handleWheel(event: WheelEvent){
  event.preventDefault()
  zoomWindow(event.deltaY)
}

function handleKeyup(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    closeWindow()
  }
}

function minimizeWindow() {
  appWindow.minimize();
}

function closeWindow() {
  appWindow.close();
}

async function saveImage() {
  if (!stage) return;
  stage?.toBlob({
    pixelRatio: pixelRatio,
    callback(blob) {
      if(!blob) return
      blob.arrayBuffer().then((imgBuf)=>{
        invoke("save_img", {imgBuf: imgBuf}).then((if_success)=>{
          if(if_success) closeWindow()
        })
      })
    }
  })
}

async function copyImage() {
  stage?.toBlob({
    pixelRatio: pixelRatio,
    callback(blob) {
      if(!blob) return
      blob.arrayBuffer().then((imgBuf)=>{
        writeImage(imgBuf)
        closeWindow()
      })
    }
  })
}

async function zoomWindow(wheel_delta: number) {
  let delta = wheel_delta > 0 ? -2 : 2 // TODO use setting
  zoom_scale += delta
  zoom_scale = Math.max(5, Math.min(zoom_scale, 500))

  tips.value = zoom_scale + "%"
  if (hideTipTimeout) { clearTimeout(hideTipTimeout) }
  show_tips.value = true
  
  await scaleWindow()

  // Hide tips after 1 second
  hideTipTimeout = setTimeout(() => {
    show_tips.value = false
  }, 1000)
}

async function scaleWindow() {
  if (!stage || !backImg.value) return
  
  const originalWidth = backImg.value.width / pixelRatio
  const originalHeight = backImg.value.height / pixelRatio
  
  // Calculate new size based on zoom scale
  const newWidth = Math.round(originalWidth * zoom_scale / 100)
  const newHeight = Math.round(originalHeight * zoom_scale / 100)
  
  // Resize the window
  await appWindow.setSize(new LogicalSize(newWidth, newHeight))
  
  // Update stage size
  stage.width(newWidth)
  stage.height(newHeight)
  
  // Update the image in the stage
  const konvaImage = backImgLayer?.findOne('Image') as Konva.Image
  if (konvaImage) {
    konvaImage.width(newWidth)
    konvaImage.height(newHeight)
    backImgLayer?.batchDraw()
  }
}

function updateToolbarVisibility() {
  const windowWidth = window.innerWidth
  const windowHeight = window.innerHeight
  
  // Hide toolbar if window is too small
  if (windowWidth < MIN_WIDTH_FOR_TOOLBAR || windowHeight < MIN_HEIGHT_FOR_TOOLBAR) {
    toolbarVisible.value = false
  } else {
    // Only show toolbar if window is focused and large enough
    appWindow.isFocused().then((focused) => {
      toolbarVisible.value = focused
    })
  }
}

function handleWindowResize() {
  updateToolbarVisibility()
}

{ // Mount something
  onMounted(async () => {
    window.addEventListener('keyup', handleKeyup);
    window.addEventListener('resize', handleWindowResize);

    appWindow.onFocusChanged((_event) => {
      updateToolbarVisibility();
    });

    const menu = await Menu.new({
      items: [
        {
          id: 'Open',
          text: 'open',
          action: () => {
            console.log('open pressed');
          },
        },
        {
          id: 'Close',
          text: 'close',
          action: () => {
            console.log('close pressed');
          },
        },
      ],
    });

    // If a window was not created with an explicit menu or had one set explicitly,
    // this menu will be assigned to it.
    menu.setAsAppMenu().then((res) => {
      console.log('menu set success', res);
    });

    window.addEventListener('contextmenu', async (event) => {
      event.preventDefault();
      menu.popup(new LogicalPosition(event.clientX, event.clientY));
    });
  });

  onBeforeUnmount(async () => {
    window.removeEventListener('keyup', handleKeyup)
    window.removeEventListener('resize', handleWindowResize)
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
  border-radius: 8px;
  font-size: 14px;
  z-index: 1000;
  cursor: default;
}

.toolbar {
  position: fixed;
  bottom: 0px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  background-color: rgba(0, 0, 0);
  border-radius: 8px 8px 0px 0px;
  padding: 4px;
  gap: 4px;
  z-index: 1000;
  transition: transform 0.5s ease;
  opacity: 0.9;
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
