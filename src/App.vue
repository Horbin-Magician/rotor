<template>
  <n-config-provider :theme="naiveTheme" :theme-overrides="themeOverrides">
    <n-message-provider placement="top">
      <RouterView />
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { darkTheme, NConfigProvider, NMessageProvider, GlobalThemeOverrides } from 'naive-ui'
import { useTheme } from './composables/useTheme'

const { currentTheme, colors } = useTheme()
const naiveTheme = computed(() => (currentTheme.value === 'dark' ? darkTheme : null)) // Naive UI Theme

// Make theme overrides reactive
const themeOverrides = computed<GlobalThemeOverrides>(() => ({
  common: {
    primaryColor: colors.value.primary,
    primaryColorHover: colors.value.primaryHover,
    primaryColorPressed: colors.value.primaryPressed,
    primaryColorSuppl: colors.value.primaryPressed,
  },
}))
</script>

<style>
body {
  user-select: none;
  -webkit-user-select: none;
  -ms-user-select: none;
  margin: 0;
  color: var(--theme-text-primary);
  background-color: var(--theme-background);
}

:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}
</style>
