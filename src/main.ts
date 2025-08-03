import { createApp } from "vue";
import App from "./App.vue";

let app = createApp(App);

// Router setting
import { createRouter, createWebHistory } from 'vue-router'

const routes = [
  { path: '/', component: () => import('./pages/Setting.vue') },
  { path: '/ScreenShotter/Mask', component: () => import('./pages/ScreenShotter/Mask.vue') },
  { path: '/ScreenShotter/Pin', component: () => import('./pages/ScreenShotter/Pin.vue') },
  { path: '/Searcher', component: () => import('./pages/Searcher.vue') },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

app.use(router);

// I18n setting
import { createI18n } from 'vue-i18n'
import zhCN from './locales/zh-CN.ts'
import enUS from './locales/en-US.ts'
import { invoke } from '@tauri-apps/api/core'

// Function to determine initial locale
function getInitialLocale(): 'zh-CN' | 'en-US' {
  const systemLang = navigator.language || navigator.languages[0]
  return systemLang.startsWith('zh') ? 'zh-CN' : 'en-US'
}

const i18n = createI18n({
  legacy: false,
  locale: getInitialLocale(),
  messages: {
    'zh-CN': zhCN,
    'en-US': enUS
  }
})

// Load language setting from config and update locale
invoke("get_all_cfg").then((config: any) => {
  const languageSetting = Number(config["language"]) || 0
  if (languageSetting === 0) { // System default
    i18n.global.locale.value = getInitialLocale()
  } else if (languageSetting === 1) { // Chinese
    i18n.global.locale.value = 'zh-CN'
  } else if (languageSetting === 2) { // English
    i18n.global.locale.value = 'en-US'
  }
})

app.use(i18n)

// mount
app.mount("#app");
