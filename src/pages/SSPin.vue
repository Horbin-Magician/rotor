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
import { getCurrentWindow, getAllWindows, PhysicalPosition, LogicalPosition, cursorPosition } from '@tauri-apps/api/window';
import Konva from "konva";
import { event } from "@tauri-apps/api";

const appWindow = getCurrentWindow()
const snapThreshold = 15; // Reduced snap threshold to make snapping less aggressive
const snapHysteresis = 25; // Distance required to break away from a snap (greater than snapThreshold)
const backImg = ref()
const backImgRef = ref<HTMLImageElement | null>(null)
const backImgURL = ref()

let backImgLayer: Konva.Layer | null = null

enum State {
  Default,
  Moving
}

let state = State.Default
let mouse_delta_x: number = 0;
let mouse_delta_y: number = 0;

// Track snapping state to prevent jittering
let isSnappedHorizontally = false;
let isSnappedVertically = false;
let lastSnapX: number | null = null;
let lastSnapY: number | null = null;
let lastUpdateTime = 0;
const updateInterval = 50; // Minimum time between position updates in ms

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
  
  // Reset snapping state when starting a new drag
  isSnappedHorizontally = false;
  isSnappedVertically = false;
  lastSnapX = null;
  lastSnapY = null;
}

// Mouse event handlers
function handleMouseUp(_event: MouseEvent) {
  state = State.Default;
}

// Mouse event handlers
function handleMouseMove(_event: MouseEvent) {
  if(state === State.Moving) {
    handleWindowMove()
  }
}

function handleKeyup(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    appWindow.close()
  }
}

// Function to handle window movement
async function handleWindowMove() {
  // Implement debouncing to prevent too frequent updates
  const now = Date.now();
  if (now - lastUpdateTime < updateInterval) {
    return; // Skip this update if it's too soon after the last one
  }
  lastUpdateTime = now;

  const allWindows = await getAllWindows();
  const currentSize = await appWindow.outerSize();
  const currentPosition = await appWindow.outerPosition();
  
  // Filter out the current window and only consider SSPin windows
  const otherWindows = allWindows.filter(window => 
    window.label !== appWindow.label && 
    window.label.includes('sspin')
  );
  
  // Track if we need to snap in either direction
  let shouldSnapHorizontal = false;
  let shouldSnapVertical = false;
  let snapToX = currentPosition.x;
  let snapToY = currentPosition.y;
  let minDistance_x = Infinity;
  let minDistance_y = Infinity;
  
  function rangesOverlap(aStart: number, aEnd: number, bStart: number, bEnd: number) {
    return Math.max(aStart, bStart) < Math.min(aEnd, bEnd);
  }

  // Check if we should break out of existing snaps based on hysteresis
  if (isSnappedHorizontally && lastSnapX !== null) {
    const distanceFromSnap = Math.abs(currentPosition.x - lastSnapX);
    if (distanceFromSnap > snapHysteresis) {
      isSnappedHorizontally = false;
      lastSnapX = null;
    }
  }
  
  if (isSnappedVertically && lastSnapY !== null) {
    const distanceFromSnap = Math.abs(currentPosition.y - lastSnapY);
    if (distanceFromSnap > snapHysteresis) {
      isSnappedVertically = false;
      lastSnapY = null;
    }
  }
  
  // Only check for new snaps if we're not already snapped
  if (!isSnappedHorizontally || !isSnappedVertically) {
    for (const otherWindow of otherWindows) {
      const otherPosition = await otherWindow.outerPosition();
      const otherSize = await otherWindow.outerSize();
      
      // Only check horizontal snapping if not already snapped horizontally
      if (!isSnappedHorizontally) {
        // Right edge snaps to left edge
        const distRightToLeft = Math.abs((currentPosition.x + currentSize.width) - otherPosition.x);
        if (distRightToLeft < snapThreshold &&
            rangesOverlap(currentPosition.y, currentPosition.y + currentSize.height,
                        otherPosition.y, otherPosition.y + otherSize.height)) {
          if (distRightToLeft < minDistance_x) {
            shouldSnapHorizontal = true;
            minDistance_x = distRightToLeft;
            snapToX = otherPosition.x - currentSize.width;
          }
        }

        // Left edge snaps to right edge
        const distLeftToRight = Math.abs(currentPosition.x - (otherPosition.x + otherSize.width));
        if (distLeftToRight < snapThreshold &&
            rangesOverlap(currentPosition.y, currentPosition.y + currentSize.height,
                        otherPosition.y, otherPosition.y + otherSize.height)) {
          if (distLeftToRight < minDistance_x) {
            shouldSnapHorizontal = true;
            minDistance_x = distLeftToRight;
            snapToX = otherPosition.x + otherSize.width;
          }
        }
      }
      
      // Only check vertical snapping if not already snapped vertically
      if (!isSnappedVertically) {
        // Bottom edge snaps to top edge
        const distBottomToTop = Math.abs((currentPosition.y + currentSize.height) - otherPosition.y);
        if (distBottomToTop < snapThreshold &&
            rangesOverlap(currentPosition.x, currentPosition.x + currentSize.width,
                        otherPosition.x, otherPosition.x + otherSize.width)) {
          if (distBottomToTop < minDistance_y) {
            shouldSnapVertical = true;
            minDistance_y = distBottomToTop;
            snapToY = otherPosition.y - currentSize.height;
          }
        }

        // Top edge snaps to bottom edge
        const distTopToBottom = Math.abs(currentPosition.y - (otherPosition.y + otherSize.height));
        if (distTopToBottom < snapThreshold &&
            rangesOverlap(currentPosition.x, currentPosition.x + currentSize.width,
                        otherPosition.x, otherPosition.x + otherSize.width)) {
          if (distTopToBottom < minDistance_y) {
            shouldSnapVertical = true;
            minDistance_y = distTopToBottom;
            snapToY = otherPosition.y + otherSize.height;
          }
        }
      }
    }
  }

  // Update snapping state
  if (shouldSnapHorizontal) {
    isSnappedHorizontally = true;
    lastSnapX = snapToX;
  }
  
  if (shouldSnapVertical) {
    isSnappedVertically = true;
    lastSnapY = snapToY;
  }

  // Only update position if we need to snap in at least one direction
  if (shouldSnapHorizontal || shouldSnapVertical) {
    // If we're only snapping in one direction, keep the current position for the other
    const finalX = shouldSnapHorizontal ? snapToX : currentPosition.x;
    const finalY = shouldSnapVertical ? snapToY : currentPosition.y;
    
    // Only update if the position has actually changed
    if (finalX !== currentPosition.x || finalY !== currentPosition.y) {
      await appWindow.setPosition(new PhysicalPosition(finalX, finalY));
    }
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
