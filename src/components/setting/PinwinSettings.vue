<template>
  <div class="settings-card">
    <div class="settings-card-title">{{ t('message.pinwin') }}</div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.defaultSavePath') }}</span>
      <div class="path-selector">
        <n-input v-model:value="savePath" placeholder="" readonly />
        <n-button  @click="handleAskSave">{{ t('message.browse') }}</n-button>
      </div>
    </div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.autoChangeSavePath') }}</span>
      <n-switch v-model:value="ifAutoChangeSavePath" />
    </div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.askSavePath') }}</span>
      <n-switch v-model:value="ifAskSavePath" />
    </div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.zoomDelta') }}</span>
      <n-slider v-model:value="zoomDelta" :step="1" :max="10" :min="1" />
    </div>
  </div>
  <div class="settings-card">
    <div class="settings-card-title">{{ t('message.shortcuts') }}</div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.closePinwin') }}</span>
      <ShortcutInput v-model:shortcut="shortcutPinwinClose"/>
    </div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.savePinwin') }}</span>
      <ShortcutInput v-model:shortcut="shortcutPinwinSave"/>
    </div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.completePinwin') }}</span>
      <ShortcutInput v-model:shortcut="shortcutPinwinCopy"/>
    </div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.hidePinwin') }}</span>
      <ShortcutInput v-model:shortcut="shortcutPinwinHide"/>
    </div>
  </div>
</template>

<script setup lang="ts">
import { NInput, NButton, NSwitch, NSlider } from 'naive-ui'
import { useI18n } from 'vue-i18n'
import ShortcutInput from '../common/ShortcutInput.vue'

const { t } = useI18n()

// Models
const shortcutPinwinClose = defineModel<string>('shortcutPinwinClose', { required: true })
const shortcutPinwinSave = defineModel<string>('shortcutPinwinSave', { required: true })
const shortcutPinwinCopy = defineModel<string>('shortcutPinwinCopy', { required: true })
const shortcutPinwinHide = defineModel<string>('shortcutPinwinHide', { required: true })
const savePath = defineModel<string>('savePath', { required: true })
const ifAutoChangeSavePath = defineModel<boolean>('ifAutoChangeSavePath', { required: true })
const ifAskSavePath = defineModel<boolean>('ifAskSavePath', { required: true })
const zoomDelta = defineModel<number>('zoomDelta', { required: true })

// Emits
const emit = defineEmits<{
  'askSave': []
}>()

function handleAskSave() {
  emit('askSave')
}
</script>

<style scoped>
.settings-card {
  margin-bottom: 20px;
}

.settings-card-title {
  font-weight: bold;
  font-size: 16px;
  height: 30px;
  border-bottom: 1px solid gray;
  margin-bottom: 10px;
}

.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 34px;
  margin-bottom: 10px;
  margin-left: 20px;
}

.setting-label {
  min-width: 130px;
}

.path-selector {
  display: flex;
  gap: 8px;
  flex: 1;
}

.path-selector .n-input {
  flex: 1;
}
</style>
