<template>
  <SettingsSection>
    <template #title>
      <span>{{ t('message.systemOverview') }}</span>
    </template>
    <template #extra>
      <n-button
        class="reload-button"
        text
        size="small"
        :keyboard="false"
        :disabled="loading"
        :class="{ 'is-loading': loading }"
        @click="loadOverview"
      >
        <template #icon>
          <n-icon class="reload-button-icon">
            <RefreshIcon />
          </n-icon>
        </template>
      </n-button>
    </template>

    <div v-if="overview" class="overview-strip">
      <div class="overview-item">
        <n-icon v-once class="overview-icon" size="16">
          <MemoryIcon />
        </n-icon>
        <span class="overview-label">{{ t('message.memoryUsage') }}</span>
        <span class="overview-value">{{ formatBytes(overview.memory.residentBytes) }}</span>
      </div>

      <div class="overview-item">
        <n-icon v-once class="overview-icon" size="16">
          <StorageIcon />
        </n-icon>
        <span class="overview-label">{{ t('message.indexOverview') }}</span>
        <span class="overview-value">
          {{ overview.searchIndex.indexedVolumeCount }}/{{ overview.searchIndex.volumeCount }}
        </span>
      </div>

      <div class="overview-item">
        <n-icon v-once class="overview-icon" size="16">
          <SecurityIcon />
        </n-icon>
        <span class="overview-label">{{ t('message.permissionOverview') }}</span>
        <span class="overview-value">{{ grantedPermissionCount }}/{{ overview.permissions.length }}</span>
      </div>
    </div>

    <n-alert v-else-if="loadError" type="error" :bordered="false">
      {{ loadError }}
    </n-alert>
    <div v-else class="overview-strip">
      <div v-for="item in 3" :key="item" class="overview-item">
        <n-skeleton class="skeleton-icon" circle />
        <n-skeleton text class="skeleton-label" />
        <n-skeleton text class="skeleton-value" />
      </div>
    </div>
  </SettingsSection>

  <SettingsSection :title="t('message.indexStatus')">
    <div v-if="overview" class="info-list">
      <div class="info-row">
        <span class="info-label">{{ t('message.indexState') }}</span>
        <n-tag size="small" :type="indexStateTagType">{{ t(searchStateLabel(overview.searchIndex.state)) }}</n-tag>
      </div>
      <div class="info-row">
        <span class="info-label">{{ t('message.indexFileSize') }}</span>
        <span>{{ formatBytes(overview.searchIndex.indexFileSizeBytes) }}</span>
      </div>
      <div class="info-row">
        <span class="info-label">{{ t('message.indexItems') }}</span>
        <span>{{ overview.searchIndex.indexItemCount }}</span>
      </div>
      <div class="info-row">
        <span class="info-label">{{ t('message.lastIndexedAt') }}</span>
        <span>{{ formatDate(overview.searchIndex.latestIndexModifiedAt) }}</span>
      </div>

      <div class="volume-list">
        <div v-for="volume in overview.searchIndex.volumes" :key="volume.name" class="volume-row">
          <div class="volume-main">
            <span class="volume-name">{{ volume.name }}</span>
            <n-tag v-if="!volume.indexed" size="small" type="warning">
              {{ t('message.notIndexed') }}
            </n-tag>
          </div>
          <div class="volume-detail">
            {{ formatVolumeDetail(volume.indexItemCount, volume.indexFileSizeBytes, volume.indexFileModifiedAt) }}
          </div>
        </div>
      </div>
    </div>
    <div v-else-if="!loadError" class="info-list">
      <div v-for="item in 4" :key="item" class="info-row">
        <n-skeleton text class="skeleton-info-label" />
        <n-skeleton text class="skeleton-info-value" />
      </div>
      <div class="volume-list">
        <div v-for="item in 2" :key="item" class="volume-row">
          <div class="volume-main">
            <n-skeleton text class="skeleton-volume-name" />
            <n-skeleton text class="skeleton-tag" />
          </div>
          <div class="volume-detail">
            <n-skeleton text class="skeleton-volume-detail" />
          </div>
        </div>
      </div>
    </div>
  </SettingsSection>

  <SettingsSection :title="t('message.permissionStatus')">
    <div v-if="overview" class="status-list">
      <div v-for="permission in overview.permissions" :key="permission.key" class="status-row">
        <div class="status-content">
          <span class="status-title">{{ permissionLabel(permission) }}</span>
          <span class="status-detail">{{ permissionDetail(permission) }}</span>
        </div>
        <n-icon class="status-icon" :class="statusClass(permission.granted)" size="18">
          <component :is="statusIcon(permission.granted)" />
        </n-icon>
      </div>
    </div>
    <div v-else-if="!loadError" class="status-list">
      <div v-for="item in 3" :key="item" class="status-row">
        <n-skeleton class="skeleton-status-icon" circle />
        <div class="status-content">
          <n-skeleton text class="skeleton-status-title" />
          <n-skeleton text class="skeleton-status-detail" />
        </div>
        <n-skeleton text class="skeleton-tag" />
      </div>
    </div>
  </SettingsSection>

</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { NAlert, NButton, NIcon, NSkeleton, NTag } from 'naive-ui'
import {
  CheckCircleRound as CheckIcon,
  ErrorOutlineRound as ErrorIcon,
  HelpOutlineRound as UnknownIcon,
  MemoryRound as MemoryIcon,
  RefreshRound as RefreshIcon,
  SecurityRound as SecurityIcon,
  StorageRound as StorageIcon,
} from '@vicons/material'
import { useI18n } from 'vue-i18n'
import SettingsSection from './SettingsSection.vue'
import { getOverviewInfo, type OverviewInfo, type PermissionStatus } from '../../shared/api/core'

const { t } = useI18n()

const overview = ref<OverviewInfo | null>(null)
const loading = ref(false)
const loadError = ref('')

const grantedPermissionCount = computed(() => {
  return overview.value?.permissions.filter((permission) => permission.granted === true).length ?? 0
})

const indexStateTagType = computed(() => {
  switch (overview.value?.searchIndex.state) {
    case 'ready':
      return 'success'
    case 'released':
      return 'info'
    case 'unbuilt':
      return 'warning'
    default:
      return 'default'
  }
})

async function loadOverview() {
  loading.value = true
  loadError.value = ''

  try {
    overview.value = await getOverviewInfo()
  } catch (error) {
    loadError.value = `${t('message.overviewLoadFailed')}: ${String(error)}`
  } finally {
    loading.value = false
  }
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B'
  }

  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unitIndex = 0

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024
    unitIndex += 1
  }

  const precision = value >= 10 || unitIndex === 0 ? 0 : 1
  return `${value.toFixed(precision)} ${units[unitIndex]}`
}

function formatDate(timestamp?: number | null) {
  if (!timestamp) {
    return t('message.notAvailable')
  }

  return new Date(timestamp).toLocaleString()
}

function formatCount(value?: number | null) {
  return value == null ? t('message.notAvailable') : String(value)
}

function formatVolumeDetail(itemCount: number | null | undefined, sizeBytes: number, modifiedAt?: number | null) {
  return `${formatCount(itemCount)} · ${formatBytes(sizeBytes)} · ${formatDate(modifiedAt)}`
}

function searchStateLabel(state: string) {
  const labels: Record<string, string> = {
    ready: 'message.indexReady',
    released: 'message.indexReleased',
    unbuilt: 'message.indexUnbuilt',
    unavailable: 'message.indexUnavailable',
  }

  return labels[state] ?? 'message.indexUnavailable'
}

function statusClass(granted?: boolean | null) {
  return {
    'status-ok': granted === true,
    'status-error': granted === false,
    'status-unknown': granted == null,
  }
}

function statusIcon(granted?: boolean | null) {
  if (granted === true) {
    return CheckIcon
  }
  if (granted === false) {
    return ErrorIcon
  }
  return UnknownIcon
}

function permissionLabel(permission: PermissionStatus) {
  const labels: Record<string, string> = {
    administrator: t('message.permissionAdministrator'),
    file_search: t('message.permissionFileSearch'),
    screen_capture: t('message.permissionScreenCapture'),
  }

  return labels[permission.key] ?? permission.name
}

function permissionDetail(permission: PermissionStatus) {
  const details: Record<string, string> = {
    administrator: t('message.permissionAdministratorDetail'),
    file_search: t('message.permissionFileSearchDetail'),
    screen_capture: t('message.permissionScreenCaptureDetail'),
  }

  return details[permission.key] ?? permission.detail
}

onMounted(loadOverview)
</script>

<style scoped>
.reload-button {
  width: 24px;
  height: 24px;
  border-radius: 50%;
}

.reload-button,
.reload-button:hover {
  background: transparent !important;
  filter: none !important;
  outline: none;
  box-shadow: none !important;
}

.reload-button-icon {
  transform-origin: center;
}

.reload-button.is-loading .reload-button-icon {
  animation: reload-spin 0.8s linear infinite;
}

@keyframes reload-spin {
  from {
    transform: rotate(0deg);
  }

  to {
    transform: rotate(360deg);
  }
}

.overview-strip {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 180px), 1fr));
  gap: 8px;
  justify-content: start;
  margin-left: 12px;
  width: calc(100% - 12px);
  min-width: 0;
  box-sizing: border-box;
  contain: layout paint;
}

.overview-item {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) minmax(5ch, auto);
  column-gap: 6px;
  align-items: center;
  height: 28px;
  min-width: 0;
  overflow: hidden;
  padding: 0 10px;
  box-sizing: border-box;
  border: 1px solid var(--theme-border);
  border-radius: 6px;
  background: var(--theme-background-secondary);
}

.overview-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 16px;
  width: 16px;
  height: 16px;
  line-height: 1;
}

.overview-icon :deep(svg) {
  display: block;
  width: 16px;
  height: 16px;
}

.overview-icon,
.overview-label {
  color: var(--theme-text-secondary);
}

.overview-label {
  min-width: 0;
}

.overview-label,
.overview-value {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.overview-value {
  min-width: 5ch;
  text-align: right;
  color: var(--theme-text-primary);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.skeleton-icon,
.skeleton-status-icon {
  width: 16px;
  height: 16px;
  flex: none;
}

.skeleton-label {
  width: 72px;
}

.skeleton-value {
  width: 44px;
  margin-left: auto;
}

.skeleton-info-label {
  width: 112px;
}

.skeleton-info-value {
  width: 86px;
}

.skeleton-volume-name {
  width: 130px;
}

.skeleton-tag {
  width: 48px;
}

.skeleton-volume-detail {
  width: 190px;
}

.skeleton-status-title {
  width: 86px;
  flex: none;
}

.skeleton-status-detail {
  width: min(240px, 55%);
}

.volume-detail,
.status-detail {
  color: var(--theme-text-secondary);
  font-size: 12px;
}

.info-list,
.status-list {
  margin-left: 12px;
  min-width: 0;
}

.info-row,
.status-row,
.volume-row {
  min-height: 28px;
  margin-bottom: 6px;
}

.info-row,
.status-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.info-label {
  min-width: 130px;
  color: var(--theme-text-secondary);
}

.volume-list {
  margin-top: 8px;
}

.volume-row {
  min-width: 0;
  padding: 6px 10px;
  border: 1px solid var(--theme-border);
  border-radius: 6px;
  background: var(--theme-background-secondary);
}

.volume-main {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.volume-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 600;
}

.status-icon {
  flex: none;
}

.status-ok {
  color: var(--theme-success);
}

.status-error {
  color: var(--theme-error);
}

.status-unknown {
  color: var(--theme-text-secondary);
}

.status-content {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.status-title {
  flex: none;
  white-space: nowrap;
  font-weight: 600;
}

.status-detail {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 520px) {
  .overview-strip {
    grid-template-columns: 1fr;
    width: calc(100% - 12px);
  }
}
</style>
