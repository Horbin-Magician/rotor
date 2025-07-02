<template>
  <n-config-provider :theme="darkTheme" :theme-overrides="themeOverrides">
    <RouterView />
  </n-config-provider>
</template>

<script setup lang="ts">
  import { darkTheme, NConfigProvider, GlobalThemeOverrides } from 'naive-ui'

  const themeOverrides: GlobalThemeOverrides = {
    common: {
      primaryColor: '#54a4db',
      primaryColorHover: '#54a4db',
      primaryColorPressed: '#54a4db',
      primaryColorSuppl: '#54a4db',
    },
  }

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

  color: #121212;
  background-color: #f6f6f6;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

@media (prefers-color-scheme: dark) {
  :root {
    color: #f6f6f6;
    background-color: #121212;
  }

  a:hover {
    color: #24c8db;
  }
}

</style>
