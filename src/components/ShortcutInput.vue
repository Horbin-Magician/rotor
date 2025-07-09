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

// Define valid keys that can be used as shortcuts
const validKeys = new Set([
  'KeyA', 'KeyB', 'KeyC', 'KeyD', 'KeyE', 'KeyF', 'KeyG', 'KeyH', 'KeyI', 'KeyJ', 'KeyK', 'KeyL', 'KeyM', 
  'KeyN', 'KeyO', 'KeyP', 'KeyQ', 'KeyR', 'KeyS', 'KeyT', 'KeyU', 'KeyV', 'KeyW', 'KeyX', 'KeyY', 'KeyZ',
  'Digit0', 'Digit1', 'Digit2', 'Digit3', 'Digit4', 'Digit5', 'Digit6', 'Digit7', 'Digit8', 'Digit9',
  'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12',
  'Escape', 'Enter', 'Tab', 'Space', 'Backspace', 'Delete',
  'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight',
  'Home', 'End', 'PageUp', 'PageDown',
]);

// Define props
const props = defineProps<{
  shortcut: string;
}>();

// Define emits
const emit = defineEmits(['update:shortcut']);

// State for recording shortcuts
const isRecording = ref(false);
const currentModifiers = ref(new Set<string>());
const currentKey = ref('');

// Format the display value // TODO 计算出显示的按键信息
const displayValue = computed(() => {
  let returnValue = props.shortcut

  if (isRecording.value) {
    const modifiers = Array.from(currentModifiers.value)
    const key = currentKey.value
    
    if (modifiers.length > 0 && key) {
      returnValue = [...modifiers, key].join('+')
    } else if (modifiers.length > 0) {
      returnValue = [...modifiers, '...'].join('+')
    } else if (key) {
      returnValue = key
    } else {
      returnValue = '按下快捷键'
    }
  }

  returnValue = returnValue.replace(/\b(Key|Digit)/g, '');
  returnValue = returnValue.replace(/Escape/gi, 'Esc');
  returnValue = returnValue.replace(/shift/gi, 'Shift');
  returnValue = returnValue.replace(/Super/gi, 'Cmd');
  returnValue = returnValue.replace(/alt/gi, 'Alt');
  returnValue = returnValue.replace(/ctrl/gi, 'Ctrl');
  return returnValue;
});

// Start recording shortcuts
function startRecording() {
  isRecording.value = true;
  currentModifiers.value.clear();
  currentKey.value = '';
}

// Stop recording shortcuts
function stopRecording() {
  isRecording.value = false;
}

// Handle keydown events
function handleKeyDown(event: KeyboardEvent) {
  // Prevent default behavior
  event.preventDefault()
  event.stopPropagation()

  // Reset current key
  currentKey.value = ''
  
  // Update modifiers
  currentModifiers.value.clear()
  if (event.ctrlKey) currentModifiers.value.add('Ctrl')
  if (event.altKey) currentModifiers.value.add('Alt')
  if (event.shiftKey) currentModifiers.value.add('Shift')
  if (event.metaKey) currentModifiers.value.add('Cmd')
  
  let key = event.code // Get the key
  // Check if the key is valid
  if (validKeys.has(key)) {
    currentKey.value = key
    const modifiers = Array.from(currentModifiers.value)
    const shortcut = [...modifiers, currentKey.value].join('+')
    emit('update:shortcut', shortcut)
  }
}

</script>

<style scoped>
.shortcut-input {
  text-align: center;
}
</style>
