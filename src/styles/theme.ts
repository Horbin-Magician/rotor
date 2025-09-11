import { Theme } from '@tauri-apps/api/window'

export interface ThemeColors {
  // base colors
  primary: string
  primaryHover: string
  primaryPressed: string

  // background colors
  background: string
  backgroundSecondary: string
  backgroundTertiary: string

  // text colors
  textPrimary: string
  textSecondary: string
  textDisabled: string

  // border colors
  border: string
  borderHover: string

  // status colors
  success: string
  warning: string
  error: string
  info: string

  // special colors
  overlay: string
  primaryOverlay: string
  shadow: string
}

// light theme
export const lightTheme: ThemeColors = {
  // base colors
  primary: '#54a4db',
  primaryHover: '#4b9df4',
  primaryPressed: '#3d8ce6',
  
  // background colors
  background: '#ffffff',
  backgroundSecondary: '#f6f6f6',
  backgroundTertiary: '#f0f0f0',
  
  // text colors
  textPrimary: '#121212',
  textSecondary: '#666666',
  textDisabled: '#999999',
  
  // border colors
  border: '#e0e0e0',
  borderHover: '#d0d0d0',
  
  // status colors
  success: '#52c41a',
  warning: '#faad14',
  error: '#ff4d4f',
  info: '#1890ff',
  
  // special colors
  overlay: 'rgba(255, 255, 255, 0.8)',
  primaryOverlay: 'rgba(33, 150, 243, 0.1)',
  shadow: 'rgba(0, 0, 0, 0.2)',
}

// dark theme
export const darkTheme: ThemeColors = {
  // base colors
  primary: '#54a4db',
  primaryHover: '#24c8db',
  primaryPressed: '#1fb8cc',
  
  // background colors
  background: '#121212',
  backgroundSecondary: '#1f1f1f',
  backgroundTertiary: '#2a2a2a',
  
  // text colors
  textPrimary: '#f6f6f6',
  textSecondary: '#cccccc',
  textDisabled: '#666666',
  
  // border colors
  border: '#333333',
  borderHover: '#444444',
  
  // status colors
  success: '#52c41a',
  warning: '#faad14',
  error: '#ff4d4f',
  info: '#1890ff',
  
  // special colors
  overlay: 'rgba(0, 0, 0, 0.8)',
  primaryOverlay: 'rgba(33, 150, 243, 0.1)',
  shadow: 'rgba(255, 255, 255, 0.2)',
}

export function generateCSSVariables(raw_theme: Theme): Record<string, string> {
  let theme = raw_theme === 'dark' ? darkTheme : lightTheme
  return {
    '--theme-primary': theme.primary,
    '--theme-primary-hover': theme.primaryHover,
    '--theme-primary-pressed': theme.primaryPressed,
    
    '--theme-background': theme.background,
    '--theme-background-secondary': theme.backgroundSecondary,
    '--theme-background-tertiary': theme.backgroundTertiary,
    
    '--theme-text-primary': theme.textPrimary,
    '--theme-text-secondary': theme.textSecondary,
    '--theme-text-disabled': theme.textDisabled,
    
    '--theme-border': theme.border,
    '--theme-border-hover': theme.borderHover,
    
    '--theme-success': theme.success,
    '--theme-warning': theme.warning,
    '--theme-error': theme.error,
    '--theme-info': theme.info,
    
    '--theme-overlay': theme.overlay,
    '--theme-primary-overlay': theme.primaryOverlay,
    '--theme-shadow': theme.shadow,
  }
}
