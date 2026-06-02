<template>
  <SettingsSection :title="t('message.searchIndexing')">
    <div class="exclude-editor">
      <div class="exclude-editor-header">
        <span class="exclude-editor-title">{{ t('message.searchExcludedDirs') }}</span>
        <div class="exclude-editor-actions">
          <n-button
            size="small"
            :keyboard="false"
            :disabled="!isDirty"
            @mousedown.prevent
            @click="resetDraft"
          >
            {{ t('message.cancel') }}
          </n-button>
          <n-button
            size="small"
            type="primary"
            :keyboard="false"
            :disabled="!isDirty"
            @mousedown.prevent
            @click="commitDraft"
          >
            {{ t('message.save') }}
          </n-button>
        </div>
      </div>
      <n-input
        v-model:value="draftExcludedDirs"
        type="textarea"
        :autosize="{ minRows: 8, maxRows: 12 }"
        spellcheck="false"
        @blur="commitDraft"
      />
    </div>
  </SettingsSection>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { NButton, NInput } from 'naive-ui'
import { useI18n } from 'vue-i18n'
import SettingsSection from './SettingsSection.vue'

const { t } = useI18n()

const excludedDirs = defineModel<string>('excludedDirs', { required: true })
const draftExcludedDirs = ref('')

const isDirty = computed(() => draftExcludedDirs.value !== excludedDirs.value)

watch(excludedDirs, (value) => {
  draftExcludedDirs.value = value
}, { immediate: true })

function resetDraft() {
  draftExcludedDirs.value = excludedDirs.value
}

function commitDraft() {
  if (!isDirty.value) {
    return
  }

  excludedDirs.value = draftExcludedDirs.value
}
</script>

<style scoped>
.exclude-editor {
  margin-left: 12px;
  min-width: 0;
}

.exclude-editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 6px;
}

.exclude-editor-title {
  min-width: 0;
  overflow: hidden;
  color: var(--theme-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.exclude-editor-actions {
  display: flex;
  flex: none;
  gap: 6px;
}

.exclude-editor :deep(.n-input) {
  width: 100%;
}
</style>
