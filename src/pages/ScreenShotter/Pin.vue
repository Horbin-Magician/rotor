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
  
  <!-- Normal Toolbar -->
  <div class="toolbar" :class="{ 'toolbar-hidden': !toolbarVisible || state === State.Drawing }">
    <div class="toolbar-item" @click="enterEditMode">
      <n-icon size="20" color="#007bff">
        <EditOutlined />
      </n-icon>
    </div>
    <div class="toolbar-divider"></div>
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

  <!-- Drawing Toolbar -->
  <div class="toolbar drawing-toolbar" :class="{ 'toolbar-hidden': !toolbarVisible || state != State.Drawing }">
    <div class="toolbar-item" @click="exitEditMode">
      <n-icon size="20" color="#007bff">
        <ArrowBackIosRound />
      </n-icon>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-item" @click="selectPenTool" :class="{ 'active': drawState === DrawState.Pen }">
      <n-icon size="20" color="#007bff">
        <EditOutlined />
      </n-icon>
    </div>
    <div class="toolbar-item" @click="selectRectTool" :class="{ 'active': drawState === DrawState.Rect }">
      <n-icon size="20" color="#007bff">
        <CropDinRound />
      </n-icon>
    </div>
    <div class="toolbar-item" @click="selectArrowTool" :class="{ 'active': drawState === DrawState.Arrow }">
      <n-icon size="20" color="#007bff">
        <ArrowDownLeft20Filled />
      </n-icon>
    </div>
    <div class="toolbar-item" @click="selectTextTool" :class="{ 'active': drawState === DrawState.Text }">
      <n-icon size="20" color="#007bff">
        <TextT20Filled />
      </n-icon>
    </div>
    <div class="toolbar-divider"></div>
    <div class="toolbar-item" @click="undoDrawing">
      <n-icon size="20" color="#007bff">
        <UndoFilled />
      </n-icon>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";
import { 
  CloseFilled, 
  SaveAltFilled, 
  ContentCopyRound, 
  MinusFilled, 
  EditOutlined,
  ArrowBackIosRound,
  CropDinRound,
  UndoFilled,
} from '@vicons/material';
import { 
  ArrowDownLeft20Filled,
  TextT20Filled,
} from '@vicons/fluent';
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalPosition, LogicalSize } from '@tauri-apps/api/window';
import { Menu } from '@tauri-apps/api/menu';
import { writeImage } from '@tauri-apps/plugin-clipboard-manager';
import Konva from "konva";

enum State {
  Default,
  Drawing
}

enum DrawState {
  Pen,
  Rect,
  Arrow,
  Text
}

const appWindow = getCurrentWindow()
appWindow.isVisible().then( (visible)=>{
  if(visible == false) {
    appWindow.show()
    appWindow.setFocus()
  }
})

const state = ref(State.Default)
const drawState = ref(DrawState.Pen)

const backImg = ref()
const backImgRef = ref<ImageBitmap | null>(null)
let backImgLayer: Konva.Layer | null = null
let stage: Konva.Stage | null = null
let pixelRatio = 1

const tips = ref("")
const show_tips = ref(false)

let zoom_scale = 100;
const toolbarVisible = ref(true)
let hideTipTimeout: number | null = null;

// Drawing state
let drawingLayer: Konva.Layer | null = null;
let currentPath: Konva.Line | null = null;
let drawingHistory: any[] = [];
let isDrawing = false;

// Minimum window dimensions to show toolbar
const MIN_WIDTH_FOR_TOOLBAR = 140;
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
  
  // Create drawing layer
  drawingLayer = new Konva.Layer();
  stage.add(drawingLayer);
}
loadScreenShot();

// Mouse event handlers
async function handleMouseDown(event: MouseEvent) {
  if (state.value === State.Drawing) {
    if (event.button === 0) { // left button
      startDrawing(event);
    }
  } else if (state.value === State.Default) {
    if (event.button === 0) { // left button
      appWindow.startDragging();
    }
  }
}

// Mouse event handlers
function handleMouseUp(_event: MouseEvent) {
  if (state.value === State.Drawing) {
    endDrawing();
  }
}

// Mouse event handlers
function handleMouseMove(event: MouseEvent) {
  if (state.value === State.Drawing && isDrawing) {
    continueDrawing(event);
  }
}

function handleWheel(event: WheelEvent){
  event.preventDefault()
  zoomWindow(event.deltaY)
}

function handleKeyup(event: KeyboardEvent) {
  switch (event.key.toLowerCase()) {
    case 'Escape'.toLowerCase():
      closeWindow()
      break
    case 'Enter'.toLowerCase():
      copyImage()
      break
    case 's'.toLowerCase():
      saveImage()
      break
    case 'h'.toLowerCase():
      minimizeWindow()
      break
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

// Edit mode functions
function enterEditMode() {
  state.value = State.Drawing
  drawState.value = DrawState.Pen
}

function exitEditMode() { state.value = State.Default }

// Drawing tool selection
function selectPenTool() { drawState.value = DrawState.Pen }

function selectRectTool() { drawState.value = DrawState.Rect }

function selectArrowTool() { drawState.value = DrawState.Arrow }

function selectTextTool() { drawState.value = DrawState.Text }

// Drawing functions
function startDrawing(_event: MouseEvent) {
  if (!drawingLayer || !stage) return;
  
  isDrawing = true;
  const pos = stage.getPointerPosition();
  if (!pos) return;

  if (drawState.value === DrawState.Pen) {
    currentPath = new Konva.Line({ // TODO smooth and speed up
      stroke: '#ff0000',
      strokeWidth: 3,
      globalCompositeOperation: 'source-over',
      lineCap: 'round',
      lineJoin: 'round',
      // draggable: true,
      points: [pos.x, pos.y, pos.x, pos.y],
    });
    drawingLayer.add(currentPath);
  }
}

// TODO
function continueDrawing(_event: MouseEvent) {
  if (!isDrawing || !currentPath || !stage) return;
  
  const pos = stage.getPointerPosition();
  if (!pos) return;

  if (drawState.value === DrawState.Pen) {
    const newPoints = currentPath.points().concat([pos.x, pos.y]);
    currentPath.points(newPoints);
  }
  
  drawingLayer?.batchDraw();
}

function endDrawing() {
  if (!isDrawing) return;
  isDrawing = false;
  
  if (currentPath) { // Save to history for undo functionality
    drawingHistory.push(currentPath.clone());
  }
  currentPath = null;
}

function undoDrawing() {
  if (!drawingLayer || drawingHistory.length === 0) return;
  
  drawingHistory.pop();
  drawingLayer.destroyChildren();
  
  // Redraw all items from history // TODO this there any better way?
  drawingHistory.forEach(item => {
    drawingLayer?.add(item.clone());
  });
  
  drawingLayer.batchDraw();
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
          id: 'minimize',
          text: '最小化',
          accelerator: 'h',
          action: () => minimizeWindow(),
        },
        {
          id: 'save',
          text: '保存图片',
          accelerator: 's',
          action: () => saveImage(),
        },
        {
          id: 'copy',
          text: '复制图片',
          accelerator: 'Enter',
          action: () => copyImage(),
        },
        {
          id: 'close',
          text: '关闭',
          accelerator: 'ESC',
          action: () => closeWindow(),
        },
      ],
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
  align-items: center;
  background-color: rgba(0, 0, 0);
  border-radius: 8px 8px 0px 0px;
  padding: 4px;
  gap: 4px;
  z-index: 1000;
  transition: transform 0.3s ease;
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

.toolbar-item.active {
  background-color: rgba(0, 123, 255, 0.3);
}

.toolbar-divider {
  height: 20px;
  width: 1px;
  background-color: #002c5b;
}
</style>
