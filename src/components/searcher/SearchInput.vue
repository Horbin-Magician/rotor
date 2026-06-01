<template>
  <div class="search-input-container">
    <div
      class="search-input-wrapper"
      :class="statusClass"
    >
      <n-icon size="24" class="search-icon">
        <SearchIcon />
      </n-icon>
      <input
        ref="inputRef"
        v-model="query"
        :placeholder="placeholder"
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
import { computed, ref, watch } from 'vue'
import { NIcon } from 'naive-ui'
import { SearchRound as SearchIcon } from '@vicons/material'
import type { SearchIndexState } from '../../features/searcher/types'

// Props
interface Props {
  modelValue: string
  placeholder?: string
  indexState?: SearchIndexState
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: 'Search...',
  indexState: 'loading',
})

// Emits
const emit = defineEmits<{
  'update:modelValue': [value: string]
  'input': []
  'keydown': [event: KeyboardEvent]
}>()

// State
const inputRef = ref<HTMLInputElement>()
const query = ref(props.modelValue)

const statusClass = computed(() => ({
  'is-indexing': props.indexState === 'building' || props.indexState === 'loading' || props.indexState === 'unbuilt' || props.indexState === 'released',
  'is-error': props.indexState === 'error' || props.indexState === 'unavailable',
}))

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

.search-input-wrapper {
  --index-line-color: var(--theme-primary);
  --index-line-width: 100%;
  position: relative;
  display: flex;
  align-items: center;
  height: 50px;
  gap: 8px;
}

.search-input-wrapper::after {
  content: '';
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  width: var(--index-line-width);
  height: 2px;
  margin: 0 auto;
  border-radius: 999px;
  background-color: var(--index-line-color);
  box-shadow: 0 0 0 transparent;
  transition:
    width 0.24s ease,
    background-color 0.2s ease,
    box-shadow 0.2s ease;
}

.search-input-wrapper.is-indexing::after {
  animation: index-line-breathe 1.15s ease-in-out infinite;
}

.search-input-wrapper.is-error {
  --index-line-color: var(--theme-error);
}

.search-input-wrapper.is-error::after {
  width: 100%;
  box-shadow: 0 0 8px rgba(255, 77, 79, 0.32);
  box-shadow: 0 0 8px color-mix(in srgb, var(--theme-error) 36%, transparent);
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

@keyframes index-line-breathe {
  0%,
  100% {
    width: 38%;
  }

  50% {
    width: 100%;
  }
}
</style>
