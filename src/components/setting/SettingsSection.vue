<template>
  <section class="settings-card">
    <header class="settings-card-title">
      <div class="settings-card-title-main">
        <slot name="title">{{ title }}</slot>
      </div>
      <div v-if="$slots.extra" class="settings-card-title-extra">
        <slot name="extra" />
      </div>
    </header>
    <div class="settings-card-body">
      <slot />
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  title?: string
}>()
</script>
