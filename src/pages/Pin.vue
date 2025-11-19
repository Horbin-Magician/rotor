<template>
  <main class="container" 
        @mousedown="handleMouseDown"
        @mouseup="handleMouseUp"
        @mousemove="handleMouseMove"
        @mouseout="handleMouseUp"
        @wheel="handleWheel">
    <PinCanvas 
      ref="canvasRef"
      :zoom-scale="zoomScale"
    />
    
    <PinTips 
      :visible="show_tips"
      :tips="tips"
    />
    
    <PinTextInput 
      :visible="showTextInput"
      :position="textInputPosition"
      v-model="textInputValue"
      @finish="finishTextInput"
      @cancel="cancelTextInput"
      @blur="finishTextInput"
    />
    
    <PinOcrOverlay 
      :visible="state === State.OCR"
      :results="ocrTextResults"
      :scale-factor="scale_factor"
      :ocr-zoom-scale="ocrZoomScale"
      :zoom-scale="zoomScale"
    />
  </main>
  
  <PinToolbar 
    :visible="toolbarVisible && state !== State.Drawing"
    :is-processing-ocr="isProcessingOcr"
    :is-ocr-active="state === State.OCR"
    :shortcuts="shortcuts"
    @enter-edit-mode="enterEditMode"
    @img-to-text="imgToText"
    @minimize="minimizeWindow"
    @save="saveImage"
    @close="closeWindow"
    @copy="copyImage"
  />

  <PinDrawingToolbar 
    :visible="toolbarVisible && state === State.Drawing"
    :active-tool="getActiveDrawTool()"
    :close-shortcut="shortcuts.close"
    @exit="exitEditMode"
    @select-pen="selectPenTool"
    @select-rect="selectRectTool"
    @select-arrow="selectArrowTool"
    @select-text="selectTextTool"
    @undo="undoDrawing"
  />

  <PinEdgeGlow :edge-glow="edgeGlow" />
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";
import { useI18n } from 'vue-i18n';
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalPosition, LogicalSize, PhysicalPosition } from '@tauri-apps/api/window';
import { Menu } from '@tauri-apps/api/menu';
import { writeImage } from '@tauri-apps/plugin-clipboard-manager';
import { UnlistenFn } from "@tauri-apps/api/event";
import { warn } from "@tauri-apps/plugin-log";

import PinCanvas from '../components/screenShotter/pin/PinCanvas.vue';
import PinTips from '../components/screenShotter/pin/PinTips.vue';
import PinTextInput from '../components/screenShotter/pin/PinTextInput.vue';
import PinOcrOverlay from '../components/screenShotter/pin/PinOcrOverlay.vue';
import PinToolbar from '../components/screenShotter/pin/PinToolbar.vue';
import PinDrawingToolbar from '../components/screenShotter/pin/PinDrawingToolbar.vue';
import PinEdgeGlow from '../components/screenShotter/pin/PinEdgeGlow.vue';

enum State {
  Default,
  Drawing,
  OCR
}

interface EdgeSnap {
  horizontal: {
    edge: 'left' | 'right' | null;
    targetX: number | null;
  };
  vertical: {
    edge: 'top' | 'bottom' | null;
    targetY: number | null;
  };
}

enum DrawState {
  Pen,
  Rect,
  Arrow,
  Text
}

const { t } = useI18n()

const appWindow = getCurrentWindow()
const pin_id = Number.parseInt(appWindow.label.split('-')[1]);
let unlisten_show_pin: UnlistenFn;

const ws = new WebSocket(`ws://localhost:48137`) // TODO: deal port being occupied
ws.binaryType = 'arraybuffer';

const state = ref(State.Default)
const drawState = ref(DrawState.Pen)

// Edge glow state
const edgeGlow = ref({
  left: false,
  right: false,
  top: false,
  bottom: false
})

// Snap configuration
let isDragging = false
let potentialSnap: EdgeSnap = {
  horizontal: { edge: null, targetX: null },
  vertical: { edge: null, targetY: null }
}

const backImg = ref()
const cropRegion = ref({ x: 0, y: 0, width: 0, height: 0 })
const canvasRef = ref<InstanceType<typeof PinCanvas> | null>(null)
const init_scale_factor = ref(1)
const scale_factor = ref(1)
const zoomScale = ref(100);
const tips = ref("")
const show_tips = ref(false)

const toolbarVisible = ref(false)
let hideTipTimeout: number | null = null;

// Text input state
const showTextInput = ref(false);
const textInputValue = ref('');
const textInputPosition = ref({ x: 0, y: 0 });

// OCR text results
const ocrTextResults = ref<TextResult[]>([]);
const isProcessingOcr = ref(false);
let ocrZoomScale = 100;

// Shortcut keys from configuration
const shortcuts = ref({
  save: 's',
  close: 'Escape',
  copy: 'Enter',
  hide: 'h'
});

// Minimum window dimensions to show toolbar
const MIN_WIDTH_FOR_TOOLBAR = 180;
const MIN_HEIGHT_FOR_TOOLBAR = 40;

// Load shortcuts from configuration
async function loadShortcuts() {
  try {
    const saveShortcut: string = await invoke("get_cfg", { k: "shortcut_pinwin_save" });
    const closeShortcut: string = await invoke("get_cfg", { k: "shortcut_pinwin_close" });
    const copyShortcut: string = await invoke("get_cfg", { k: "shortcut_pinwin_copy" });
    const hideShortcut: string = await invoke("get_cfg", { k: "shortcut_pinwin_hide" });
    
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

function parseShortcutKey(shortcutStr: string): string {
  if (!shortcutStr) return '';
  
  if (shortcutStr.includes('+')) {
    const parts = shortcutStr.split('+');
    return parts[parts.length - 1].trim();
  }
  
  if (shortcutStr.startsWith('Key')) {
    return shortcutStr.substring(3);
  }
  
  return shortcutStr;
}

interface PinConfig {
  monitor_pos: [number, number],
  monitor_size: [number, number],
  rect: [number, number, number, number];
  offset: [number, number],
  zoom_factor: number;
  mask_label: string;
  minimized: boolean;
}

interface TextResult {
  left: number;
  top: number;
  width: number;
  height: number;
  text: string;
}

async function tryLoadScreenShot(id: number): Promise<boolean> {
  const pin_config = await invoke("get_pin_state", { id }) as PinConfig;
  if (!pin_config) {
    warn(`No pin config found for id: ${id}`);
    return false;
  }
  
  init_scale_factor.value = window.devicePixelRatio;
  scale_factor.value = window.devicePixelRatio;
  
  const crop_width = pin_config.rect[2];
  const crop_height = pin_config.rect[3];
  const win_width = Math.round(crop_width / scale_factor.value);
  const win_height = Math.round(crop_height / scale_factor.value);
  
  await appWindow.setSize(new LogicalSize(win_width, win_height))
  await appWindow.setPosition(new PhysicalPosition((
    pin_config.monitor_pos[0] + pin_config.rect[0] + pin_config.offset[0] || 0),
    (pin_config.monitor_pos[1] + pin_config.rect[1] + pin_config.offset[1] || 0)
  ));

  // Request image data via WebSocket
  return new Promise<boolean>((resolve) => {
    const handleMessage = async (event: MessageEvent) => {
      try {
        const imgBuf: ArrayBuffer = event.data;
        const full_monitor_width = pin_config.monitor_size[0];
        const full_monitor_height = pin_config.monitor_size[1];
        const fullImgData = new ImageData(new Uint8ClampedArray(imgBuf), full_monitor_width, full_monitor_height);
        backImg.value = await createImageBitmap(fullImgData);
        
        // Remove this one-time message handler
        ws.removeEventListener('message', handleMessage);
        
        // Continue with initialization
        await completeInitialization(pin_config);
        resolve(true);
      } catch (error) {
        warn(`Failed to create image bitmap for pin id ${id}: ${error}`);
        await invoke("delete_pin_record", { id: pin_id });
        ws.removeEventListener('message', handleMessage);
        resolve(false);
      }
    };
    
    ws.addEventListener('message', handleMessage);
    ws.send(pin_config.mask_label);
  });
}

async function completeInitialization(pin_config: PinConfig) {
  const crop_width = pin_config.rect[2];
  const crop_height = pin_config.rect[3];
  
  // Define crop region
  cropRegion.value = {
    x: pin_config.rect[0],
    y: pin_config.rect[1],
    width: crop_width,
    height: crop_height
  };
  
  canvasRef.value?.initStage(backImg.value, cropRegion.value);
  
  zoomScale.value = pin_config.zoom_factor || 100;
  await scaleWindow();
  await appWindow.setPosition(new PhysicalPosition((
    pin_config.monitor_pos[0] + pin_config.rect[0] + pin_config.offset[0] || 0),
    (pin_config.monitor_pos[1] + pin_config.rect[1] + pin_config.offset[1] || 0)
  ));

  updateToolbarVisibility();
  appWindow.isVisible().then( (visible)=>{
    if(visible == false) {
      appWindow.show()
      appWindow.setFocus()
      if (pin_config.minimized) appWindow.minimize()
    }
  })
}

async function handleMouseDown(event: MouseEvent) {
  if (state.value === State.Drawing) {
    if (event.button === 0) {
      startDrawing(event);
    }
  } else if (state.value === State.Default) {
    if (event.button === 0) {
      startDragging();
    }
  } else if (state.value === State.OCR) {
    const target = event.target as HTMLElement;
    const isOcrText = target.classList.contains('ocr-text-content') || 
                      target.closest('.ocr-text-overlay');

    if (event.button === 0 && !isOcrText) {
      startDragging();
    }
  }
}

function startDragging() {
  isDragging = true;
  potentialSnap = {
    horizontal: { edge: null, targetX: null },
    vertical: { edge: null, targetY: null }
  };
  appWindow.startDragging();
}

async function endDragging() {
  isDragging = false;
  
  // Apply snap if there's a potential snap position
  // Get current position first
  const currentPos = await appWindow.outerPosition();
  let targetX = currentPos.x;
  let targetY = currentPos.y;
  
  // Apply horizontal snap if exists
  if (potentialSnap.horizontal.edge && potentialSnap.horizontal.targetX !== null) {
    targetX = potentialSnap.horizontal.targetX;
  }
  
  // Apply vertical snap if exists
  if (potentialSnap.vertical.edge && potentialSnap.vertical.targetY !== null) {
    targetY = potentialSnap.vertical.targetY;
  }
  
  // Only set position if at least one edge snap is active
  if (potentialSnap.horizontal.edge || potentialSnap.vertical.edge) {
    try {
      await appWindow.setPosition(new PhysicalPosition(targetX, targetY));
    } catch (error) {
      console.error("Failed to snap window:", error);
    }
  }
  
  // Reset snap state
  potentialSnap = {
    horizontal: { edge: null, targetX: null },
    vertical: { edge: null, targetY: null }
  };
  
  // Clear glow
  setTimeout(() => {
    edgeGlow.value = {
      left: false,
      right: false,
      top: false,
      bottom: false
    };
  }, 50);
}

function handleMouseUp(_event: MouseEvent) {
  if (state.value === State.Drawing) {
    endDrawing();
    return;
  }
  
  if (isDragging) { // Handle snap-to-edge
    endDragging();
  }
}

function handleMouseMove(_event: MouseEvent) {
  if (state.value === State.Drawing) {
    const mode = getActiveDrawTool();
    canvasRef.value?.continueDrawing(mode);
  }
}

function handleWheel(event: WheelEvent){
  event.preventDefault()
  zoomWindow(event.deltaY)
}

function handleKeyup(event: KeyboardEvent) {
  const key = event.key.toLowerCase();

  if (event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) {
    return;
  }

  if (key === shortcuts.value.close.toLowerCase()) {
    if (state.value !== State.Default) {
      state.value = State.Default;
    } else {
      closeWindow();
    }
  } else if (key === shortcuts.value.copy.toLowerCase()) {
    copyImage();
  } else if (key === shortcuts.value.save.toLowerCase()) {
    saveImage();
  } else if (key === shortcuts.value.hide.toLowerCase()) {
    minimizeWindow();
  }
}

function minimizeWindow() {
  appWindow.minimize();
}

async function closeWindow() {
  try {
    await invoke("delete_pin_record", { id: pin_id });
  } catch (error) {
    warn(`Failed to delete pin record ${pin_id}: ${error}`);
  }
  appWindow.close();
}

async function saveImage() {
  const stage = canvasRef.value?.getStage()
  stage?.toBlob({
    pixelRatio: window.devicePixelRatio,
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
  const stage = canvasRef.value?.getStage()
  stage?.toBlob({
    pixelRatio: window.devicePixelRatio,
    callback(blob) {
      if(!blob) return
      blob.arrayBuffer().then((imgBuf)=>{
        writeImage(imgBuf)
        closeWindow()
      })
    }
  })
}

async function imgToText() {
  const stage = canvasRef.value?.getStage()
  if (!stage || isProcessingOcr.value) return;
  
  if (state.value != State.OCR) {
    if (ocrTextResults.value.length > 0) {
      state.value = State.OCR;
    } else {
      isProcessingOcr.value = true;
      ocrZoomScale = zoomScale.value;
      stage.toBlob({
        pixelRatio: window.devicePixelRatio,
        callback(blob) {
          if(!blob) {
            isProcessingOcr.value = false;
            return;
          }
          blob.arrayBuffer().then((imgBuf)=>{
            invoke<TextResult[]>("img2text", { imgBuf: imgBuf })
              .then((textResults)=>{
                ocrTextResults.value = textResults;
                isProcessingOcr.value = false;
                scale_factor.value = window.devicePixelRatio;
                state.value = State.OCR;
              })
              .catch((error) => {
                console.error("OCR processing failed:", error);
                isProcessingOcr.value = false;
              });
          })
        }
      })
    }
  } else {
    state.value = State.Default;
  }
}

async function zoomWindow(wheel_delta: number) {
  const zoom_delta: number = parseInt(await invoke("get_cfg", { k: "zoom_delta" }), 10);

  let delta = wheel_delta > 0 ? -zoom_delta : zoom_delta
  zoomScale.value += delta
  zoomScale.value = Math.max(5, Math.min(zoomScale.value, 500))
  
  await scaleWindow()

  // Show zoom tip
  tips.value = zoomScale.value + "%"
  if (hideTipTimeout) { clearTimeout(hideTipTimeout) }
  show_tips.value = true
  hideTipTimeout = setTimeout(() => {
    show_tips.value = false
  }, 1000)
}

async function scaleWindow() {
  if (!backImg.value) return
  
  const logicalWidth = cropRegion.value.width / init_scale_factor.value
  const logicalHeight = cropRegion.value.height / init_scale_factor.value
  
  const newWidth = Math.round(logicalWidth * zoomScale.value / 100)
  const newHeight = Math.round(logicalHeight * zoomScale.value / 100)
  
  await appWindow.setSize(new LogicalSize(newWidth, newHeight))
  canvasRef.value?.updateSize()
}

function updateToolbarVisibility() {
  const windowWidth = window.innerWidth
  const windowHeight = window.innerHeight
  
  if (windowWidth < MIN_WIDTH_FOR_TOOLBAR || windowHeight < MIN_HEIGHT_FOR_TOOLBAR) {
    toolbarVisible.value = false
  } else {
    appWindow.isFocused().then((focused) => {
      toolbarVisible.value = focused
    })
  }
}

async function handleWindowResize() {
  updateToolbarVisibility()
}

function enterEditMode() {
  state.value = State.Drawing
  drawState.value = DrawState.Pen
}

function exitEditMode() { 
  state.value = State.Default 
}

function selectPenTool() { 
  drawState.value = DrawState.Pen 
}

function selectRectTool() { 
  drawState.value = DrawState.Rect 
}

function selectArrowTool() { 
  drawState.value = DrawState.Arrow 
}

function selectTextTool() { 
  drawState.value = DrawState.Text 
}

function getActiveDrawTool(): 'pen' | 'rect' | 'arrow' | 'text' {
  switch (drawState.value) {
    case DrawState.Pen: return 'pen';
    case DrawState.Rect: return 'rect';
    case DrawState.Arrow: return 'arrow';
    case DrawState.Text: return 'text';
    default: return 'pen';
  }
}

function startDrawing(event: MouseEvent) {
  const mode = getActiveDrawTool();
  
  if (mode === 'text') {
    finishTextInput();
    canvasRef.value?.startDrawing(mode, (pos) => {
      startTextInput(event, pos);
    });
  } else {
    canvasRef.value?.startDrawing(mode);
  }
}

function endDrawing() {
  canvasRef.value?.endDrawing();
}

function undoDrawing() {
  canvasRef.value?.undoDrawing();
}

function startTextInput(event: MouseEvent, stagePos: { x: number; y: number }) {
  const rect = (event.target as HTMLElement).getBoundingClientRect();
  textInputPosition.value = {
    x: event.clientX - rect.left,
    y: event.clientY - rect.top
  };
  
  const scaleRatio = zoomScale.value / 100;
  const point = { x: stagePos.x / scaleRatio, y: stagePos.y / scaleRatio };
  canvasRef.value?.setStartPoint(point);
  
  showTextInput.value = true;
  textInputValue.value = '';
}

function finishTextInput() {
  const startPoint = canvasRef.value?.getStartPoint();
  if (!startPoint || !textInputValue.value.trim()) {
    cancelTextInput();
    return;
  }

  canvasRef.value?.addText(textInputValue.value, startPoint);
  cancelTextInput();
}

function cancelTextInput() {
  showTextInput.value = false;
  textInputValue.value = '';
  canvasRef.value?.setStartPoint(null);
}

// do something on mounted
onMounted(async () => {
  await loadShortcuts();
  
  window.addEventListener('keyup', handleKeyup);
  window.addEventListener('resize', handleWindowResize);

  appWindow.onFocusChanged(async (event) => {
    updateToolbarVisibility();

    const focused = event.payload;
    if(!focused) {
      const position = await appWindow.outerPosition();
      try {
        await invoke("update_pin_state", { 
          id: pin_id, 
          x: position.x,
          y: position.y,
          zoom: zoomScale.value,
          minimized: await appWindow.isMinimized()
        });
      } catch (error) {
        warn(`Failed to update pin state: ${error}`);
      }
    }
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

  {
    let result = await tryLoadScreenShot(pin_id);
    if (!result) {
      unlisten_show_pin = await appWindow.listen('show-pin', async (_event) => {
        await tryLoadScreenShot(pin_id);
        if(unlisten_show_pin) { unlisten_show_pin() }
      });
    }
  }
});

// do something before unmounted
onBeforeUnmount(async () => {
  window.removeEventListener('keyup', handleKeyup)
  window.removeEventListener('resize', handleWindowResize)
})
</script>

<style>
body {
  background-color: transparent !important;
}
</style>

<style scoped>
.container {
  position: relative;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  transition: box-shadow 0.2s ease-in-out;
}
</style>
