<template>
  <div class="search-input-container">
    <div class="search-input-wrapper">
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
import { ref, watch } from 'vue'
import { NIcon } from 'naive-ui'
import { SearchRound as SearchIcon } from '@vicons/material'

// Props
interface Props {
  modelValue: string
  placeholder?: string
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: 'Search...'
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
