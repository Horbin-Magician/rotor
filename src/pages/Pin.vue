<template>
  <main class="container" 
        @mousedown="handleMouseDown"
        @mouseup="handleMouseUp"
        @mousemove="handleMouseMove"
        @mouseout="handleMouseUp"
        @wheel="handleWheel">
    <PinCanvas 
      ref="canvasRef"
      :zoom-scale="zoom_scale"
      @ready="handleCanvasReady"
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
      :zoom-scale="zoom_scale"
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

  <div v-if="edgeGlow.left" class="edge-glow-left"></div>
  <div v-if="edgeGlow.right" class="edge-glow-right"></div>
  <div v-if="edgeGlow.top" class="edge-glow-top"></div>
  <div v-if="edgeGlow.bottom" class="edge-glow-bottom"></div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";
import { useI18n } from 'vue-i18n';
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalPosition, LogicalSize, PhysicalPosition } from '@tauri-apps/api/window';
import { Menu } from '@tauri-apps/api/menu';
import { writeImage } from '@tauri-apps/plugin-clipboard-manager';
import Konva from "konva";
import { UnlistenFn } from "@tauri-apps/api/event";
import { warn } from "@tauri-apps/plugin-log";

import PinCanvas from '../components/screenShotter/pin/PinCanvas.vue';
import PinTips from '../components/screenShotter/pin/PinTips.vue';
import PinTextInput from '../components/screenShotter/pin/PinTextInput.vue';
import PinOcrOverlay from '../components/screenShotter/pin/PinOcrOverlay.vue';
import PinToolbar from '../components/screenShotter/pin/PinToolbar.vue';
import PinDrawingToolbar from '../components/screenShotter/pin/PinDrawingToolbar.vue';

enum State {
  Default,
  Drawing,
  OCR
}

interface OtherPinWindow {
  x: number;
  y: number;
  width: number;
  height: number;
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
const SNAP_THRESHOLD = 20 // pixels
let isDragging = false
let potentialSnap: EdgeSnap = {
  horizontal: { edge: null, targetX: null },
  vertical: { edge: null, targetY: null }
}
let dragCheckInterval: number | null = null

const backImg = ref()
const canvasRef = ref<InstanceType<typeof PinCanvas> | null>(null)
let backImgLayer: Konva.Layer | null = null
let drawingLayer: Konva.Layer | null = null
let stage: Konva.Stage | null = null
const init_scale_factor = ref(1)
const scale_factor = ref(1)

const tips = ref("")
const show_tips = ref(false)

let zoom_scale = 100;
const toolbarVisible = ref(false)
let hideTipTimeout: number | null = null;

// Drawing state
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
const MIN_WIDTH_FOR_TOOLBAR = 140;
const MIN_HEIGHT_FOR_TOOLBAR = 80;

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

function handleCanvasReady(stageInstance: Konva.Stage, backImgLayerInstance: Konva.Layer, drawingLayerInstance: Konva.Layer) {
  stage = stageInstance;
  backImgLayer = backImgLayerInstance;
  drawingLayer = drawingLayerInstance;
}

async function tryLoadScreenShot(id: number): Promise<boolean> {
  const pin_config = await invoke("get_pin_state", { id }) as PinConfig;
  if (!pin_config) {
    warn(`No pin config found for id: ${id}`);
    return false;
  }
  
  init_scale_factor.value = window.devicePixelRatio;
  scale_factor.value = window.devicePixelRatio;

  const img_width = pin_config.rect[2];
  const img_height = pin_config.rect[3];
  const win_width = Math.round(img_width / scale_factor.value);
  const win_height = Math.round(img_height / scale_factor.value);

  await appWindow.setSize(new LogicalSize(win_width, win_height))
  await appWindow.setPosition(new PhysicalPosition((
    pin_config.monitor_pos[0] + pin_config.rect[0] + pin_config.offset[0] || 0),
    (pin_config.monitor_pos[1] + pin_config.rect[1] + pin_config.offset[1] || 0)
  ));

  try {
    let imgBuf: ArrayBuffer = await invoke("get_pin_img", {id: id.toString()});
    const imgData = new ImageData(new Uint8ClampedArray(imgBuf), img_width, img_height);
    backImg.value = await createImageBitmap(imgData)
  } catch (error) {
    warn(`Failed to create image bitmap for pin id ${id}: ${error}`);
    await invoke("delete_pin_record", { id: pin_id });
    return false;
  }

  canvasRef.value?.initStage(img_width, img_height, backImg.value, scale_factor.value);

  zoom_scale = pin_config.zoom_factor || 100;
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

  return true
}

async function getOtherPinWindows(): Promise<OtherPinWindow[]> {
  const otherWindows: OtherPinWindow[] = [];
  
  try {
    // Iterate through all possible pin windows to get their positions
    for (let i = 1; i <= 100; i++) {
      if (i === pin_id) continue;
      
      try {
        const config = await invoke("get_pin_state", { id: i }) as PinConfig | null;
        if (config) {
          const x = config.monitor_pos[0] + config.rect[0] + config.offset[0];
          const y = config.monitor_pos[1] + config.rect[1] + config.offset[1];
          const width = Math.round(config.rect[2] * config.zoom_factor / 100);
          const height = Math.round(config.rect[3] * config.zoom_factor / 100);
          
          otherWindows.push({ x, y, width, height });
          console.log(`Found pin ${i} at (${x}, ${y}) size ${width}x${height}`);
        }
      } catch {
        // Pin doesn't exist, continue
      }
    }
    
    console.log(`Total other pins found: ${otherWindows.length}`);
  } catch (error) {
    console.error("Failed to get other pin windows:", error);
  }
  
  return otherWindows;
}

async function checkEdgeProximity() {
  if (!isDragging) return;
  
  try {
    const position = await appWindow.outerPosition();
    const size = await appWindow.outerSize();
    const currentX = position.x;
    const currentY = position.y;
    const currentWidth = size.width;
    const currentHeight = size.height;
    
    const otherWindows = await getOtherPinWindows();
    
    // Reset glow and snap
    edgeGlow.value = {
      left: false,
      right: false,
      top: false,
      bottom: false
    };
    potentialSnap = {
      horizontal: { edge: null, targetX: null },
      vertical: { edge: null, targetY: null }
    };
    
    for (const other of otherWindows) {
      // Check horizontal edges (left/right)
      // Right edge of current to left edge of other
      if (Math.abs((currentX + currentWidth) - other.x) < SNAP_THRESHOLD &&
          currentY < other.y + other.height &&
          currentY + currentHeight > other.y) {
        edgeGlow.value.right = true;
        potentialSnap.horizontal = {
          edge: 'right',
          targetX: other.x - currentWidth
        };
      }
      
      // Left edge of current to right edge of other
      if (Math.abs(currentX - (other.x + other.width)) < SNAP_THRESHOLD &&
          currentY < other.y + other.height &&
          currentY + currentHeight > other.y) {
        edgeGlow.value.left = true;
        potentialSnap.horizontal = {
          edge: 'left',
          targetX: other.x + other.width
        };
      }
      
      // Check vertical edges (top/bottom)
      // Bottom edge of current to top edge of other
      if (Math.abs((currentY + currentHeight) - other.y) < SNAP_THRESHOLD &&
          currentX < other.x + other.width &&
          currentX + currentWidth > other.x) {
        edgeGlow.value.bottom = true;
        potentialSnap.vertical = {
          edge: 'bottom',
          targetY: other.y - currentHeight
        };
      }
      
      // Top edge of current to bottom edge of other
      if (Math.abs(currentY - (other.y + other.height)) < SNAP_THRESHOLD &&
          currentX < other.x + other.width &&
          currentX + currentWidth > other.x) {
        edgeGlow.value.top = true;
        potentialSnap.vertical = {
          edge: 'top',
          targetY: other.y + other.height
        };
      }
    }
  } catch (error) {
    console.error("Failed to check edge proximity:", error);
  }
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
  
  // Start checking for edge proximity while dragging
  dragCheckInterval = window.setInterval(checkEdgeProximity, 50);
  
  appWindow.startDragging();
}

async function endDragging() {
  isDragging = false;
  
  if (dragCheckInterval !== null) {
    clearInterval(dragCheckInterval);
    dragCheckInterval = null;
  }
  
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

async function handleMouseUp(_event: MouseEvent) {
  if (state.value === State.Drawing) {
    endDrawing();
    return;
  }
  
  if (isDragging) { // Handle snap-to-edge
    endDragging();
  }
}

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
  if (!stage) return;
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
  if (!stage || isProcessingOcr.value) return;
  
  if (state.value != State.OCR) {
    if (ocrTextResults.value.length > 0) {
      state.value = State.OCR;
    } else {
      isProcessingOcr.value = true;
      ocrZoomScale = zoom_scale;
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
  zoom_scale += delta
  zoom_scale = Math.max(5, Math.min(zoom_scale, 500))

  tips.value = zoom_scale + "%"
  if (hideTipTimeout) { clearTimeout(hideTipTimeout) }
  show_tips.value = true
  
  await scaleWindow()

  hideTipTimeout = setTimeout(() => {
    show_tips.value = false
  }, 1000)
}

async function scaleWindow() {
  if (!backImg.value) return
  
  const originalWidth = backImg.value.width / init_scale_factor.value
  const originalHeight = backImg.value.height / init_scale_factor.value
  
  const newWidth = Math.round(originalWidth * zoom_scale / 100)
  const newHeight = Math.round(originalHeight * zoom_scale / 100)
  
  await appWindow.setSize(new LogicalSize(newWidth, newHeight))
  
  canvasRef.value?.updateSize(newWidth, newHeight);
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

function handleWindowResize() {
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
  if (!drawingLayer || !stage) return;
  
  const pos = stage.getPointerPosition();
  if (!pos) return;

  if (drawState.value === DrawState.Text) {
    finishTextInput()
    startTextInput(event, pos);
    return;
  }
  
  isDrawing = true;
  const scaleRatio = zoom_scale / 100;
  startPoint = { x: pos.x / scaleRatio, y: pos.y / scaleRatio };

  if (drawState.value === DrawState.Pen) {
    currentPath = new Konva.Line({
      stroke: '#ff0000',
      strokeWidth: 3 / scaleRatio,
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
      strokeWidth: 3 / scaleRatio,
      fill: 'transparent',
    });
    drawingLayer.add(currentRect);
  } else if (drawState.value === DrawState.Arrow) {
    currentArrow = new Konva.Arrow({
      points: [pos.x / scaleRatio, pos.y / scaleRatio, pos.x / scaleRatio, pos.y / scaleRatio],
      stroke: '#ff0000',
      strokeWidth: 3 / scaleRatio,
      fill: '#ff0000',
      pointerLength: 15 / scaleRatio,
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
    const width = scaledX - startPoint.x;
    const height = scaledY - startPoint.y;
    
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
    currentArrow.points([startPoint.x, startPoint.y, scaledX, scaledY]);
  }
  
  drawingLayer?.batchDraw();
}

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
  
  if (currentPath) {
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
  
  const children = drawingLayer.getChildren();
  if (children.length > 0) {
    children[children.length - 1].destroy();
    drawingLayer.batchDraw();
  }
}

function startTextInput(event: MouseEvent, stagePos: { x: number; y: number }) {
  const rect = (event.target as HTMLElement).getBoundingClientRect();
  textInputPosition.value = {
    x: event.clientX - rect.left,
    y: event.clientY - rect.top
  };
  
  const scaleRatio = zoom_scale / 100;
  startPoint = { x: stagePos.x / scaleRatio, y: stagePos.y / scaleRatio };
  
  showTextInput.value = true;
  textInputValue.value = '';
}

function finishTextInput() {
  if (!drawingLayer || !startPoint || !textInputValue.value.trim()) {
    cancelTextInput();
    return;
  }

  const scaleRatio = zoom_scale / 100;
  currentText = new Konva.Text({
    x: startPoint.x,
    y: startPoint.y,
    text: textInputValue.value,
    fontSize: 16 / scaleRatio,
    fill: '#ff0000',
  });
  
  drawingLayer.add(currentText);
  drawingLayer.batchDraw();
  
  drawingHistory.push(currentText.clone());
  
  cancelTextInput();
  currentText = null;
  startPoint = null;
}

function cancelTextInput() {
  showTextInput.value = false;
  textInputValue.value = '';
  startPoint = null;
}

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
          zoom: zoom_scale,
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

.edge-glow-left {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  width: 10px;
  height: 100%;
  border-radius: 8px;
  pointer-events: none;
  background: linear-gradient(to right, var(--theme-primary), rgba(255,0,0,0));
  filter: blur(4px);
  z-index: 2;
}

.edge-glow-right {
  content: '';
  position: absolute;
  top: 0;
  right: 0;
  width: 10px;
  height: 100%;
  border-radius: 8px;
  pointer-events: none;
  background: linear-gradient(to left, var(--theme-primary), rgba(255,0,0,0));
  filter: blur(4px);
  z-index: 2;
}

.edge-glow-top {
  content: '';
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  height: 10px;
  border-radius: 8px;
  pointer-events: none;
  background: linear-gradient(to bottom, var(--theme-primary), rgba(255,0,0,0));
  filter: blur(4px);
  z-index: 2;
}

.edge-glow-bottom {
  content: '';
  position: absolute;
  left: 0;
  bottom: 0;
  width: 100%;
  height: 10px;
  border-radius: 8px;
  pointer-events: none;
  background: linear-gradient(to top, var(--theme-primary), rgba(255,0,0,0));
  filter: blur(4px);
  z-index: 2;
}
</style>
