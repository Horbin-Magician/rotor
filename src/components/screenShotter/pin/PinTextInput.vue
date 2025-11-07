<template>
  <div v-if="visible" 
       class="text-input-overlay"
       :style="{ left: position.x + 'px', top: position.y + 'px' }">
    <input 
      ref="inputRef"
      :value="modelValue"
      @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
      @keyup="handleKeyup"
      @blur="$emit('blur')"
      class="text-input"
      autofocus
    />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';

interface Props {
  visible: boolean;
  position: { x: number; y: number };
  modelValue: string;
}

interface Emits {
  (e: 'update:modelValue', value: string): void;
  (e: 'blur'): void;
  (e: 'finish'): void;
  (e: 'cancel'): void;
}

defineProps<Props>();
const emit = defineEmits<Emits>();

const inputRef = ref<HTMLInputElement | null>(null);

function handleKeyup(event: KeyboardEvent) {
  if (event.key === 'Enter') {
    event.preventDefault();
    emit('finish');
  } else if (event.key === 'Escape') {
    event.preventDefault();
    emit('cancel');
  }
  event.stopPropagation();
}

defineExpose({
  inputRef
});
</script>

<style scoped>
.text-input-overlay {
  position: absolute;
  z-index: 2000;
}

.text-input {
  padding: 4px 8px;
  border: 1px solid var(--theme-primary);
  border-radius: 4px;
  background-color: var(--theme-background-secondary);
  font-size: 16px;
  outline: none;
  color: var(--theme-text-primary);
  width: 100px;
}
</style>
