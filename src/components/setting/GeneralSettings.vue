<template>
  <div class="settings-card">
    <div class="settings-card-title">{{ t('message.common') }}</div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.language') }}</span>
      <n-select v-model:value="language" :options="languageOptions" />
    </div>
    
    <div class="setting-item">
      <span class="setting-label">{{ t('message.theme') }}</span>
      <n-select v-model:value="theme" :options="themeOptions" />
    </div>

    <div class="setting-item">
      <span class="setting-label">{{ t('message.powerBoot') }}</span>
      <n-switch v-model:value="powerBoot" />
    </div>

    <div class="setting-item">
      <span class="setting-label">{{ t('message.currentVersion') }}{{ currentVersion }}</span>
      <n-button @click="handleCheckUpdate" :loading="isCheckingUpdate" :disabled="isCheckingUpdate">
        {{ t('message.checkUpdate') }}
      </n-button>
    </div>
  </div>

  <div class="settings-card">
    <div class="settings-card-title">{{ t('message.globalShortcuts') }}</div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.screenshot') }}</span>
      <ShortcutInput v-model:shortcut="shortcutScreenshot" />
    </div>
    <div class="setting-item">
      <span class="setting-label">{{ t('message.search') }}</span>
      <ShortcutInput v-model:shortcut="shortcutSearch" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { NSelect, NSwitch, NButton } from 'naive-ui'
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import ShortcutInput from '../common/ShortcutInput.vue'

const { t, locale } = useI18n()

// Props
interface Props {
  currentVersion: string
  isCheckingUpdate: boolean
}

defineProps<Props>()

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
</style>
