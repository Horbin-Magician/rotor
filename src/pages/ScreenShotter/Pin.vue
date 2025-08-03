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
    
    <!-- Text input overlay -->
    <div v-if="showTextInput" 
         class="text-input-overlay"
         :style="{ left: textInputPosition.x + 'px', top: textInputPosition.y + 'px' }">
      <input 
        ref="textInputRef"
        v-model="textInputValue"
        @keyup="textKeyUp"
        @blur="finishTextInput"
        class="text-input"
        autofocus
      />
    </div>
  </main>
  
  <!-- Normal Toolbar -->
  <div class="toolbar" :class="{ 'toolbar-hidden': !toolbarVisible || state === State.Drawing }">
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="enterEditMode">
          <n-icon size="20" color="#007bff">
            <EditOutlined />
          </n-icon>
        </div>
      </template>
      {{ t('message.annotationMode') }}
    </n-tooltip>
    <div class="toolbar-divider"></div>
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="minimizeWindow">
          <n-icon size="20" color="#007bff">
            <MinusFilled />
          </n-icon>
        </div>
      </template>
      {{ t('message.minimize') }} ({{ shortcuts.hide }})
    </n-tooltip>
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="saveImage">
          <n-icon size="20" color="#007bff">
            <SaveAltFilled />
          </n-icon>
        </div>
      </template>
      {{ t('message.saveImage') }} ({{ shortcuts.save }})
    </n-tooltip>
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="closeWindow">
          <n-icon size="20" color="#007bff">
            <CloseFilled />
          </n-icon>
        </div>
      </template>
      {{ t('message.close') }} ({{ shortcuts.close }})
    </n-tooltip>
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="copyImage">
          <n-icon size="20" color="#007bff">
            <ContentCopyRound />
          </n-icon>
        </div>
      </template>
      {{ t('message.copyImage') }} ({{ shortcuts.copy }})
    </n-tooltip>
  </div>

  <!-- Drawing Toolbar -->
  <div class="toolbar drawing-toolbar" :class="{ 'toolbar-hidden': !toolbarVisible || state != State.Drawing }">
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="exitEditMode">
          <n-icon size="20" color="#007bff">
            <ArrowBackIosRound />
          </n-icon>
        </div>
      </template>
      {{ t('message.exitAnnotation') }} ({{ shortcuts.close }})
    </n-tooltip>
    <div class="toolbar-divider"></div>
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="selectPenTool" :class="{ 'active': drawState === DrawState.Pen }">
          <n-icon size="20" color="#007bff">
            <EditOutlined />
          </n-icon>
        </div>
      </template>
      {{ t('message.penTool') }}
    </n-tooltip>
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="selectRectTool" :class="{ 'active': drawState === DrawState.Rect }">
          <n-icon size="20" color="#007bff">
            <CropDinRound />
          </n-icon>
        </div>
      </template>
      {{ t('message.rectangleTool') }}
    </n-tooltip>
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="selectArrowTool" :class="{ 'active': drawState === DrawState.Arrow }">
          <n-icon size="20" color="#007bff">
            <ArrowDownLeft20Filled />
          </n-icon>
        </div>
      </template>
      {{ t('message.arrowTool') }}
    </n-tooltip>
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="selectTextTool" :class="{ 'active': drawState === DrawState.Text }">
          <n-icon size="20" color="#007bff">
            <TextT20Filled />
          </n-icon>
        </div>
      </template>
      {{ t('message.textTool') }}
    </n-tooltip>
    <div class="toolbar-divider"></div>
    <n-tooltip trigger="hover" placement="top" :delay="800">
      <template #trigger>
        <div class="toolbar-item" @click="undoDrawing">
          <n-icon size="20" color="#007bff">
            <UndoFilled />
          </n-icon>
        </div>
      </template>
      {{ t('message.undo') }}
    </n-tooltip>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";
import { useI18n } from 'vue-i18n';
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
import { NTooltip, NIcon } from 'naive-ui';
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

const { t } = useI18n()

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
const toolbarVisible = ref(false)
let hideTipTimeout: number | null = null;

// Drawing state
let drawingLayer: Konva.Layer | null = null;
let currentPath: Konva.Line | null = null;
let currentArrow: Konva.Arrow | null = null;
let currentRect: Konva.Rect | null = null;
let currentText: Konva.Text | null = null;
let drawingHistory: any[] = [];
let isDrawing = false;
let startPoint: { x: number; y: number } | null = null;

// Text input state
const showTextInput = ref(false);
const textInputValue = ref('');
const textInputPosition = ref({ x: 0, y: 0 });
const textInputRef = ref<HTMLInputElement | null>(null);

// Shortcut keys from configuration
const shortcuts = ref({
  save: 's',
  close: 'Escape',
  copy: 'Enter',
  hide: 'h'
});

// Minimum window dimensions to show toolbar
const MIN_WIDTH_FOR_TOOLBAR = 140;
const MIN_HEIGHT_FOR_TOOLBAR = 80;

// Load shortcuts from configuration
async function loadShortcuts() {
  try {
    const saveShortcut: string = await invoke("get_cfg", { k: "shortcut_pinwin_save" });
    const closeShortcut: string = await invoke("get_cfg", { k: "shortcut_pinwin_close" });
    const copyShortcut: string = await invoke("get_cfg", { k: "shortcut_pinwin_copy" });
    const hideShortcut: string = await invoke("get_cfg", { k: "shortcut_pinwin_hide" });
    
    // Parse shortcut strings to get the key part
    shortcuts.value = {
      save: parseShortcutKey(saveShortcut) || 's',
      close: parseShortcutKey(closeShortcut) || 'Escape',
      copy: parseShortcutKey(copyShortcut) || 'Enter',
      hide: parseShortcutKey(hideShortcut) || 'h'
    };
  } catch (error) {
    console.error("Failed to load shortcuts:", error);
  }
}

// Parse shortcut string to extract the key part
function parseShortcutKey(shortcutStr: string): string {
  if (!shortcutStr) return '';
  
  // Handle different shortcut formats
  if (shortcutStr.includes('+')) {
    // For shortcuts like "Ctrl+Shift+S", we only want the last part
    const parts = shortcutStr.split('+');
    return parts[parts.length - 1].trim();
  }
  
  // For single keys like "Escape", "Enter", "KeyS"
  if (shortcutStr.startsWith('Key')) {
    return shortcutStr.substring(3); // "KeyS" -> "s"
  }
  
  return shortcutStr;
}

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
  if (state.value === State.Default) {
    const key = event.key.toLowerCase();
    
    if (key === shortcuts.value.close.toLowerCase()) {
      closeWindow();
    } else if (key === shortcuts.value.copy.toLowerCase()) {
      copyImage();
    } else if (key === shortcuts.value.save.toLowerCase()) {
      saveImage();
    } else if (key === shortcuts.value.hide.toLowerCase()) {
      minimizeWindow();
    }
  } else if (state.value === State.Drawing) {
    if (event.key.toLowerCase() === shortcuts.value.close.toLowerCase()) {
      state.value = State.Default;
    }
  }
}

function textKeyUp(event: KeyboardEvent) {
  if (event.key === 'Enter') {
    event.preventDefault()
    finishTextInput()
  } else if (event.key === 'Escape') {
    event.preventDefault()
    cancelTextInput()
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
  const zoom_delta: number = parseInt(await invoke("get_cfg", { k: "zoom_delta" }), 10);

  let delta = wheel_delta > 0 ? -zoom_delta : zoom_delta
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
  
  // Scale all drawing elements
  if (drawingLayer) {
    const scaleRatio = zoom_scale / 100
    drawingLayer.scale({ x: scaleRatio, y: scaleRatio })
    drawingLayer.batchDraw()
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

// TODO
// Drawing functions
function startDrawing(event: MouseEvent) {
  if (!drawingLayer || !stage) return;
  
  const pos = stage.getPointerPosition();
  if (!pos) return;

  if (drawState.value === DrawState.Text) {
    // Handle text input
    finishTextInput()
    startTextInput(event, pos);
    return;
  }
  
  isDrawing = true;
  // Convert position to account for current scale
  const scaleRatio = zoom_scale / 100;
  startPoint = { x: pos.x / scaleRatio, y: pos.y / scaleRatio };

  if (drawState.value === DrawState.Pen) {
    currentPath = new Konva.Line({
      stroke: '#ff0000',
      strokeWidth: 3 / scaleRatio, // Adjust stroke width for scale
      globalCompositeOperation: 'source-over',
      lineCap: 'round',
      lineJoin: 'round',
      points: [pos.x / scaleRatio, pos.y / scaleRatio, pos.x / scaleRatio, pos.y / scaleRatio],
    });
    drawingLayer.add(currentPath);
  } else if (drawState.value === DrawState.Rect) {
    currentRect = new Konva.Rect({
      x: pos.x / scaleRatio,
      y: pos.y / scaleRatio,
      width: 0,
      height: 0,
      stroke: '#ff0000',
      strokeWidth: 3 / scaleRatio, // Adjust stroke width for scale
      fill: 'transparent',
    });
    drawingLayer.add(currentRect);
  } else if (drawState.value === DrawState.Arrow) {
    // Create arrow using Konva.Arrow
    currentArrow = new Konva.Arrow({
      points: [pos.x / scaleRatio, pos.y / scaleRatio, pos.x / scaleRatio, pos.y / scaleRatio],
      stroke: '#ff0000',
      strokeWidth: 3 / scaleRatio, // Adjust stroke width for scale
      fill: '#ff0000',
      pointerLength: 15 / scaleRatio, // Adjust pointer size for scale
      pointerWidth: 15 / scaleRatio,
    });
    drawingLayer.add(currentArrow);
  }
}

function continueDrawing(_event: MouseEvent) {
  if (!isDrawing || !stage) return;
  
  const pos = stage.getPointerPosition();
  if (!pos) return;

  const scaleRatio = zoom_scale / 100;
  const scaledX = pos.x / scaleRatio;
  const scaledY = pos.y / scaleRatio;

  if (drawState.value === DrawState.Pen && currentPath) {
    const newPoints = currentPath.points().concat([scaledX, scaledY]);
    currentPath.points(newPoints);
  } else if (drawState.value === DrawState.Rect && currentRect && startPoint) {
    // Update rectangle dimensions based on current mouse position
    const width = scaledX - startPoint.x;
    const height = scaledY - startPoint.y;
    
    // Handle negative dimensions (drawing from bottom-right to top-left)
    if (width < 0) {
      currentRect.x(scaledX);
      currentRect.width(Math.abs(width));
    } else {
      currentRect.x(startPoint.x);
      currentRect.width(width);
    }
    
    if (height < 0) {
      currentRect.y(scaledY);
      currentRect.height(Math.abs(height));
    } else {
      currentRect.y(startPoint.y);
      currentRect.height(height);
    }
  } else if (drawState.value === DrawState.Arrow && currentArrow && startPoint) {
    currentArrow.points([startPoint.x, startPoint.y, scaledX, scaledY]); // Update arrow points
  }
  
  drawingLayer?.batchDraw();
}

// Smoothing function for free drawing lines
function smoothPoints(points: number[], alpha: number = 0.3): number[] {
  const n = points.length;
  if (n < 6) return points.slice();
  const smoothed: number[] = [];

  smoothed.push(points[0], points[1]);

  for (let i = 2; i < n - 2; i += 2) {
    const prevX = points[i - 2], prevY = points[i - 1];
    const currX = points[i],   currY = points[i + 1];
    const nextX = points[i + 2], nextY = points[i + 3];

    const x = prevX * alpha + currX * (1 - 2 * alpha) + nextX * alpha;
    const y = prevY * alpha + currY * (1 - 2 * alpha) + nextY * alpha;

    smoothed.push(x, y);
  }

  smoothed.push(points[n - 2], points[n - 1]);

  return smoothed;
}

function endDrawing() {
  if (!isDrawing) return;
  isDrawing = false;
  
  // Save to history for undo functionality
  if (currentPath) { // Apply smoothing to pen tool drawings before adding to history
    const smoothedPoints = smoothPoints(currentPath.points());
    currentPath.points(smoothedPoints);
    drawingHistory.push(currentPath);
  } else if (currentArrow) {
    drawingHistory.push(currentArrow.clone());
  } else if (currentRect) {
    drawingHistory.push(currentRect.clone());
  } else if (currentText) {
    drawingHistory.push(currentText.clone());
  }
  
  currentPath = null;
  currentArrow = null;
  currentRect = null;
  currentText = null;
  startPoint = null;
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

// Text input functions
function startTextInput(event: MouseEvent, stagePos: { x: number; y: number }) {
  // Convert stage position to screen position for input overlay
  const rect = (event.target as HTMLElement).getBoundingClientRect();
  textInputPosition.value = {
    x: event.clientX - rect.left,
    y: event.clientY - rect.top
  };
  
  // Store the stage position for text placement, accounting for scale
  const scaleRatio = zoom_scale / 100;
  startPoint = { x: stagePos.x / scaleRatio, y: stagePos.y / scaleRatio };
  
  // Show text input
  showTextInput.value = true;
  textInputValue.value = '';
  
  // Focus the input after Vue updates the DOM
  setTimeout(() => {
    textInputRef.value?.focus();
  }, 0);
}

function finishTextInput() {
  if (!drawingLayer || !startPoint || !textInputValue.value.trim()) {
    cancelTextInput();
    return;
  }

  // Create text object with scale-adjusted font size
  const scaleRatio = zoom_scale / 100;
  currentText = new Konva.Text({
    x: startPoint.x,
    y: startPoint.y,
    text: textInputValue.value,
    fontSize: 16 / scaleRatio, // Adjust font size for scale
    fill: '#ff0000',
  });
  
  drawingLayer.add(currentText);
  drawingLayer.batchDraw();
  
  // Add to history
  drawingHistory.push(currentText.clone());
  
  // Clean up
  cancelTextInput();
  currentText = null;
  startPoint = null;
}

function cancelTextInput() {
  showTextInput.value = false;
  textInputValue.value = '';
  startPoint = null;
}

{ // Mount something
  onMounted(async () => {
    // Load shortcuts from configuration
    await loadShortcuts();
    updateToolbarVisibility();
    
    window.addEventListener('keyup', handleKeyup);
    window.addEventListener('resize', handleWindowResize);

    appWindow.onFocusChanged((_event) => {
      updateToolbarVisibility();
    });

    const menu = await Menu.new({
      items: [
        {
          id: 'minimize',
          text: t('message.minimize'),
          accelerator: shortcuts.value.hide,
          action: () => minimizeWindow(),
        },
        {
          id: 'save',
          text: t('message.saveImage'),
          accelerator: shortcuts.value.save,
          action: () => saveImage(),
        },
        {
          id: 'copy',
          text: t('message.copyImage'),
          accelerator: shortcuts.value.copy,
          action: () => copyImage(),
        },
        {
          id: 'close',
          text: t('message.close'),
          accelerator: shortcuts.value.close,
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

.text-input-overlay {
  position: absolute;
  z-index: 2000;
}

.text-input {
  padding: 4px 8px;
  border: 1px solid #007bff;
  border-radius: 4px;
  background-color: transparent;
  font-size: 16px;
  outline: none;
  color: white;
  width: 100px;
}
</style>
