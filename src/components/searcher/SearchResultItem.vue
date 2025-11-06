<template>
  <div
    :class="['search-item', { selected: isSelected }]"
    @click="handleClick"
    @mouseenter="handleMouseEnter"
  >
    <div class="item-icon">
      <img 
        v-if="item.icon_data" 
        :src="`data:image/png;base64,${item.icon_data}`" 
        alt="Icon"
        loading="lazy"
      />
    </div>
    <div class="item-content">
      <div class="item-title">
        <span class="title-text">{{ getDisplayTitle(item) }}</span>
        <span :class="['type-tag', `type-tag-${item.type}`]">
          {{ getTypeLabel(item.type) }}
        </span>
      </div>
      <div class="item-subtitle">{{ item.subtitle }}</div>
    </div>
    <div v-if="item.actions" class="item-actions">
      <div
        v-for="action in item.actions"
        :key="action.type"
        class="item-action-btn"
        :title="$t(action.title)"
        @click.stop="handleActionClick(action)"
      >
        <n-icon size="20">
          <component :is="getActionIcon(action.type)" />
        </n-icon>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { NIcon } from 'naive-ui'
import {
  AdminPanelSettingsFilled as OpenAsAdminIcon,
  FolderCopyRound as OpenFolderIcon,
  ErrorFilled as ErrorIcon,
} from '@vicons/material'

// Types
interface Action {
  type: string
  title: string
}

export interface SearchItem {
  title: string
  subtitle: string
  type: ItemType
  actions?: Action[]
  icon_data?: string
  alias?: string
}

type ItemType = 'app' | 'file'
type ActionType = 'OpenAsAdmin' | 'OpenFolder'

// Props
interface Props {
  item: SearchItem
  isSelected: boolean
}

const props = defineProps<Props>()

// Emits
const emit = defineEmits<{
  'click': [item: SearchItem]
  'action-click': [action: Action, item: SearchItem]
  'mouse-enter': []
}>()

// Constants
const ACTION_ICONS: Record<ActionType, any> = {
  OpenAsAdmin: OpenAsAdminIcon,
  OpenFolder: OpenFolderIcon
} as const

// Methods
const getDisplayTitle = (item: SearchItem): string => {
  const title = item.alias || item.title
  
  // If item type is 'app', remove file extension
  if (item.type === 'app') {
    const lastDotIndex = title.lastIndexOf('.')
    if (lastDotIndex > 0) {
      return title.substring(0, lastDotIndex)
    }
  }
  
  return title
}

const getActionIcon = (type: string) => ACTION_ICONS[type as ActionType] || ErrorIcon

const getTypeLabel = (type: ItemType): string => {
  const labels: Record<ItemType, string> = {
    app: 'APP',
    file: 'FILE'
  }
  return labels[type] || type.toUpperCase()
}

const handleClick = () => {
  emit('click', props.item)
}

const handleActionClick = (action: Action) => {
  emit('action-click', action, props.item)
}

const handleMouseEnter = () => {
  emit('mouse-enter')
}
</script>

<style scoped>
.search-item {
  width: 100%;
  display: flex;
  align-items: center;
  transition: background-color 0.1s ease;
  height: 60px;
  position: relative;
  overflow: hidden;
}

.search-item.selected {
  background-color: var(--theme-primary-overlay);
}

.item-icon {
  padding: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.item-icon img {
  width: 34px;
  height: 34px;
}

.item-content {
  flex: 1;
  min-width: 0;
  padding-right: 10px;
  transition: padding-right 0.3s ease;
}

.item-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--theme-text-primary);
  margin-bottom: 2px;
  display: flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
  overflow: hidden;
}

.title-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.type-tag {
  flex-shrink: 0;
  font-size: 10px;
  padding: 0px 6px;
  border-radius: 10px;
  letter-spacing: 0.5px;
  opacity: 0.8;
}

.type-tag-app {
  background-color: #3b82f6;
  color: #ffffff;
}

.type-tag-file {
  background-color: #10b981;
  color: #ffffff;
}

.item-subtitle {
  font-size: 12px;
  color: var(--theme-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.item-actions {
  height: 100%;
  display: flex;
  align-items: center;
  transform: translateX(100%);
  transition: transform 0.3s ease;
  position: absolute;
  right: 0;
  top: 0;
}

.search-item:hover .item-actions {
  transform: translateX(0);
}

.search-item:hover .item-content {
  padding-right: 80px;
}

.item-action-btn {
  width: 38px;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.2s ease;
  cursor: pointer;
}

.item-action-btn:hover {
  color: var(--theme-primary-hover);
}
</style>
