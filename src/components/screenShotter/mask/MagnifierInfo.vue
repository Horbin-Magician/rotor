<template>
  <div class="magnifier-info">
    <!-- Rect size info -->
    <div class="magnifier-info-item">{{ props.selectionWidth }} × {{ props.selectionHeight }}</div>
    <!-- Point color info -->
    <div class="magnifier-info-item">
      <div class="color-preview" :style="{ backgroundColor: props.pixelColor }"></div>
      <span>{{ props.pixelColor }}</span>
    </div>
    <div class="magnifier-info-item">
      <span>{{ $t('message.copyColorHint') }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
interface Props {
  selectionWidth: number;
  selectionHeight: number;
  pixelColor: string;
}

const props = defineProps<Props>();
</script>

<style scoped>
.magnifier-info {
  padding: 4px;
  color: var(--theme-text-primary);
  font-size: 12px;
  display: flex;
  flex-direction: column;
}

.magnifier-info-item {
  width: 100%;
  height: 14px;
  align-items: center;
  text-align: center;
  display: flex;
  justify-content: center;
}

.color-preview {
  width: 10px;
  height: 10px;
  margin-right: 6px;
  display: inline-block;
}
</style>
