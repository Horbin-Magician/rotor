<template>
  <div ref="stageContainer" id="stage"></div>
</template>

<script setup lang="ts">
import { ref, onBeforeUnmount } from 'vue';
import Konva from 'konva';

interface Props {
  zoomScale: number;
}

interface Emits {
  (e: 'ready', stage: Konva.Stage, backImgLayer: Konva.Layer, drawingLayer: Konva.Layer): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

const stageContainer = ref<HTMLDivElement | null>(null);

let stage: Konva.Stage | null = null;
let backImgLayer: Konva.Layer | null = null;
let drawingLayer: Konva.Layer | null = null;

function initStage(width: number, height: number, backImg: ImageBitmap, scaleFactor: number) {
  if (!stageContainer.value) return;

  // Create background layer
  backImgLayer = new Konva.Layer();
  const konvaImage = new Konva.Image({
    x: 0,
    y: 0,
    image: backImg,
    width: width / scaleFactor,
    height: height / scaleFactor,
  });
  backImgLayer.add(konvaImage);

  // Create stage
  stage = new Konva.Stage({
    container: 'stage',
    width: width / scaleFactor,
    height: height / scaleFactor,
  });
  stage.add(backImgLayer);

  // Create drawing layer
  drawingLayer = new Konva.Layer();
  stage.add(drawingLayer);

  emit('ready', stage, backImgLayer, drawingLayer);
}

function updateSize(width: number, height: number) {
  if (!stage || !backImgLayer) return;

  stage.width(width);
  stage.height(height);

  const konvaImage = backImgLayer.findOne('Image') as Konva.Image;
  if (konvaImage) {
    konvaImage.width(width);
    konvaImage.height(height);
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

function getBackImgLayer() {
  return backImgLayer;
}

function getDrawingLayer() {
  return drawingLayer;
}

defineExpose({
  initStage,
  updateSize,
  getStage,
  getBackImgLayer,
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
