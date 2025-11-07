<template>
  <div v-if="visible" class="tips">
    {{ tips }}
  </div>
</template>

<script setup lang="ts">
interface Props {
  visible: boolean;
  tips: string;
}

defineProps<Props>();
</script>

<style scoped>
.tips {
  position: fixed;
  left: 50%;
  top: 50%;
  padding: 2px 8px 2px 8px;
  transform: translate(-50%, -50%);
  background-color: var(--theme-overlay);
  color: var(--theme-text-primary);
  border-radius: 8px;
  font-size: 14px;
  z-index: 1000;
  cursor: default;
}
</style>
