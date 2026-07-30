<template>
  <SettingsSection :title="t('message.translatorEngine')">
    <SettingRow :label="t('message.translatorEngine')">
      <n-select v-model:value="engine" :options="engineOptions" />
    </SettingRow>

    <SettingRow v-if="engine === 'deepseek'" :label="t('message.translatorDeepseekApiKey')">
      <n-input
        v-model:value="deepseekApiKey"
        type="password"
        show-password-on="click"
        :placeholder="t('message.translatorDeepseekApiKeyPlaceholder')"
        spellcheck="false"
      />
    </SettingRow>

    <template v-if="engine === 'custom'">
      <SettingRow :label="t('message.translatorCustomUrl')">
        <n-input
          v-model:value="customUrl"
          :placeholder="t('message.translatorCustomUrlPlaceholder')"
          spellcheck="false"
        />
      </SettingRow>
      <SettingRow :label="t('message.translatorCustomKey')">
        <n-input
          v-model:value="customKey"
          type="password"
          show-password-on="click"
          spellcheck="false"
        />
      </SettingRow>
    </template>

    <SettingRow :label="t('message.translatorTargetLang')">
      <n-select v-model:value="targetLang" :options="targetLangOptions" />
    </SettingRow>
  </SettingsSection>
</template>

<script setup lang="ts">
import { NSelect, NInput } from 'naive-ui'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import SettingRow from './SettingRow.vue'
import SettingsSection from './SettingsSection.vue'

const { t } = useI18n()

const engine = defineModel<string>('engine', { required: true })
const deepseekApiKey = defineModel<string>('deepseekApiKey', { required: true })
const customUrl = defineModel<string>('customUrl', { required: true })
const customKey = defineModel<string>('customKey', { required: true })
const targetLang = defineModel<string>('targetLang', { required: true })

const engineOptions = computed(() => [
  { label: t('message.translatorEngineGoogle'), value: 'google' },
  { label: t('message.translatorEngineDeepseek'), value: 'deepseek' },
  { label: t('message.translatorEngineCustom'), value: 'custom' },
])

const targetLangOptions = computed(() => [
  { label: t('message.translatorTargetAuto'), value: 'auto' },
  { label: t('message.chinese'), value: 'zh-CN' },
  { label: t('message.english'), value: 'en' },
  { label: t('message.japanese'), value: 'ja' },
  { label: t('message.korean'), value: 'ko' },
])
</script>
