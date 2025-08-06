import { createI18n } from 'vue-i18n'
import zhCN from '../locales/zh-CN'
import enUS from '../locales/en-US'
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
}).catch(error => {
  console.error('Failed to load language config:', error)
})

export default i18n
