<template>
  <div class="toolbar" :class="{ 'toolbar-hidden': !visible }">
    <div class="toolbar-item" :title="$t('message.annotationMode')" @click="$emit('enterEditMode')">
      <n-icon size="20">
        <EditOutlined />
      </n-icon>
    </div>

    <div class="toolbar-item" :title="$t('message.ocrMode')" @click="$emit('imgToText')" v-if="!isProcessingOcr" :class="{ 'active': isOcrActive }">
      <n-icon size="20">
        <ScanText24Filled />
      </n-icon>
    </div>
    <div class="toolbar-item" v-else> 
      <n-spin :size="20" /> 
    </div>

    <div class="toolbar-divider"></div>
    
    <div class="toolbar-item" :title="`${$t('message.minimize')} (${shortcuts.hide})`" @click="$emit('minimize')">
      <n-icon size="20">
        <MinusFilled />
      </n-icon>
    </div>
    
    <div class="toolbar-item" :title="`${$t('message.saveImage')} (${shortcuts.save})`" @click="$emit('save')">
      <n-icon size="20">
        <SaveAltFilled />
      </n-icon>
    </div>
    
    <div class="toolbar-item" :title="`${$t('message.close')} (${shortcuts.close})`" @click="$emit('close')">
      <n-icon size="20">
        <CloseFilled />
      </n-icon>
    </div>
    
    <div class="toolbar-item" :title="`${$t('message.copyImage')} (${shortcuts.copy})`" @click="$emit('copy')">
      <n-icon size="20">
        <ContentCopyRound />
      </n-icon>
    </div>
  </div>
</template>

<script setup lang="ts">
import { 
  CloseFilled, 
  SaveAltFilled, 
  ContentCopyRound, 
  MinusFilled, 
  EditOutlined,
} from '@vicons/material';
import { 
  ScanText24Filled
} from '@vicons/fluent';
import { NIcon, NSpin } from 'naive-ui';

interface Props {
  visible: boolean;
  isProcessingOcr: boolean;
  isOcrActive: boolean;
  shortcuts: {
    save: string;
    close: string;
    copy: string;
    hide: string;
  };
}

interface Emits {
  (e: 'enterEditMode'): void;
  (e: 'imgToText'): void;
  (e: 'minimize'): void;
  (e: 'save'): void;
  (e: 'close'): void;
  (e: 'copy'): void;
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
