<template>
  <div class="magnifier" :style="magnifierStyle">
    <canvas 
      ref="magnifierCanvasRef" 
      class="magnifier-canvas" 
      :width="magnifierSize" 
      :height="magnifierSize"
    ></canvas>
    <div class="magnifier-crosshair" :style="{ width: magnifierSize + 'px', height: magnifierSize + 'px' }"></div>
    <MagnifierInfo 
      :selection-width="selectionWidth"
      :selection-height="selectionHeight"
      :pixel-color="pixelColor"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import MagnifierInfo from "./MagnifierInfo.vue";

interface Props {
  currentX: number;
  currentY: number;
  magnifierSize: number;
  selectionWidth: number;
  selectionHeight: number;
  pixelColor: string;
  isWindowFocused: boolean;
}

const props = defineProps<Props>();

const magnifierCanvasRef = ref<HTMLCanvasElement | null>(null);

const emit = defineEmits<{
  canvasReady: [canvas: HTMLCanvasElement, ctx: CanvasRenderingContext2D];
}>();

const mgnfHeight = props.magnifierSize + 50;
const mgnfOffset = 20;
const viewportWidth = window.screen.width;
const viewportHeight = window.screen.height;

const magnifierStyle = computed(() => {
  if (!props.isWindowFocused) {
    return { display: 'none' };
  }
  
  let left = (props.currentX + props.magnifierSize > viewportWidth) ? 
    props.currentX - props.magnifierSize : props.currentX;
  let top = (props.currentY + mgnfOffset + mgnfHeight > viewportHeight) ? 
    props.currentY - mgnfOffset - mgnfHeight : (props.currentY + mgnfOffset);

  return {
    left: `${left}px`,
    top: `${top}px`,
    width: `${props.magnifierSize}px`,
    height: `${mgnfHeight}px`
  };
});

onMounted(() => {
  if (!magnifierCanvasRef.value) return;
  
  const canvas = magnifierCanvasRef.value;
  const ctx = canvas.getContext('2d', { alpha: false, willReadFrequently: true });
  
  if (!ctx) return;
  
  ctx.imageSmoothingEnabled = false;
  
  emit('canvasReady', canvas, ctx);
});
</script>

<style scoped>
.magnifier {
  position: absolute;
  border: 1px solid var(--theme-border-hover);
  overflow: hidden;
  pointer-events: none;
  z-index: 1000;
  background-color: var(--theme-overlay);
  display: flex;
  flex-direction: column;
}

.magnifier-crosshair {
  position: absolute;
  top: 0;
  left: 0;
  pointer-events: none;
  z-index: 1001;
  border-bottom: 1px solid var(--theme-border-hover);
}

.magnifier-crosshair::before,
.magnifier-crosshair::after {
  content: '';
  position: absolute;
  background-color: var(--theme-primary);
}

.magnifier-crosshair::before {
  top: 50%;
  left: 0;
  width: 100%;
  height: 1px;
}

.magnifier-crosshair::after {
  top: 0;
  left: 50%;
  width: 1px;
  height: 100%;
}
</style>
