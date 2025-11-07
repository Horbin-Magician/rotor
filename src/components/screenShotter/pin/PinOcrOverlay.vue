<template>
  <div v-for="(result, index) in results" 
       v-show="visible"
       :key="'ocr-text-' + index"
       class="ocr-text-overlay"
       :style="{
         left: (result.left / scaleFactor / ocrZoomScale * zoomScale) + 'px',
         top: (result.top / scaleFactor / ocrZoomScale * zoomScale) + 'px',
         width: (result.width / scaleFactor / ocrZoomScale * zoomScale) + 'px',
         height: (result.height / scaleFactor / ocrZoomScale * zoomScale) + 'px',
         fontSize: (result.height / scaleFactor / ocrZoomScale * zoomScale) * 0.6 + 'px',
         lineHeight: (result.height / scaleFactor / ocrZoomScale * zoomScale) + 'px',
       }">
    <span class="ocr-text-content">{{ result.text }}</span>
  </div>
</template>

<script setup lang="ts">
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

defineProps<Props>();
</script>

<style scoped>
.ocr-text-overlay {
  position: absolute;
  background-color: var(--theme-background);
  display: flex;
  align-items: center;
  justify-content: flex-start;
  z-index: 500;
  overflow: hidden;
}

.ocr-text-content {
  color: var(--theme-text-primary);
  text-align: left;
  overflow: hidden;
  width: 100%;
  user-select: text;
  -webkit-user-select: text;
}
</style>
