<template>
  <main class="container" 
        :class="{ 'resizing-selection': isResizingSelection }"
        :style="{ cursor: resizeCursor }"
        @mousedown="handleMouseDown"
        @mouseup="handleMouseUp"
        @mousemove="handleMouseMove"
        @mouseleave="handleMouseLeave"
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

    <div v-if="canResizeSelection" class="resize-handles">
      <div class="resize-handle resize-top-left" @mousedown.stop.prevent="startSelectionResizeFromHandle($event, resizeHandleEdges.topLeft)" />
      <div class="resize-handle resize-top-right" @mousedown.stop.prevent="startSelectionResizeFromHandle($event, resizeHandleEdges.topRight)" />
      <div class="resize-handle resize-bottom-left" @mousedown.stop.prevent="startSelectionResizeFromHandle($event, resizeHandleEdges.bottomLeft)" />
      <div class="resize-handle resize-bottom-right" @mousedown.stop.prevent="startSelectionResizeFromHandle($event, resizeHandleEdges.bottomRight)" />
      <div class="resize-handle resize-left" @mousedown.stop.prevent="startSelectionResizeFromHandle($event, resizeHandleEdges.left)" />
      <div class="resize-handle resize-right" @mousedown.stop.prevent="startSelectionResizeFromHandle($event, resizeHandleEdges.right)" />
      <div class="resize-handle resize-top" @mousedown.stop.prevent="startSelectionResizeFromHandle($event, resizeHandleEdges.top)" />
      <div class="resize-handle resize-bottom" @mousedown.stop.prevent="startSelectionResizeFromHandle($event, resizeHandleEdges.bottom)" />
    </div>
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
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { useI18n } from 'vue-i18n';
import { getCurrentWindow, LogicalPosition, LogicalSize, monitorFromPoint, PhysicalPosition } from '@tauri-apps/api/window';
import { Menu } from '@tauri-apps/api/menu';
import { writeImage } from '@tauri-apps/plugin-clipboard-manager';
import { UnlistenFn } from "@tauri-apps/api/event";
import { warn } from "@tauri-apps/plugin-log";
import { getConfig } from '../shared/api/core';
import { connectDataSocket, requestDataSocketBytes } from '../shared/api/dataSocket';
import {
  deletePinRecord,
  getPinState,
  imageToText,
  saveImage as saveScreenshotImage,
  updatePinSelection,
  type PinConfig,
  type TextResult,
} from '../features/screenshot/api';

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

interface ResizeEdges {
  horizontal: 'left' | 'right' | null;
  vertical: 'top' | 'bottom' | null;
}

interface CropRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface ResizeStartState {
  edges: ResizeEdges;
  screenX: number;
  screenY: number;
  crop: CropRegion;
  windowPosition: PhysicalPosition;
  scaleFactor: number;
  contentScaleFactor: number;
}

interface ResizeWindowState {
  size: {
    width: number;
    height: number;
  };
  position: {
    x: number;
    y: number;
  } | null;
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
let unlisten_show_pin: UnlistenFn | null = null;
let unlistenFocusChanged: UnlistenFn | null = null;
let unlistenScaleChanged: UnlistenFn | null = null;

let ws: WebSocket | null = null

// Initialize WebSocket connection with dynamic port
async function initWebSocket() {
  ws = await connectDataSocket()
}

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
let pendingDragViewportUpdate = false
let potentialSnap: EdgeSnap = {
  horizontal: { edge: null, targetX: null },
  vertical: { edge: null, targetY: null }
}

const resizeCursor = ref('default')
const isResizingSelection = ref(false)
let resizeStartState: ResizeStartState | null = null
let pendingResizeWindowState: ResizeWindowState | null = null
let resizeFrameId: number | null = null
let resizeApplyPromise: Promise<void> | null = null

const backImg = ref<ImageBitmap | null>(null)
const cropRegion = ref<CropRegion>({ x: 0, y: 0, width: 0, height: 0 })
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

const RESIZE_HANDLE_SIZE = 8;
const MIN_CROP_SIZE = 12;
const resizeHandleEdges = {
  topLeft: { horizontal: 'left', vertical: 'top' },
  topRight: { horizontal: 'right', vertical: 'top' },
  bottomLeft: { horizontal: 'left', vertical: 'bottom' },
  bottomRight: { horizontal: 'right', vertical: 'bottom' },
  left: { horizontal: 'left', vertical: null },
  right: { horizontal: 'right', vertical: null },
  top: { horizontal: null, vertical: 'top' },
  bottom: { horizontal: null, vertical: 'bottom' },
} satisfies Record<string, ResizeEdges>;

const canResizeSelection = computed(() =>
  state.value !== State.Drawing &&
  !!backImg.value &&
  cropRegion.value.width > 0 &&
  cropRegion.value.height > 0
);

// Load shortcuts from configuration
async function loadShortcuts() {
  try {
    const saveShortcut = await getConfig("shortcut_pinwin_save");
    const closeShortcut = await getConfig("shortcut_pinwin_close");
    const copyShortcut = await getConfig("shortcut_pinwin_copy");
    const hideShortcut = await getConfig("shortcut_pinwin_hide");
    
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

async function getPinContentScaleFactor(pinConfig: PinConfig) {
  const sourceX = pinConfig.monitor_pos[0] + pinConfig.rect[0];
  const sourceY = pinConfig.monitor_pos[1] + pinConfig.rect[1];

  try {
    const monitor = await monitorFromPoint(sourceX, sourceY);
    if (monitor?.scaleFactor) {
      return monitor.scaleFactor;
    }
  } catch (error) {
    warn(`Failed to read pin source monitor scale: ${error}`);
  }

  return window.devicePixelRatio || await appWindow.scaleFactor().catch(() => 1);
}

async function syncCurrentScaleFactor(scaleFactor?: number) {
  const nextScaleFactor = scaleFactor ?? await appWindow.scaleFactor().catch(() => window.devicePixelRatio || 1);
  scale_factor.value = nextScaleFactor || window.devicePixelRatio || 1;
  return scale_factor.value;
}

async function tryLoadScreenShot(id: number): Promise<boolean> {
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    try {
      await initWebSocket()
    } catch (error) {
      warn(`WebSocket not ready for pin id ${id}: ${error}`)
      return false
    }
  }

  const pin_config = await getPinState(id);
  if (!pin_config) {
    warn(`No pin config found for id: ${id}`);
    return false;
  }
  
  init_scale_factor.value = await getPinContentScaleFactor(pin_config);
  await syncCurrentScaleFactor();
  
  const crop_width = pin_config.rect[2];
  const crop_height = pin_config.rect[3];
  const win_width = Math.round(crop_width / init_scale_factor.value);
  const win_height = Math.round(crop_height / init_scale_factor.value);
  
  await appWindow.setSize(new LogicalSize(win_width, win_height))
  await appWindow.setPosition(new PhysicalPosition((
    pin_config.monitor_pos[0] + pin_config.rect[0] + pin_config.offset[0] || 0),
    (pin_config.monitor_pos[1] + pin_config.rect[1] + pin_config.offset[1] || 0)
  ));

  const socket = ws;
  if (!socket) {
    warn(`WebSocket not initialized for pin id ${id}`);
    return false;
  }

  try {
    const imgBuf = await requestDataSocketBytes(socket, appWindow.label)
    if (imgBuf.byteLength === 0) {
      throw new Error(`No image data returned for pin id ${id}`);
    }

    const full_monitor_width = pin_config.monitor_size[0];
    const full_monitor_height = pin_config.monitor_size[1];
    const fullImgData = new ImageData(new Uint8ClampedArray(imgBuf), full_monitor_width, full_monitor_height);
    backImg.value = await createImageBitmap(fullImgData);
    await completeInitialization(pin_config);
    return true;
  } catch (error) {
    warn(`Failed to create image bitmap for pin id ${id}: ${error}`);
    await deletePinRecord(pin_id);
    await appWindow.close();
    return false;
  }
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
  
  if (!backImg.value) return;
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

function clamp(value: number, min: number, max: number) {
  if (max < min) return min;
  return Math.min(Math.max(value, min), max);
}

function getResizeEdges(event: MouseEvent): ResizeEdges | null {
  if (!backImg.value || cropRegion.value.width <= 0 || cropRegion.value.height <= 0) {
    return null;
  }

  const left = event.clientX <= RESIZE_HANDLE_SIZE;
  const right = event.clientX >= window.innerWidth - RESIZE_HANDLE_SIZE;
  const top = event.clientY <= RESIZE_HANDLE_SIZE;
  const bottom = event.clientY >= window.innerHeight - RESIZE_HANDLE_SIZE;

  const horizontal = left ? 'left' : right ? 'right' : null;
  const vertical = top ? 'top' : bottom ? 'bottom' : null;

  if (!horizontal && !vertical) return null;
  return { horizontal, vertical };
}

function getResizeCursor(edges: ResizeEdges | null) {
  if (!edges) return 'default';

  if (
    (edges.horizontal === 'left' && edges.vertical === 'top') ||
    (edges.horizontal === 'right' && edges.vertical === 'bottom')
  ) {
    return 'nwse-resize';
  }

  if (
    (edges.horizontal === 'right' && edges.vertical === 'top') ||
    (edges.horizontal === 'left' && edges.vertical === 'bottom')
  ) {
    return 'nesw-resize';
  }

  if (edges.horizontal) return 'ew-resize';
  if (edges.vertical) return 'ns-resize';
  return 'default';
}

function updateResizeCursor(event: MouseEvent) {
  if (state.value === State.Drawing || isDragging || isResizingSelection.value) return;
  resizeCursor.value = getResizeCursor(getResizeEdges(event));
}

function getLogicalSizeForCrop(crop: CropRegion, scaleFactor: number) {
  const zoomRatio = zoomScale.value / 100;
  return {
    width: Math.max(1, Math.round((crop.width / scaleFactor) * zoomRatio)),
    height: Math.max(1, Math.round((crop.height / scaleFactor) * zoomRatio)),
  };
}

function getWindowPositionForCrop(crop: CropRegion, startState: ResizeStartState) {
  const zoomRatio = zoomScale.value / 100;
  const physicalRatio = (startState.scaleFactor / startState.contentScaleFactor) * zoomRatio;
  let x = startState.windowPosition.x;
  let y = startState.windowPosition.y;

  if (startState.edges.horizontal === 'left') {
    x += Math.round((crop.x - startState.crop.x) * physicalRatio);
  }
  if (startState.edges.vertical === 'top') {
    y += Math.round((crop.y - startState.crop.y) * physicalRatio);
  }

  return { x, y };
}

function getResizedCrop(screenX: number, screenY: number, startState: ResizeStartState): CropRegion {
  const zoomRatio = zoomScale.value / 100;
  const sourceWidth = backImg.value?.width ?? startState.crop.x + startState.crop.width;
  const sourceHeight = backImg.value?.height ?? startState.crop.y + startState.crop.height;
  const deltaX = Math.round(
    (screenX - startState.screenX) * startState.contentScaleFactor / zoomRatio
  );
  const deltaY = Math.round(
    (screenY - startState.screenY) * startState.contentScaleFactor / zoomRatio
  );

  let left = startState.crop.x;
  let top = startState.crop.y;
  let right = startState.crop.x + startState.crop.width;
  let bottom = startState.crop.y + startState.crop.height;

  if (startState.edges.horizontal === 'left') {
    left = clamp(startState.crop.x + deltaX, 0, right - MIN_CROP_SIZE);
  } else if (startState.edges.horizontal === 'right') {
    right = clamp(right + deltaX, left + MIN_CROP_SIZE, sourceWidth);
  }

  if (startState.edges.vertical === 'top') {
    top = clamp(startState.crop.y + deltaY, 0, bottom - MIN_CROP_SIZE);
  } else if (startState.edges.vertical === 'bottom') {
    bottom = clamp(bottom + deltaY, top + MIN_CROP_SIZE, sourceHeight);
  }

  return {
    x: left,
    y: top,
    width: right - left,
    height: bottom - top,
  };
}

function getResizeWindowState(crop: CropRegion, startState: ResizeStartState): ResizeWindowState {
  const nextSize = getLogicalSizeForCrop(crop, startState.contentScaleFactor);
  const nextPosition = getWindowPositionForCrop(crop, startState);
  const shouldMove =
    startState.edges.horizontal === 'left' ||
    startState.edges.vertical === 'top';

  return {
    size: nextSize,
    position: shouldMove ? nextPosition : null,
  };
}

function applySelectionResize(screenX: number, screenY: number) {
  if (!resizeStartState || !backImg.value) return;

  const nextCrop = getResizedCrop(screenX, screenY, resizeStartState);
  const nextWindowState = getResizeWindowState(nextCrop, resizeStartState);
  cropRegion.value = nextCrop;
  canvasRef.value?.updateCrop(nextCrop, nextWindowState.size);
  pendingResizeWindowState = nextWindowState;
  requestResizeWindowApply();
}

async function startSelectionResize(event: MouseEvent, edges: ResizeEdges) {
  event.preventDefault();
  event.stopPropagation();

  resizeStartState = {
    edges,
    screenX: event.screenX,
    screenY: event.screenY,
    crop: { ...cropRegion.value },
    windowPosition: await appWindow.outerPosition(),
    scaleFactor: await syncCurrentScaleFactor(),
    contentScaleFactor: init_scale_factor.value,
  };
  isResizingSelection.value = true;
  resizeCursor.value = getResizeCursor(edges);

  window.addEventListener('mousemove', handleSelectionResizeMove, true);
  window.addEventListener('mouseup', handleSelectionResizeEnd, true);
}

async function startSelectionResizeFromHandle(event: MouseEvent, edges: ResizeEdges) {
  if (!canResizeSelection.value || event.button !== 0) return;
  await startSelectionResize(event, edges);
}

function handleSelectionResizeMove(event: MouseEvent) {
  if (!isResizingSelection.value) return;
  event.preventDefault();
  applySelectionResize(event.screenX, event.screenY);
}

async function flushResizeWindowApply() {
  if (!pendingResizeWindowState) return;

  const nextState = pendingResizeWindowState;
  pendingResizeWindowState = null;
  const resizeTasks: Promise<void>[] = [
    appWindow.setSize(new LogicalSize(nextState.size.width, nextState.size.height)),
  ];

  if (nextState.position) {
    resizeTasks.push(appWindow.setPosition(new PhysicalPosition(nextState.position.x, nextState.position.y)));
  }

  try {
    await Promise.all(resizeTasks);
  } catch (error) {
    warn(`Failed to apply pin selection resize: ${error}`);
  }
}

function requestResizeWindowApply() {
  if (resizeFrameId !== null) return;

  resizeFrameId = requestAnimationFrame(() => {
    resizeFrameId = null;
    if (!resizeApplyPromise) {
      resizeApplyPromise = flushResizeWindowApply().finally(() => {
        resizeApplyPromise = null;
        if (pendingResizeWindowState) {
          requestResizeWindowApply();
        }
      });
    }
  });
}

async function handleSelectionResizeEnd(event?: MouseEvent) {
  if (!isResizingSelection.value) return;
  event?.preventDefault();

  window.removeEventListener('mousemove', handleSelectionResizeMove, true);
  window.removeEventListener('mouseup', handleSelectionResizeEnd, true);
  const startState = resizeStartState;
  if (event) {
    applySelectionResize(event.screenX, event.screenY);
  }
  if (resizeApplyPromise) {
    await resizeApplyPromise;
  }
  if (resizeFrameId !== null) {
    cancelAnimationFrame(resizeFrameId);
    resizeFrameId = null;
  }
  if (pendingResizeWindowState) {
    await flushResizeWindowApply();
  }

  isResizingSelection.value = false;
  resizeStartState = null;
  pendingResizeWindowState = null;
  resizeCursor.value = 'default';
  canvasRef.value?.updateSize();
  updateToolbarVisibility();

  if (startState) {
    await persistPinState();
  }
}

async function persistPinState() {
  if (cropRegion.value.width <= 0 || cropRegion.value.height <= 0) return;

  const position = await appWindow.outerPosition();
  try {
    await updatePinSelection(
      pin_id,
      Math.round(cropRegion.value.x),
      Math.round(cropRegion.value.y),
      Math.round(cropRegion.value.width),
      Math.round(cropRegion.value.height),
      position.x,
      position.y,
      Math.round(zoomScale.value),
      await appWindow.isMinimized(),
    );
  } catch (error) {
    warn(`Failed to update pin state: ${error}`);
  }
}

async function handleMouseDown(event: MouseEvent) {
  if (state.value === State.Drawing) {
    if (event.button === 0) {
      startDrawing(event);
    }
  } else if (state.value === State.Default) {
    if (event.button === 0) {
      const resizeEdges = getResizeEdges(event);
      if (resizeEdges) {
        await startSelectionResize(event, resizeEdges);
      } else {
        startDragging();
      }
    }
  } else if (state.value === State.OCR) {
    if (event.button === 0 && !isOcrTextTarget(event.target)) {
      const resizeEdges = getResizeEdges(event);
      if (resizeEdges) {
        await startSelectionResize(event, resizeEdges);
      } else {
        startDragging();
      }
    }
  }
}

function isOcrTextTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false;
  return target.classList.contains('ocr-text-content') || !!target.closest('.ocr-text-overlay');
}

function startDragging() {
  isDragging = true;
  pendingDragViewportUpdate = false;
  potentialSnap = {
    horizontal: { edge: null, targetX: null },
    vertical: { edge: null, targetY: null }
  };
  window.addEventListener('mouseup', handleDragMouseUp, true);
  appWindow.startDragging().catch((error) => {
    warn(`Failed to start dragging pin window: ${error}`);
    isDragging = false;
    pendingDragViewportUpdate = false;
    window.removeEventListener('mouseup', handleDragMouseUp, true);
    updateToolbarVisibility();
  });
}

async function endDragging() {
  if (!isDragging) return;

  isDragging = false;
  window.removeEventListener('mouseup', handleDragMouseUp, true);
  
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

  if (pendingDragViewportUpdate) {
    pendingDragViewportUpdate = false;
    await syncCurrentScaleFactor();
    await scaleWindow();
  } else {
    canvasRef.value?.updateSize();
    updateToolbarVisibility();
  }

  await persistPinState();
}

function handleDragMouseUp(_event: MouseEvent) {
  if (!isDragging) return;
  void endDragging();
}

function handleMouseUp(event: MouseEvent) {
  if (isResizingSelection.value) {
    event.preventDefault();
    return;
  }

  if (state.value === State.Drawing) {
    endDrawing();
    return;
  }
  
  if (isDragging) { // Handle snap-to-edge
    endDragging();
  }
}

function handleMouseMove(event: MouseEvent) {
  if (isResizingSelection.value) {
    if (event.buttons === 0) {
      void handleSelectionResizeEnd(event);
    }
    return;
  }

  if (state.value === State.Drawing) {
    const mode = getActiveDrawTool();
    canvasRef.value?.continueDrawing(mode);
  } else if (state.value === State.Default || state.value === State.OCR) {
    updateResizeCursor(event);
  }
}

function handleMouseLeave(event: MouseEvent) {
  if (isResizingSelection.value) return;
  resizeCursor.value = 'default';
  if (isDragging) return;
  handleMouseUp(event);
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
    await deletePinRecord(pin_id);
  } catch (error) {
    warn(`Failed to delete pin record ${pin_id}: ${error}`);
  }
  appWindow.close();
}

async function saveImage() {
  const stage = canvasRef.value?.getStage()
  const pixelRatio = await syncCurrentScaleFactor()
  stage?.toBlob({
    pixelRatio,
    callback(blob) {
      if(!blob) return
      blob.arrayBuffer().then((imgBuf)=>{
        saveScreenshotImage(imgBuf).then((if_success)=>{
          if(if_success) closeWindow()
        })
      })
    }
  })
}

async function copyImage() {
  const stage = canvasRef.value?.getStage()
  const pixelRatio = await syncCurrentScaleFactor()
  stage?.toBlob({
    pixelRatio,
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
      const pixelRatio = await syncCurrentScaleFactor();
      stage.toBlob({
        pixelRatio,
        callback(blob) {
          if(!blob) {
            isProcessingOcr.value = false;
            return;
          }
          blob.arrayBuffer().then((imgBuf)=>{
            imageToText(imgBuf)
              .then((textResults)=>{
                ocrTextResults.value = textResults;
                isProcessingOcr.value = false;
                scale_factor.value = pixelRatio;
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
  const zoom_delta = parseInt(await getConfig("zoom_delta"), 10);

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

let toolbarVisibilityCheckId = 0;

async function scaleWindow() {
  if (!backImg.value) return
  
  const logicalWidth = cropRegion.value.width / init_scale_factor.value
  const logicalHeight = cropRegion.value.height / init_scale_factor.value
  
  const newWidth = Math.round(logicalWidth * zoomScale.value / 100)
  const newHeight = Math.round(logicalHeight * zoomScale.value / 100)
  
  await appWindow.setSize(new LogicalSize(newWidth, newHeight))
  canvasRef.value?.updateSize()
}

function updateToolbarVisibility(focused?: boolean) {
  toolbarVisibilityCheckId += 1;

  if (typeof focused === 'boolean') {
    toolbarVisible.value = focused;
    return;
  }

  const checkId = toolbarVisibilityCheckId;
  appWindow.isFocused()
    .then((focused) => {
      if (checkId === toolbarVisibilityCheckId) {
        toolbarVisible.value = focused
      }
    })
    .catch((error) => {
      warn(`Failed to update pin toolbar visibility: ${error}`);
    });
}

async function handleWindowResize() {
  if (isDragging) {
    pendingDragViewportUpdate = true;
    return;
  }

  if (isResizingSelection.value) {
    return;
  }

  await syncCurrentScaleFactor()
  canvasRef.value?.updateSize()
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
  try {
    await initWebSocket(); // Initialize WebSocket connection
  } catch (error) {
    warn(`Failed to initialize websocket for pin ${pin_id}: ${error}`);
  }

  await loadShortcuts();
  
  window.addEventListener('keyup', handleKeyup);
  window.addEventListener('resize', handleWindowResize);

  unlistenFocusChanged = await appWindow.onFocusChanged(async (event) => {
    const focused = event.payload;
    updateToolbarVisibility(focused);
    if(!focused) {
      await persistPinState();
    }
  });

  unlistenScaleChanged = await appWindow.onScaleChanged(async ({ payload }) => {
    if (isDragging) {
      pendingDragViewportUpdate = true;
      return;
    }

    await syncCurrentScaleFactor(payload.scaleFactor);
    await scaleWindow();
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
    if (state.value === State.OCR && isOcrTextTarget(event.target)) {
      return;
    }

    event.preventDefault();
    menu.popup(new LogicalPosition(event.clientX, event.clientY));
  });

  {
    let result = await tryLoadScreenShot(pin_id);
    if (!result) {
      unlisten_show_pin = await appWindow.listen('show-pin', async (_event) => {
        const loaded = await tryLoadScreenShot(pin_id);
        if(loaded && unlisten_show_pin) {
          unlisten_show_pin()
          unlisten_show_pin = null
        }
      });
    }
  }
});

// do something before unmounted
onBeforeUnmount(async () => {
  window.removeEventListener('keyup', handleKeyup)
  window.removeEventListener('resize', handleWindowResize)
  window.removeEventListener('mousemove', handleSelectionResizeMove, true)
  window.removeEventListener('mouseup', handleSelectionResizeEnd, true)
  window.removeEventListener('mouseup', handleDragMouseUp, true)
  if (unlisten_show_pin) { unlisten_show_pin() }
  if (unlistenFocusChanged) { unlistenFocusChanged() }
  if (unlistenScaleChanged) { unlistenScaleChanged() }
  ws?.close()
  ws = null
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

.container.resizing-selection {
  box-shadow: inset 0 0 0 1px var(--theme-primary-pressed);
}

.resize-handles {
  position: absolute;
  inset: 0;
  z-index: 400;
  pointer-events: none;
}

.resize-handle {
  position: absolute;
  pointer-events: auto;
}

.resize-left,
.resize-right {
  top: 0;
  bottom: 0;
  width: 8px;
  cursor: ew-resize;
}

.resize-left {
  left: 0;
}

.resize-right {
  right: 0;
}

.resize-top,
.resize-bottom {
  left: 0;
  right: 0;
  height: 8px;
  cursor: ns-resize;
}

.resize-top {
  top: 0;
}

.resize-bottom {
  bottom: 0;
}

.resize-top-left,
.resize-top-right,
.resize-bottom-left,
.resize-bottom-right {
  width: 12px;
  height: 12px;
  z-index: 1;
}

.resize-top-left {
  top: 0;
  left: 0;
  cursor: nwse-resize;
}

.resize-top-right {
  top: 0;
  right: 0;
  cursor: nesw-resize;
}

.resize-bottom-left {
  bottom: 0;
  left: 0;
  cursor: nesw-resize;
}

.resize-bottom-right {
  bottom: 0;
  right: 0;
  cursor: nwse-resize;
}
</style>
