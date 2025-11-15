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

defineExpose({
  initStage,
  updateSize,
  getStage,
  getDrawingLayer,
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
