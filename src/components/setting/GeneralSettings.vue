<template>
  <SettingsSection :title="t('message.common')">
    <SettingRow :label="t('message.language')">
      <n-select v-model:value="language" :options="languageOptions" />
    </SettingRow>
    
    <SettingRow :label="t('message.theme')">
      <n-select v-model:value="theme" :options="themeOptions" />
    </SettingRow>

    <SettingRow :label="t('message.powerBoot')">
      <n-switch v-model:value="powerBoot" />
    </SettingRow>

    <SettingRow :label="`${t('message.currentVersion')}${currentVersion}`">
      <n-button @click="handleCheckUpdate" :loading="isCheckingUpdate" :disabled="isCheckingUpdate">
        {{ t('message.checkUpdate') }}
      </n-button>
    </SettingRow>
  </SettingsSection>

  <SettingsSection :title="t('message.globalShortcuts')">
    <SettingRow :label="t('message.screenshot')" :conflict="highlightedSetting === 'shortcut_screenshot'">
      <ShortcutInput v-model:shortcut="shortcutScreenshot" />
    </SettingRow>
    <SettingRow :label="t('message.search')" :conflict="highlightedSetting === 'shortcut_search'">
      <ShortcutInput v-model:shortcut="shortcutSearch" />
    </SettingRow>
  </SettingsSection>
</template>

<script setup lang="ts">
import { NSelect, NSwitch, NButton } from 'naive-ui'
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import SettingRow from './SettingRow.vue'
import SettingsSection from './SettingsSection.vue'
import ShortcutInput from '../common/ShortcutInput.vue'

const { t, locale } = useI18n()

// Props
interface Props {
  currentVersion: string
  isCheckingUpdate: boolean
  highlightedSetting?: string
}

withDefaults(defineProps<Props>(), {
  highlightedSetting: ''
})

// Emits
const emit = defineEmits<{
  'update:language': [value: number]
  'update:theme': [value: number]
  'update:powerBoot': [value: boolean]
  'checkUpdate': []
}>()

// Local state
const language = defineModel<number>('language', { required: true })
const theme = defineModel<number>('theme', { required: true })
const powerBoot = defineModel<boolean>('powerBoot', { required: true })
const shortcutScreenshot = defineModel<string>('shortcutScreenshot', { required: true })
const shortcutSearch = defineModel<string>('shortcutSearch', { required: true })

const languageOptions = ref([
  { label: t('message.systemDefault'), value: 0 },
  { label: t('message.chinese'), value: 1 },
  { label: t('message.english'), value: 2 }
])

const themeOptions = ref([
  { label: t('message.followSystem'), value: 0 },
  { label: t('message.light'), value: 1 },
  { label: t('message.dark'), value: 2 }
])

// Update options when language changes
watch(locale, () => {
  languageOptions.value = [
    { label: t('message.systemDefault'), value: 0 },
    { label: t('message.chinese'), value: 1 },
    { label: t('message.english'), value: 2 }
  ]
  
  themeOptions.value = [
    { label: t('message.followSystem'), value: 0 },
    { label: t('message.light'), value: 1 },
    { label: t('message.dark'), value: 2 }
  ]
})

function handleCheckUpdate() {
  emit('checkUpdate')
}
</script>
