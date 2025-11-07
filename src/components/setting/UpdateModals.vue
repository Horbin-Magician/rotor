<template>
  <!-- Update Confirmation Modal -->
  <n-modal v-model:show="showUpdateModal" preset="dialog" :title="t('message.updateAvailable')" style="width: 400px;">
    <div class="update-modal-content">
      <p>{{ t('message.newVersionAvailable') }} <strong>{{ updateVersion }}</strong> {{ t('message.isAvailable') }}</p>
      <p>{{ t('message.installNow') }}</p>
    </div>
    <template #action>
      <div class="modal-actions">
        <n-button @click="handleCancelUpdate" class="cancel-btn">
          {{ t('message.cancel') }}
        </n-button>
        <n-button type="primary" @click="handleConfirmUpdate" class="confirm-btn">
          {{ t('message.install') }}
        </n-button>
      </div>
    </template>
  </n-modal>

  <!-- Update Progress Modal -->
  <n-modal v-model:show="showProgressModal" :closable="false" :mask-closable="false">
    <n-card style="width: 400px" :title="t('message.updatingApp')">
      <div class="progress-content">
        <n-progress 
          type="line" 
          :percentage="updateProgress" 
          indicator-placement="inside"
          :color="themeVars.primaryColor"
          status="success"
        />
        <p class="progress-status">{{ updateStatus }}</p>
      </div>
    </n-card>
  </n-modal>
</template>

<script setup lang="ts">
import { NModal, NButton, NCard, NProgress, useThemeVars } from 'naive-ui'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const themeVars = useThemeVars()

// Props
interface Props {
  updateVersion: String
  updateProgress: number
  updateStatus: string
}

defineProps<Props>()

// Models
const showUpdateModal = defineModel<boolean>('showUpdateModal', { required: true })
const showProgressModal = defineModel<boolean>('showProgressModal', { required: true })

// Emits
const emit = defineEmits<{
  'confirmUpdate': []
  'cancelUpdate': []
}>()

function handleConfirmUpdate() {
  emit('confirmUpdate')
}

function handleCancelUpdate() {
  emit('cancelUpdate')
}
</script>

<style scoped>
/* Update Modal Styles */
.update-modal-content p {
  margin: 6px 0;
}

.modal-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.cancel-btn {
  min-width: 80px;
}

.confirm-btn {
  min-width: 80px;
}

.progress-status {
  margin-top: 10px;
  text-align: center;
  color: #666;
}
</style>
