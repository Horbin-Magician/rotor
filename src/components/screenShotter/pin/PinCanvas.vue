<template>
  <div ref="stageContainer" id="stage"></div>
</template>

<script setup lang="ts">
import { ref, onBeforeUnmount } from 'vue';
import Konva from 'konva';

interface Props {
  zoomScale: number;
}

const props = defineProps<Props>();

const stageContainer = ref<HTMLDivElement | null>(null);

let stage: Konva.Stage | null = null;
let backImgLayer: Konva.Layer | null = null;
let drawingLayer: Konva.Layer | null = null;

// Drawing state
let currentPath: Konva.Line | null = null;
let currentArrow: Konva.Arrow | null = null;
let currentRect: Konva.Rect | null = null;
let currentText: Konva.Text | null = null;
let drawingHistory: any[] = [];
let isDrawing = false;
let startPoint: { x: number; y: number } | null = null;

type DrawMode = 'pen' | 'rect' | 'arrow' | 'text';

function initStage(
  backImg: ImageBitmap,
  crop: { x: number; y: number; width: number; height: number }
) {
  if (!stageContainer.value) return;

  // Add the image with proper crop and positioning
  backImgLayer = new Konva.Layer(); // Create background layer with black background
  const konvaImage = new Konva.Image({
    x: 0,
    y: 0,
    image: backImg,
    crop: {
      x: crop.x,
      y: crop.y,
      width: crop.width,
      height: crop.height,
    },
    width: window.innerWidth,
    height: window.innerHeight,
  });
  backImgLayer.add(konvaImage);

  // Create stage with crop dimensions
  stage = new Konva.Stage({
    container: 'stage',
    width: window.innerWidth,
    height: window.innerHeight,
  });
  stage.add(backImgLayer);

  // Create drawing layer
  drawingLayer = new Konva.Layer();
  stage.add(drawingLayer);
}

function updateSize() {
  if (!stage || !backImgLayer) return;

  stage.width(window.innerWidth);
  stage.height(window.innerHeight);
  
  const konvaImage = backImgLayer.findOne('Image') as Konva.Image;
  if (konvaImage) {
    konvaImage.width(window.innerWidth);
    konvaImage.height(window.innerHeight);
    backImgLayer.batchDraw();
  }

  if (drawingLayer) {
    const scaleRatio = props.zoomScale / 100;
    drawingLayer.scale({ x: scaleRatio, y: scaleRatio });
    drawingLayer.batchDraw();
  }
}

function getStage() {
  return stage;
}

function getDrawingLayer() {
  return drawingLayer;
}

/**
 * Start drawing based on the current draw mode
 * @param mode - The drawing mode (pen, rect, arrow, text)
 * @param onTextStart - Callback for when text input should start, returns stage position
 */
function startDrawing(mode: DrawMode, onTextStart?: (pos: { x: number; y: number }) => void) {
  if (!drawingLayer || !stage) return;
  
  const pos = stage.getPointerPosition();
  if (!pos) return;

  // Handle text mode differently
  if (mode === 'text') {
    if (onTextStart) {
      onTextStart(pos);
    }
    return;
  }
  
  isDrawing = true;
  const scaleRatio = props.zoomScale / 100;
  startPoint = { x: pos.x / scaleRatio, y: pos.y / scaleRatio };

  if (mode === 'pen') {
    currentPath = new Konva.Line({
      stroke: '#ff0000',
      strokeWidth: 3 / scaleRatio,
      globalCompositeOperation: 'source-over',
      lineCap: 'round',
      lineJoin: 'round',
      points: [pos.x / scaleRatio, pos.y / scaleRatio, pos.x / scaleRatio, pos.y / scaleRatio],
    });
    drawingLayer.add(currentPath);
  } else if (mode === 'rect') {
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
  } else if (mode === 'arrow') {
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

/**
 * Continue drawing based on the current mode
 * @param mode - The drawing mode (pen, rect, arrow)
 */
function continueDrawing(mode: DrawMode) {
  if (!isDrawing || !stage || !drawingLayer) return;
  
  const pos = stage.getPointerPosition();
  if (!pos) return;

  const scaleRatio = props.zoomScale / 100;
  const scaledX = pos.x / scaleRatio;
  const scaledY = pos.y / scaleRatio;

  if (mode === 'pen' && currentPath) {
    const newPoints = currentPath.points().concat([scaledX, scaledY]);
    currentPath.points(newPoints);
  } else if (mode === 'rect' && currentRect && startPoint) {
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
  } else if (mode === 'arrow' && currentArrow && startPoint) {
    currentArrow.points([startPoint.x, startPoint.y, scaledX, scaledY]);
  }
  
  drawingLayer.batchDraw();
}

/**
 * Smooth the points of a line using a simple averaging algorithm
 * @param points - Array of x,y coordinates
 * @param alpha - Smoothing factor (default: 0.3)
 * @returns Smoothed array of points
 */
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

/**
 * End the current drawing operation
 */
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

/**
 * Undo the last drawing operation
 */
function undoDrawing() {
  if (!drawingLayer || drawingHistory.length === 0) return;
  
  drawingHistory.pop();
  
  const children = drawingLayer.getChildren();
  if (children.length > 0) {
    children[children.length - 1].destroy();
    drawingLayer.batchDraw();
  }
}

/**
 * Add text to the drawing layer
 * @param text - The text content
 * @param pos - Position to place the text (in stage coordinates)
 */
function addText(text: string, pos: { x: number; y: number }) {
  if (!drawingLayer || !text.trim()) return;

  const scaleRatio = props.zoomScale / 100;
  currentText = new Konva.Text({
    x: pos.x,
    y: pos.y,
    text: text,
    fontSize: 16 / scaleRatio,
    fill: '#ff0000',
  });
  
  drawingLayer.add(currentText);
  drawingLayer.batchDraw();
  
  drawingHistory.push(currentText.clone());
  
  currentText = null;
}

/**
 * Get the start point for text input
 * @returns The current start point or null
 */
function getStartPoint() {
  return startPoint;
}

/**
 * Set the start point for drawing operations
 * @param point - The point to set
 */
function setStartPoint(point: { x: number; y: number } | null) {
  startPoint = point;
}

defineExpose({
  initStage,
  updateSize,
  getStage,
  getDrawingLayer,
  startDrawing,
  continueDrawing,
  smoothPoints,
  endDrawing,
  undoDrawing,
  addText,
  getStartPoint,
  setStartPoint,
});

onBeforeUnmount(() => {
  if (stage) {
    stage.destroy();
  }
});
</script>

<style scoped>
#stage {
  width: 100%;
  height: 100%;
}
</style>
