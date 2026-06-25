<template>
  <div
    class="setting-item"
    :class="{
      'setting-item-has-label': hasLabel,
      'setting-item-conflict': conflict,
    }"
  >
    <span v-if="hasLabel" class="setting-label">
      <slot name="label">{{ label }}</slot>
    </span>
    <div class="setting-control">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, useSlots } from 'vue'

const props = withDefaults(
  defineProps<{
    label?: string
    conflict?: boolean
  }>(),
  {
    label: '',
    conflict: false,
  },
)

const slots = useSlots()
const hasLabel = computed(() => Boolean(props.label || slots.label))
</script>
