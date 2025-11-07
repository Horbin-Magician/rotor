<template>
  <div class="selection-rect" :style="selectionStyle"></div>
</template>

<script setup lang="ts">
import { computed } from "vue";

interface Props {
  isSelecting: boolean;
  startX: number;
  startY: number;
  endX: number;
  endY: number;
  autoSelectRect: { x: number; y: number; width: number; height: number } | null;
  isWindowFocused: boolean;
}

const props = defineProps<Props>();

const selectionStyle = computed(() => {
  if (!props.isWindowFocused) {
    return { display: 'none' };
  }
  
  let left = -2, top = -2, width = 0, height = 0;
  
  if (props.isSelecting === true) {
    width = Math.abs(props.endX - props.startX);
    height = Math.abs(props.endY - props.startY);
    if (width > 5 && height > 5) {
      left = Math.min(props.startX, props.endX);
      top = Math.min(props.startY, props.endY);
    } else {
      width = 0;
      height = 0;
    }
  } else if (props.autoSelectRect) {
    left = props.autoSelectRect.x;
    top = props.autoSelectRect.y;
    width = props.autoSelectRect.width;
    height = props.autoSelectRect.height;
  }

  return {
    left: `${left}px`,
    top: `${top}px`,
    width: `${width}px`,
    height: `${height}px`
  };
});
</script>

<style scoped>
.selection-rect {
  position: absolute;
  border: 1px solid var(--theme-primary-pressed);
  background-color: var(--theme-primary-overlay);
  pointer-events: none;
}
</style>
