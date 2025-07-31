<template>
  <n-input 
    :value="displayValue" 
    readonly 
    class="shortcut-input"
    :status="isRecording ? 'info' : ''"
    @focus="startRecording"
    @blur="stopRecording"
    @keydown.stop.prevent="handleKeyDown"
  />
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useI18n } from 'vue-i18n';

const { t } = useI18n();

// Constants
const VALID_KEYS = new Set([
  // Letters
  'KeyA', 'KeyB', 'KeyC', 'KeyD', 'KeyE', 'KeyF', 'KeyG', 'KeyH', 'KeyI', 'KeyJ', 'KeyK', 'KeyL', 'KeyM', 
  'KeyN', 'KeyO', 'KeyP', 'KeyQ', 'KeyR', 'KeyS', 'KeyT', 'KeyU', 'KeyV', 'KeyW', 'KeyX', 'KeyY', 'KeyZ',
  // Numbers
  'Digit0', 'Digit1', 'Digit2', 'Digit3', 'Digit4', 'Digit5', 'Digit6', 'Digit7', 'Digit8', 'Digit9',
  // Function keys
  'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12',
  // Special keys
  'Escape', 'Enter', 'Tab', 'Space', 'Backspace', 'Delete',
  // Arrow keys
  'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight',
  // Navigation keys
  'Home', 'End', 'PageUp', 'PageDown',
]);

const KEY_REPLACEMENTS = {
  'Key': '',
  'Digit': '',
  'Escape': 'Esc',
  'shift': 'Shift',
  'Super': 'Cmd',
  'alt': 'Alt',
  'ctrl': 'Ctrl'
} as const;

// Props and emits
const props = defineProps<{
  shortcut: string;
}>();

const emit = defineEmits<{
  'update:shortcut': [value: string]
}>();

// State
const isRecording = ref(false);
const currentModifiers = ref(new Set<string>());
const currentKey = ref('');

// Computed properties
const displayValue = computed(() => {
  let value = props.shortcut;

  if (isRecording.value) {
    const modifiers = Array.from(currentModifiers.value);
    const key = currentKey.value;
    
    if (modifiers.length > 0 && key) {
      value = [...modifiers, key].join('+');
    } else if (modifiers.length > 0) {
      value = [...modifiers, '...'].join('+');
    } else if (key) {
      value = key;
    } else {
      value = t('message.pressShortcut');
    }
  }

  // Apply key replacements
  return Object.entries(KEY_REPLACEMENTS).reduce((acc, [pattern, replacement]) => {
    const regex = pattern === 'Key' || pattern === 'Digit' 
      ? new RegExp(`\\b${pattern}`, 'g')
      : new RegExp(pattern, 'gi');
    return acc.replace(regex, replacement);
  }, value);
});

// Methods
const startRecording = () => {
  isRecording.value = true;
  currentModifiers.value.clear();
  currentKey.value = '';
};

const stopRecording = () => {
  isRecording.value = false;
};

const handleKeyDown = (event: KeyboardEvent) => {
  event.preventDefault();
  event.stopPropagation();

  // Reset current key
  currentKey.value = '';
  
  // Update modifiers
  currentModifiers.value.clear();
  if (event.ctrlKey) currentModifiers.value.add('Ctrl');
  if (event.altKey) currentModifiers.value.add('Alt');
  if (event.shiftKey) currentModifiers.value.add('Shift');
  if (event.metaKey) currentModifiers.value.add('Cmd');
  
  const key = event.code;
  if (VALID_KEYS.has(key)) {
    currentKey.value = key;
    const modifiers = Array.from(currentModifiers.value);
    const shortcut = [...modifiers, key].join('+');
    emit('update:shortcut', shortcut);
  }
};

</script>

<style scoped>
.shortcut-input {
  text-align: center;
}
</style>
