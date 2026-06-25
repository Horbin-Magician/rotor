import { ref, computed, onMounted } from 'vue'
import { generateCSSVariables, darkTheme, lightTheme, ThemeColors } from '../styles/theme'
import { getCurrentWindow, Theme } from '@tauri-apps/api/window'
import { getConfig } from '../shared/api/core'

// global state
const getSystemTheme = (): Theme =>
  window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'

const currentThemeMode = ref(0)
const currentTheme = ref<Theme>(getSystemTheme())

// system theme change listener
let cleanupSystemThemeListener: (() => void) | null = null

// initialize CSS variables immediately
if (typeof window !== 'undefined') {
  const initialCssVars = generateCSSVariables(currentTheme.value)
  const root = document.documentElement
  Object.entries(initialCssVars).forEach(([key, value]) => {
    root.style.setProperty(key, value)
  })
}

async function getThemeModeFromConfig() {
  const theme = await getConfig('theme')
  return normalizeThemeMode(Number.parseInt(theme, 10))
}

function normalizeThemeMode(mode: number) {
  return mode === 1 || mode === 2 ? mode : 0
}

// convert numeric mode to string mode
function numberToTheme(mode: number): Theme {
  // 0: system, 1: light, 2: dark
  switch (normalizeThemeMode(mode)) {
    case 1:
      return 'light'
    case 2:
      return 'dark'
    default:
      return getSystemTheme()
  }
}

// update theme
function updateTheme() {
  const cssVars = generateCSSVariables(currentTheme.value)
  const root = document.documentElement
  Object.entries(cssVars).forEach(([key, value]) => {
    root.style.setProperty(key, value)
  })
}

// setup system theme listener
function setupSystemThemeListener(callback: (theme: Theme) => void) {
  cleanupSystemThemeListener?.()

  const systemThemeListener = window.matchMedia('(prefers-color-scheme: dark)')

  function handleSystemThemeChange(e: MediaQueryListEvent) {
    const newTheme: Theme = e.matches ? 'dark' : 'light'
    callback(newTheme)
  }

  systemThemeListener.addEventListener('change', handleSystemThemeChange)

  cleanupSystemThemeListener = () => {
    systemThemeListener.removeEventListener('change', handleSystemThemeChange)
    cleanupSystemThemeListener = null
  }
}

export function useTheme() {
  const appWindow = getCurrentWindow()

  // get specific color - make it reactive
  const getColor = (colorKey: keyof ThemeColors) => {
    return computed(() =>
      currentTheme.value === 'dark' ? darkTheme[colorKey] : lightTheme[colorKey],
    )
  }

  const applyThemeMode = (mode: number) => {
    currentThemeMode.value = normalizeThemeMode(mode)
    currentTheme.value = numberToTheme(currentThemeMode.value)
    void appWindow.setTheme(currentTheme.value)
    updateTheme()
  }

  const changeTheme = applyThemeMode

  onMounted(async () => {
    applyThemeMode(await getThemeModeFromConfig())

    setupSystemThemeListener((newTheme: Theme) => {
      if (currentThemeMode.value !== 0) return
      currentTheme.value = newTheme
      void appWindow.setTheme(newTheme)
      updateTheme()
    })
  })

  return {
    // status
    currentTheme: computed(() => currentTheme.value),

    // methods
    getColor,
    changeTheme,
  }
}
