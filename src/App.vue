<template>
  <n-config-provider :theme="currentTheme" :theme-overrides="themeOverrides">
    <n-notification-provider placement="top">
      <RouterView />
    </n-notification-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watchEffect } from 'vue'
import { darkTheme, NConfigProvider, NNotificationProvider, GlobalThemeOverrides } from 'naive-ui'
import { BuiltInGlobalTheme } from 'naive-ui/es/themes/interface'
import { invoke } from '@tauri-apps/api/core'

const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#54a4db',
    primaryColorHover: '#54a4db',
    primaryColorPressed: '#54a4db',
    primaryColorSuppl: '#54a4db',
  },
}

const themeMode = ref(0) // 0: follow system, 1: light, 2: dark
const currentTheme = ref<BuiltInGlobalTheme | null>(null)

// Create a media query listener for system theme changes
const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')

const updateThemeClass = () => {
  const body = document.body
  body.classList.remove('light-theme', 'dark-theme', 'system-theme')
  
  if (themeMode.value === 1) {
    body.classList.add('light-theme')
    currentTheme.value = null
  } else if (themeMode.value === 2) {
    body.classList.add('dark-theme')
    currentTheme.value = darkTheme
  } else {
    body.classList.add('system-theme')
    currentTheme.value = mediaQuery.matches ? darkTheme : null
  }
}

// Watch for system theme changes
const handleSystemThemeChange = () => {
  if (themeMode.value === 0) {
    updateThemeClass()
  }
}

// Load theme setting from config
onMounted(async () => {
  try {
    const config: any = await invoke("get_all_cfg")
    themeMode.value = Number(config["theme"]) || 0
    updateThemeClass()
  } catch (error) {
    console.error('Failed to load theme config:', error)
    updateThemeClass()
  }
  
  // Add listener for system theme changes
  mediaQuery.addEventListener('change', handleSystemThemeChange)
})

// Watch for theme mode changes
watchEffect(() => {
  updateThemeClass()
})

// Provide global theme update method
const updateTheme = (newTheme: number) => {
  themeMode.value = newTheme
}

// Expose to window for external access
if (typeof window !== 'undefined') {
  (window as any).updateAppTheme = updateTheme
}

// Cleanup
onUnmounted(() => {
  mediaQuery.removeEventListener('change', handleSystemThemeChange)
})
</script>

<style>
body {
  user-select: none;
  -webkit-user-select: none;
  -ms-user-select: none;
  margin: 0;
}

:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  /* Light theme variables */
  --light-text-color: #121212;
  --light-bg-color: #f6f6f6;
  --light-border-color: #e0e0e0;
  --light-hover-color: #007bff;

  /* Dark theme variables */
  --dark-text-color: #f6f6f6;
  --dark-bg-color: #121212;
  --dark-border-color: #333333;
  --dark-hover-color: #24c8db;

  /* Default to light theme */
  color: var(--light-text-color);
  background-color: var(--light-bg-color);

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

/* Dark theme class */
.dark-theme {
  color: var(--dark-text-color) !important;
  background-color: var(--dark-bg-color) !important;
}

/* Light theme class */
.light-theme {
  color: var(--light-text-color) !important;
  background-color: var(--light-bg-color) !important;
}

/* Follow system theme */
@media (prefers-color-scheme: dark) {
  .system-theme {
    color: var(--dark-text-color);
    background-color: var(--dark-bg-color);
  }

  .system-theme a:hover {
    color: var(--dark-hover-color);
  }
}
</style>
