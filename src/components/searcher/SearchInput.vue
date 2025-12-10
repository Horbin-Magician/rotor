<template>
  <div class="search-input-container">
    <div class="search-input-wrapper">
      <n-icon size="24" class="search-icon">
        <SearchIcon v-if="!isAiMode" />
        <AiIcon v-else />
      </n-icon>
      <input
        ref="inputRef"
        v-model="query"
        :placeholder="currentPlaceholder"
        autofocus
        @input="handleInput"
        @keydown="handleKeydown"
        class="search-input"
        autocomplete="off"
        spellcheck="false"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { NIcon } from 'naive-ui'
import { SearchRound as SearchIcon, SmartToyOutlined as AiIcon } from '@vicons/material'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

// Props
interface Props {
  modelValue: string
  placeholder?: string
  isAiMode?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: 'Search...',
  isAiMode: false
})

// Emits
const emit = defineEmits<{
  'update:modelValue': [value: string]
  'input': []
  'keydown': [event: KeyboardEvent]
  'toggle-mode': []
}>()

// State
const inputRef = ref<HTMLInputElement>()
const query = ref(props.modelValue)

// Computed
const currentPlaceholder = computed(() => {
  return props.isAiMode ? t('message.aiPlaceholder') : props.placeholder
})

// Watch for external changes
watch(() => props.modelValue, (newVal) => {
  query.value = newVal
})

// Watch for internal changes
watch(query, (newVal) => {
  emit('update:modelValue', newVal)
})

// Methods
const handleInput = () => {
  emit('input')
}

const handleKeydown = (event: KeyboardEvent) => {
  // Tab key to toggle mode
  if (event.key === 'Tab') {
    event.preventDefault()
    emit('toggle-mode')
    return
  }
  emit('keydown', event)
}

const focus = () => {
  inputRef.value?.focus()
}

// Expose methods
defineExpose({
  focus
})
</script>

<style scoped>
.search-input-container {
  position: relative;
  padding: 0 12px;
  background-color: var(--theme-background);
}

.search-input-container::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 10px;
  right: 10px;
  height: 2px;
  border-radius: 1px;
  background-color: var(--theme-primary);
}

.search-input-wrapper {
  display: flex;
  align-items: center;
  height: 50px;
  gap: 8px;
}

.search-icon {
  flex-shrink: 0;
  color: var(--theme-text-secondary);
}

.search-input {
  flex: 1;
  height: 100%;
  border: none;
  outline: none;
  background: transparent;
  font-size: 16px;
  color: var(--theme-text-primary);
  padding: 0;
}

.search-input::placeholder {
  color: var(--theme-text-disabled);
}
</style>
