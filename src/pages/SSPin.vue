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
const snapThreshold = 20; // Distance in pixels for snapping windows together
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
  appWindow.startDragging()
  state = State.Moving // appWindow.startDragging();
}

// Mouse event handlers
function handleMouseUp(_event: MouseEvent) {
  state = State.Default
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
  const allWindows = await getAllWindows();
  const currentSize = await appWindow.outerSize();
  const currentPosition = await appWindow.outerPosition();
  let minDistance_x = Infinity;
  let minDistance_y = Infinity;
  
  // Filter out the current window and only consider SSPin windows
  const otherWindows = allWindows.filter(window => 
    window.label !== appWindow.label && 
    window.label.includes('sspin')
  );
  
  let shouldSnap = false;
  let snapToX = currentPosition.x;
  let snapToY = currentPosition.y;
  
  function rangesOverlap(aStart: number, aEnd: number, bStart: number, bEnd: number) {
    return Math.max(aStart, bStart) < Math.min(aEnd, bEnd);
  }

  for (const otherWindow of otherWindows) {
    const otherPosition = await otherWindow.outerPosition();
    const otherSize = await otherWindow.outerSize();
    
    // 右边缘吸附到左边缘
    const distRightToLeft = Math.abs((currentPosition.x + currentSize.width) - otherPosition.x);
    if (distRightToLeft < snapThreshold &&
        rangesOverlap(currentPosition.y, currentPosition.y + currentSize.height,
                      otherPosition.y, otherPosition.y + otherSize.height)) {
      if (distRightToLeft < minDistance_x) {
        shouldSnap = true;
        minDistance_x = distRightToLeft;
        snapToX = otherPosition.x - currentSize.width;
      }
    }

    // 左边缘吸附到右边缘
    const distLeftToRight = Math.abs(currentPosition.x - (otherPosition.x + otherSize.width));
    if (distLeftToRight < snapThreshold &&
        rangesOverlap(currentPosition.y, currentPosition.y + currentSize.height,
                      otherPosition.y, otherPosition.y + otherSize.height)) {
      if (distLeftToRight < minDistance_x) {
        shouldSnap = true;
        minDistance_x = distLeftToRight;
        snapToX = otherPosition.x + otherSize.width;
      }
    }

    // 下边缘吸附到上边缘
    const distBottomToTop = Math.abs((currentPosition.y + currentSize.height) - otherPosition.y);
    if (distBottomToTop < snapThreshold &&
        rangesOverlap(currentPosition.x, currentPosition.x + currentSize.width,
                      otherPosition.x, otherPosition.x + otherSize.width)) {
      if (distBottomToTop < minDistance_y) {
        shouldSnap = true;
        minDistance_y = distBottomToTop;
        snapToY = otherPosition.y - currentSize.height;
      }
    }

    // 上边缘吸附到下边缘
    const distTopToBottom = Math.abs(currentPosition.y - (otherPosition.y + otherSize.height));
    if (distTopToBottom < snapThreshold &&
        rangesOverlap(currentPosition.x, currentPosition.x + currentSize.width,
                      otherPosition.x, otherPosition.x + otherSize.width)) {
      if (distTopToBottom < minDistance_y) {
        shouldSnap = true;
        minDistance_y = distTopToBottom;
        snapToY = otherPosition.y + otherSize.height;
      }
    }
  }

  if(shouldSnap) {
    await appWindow.setPosition(new PhysicalPosition(snapToX, snapToY));
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
