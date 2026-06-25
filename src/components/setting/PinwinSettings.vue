<template>
  <SettingsSection :title="t('message.pinwin')">
    <SettingRow :label="t('message.defaultSavePath')">
      <div class="path-selector">
        <n-input v-model:value="savePath" placeholder="" readonly />
        <n-button @click="handleAskSave">{{ t('message.browse') }}</n-button>
      </div>
    </SettingRow>
    <SettingRow :label="t('message.autoChangeSavePath')">
      <n-switch v-model:value="ifAutoChangeSavePath" />
    </SettingRow>
    <SettingRow :label="t('message.askSavePath')">
      <n-switch v-model:value="ifAskSavePath" />
    </SettingRow>
    <SettingRow :label="t('message.zoomDelta')">
      <n-slider v-model:value="zoomDelta" :step="1" :max="10" :min="1" />
    </SettingRow>
  </SettingsSection>
  <SettingsSection :title="t('message.shortcuts')">
    <SettingRow
      :label="t('message.closePinwin')"
      :conflict="highlightedSetting === 'shortcut_pinwin_close'"
    >
      <ShortcutInput v-model:shortcut="shortcutPinwinClose" />
    </SettingRow>
    <SettingRow
      :label="t('message.savePinwin')"
      :conflict="highlightedSetting === 'shortcut_pinwin_save'"
    >
      <ShortcutInput v-model:shortcut="shortcutPinwinSave" />
    </SettingRow>
    <SettingRow
      :label="t('message.completePinwin')"
      :conflict="highlightedSetting === 'shortcut_pinwin_copy'"
    >
      <ShortcutInput v-model:shortcut="shortcutPinwinCopy" />
    </SettingRow>
    <SettingRow
      :label="t('message.hidePinwin')"
      :conflict="highlightedSetting === 'shortcut_pinwin_hide'"
    >
      <ShortcutInput v-model:shortcut="shortcutPinwinHide" />
    </SettingRow>
  </SettingsSection>
</template>

<script setup lang="ts">
import { NInput, NButton, NSwitch, NSlider } from 'naive-ui'
import { useI18n } from 'vue-i18n'
import SettingRow from './SettingRow.vue'
import SettingsSection from './SettingsSection.vue'
import ShortcutInput from '../common/ShortcutInput.vue'

const { t } = useI18n()

withDefaults(
  defineProps<{
    highlightedSetting?: string
  }>(),
  {
    highlightedSetting: '',
  },
)

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
  askSave: []
}>()

function handleAskSave() {
  emit('askSave')
}
</script>

<style scoped>
.path-selector {
  display: flex;
  gap: 8px;
  flex: 1;
  width: 100%;
}

.path-selector .n-input {
  flex: 1;
}
</style>
