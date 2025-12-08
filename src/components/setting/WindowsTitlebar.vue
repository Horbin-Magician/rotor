<template>
  <div class="windows-titlebar" data-tauri-drag-region>
      <div class="titlebar-controls">
        <button class="titlebar-button minimize-button" @click="minimizeWindow" :title="t('message.minimize')">
          <svg width="12" height="12" viewBox="0 0 12 12">
            <rect x="0" y="5" width="12" height="1" />
          </svg>
        </button>
        <button class="titlebar-button close-button" @click="closeWindow" :title="t('message.close')">
          <svg width="12" height="12" viewBox="0 0 12 12">
            <path d="M0,0 L12,12 M12,0 L0,12" stroke-width="1" />
          </svg>
        </button>
      </div>
  </div>
</template>

<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const appWindow = getCurrentWindow()

function minimizeWindow() {
  appWindow.minimize()
}

function closeWindow() {
  appWindow.hide()
}
</script>

<style scoped>
.windows-titlebar {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: 32px;
  z-index: 1000;
  user-select: none;
  -webkit-user-select: none;
  -webkit-app-region: drag;
  display: flex;
  justify-content: right;
}

.titlebar-controls {
  display: flex;
  -webkit-app-region: no-drag;
}

.titlebar-button {
  width: 46px;
  height: 30px;
  border: none;
  background: transparent;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background-color 0.2s;
  color: var(--n-text-color);
}

.titlebar-button svg {
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
}

.titlebar-button.minimize-button:hover {
  background-color: rgba(255, 255, 255, 0.1);
}

.titlebar-button.close-button:hover {
  background-color: #e81123;
  color: white;
}
</style>
