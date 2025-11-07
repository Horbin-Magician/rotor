<template>
  <canvas 
    ref="canvasRef" 
    id="main-canvas"
    :style="{ width: windowWidth + 'px', height: windowHeight + 'px' }"
    :width="bacImgWidth"
    :height="bacImgHeight"
  />
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";

interface Props {
  windowWidth: number;
  windowHeight: number;
  bacImgWidth: number;
  bacImgHeight: number;
}

const props = defineProps<Props>();

const canvasRef = ref<HTMLCanvasElement | null>(null);

const emit = defineEmits<{
  canvasReady: [canvas: HTMLCanvasElement, ctx: CanvasRenderingContext2D];
}>();

onMounted(() => {
  if (!canvasRef.value) return;
  
  const canvas = canvasRef.value;
  const ctx = canvas.getContext('2d', { 
    alpha: false, 
    desynchronized: true, 
    willReadFrequently: true 
  });
  
  if (!ctx) return;
  
  const dpr = window.devicePixelRatio;
  ctx.scale(dpr, dpr);
  ctx.imageSmoothingEnabled = false;
  
  emit('canvasReady', canvas, ctx);
});
</script>

<style scoped>
#main-canvas {
  position: absolute;
  top: 0;
  left: 0;
}
</style>
