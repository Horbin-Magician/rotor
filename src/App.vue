<template>
  <n-config-provider :theme="currentTheme" :theme-overrides="themeOverrides">
    <RouterView />
  </n-config-provider>
</template>

<script setup lang="ts">
  import { ref, onMounted } from 'vue'
  import { darkTheme, NConfigProvider, GlobalThemeOverrides } from 'naive-ui'
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

  // load theme setting from config
  onMounted(async () => {
    try {
      const config: any = await invoke("get_all_cfg")
      themeMode.value = Number(config["theme"]) || 0
      updateThemeClass()
    } catch (error) {
      console.error('Failed to load theme config:', error)
      updateThemeClass()
    }
  })

  const updateThemeClass = () => {
    const body = document.body
    body.classList.remove('light-theme', 'dark-theme', 'system-theme') // remove all theme class
    
    if (themeMode.value === 1) {
      body.classList.add('light-theme')
      currentTheme.value = null
    } else if (themeMode.value === 2) {
      body.classList.add('dark-theme')
      currentTheme.value = darkTheme
    } else {
      body.classList.add('system-theme')
      currentTheme.value = window.matchMedia('(prefers-color-scheme: dark)').matches ? darkTheme : null
    }
  }

  // Provide global topic update methods
  const updateTheme = (newTheme: number) => {
    themeMode.value = newTheme
    updateThemeClass()
  }
  ;(window as any).updateAppTheme = updateTheme

</script>

<style>
body {
  user-select: none;
  -webkit-user-select: none; /* 兼容 Safari 和 Chrome */
  -ms-user-select: none;     /* 兼容 IE/Edge */
  margin: 0;
}

:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  /* 浅色主题变量 */
  --light-text-color: #121212;
  --light-bg-color: #f6f6f6;
  --light-border-color: #e0e0e0;
  --light-hover-color: #007bff;

  /* 深色主题变量 */
  --dark-text-color: #f6f6f6;
  --dark-bg-color: #121212;
  --dark-border-color: #333333;
  --dark-hover-color: #24c8db;

  /* 默认使用浅色主题 */
  color: var(--light-text-color);
  background-color: var(--light-bg-color);

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

/* 深色主题类 */
.dark-theme {
  color: var(--dark-text-color) !important;
  background-color: var(--dark-bg-color) !important;
}

/* 浅色主题类 */
.light-theme {
  color: var(--light-text-color) !important;
  background-color: var(--light-bg-color) !important;
}

/* 跟随系统主题 */
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
