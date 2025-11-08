<template>
  <div v-if="edgeGlow.left" class="edge-glow-left"></div>
  <div v-if="edgeGlow.right" class="edge-glow-right"></div>
  <div v-if="edgeGlow.top" class="edge-glow-top"></div>
  <div v-if="edgeGlow.bottom" class="edge-glow-bottom"></div>
</template>

<script setup lang="ts">
interface EdgeGlow {
  left: boolean;
  right: boolean;
  top: boolean;
  bottom: boolean;
}

defineProps<{
  edgeGlow: EdgeGlow;
}>();
</script>

<style scoped>
.edge-glow-left {
  position: absolute;
  top: 0;
  left: 0;
  width: 10px;
  height: 100%;
  pointer-events: none;
  background: linear-gradient(to right, 
    rgba(75, 160, 244, 0.8) 0%,
    rgba(59, 130, 246, 0.4) 40%,
    rgba(59, 130, 246, 0) 100%
  );
  filter: blur(4px);
  opacity: 0.9;
  z-index: 10000;
  animation: pulse-glow 1.5s ease-in-out infinite;
}

.edge-glow-right {
  position: absolute;
  top: 0;
  right: 0;
  width: 10px;
  height: 100%;
  pointer-events: none;
  background: linear-gradient(to left, 
    rgba(59, 130, 246, 0.8) 0%,
    rgba(59, 130, 246, 0.4) 40%,
    rgba(59, 130, 246, 0) 100%
  );
  filter: blur(4px);
  opacity: 0.9;
  z-index: 10000;
  animation: pulse-glow 1.5s ease-in-out infinite;
}

.edge-glow-top {
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  height: 10px;
  pointer-events: none;
  background: linear-gradient(to bottom, 
    rgba(59, 130, 246, 0.8) 0%,
    rgba(59, 130, 246, 0.4) 40%,
    rgba(59, 130, 246, 0) 100%
  );
  filter: blur(4px);
  opacity: 0.9;
  z-index: 10000;
  animation: pulse-glow 1.5s ease-in-out infinite;
}

.edge-glow-bottom {
  position: absolute;
  left: 0;
  bottom: 0;
  width: 100%;
  height: 10px;
  pointer-events: none;
  background: linear-gradient(to top, 
    rgba(59, 130, 246, 0.8) 0%,
    rgba(59, 130, 246, 0.4) 40%,
    rgba(59, 130, 246, 0) 100%
  );
  filter: blur(4px);
  opacity: 0.9;
  z-index: 10000;
  animation: pulse-glow 1.5s ease-in-out infinite;
}

@keyframes pulse-glow {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}
</style>
