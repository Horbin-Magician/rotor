<template>
  <div v-for="(result, index) in results" 
       v-show="visible"
       :key="'ocr-text-' + index"
       class="ocr-text-overlay"
       :style="getOverlayStyle(result)">
    <span class="ocr-text-content" :style="getTextStyle(result)">{{ result.text }}</span>
  </div>
</template>

<script setup lang="ts">
import type { CSSProperties } from 'vue';

interface TextResult {
  left: number;
  top: number;
  width: number;
  height: number;
  text: string;
}

interface Props {
  visible: boolean;
  results: TextResult[];
  scaleFactor: number;
  ocrZoomScale: number;
  zoomScale: number;
}

const props = defineProps<Props>();

const OCR_FONT_SIZE_RATIO = 0.6;
const OCR_FONT_FAMILY = 'Inter, Avenir, Helvetica, Arial, sans-serif';
const MIN_TEXT_WIDTH = 1;

let measureContext: CanvasRenderingContext2D | null = null;

function getDisplayScale() {
  return props.zoomScale / props.scaleFactor / props.ocrZoomScale;
}

function getDisplayBox(result: TextResult) {
  const scale = getDisplayScale();
  return {
    left: result.left * scale,
    top: result.top * scale,
    width: result.width * scale,
    height: result.height * scale,
  };
}

function getOverlayStyle(result: TextResult): CSSProperties {
  const box = getDisplayBox(result);
  return {
    left: `${box.left}px`,
    top: `${box.top}px`,
    width: `${box.width}px`,
    height: `${box.height}px`,
  };
}

function getTextStyle(result: TextResult): CSSProperties {
  const box = getDisplayBox(result);
  const fontSize = box.height * OCR_FONT_SIZE_RATIO;
  const textWidth = measureTextWidth(result.text, fontSize);
  const scaleX = box.width > 0 && textWidth > 0 ? box.width / textWidth : 1;

  return {
    fontSize: `${fontSize}px`,
    lineHeight: `${box.height}px`,
    width: `${textWidth}px`,
    transform: `scaleX(${scaleX})`,
  };
}

function measureTextWidth(text: string, fontSize: number) {
  const normalizedText = text || ' ';
  const context = getMeasureContext();
  if (!context) return MIN_TEXT_WIDTH;

  context.font = `${fontSize}px ${OCR_FONT_FAMILY}`;
  return Math.max(MIN_TEXT_WIDTH, context.measureText(normalizedText).width);
}

function getMeasureContext() {
  if (measureContext) return measureContext;
  if (typeof document === 'undefined') return null;

  measureContext = document.createElement('canvas').getContext('2d');
  return measureContext;
}
</script>

<style scoped>
.ocr-text-overlay {
  position: absolute;
  display: flex;
  align-items: center;
  justify-content: flex-start;
  z-index: 500;
  overflow: hidden;
  background-color: transparent;
  cursor: text;
  pointer-events: auto;
  user-select: text;
  -webkit-user-select: text;
}

.ocr-text-content {
  color: transparent;
  -webkit-text-fill-color: transparent;
  display: inline-block;
  flex: 0 0 auto;
  text-align: left;
  overflow: hidden;
  white-space: pre;
  transform-origin: left center;
  user-select: text;
  -webkit-user-select: text;
}

.ocr-text-content::selection {
  color: transparent;
  -webkit-text-fill-color: transparent;
  background-color: color-mix(in srgb, var(--theme-primary) 28%, transparent);
}
</style>
