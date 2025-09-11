import { ref, computed, onMounted } from 'vue'
import { generateCSSVariables, darkTheme, lightTheme, ThemeColors } from '../styles/theme'
import { getCurrentWindow, Theme } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'

// global state
const defaultTheme: Theme = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
const currentTheme = ref<Theme>(defaultTheme)

// system theme change listener
let systemThemeListener: MediaQueryList | null = null

// initialize CSS variables immediately
if (typeof window !== 'undefined') {
  const initialCssVars = generateCSSVariables(currentTheme.value)
  const root = document.documentElement
  Object.entries(initialCssVars).forEach(([key, value]) => {
    root.style.setProperty(key, value)
  })
}

async function get_theme_from_config(): Promise<Theme> {
  const theme: string = await invoke("get_cfg", { k: "theme" });
  let res = numberToTheme(parseInt(theme));
  return res;
}

// convert numeric mode to string mode
function numberToTheme(mode: number): Theme { // 0: system, 1: light, 2: dark
  switch (mode) {
    case 1: return 'light'
    case 2: return 'dark'
    default: return defaultTheme
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
  if (systemThemeListener) {
    systemThemeListener.removeEventListener('change', handleSystemThemeChange)
  }
  
  systemThemeListener = window.matchMedia('(prefers-color-scheme: dark)')
  
  function handleSystemThemeChange(e: MediaQueryListEvent) {
    const newTheme: Theme = e.matches ? 'dark' : 'light'
    callback(newTheme)
  }
  
  systemThemeListener.addEventListener('change', handleSystemThemeChange)
  
  return () => {
    if (systemThemeListener) {
      systemThemeListener.removeEventListener('change', handleSystemThemeChange)
      systemThemeListener = null
    }
  }
}

export function useTheme() {
  let appWindow = getCurrentWindow();

  // get specific color - make it reactive
  const getColor = (colorKey: keyof ThemeColors) => {
    return computed(() => currentTheme.value === 'dark' ? darkTheme[colorKey] : lightTheme[colorKey])
  }

  const changeTheme = (newTheme: number) => { // 0: system, 1: light, 2: dark
    currentTheme.value = numberToTheme(newTheme);
    appWindow.setTheme(currentTheme.value);
    updateTheme();
  }

  onMounted(async () => {
    currentTheme.value = await get_theme_from_config();
    updateTheme();

    setupSystemThemeListener((newTheme: Theme) => {
      currentTheme.value = newTheme;
      updateTheme();
    })
  })
  
  return {
    // status
    currentTheme: computed(() => currentTheme.value),

    // methods
    getColor,
    changeTheme
  }
}
