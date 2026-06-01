<template>
  <SettingsSection>
    <template #title>
        <span>{{ t('message.quickActions') }}</span>
        <span class="title-count">{{ enabledCount }}/{{ draftActions.length }}</span>
    </template>
    <template #extra>
      <n-button text circle size="small" :keyboard="false" :title="t('message.addQuickAction')" @click="handleAddAction">
        <template #icon>
          <n-icon>
            <AddIcon />
          </n-icon>
        </template>
      </n-button>
    </template>

    <div v-if="draftActions.length > 0" class="quick-action-list">
      <div
        v-for="(action, index) in draftActions"
        :key="action.id"
        class="quick-action-item"
        :class="{
          'quick-action-disabled': !action.enabled,
          'setting-item-conflict': highlightedSetting === quickActionSettingKey(action)
        }"
      >
        <div class="quick-action-summary">
          <n-switch v-model:value="action.enabled" @update:value="commitActions" />
          <div class="action-content">
            <div class="action-mainline">
              <span class="action-title">{{ action.name || t('message.unnamedQuickAction') }}</span>
              <span class="shortcut-pill">{{ displayShortcut(action.shortcut) }}</span>
            </div>
            <div class="command-preview">{{ action.command || t('message.quickActionCommand') }}</div>
          </div>
          <n-button
            quaternary
            circle
            :keyboard="false"
            :disabled="!action.command.trim()"
            :title="t('message.runQuickAction')"
            @click="handleRunAction(action)"
          >
            <template #icon>
              <n-icon>
                <RunIcon />
              </n-icon>
            </template>
          </n-button>
          <n-button
            quaternary
            circle
            :keyboard="false"
            :title="t('message.editQuickAction')"
            @click="handleEditAction(index)"
          >
            <template #icon>
              <n-icon>
                <EditIcon />
              </n-icon>
            </template>
          </n-button>
          <n-button
            quaternary
            circle
            type="error"
            :keyboard="false"
            :title="t('message.deleteQuickAction')"
            @click="handleDeleteAction(index)"
          >
            <template #icon>
              <n-icon>
                <DeleteIcon />
              </n-icon>
            </template>
          </n-button>
        </div>
      </div>
    </div>

    <div v-else class="empty-state">
      {{ t('message.noQuickActions') }}
    </div>

    <n-modal
      v-model:show="showEditor"
      preset="card"
      class="quick-action-modal"
      :closable="false"
    >
      <div v-if="editingAction" class="editor-body">
        <label class="editor-field">
          <span class="editor-label">{{ t('message.quickActionName') }}</span>
          <n-input
            v-model:value="editingAction.name"
            :placeholder="t('message.quickActionName')"
          />
        </label>

        <label class="editor-field">
          <span class="editor-label">{{ t('message.shortcuts') }}</span>
          <ShortcutInput v-model:shortcut="editingAction.shortcut" />
        </label>

        <label class="editor-field">
          <span class="editor-label">{{ t('message.quickActionCommand') }}</span>
          <n-input
            v-model:value="editingAction.command"
            :placeholder="t('message.quickActionCommand')"
          />
        </label>
      </div>

      <template #footer>
        <div class="editor-actions">
          <n-button :keyboard="false" @click="closeEditor">
            <template #icon>
              <n-icon>
                <CloseIcon />
              </n-icon>
            </template>
            {{ t('message.cancel') }}
          </n-button>
          <n-button type="primary" :keyboard="false" @click="saveEditor">
            <template #icon>
              <n-icon>
                <SaveIcon />
              </n-icon>
            </template>
            {{ t('message.save') }}
          </n-button>
        </div>
      </template>
    </n-modal>
  </SettingsSection>
</template>

<script setup lang="ts">
import { NButton, NIcon, NInput, NModal, NSwitch } from 'naive-ui'
import {
  AddCircleOutlineRound as AddIcon,
  CloseRound as CloseIcon,
  DeleteOutlineRound as DeleteIcon,
  EditRound as EditIcon,
  PlayArrowRound as RunIcon,
  SaveRound as SaveIcon,
} from '@vicons/material'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import SettingsSection from './SettingsSection.vue'
import ShortcutInput from '../common/ShortcutInput.vue'
import type { QuickAction } from '../../features/quick/types'
import { formatShortcut } from '../../shared/shortcut'

withDefaults(defineProps<{
  highlightedSetting?: string
}>(), {
  highlightedSetting: ''
})

const quickActions = defineModel<QuickAction[]>('quickActions', { required: true })
const draftActions = ref<QuickAction[]>([])
const showEditor = ref(false)
const editingIndex = ref<number | null>(null)
const editingAction = ref<QuickAction | null>(null)

const emit = defineEmits<{
  'runAction': [id: string]
}>()

const { t } = useI18n()

const enabledCount = computed(() => {
  return draftActions.value.filter((action) => action.enabled).length
})

watch(quickActions, (actions) => {
  draftActions.value = cloneActions(actions)
}, { immediate: true })

function quickActionSettingKey(action: QuickAction) {
  return `quick_action_${action.id}`
}

function defaultCommand() {
  return navigator.platform.startsWith('Mac') ? 'open -a Terminal' : 'start "" cmd.exe'
}

function createAction(): QuickAction {
  const timestamp = Date.now()

  return {
    id: `quick-${timestamp}`,
    name: t('message.newQuickAction'),
    shortcut: '',
    command: defaultCommand(),
    enabled: false,
  }
}

function cloneActions(actions: QuickAction[]) {
  return actions.map((action) => ({ ...action }))
}

function displayShortcut(shortcut: string) {
  return formatShortcut(shortcut) || t('message.pressShortcut')
}

function commitActions() {
  quickActions.value = cloneActions(draftActions.value)
}

function handleAddAction() {
  editingIndex.value = null
  editingAction.value = createAction()
  showEditor.value = true
}

function handleEditAction(index: number) {
  editingIndex.value = index
  editingAction.value = { ...draftActions.value[index] }
  showEditor.value = true
}

function closeEditor() {
  showEditor.value = false
  editingIndex.value = null
  editingAction.value = null
}

function saveEditor() {
  if (!editingAction.value) {
    return
  }

  const action = { ...editingAction.value }
  if (editingIndex.value === null) {
    draftActions.value = [...draftActions.value, action]
  } else {
    draftActions.value = draftActions.value.map((currentAction, actionIndex) => (
      actionIndex === editingIndex.value ? action : currentAction
    ))
  }

  commitActions()
  closeEditor()
}

function handleDeleteAction(index: number) {
  draftActions.value = draftActions.value.filter((_, actionIndex) => actionIndex !== index)
  commitActions()
}

function handleRunAction(action: QuickAction) {
  emit('runAction', action.id)
}
</script>

<style scoped>
.title-count {
  color: #888;
  font-size: 12px;
  font-weight: normal;
}

.quick-action-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.quick-action-item {
  border: 1px solid var(--n-border-color);
  border-radius: 6px;
  padding: 6px 8px;
  background-color: var(--theme-background-secondary);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
  transition: border-color 0.2s, background-color 0.2s, opacity 0.2s;
}

.quick-action-item:hover {
  background-color: var(--theme-background-tertiary);
}

.quick-action-disabled {
  opacity: 0.62;
}

.quick-action-summary {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) 30px 30px 30px;
  gap: 6px;
  align-items: center;
}

.action-content {
  min-width: 0;
}

.action-mainline {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.action-title {
  font-weight: 600;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shortcut-pill {
  flex: none;
  max-width: 128px;
  border-radius: 4px;
  padding: 1px 6px;
  background-color: rgba(32, 128, 240, 0.12);
  color: #2080f0;
  font-size: 12px;
  line-height: 18px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command-preview {
  margin-top: 1px;
  color: #777;
  font-size: 12px;
  line-height: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.empty-state {
  min-height: 56px;
  border: 1px dashed var(--n-border-color);
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #888;
  background-color: rgba(255, 255, 255, 0.42);
}

.quick-action-modal,
:deep(.n-modal.quick-action-modal) {
  width: 300px !important;
  max-width: calc(100vw - 48px) !important;
}

.quick-action-modal :deep(.n-card),
:deep(.n-modal.quick-action-modal) {
  border-radius: 8px;
  overflow: hidden;
}

.quick-action-modal :deep(.n-card__content) {
  padding: 8px 10px 7px;
}

.quick-action-modal :deep(.n-card__footer) {
  padding: 7px 10px 8px;
  border-top: 1px solid var(--theme-border);
  background: var(--theme-background-secondary);
}

.editor-body {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.editor-field {
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr);
  gap: 6px;
  align-items: center;
  min-height: 30px;
  padding: 2px 7px;
  border: 1px solid var(--theme-border);
  border-radius: 6px;
  background: var(--theme-background);
  transition: border-color 0.2s, background-color 0.2s;
}

.editor-field:focus-within {
  border-color: var(--theme-primary);
  background: var(--theme-background-secondary);
}

.editor-label {
  color: var(--theme-text-secondary);
  font-size: 12px;
  line-height: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.editor-field :deep(.n-input) {
  --n-border: 1px solid transparent !important;
  --n-border-hover: 1px solid transparent !important;
  --n-border-focus: 1px solid transparent !important;
  --n-box-shadow-focus: none !important;
  --n-padding-left: 0 !important;
  --n-padding-right: 0 !important;
  --n-height: 26px !important;
  background: transparent;
}

.editor-field :deep(.n-input__input-el) {
  text-align: center;
}

.editor-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
}

.editor-actions :deep(.n-button) {
  min-width: 66px;
  --n-height: 28px !important;
}

</style>
