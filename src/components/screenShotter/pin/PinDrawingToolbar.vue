<template>
  <div class="toolbar drawing-toolbar" :class="{ 'toolbar-hidden': !visible }">
    <div class="toolbar-item" :title="`${$t('message.exitAnnotation')} (${closeShortcut})`" @click="$emit('exit')">
      <n-icon size="20">
        <ArrowBackIosRound />
      </n-icon>
    </div>
    
    <div class="toolbar-divider"></div>
    
    <div class="toolbar-item" :title="$t('message.penTool')" @click="$emit('selectPen')" :class="{ 'active': activeTool === 'pen' }">
      <n-icon size="20">
        <EditOutlined />
      </n-icon>
    </div>
    
    <div class="toolbar-item" :title="$t('message.rectangleTool')" @click="$emit('selectRect')" :class="{ 'active': activeTool === 'rect' }">
      <n-icon size="20">
        <CropDinRound />
      </n-icon>
    </div>
    
    <div class="toolbar-item" :title="$t('message.arrowTool')" @click="$emit('selectArrow')" :class="{ 'active': activeTool === 'arrow' }">
      <n-icon size="20">
        <ArrowDownLeft20Filled />
      </n-icon>
    </div>
    
    <div class="toolbar-item" :title="$t('message.textTool')" @click="$emit('selectText')" :class="{ 'active': activeTool === 'text' }">
      <n-icon size="20">
        <TextT20Filled />
      </n-icon>
    </div>
    
    <div class="toolbar-divider"></div>
    
    <div class="toolbar-item" :title="$t('message.undo')" @click="$emit('undo')">
      <n-icon size="20">
        <UndoFilled />
      </n-icon>
    </div>
  </div>
</template>

<script setup lang="ts">
import { 
  EditOutlined,
  ArrowBackIosRound,
  CropDinRound,
  UndoFilled,
} from '@vicons/material';
import { 
  ArrowDownLeft20Filled,
  TextT20Filled,
} from '@vicons/fluent';
import { NIcon } from 'naive-ui';

interface Props {
  visible: boolean;
  activeTool: 'pen' | 'rect' | 'arrow' | 'text';
  closeShortcut: string;
}

interface Emits {
  (e: 'exit'): void;
  (e: 'selectPen'): void;
  (e: 'selectRect'): void;
  (e: 'selectArrow'): void;
  (e: 'selectText'): void;
  (e: 'undo'): void;
}

defineProps<Props>();
defineEmits<Emits>();
</script>

<style scoped>
.toolbar {
  position: fixed;
  bottom: 0px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  background-color: var(--theme-background);
  border-radius: 8px 8px 0px 0px;
  padding: 4px;
  gap: 4px;
  z-index: 1000;
  transition: transform 0.2s ease;
  opacity: 0.9;
}

.toolbar-hidden {
  transform: translate(-50%, 100%);
}

.toolbar-item {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  border-radius: 4px;
  transition: background-color 0.2s;
  color: var(--theme-primary);
}

.toolbar-item:hover {
  background-color: var(--theme-shadow);
}

.toolbar-item.active {
  background-color: var(--theme-shadow);
}

.toolbar-divider {
  height: 20px;
  width: 1px;
  background-color: var(--theme-border);
}
</style>
