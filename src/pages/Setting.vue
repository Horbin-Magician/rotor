<template>
  <WindowsTitlebar v-if="isWindows" />
  <div class="tab-wrapper settings-window">
    <n-tabs
      v-model:value="activeTab"
      placement = "left"
      size = "large"
      type = "line"
      class = "sidebar"
      pane-wrapper-class = "tab-pane"
    >
      <template #prefix>
        <div class= "logo">
          <img src="/assets/logo.svg" width="60px" @click="openGitHome" :draggable="false"/>
        </div>
      </template>
      <n-tab-pane class="tab-pane" name="Overview" :tab="t('message.overview')">
        <n-scrollbar
          style="max-height: 100vh"
          trigger="none"
        >
          <div class="settings-container">
            <OverviewSettings />
          </div>
        </n-scrollbar>
      </n-tab-pane>
      <n-tab-pane class="tab-pane" name="Base" :tab="t('message.base')">
        <n-scrollbar
          style="max-height: 100vh"
          trigger="none"
        >
          <div class="settings-container">
            <GeneralSettings 
              v-model:language="language"
              v-model:theme="theme"
              v-model:power-boot="powerBoot"
              v-model:shortcut-screenshot="shortcutScreenshot"
              v-model:shortcut-search="shortcutSearch"
              :highlighted-setting="highlightedSetting"
              :current-version="currentVersion"
              :is-checking-update="isCheckingUpdate"
              @check-update="checkUpdate"
            />
          </div>
        </n-scrollbar>
      </n-tab-pane>
      <n-tab-pane
        class="tab-pane"
        name="Screen shotter"
        :tab="t('message.screenShotter')"
        :tab-props="{ class: 'settings-sidebar-plugin-start' }"
      >
        <n-scrollbar
          style="max-height: 100vh"
          trigger="none"
        >
          <div class="settings-container">
            <PinwinSettings 
              v-model:shortcut-pinwin-close="shortcutPinwinClose"
              v-model:shortcut-pinwin-save="shortcutPinwinSave"
              v-model:shortcut-pinwin-copy="shortcutPinwinCopy"
              v-model:shortcut-pinwin-hide="shortcutPinwinHide"
              :highlighted-setting="highlightedSetting"
              v-model:save-path="savePath"
              v-model:if-auto-change-save-path="ifAutoChangeSavePath"
              v-model:if-ask-save-path="ifAskSavePath"
              v-model:zoom-delta="zoomDelta"
              @ask-save="askSave"
            />
          </div>
        </n-scrollbar>
      </n-tab-pane>
      <n-tab-pane class="tab-pane" name="Search" :tab="t('message.search')">
        <n-scrollbar
          style="max-height: 100vh"
          trigger="none"
        >
          <div class="settings-container">
            <SearchSettings
              v-model:excluded-dirs="searchExcludedDirs"
            />
          </div>
        </n-scrollbar>
      </n-tab-pane>
      <n-tab-pane class="tab-pane" name="Quick" :tab="t('message.quick')">
        <n-scrollbar
          style="max-height: 100vh"
          trigger="none"
        >
          <div class="settings-container">
            <QuickSettings
              v-model:quick-actions="quickActions"
              :highlighted-setting="highlightedSetting"
              @run-action="runAction"
            />
          </div>
        </n-scrollbar>
      </n-tab-pane>
    </n-tabs>
  </div>
  <div class="drag-region"  data-tauri-drag-region></div>

  <UpdateModals 
    v-model:show-update-modal="showUpdateModal"
    v-model:show-progress-modal="showProgressModal"
    :update-version="updateVersion"
    :update-progress="updateProgress"
    :update-status="updateStatus"
    @confirm-update="confirmUpdate"
    @cancel-update="cancelUpdate"
  />
</template>

<script setup lang="ts">
import { NTabs, NTabPane, NScrollbar, useMessage } from 'naive-ui'
import { defineAsyncComponent, onMounted, onUnmounted, ref, watch, h } from 'vue'
import type { Ref } from 'vue'
import { enable, isEnabled, disable } from '@tauri-apps/plugin-autostart';
import { open } from '@tauri-apps/plugin-dialog';
import { check, Update, CheckOptions } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  getAllConfig,
  getAppVersion,
  openUrl,
  setConfig,
  takeShortcutRegistrationNotices,
  type ShortcutRegistrationNotice,
} from '../shared/api/core'

import { useTheme } from '../composables/useTheme';

// Import setting components
import GeneralSettings from '../components/setting/GeneralSettings.vue'
import PinwinSettings from '../components/setting/PinwinSettings.vue'
import QuickSettings from '../components/setting/QuickSettings.vue'
import SearchSettings from '../components/setting/SearchSettings.vue'
import UpdateModals from '../components/setting/UpdateModals.vue'
import WindowsTitlebar from '../components/setting/WindowsTitlebar.vue'
import { getQuickActions, runQuickAction, setQuickActions } from '../features/quick/api'
import type { QuickAction } from '../features/quick/types'
import '../components/setting/settings.css'

const OverviewSettings = defineAsyncComponent(() => import('../components/setting/OverviewSettings.vue'))

import { useI18n } from 'vue-i18n'
const { t, locale } = useI18n()
const message = useMessage()
const { changeTheme } = useTheme()

import { getCurrentWindow } from '@tauri-apps/api/window'
import { info, error } from '@tauri-apps/plugin-log';

const appWindow = getCurrentWindow()
const isWindows = ref(false)
type SettingsTabName = 'Overview' | 'Base' | 'Screen shotter' | 'Search' | 'Quick'

const activeTab = ref<SettingsTabName>('Overview')
const highlightedSetting = ref('')
const hasLoadedConfig = ref(false)
let unlistenShortcutConflict: UnlistenFn | null = null
let highlightTimer: number | null = null
let isRevertingSetting = false

const shortcutTabByKey: Record<string, SettingsTabName> = {
  shortcut_screenshot: 'Base',
  shortcut_search: 'Base',
  shortcut_pinwin_close: 'Screen shotter',
  shortcut_pinwin_save: 'Screen shotter',
  shortcut_pinwin_copy: 'Screen shotter',
  shortcut_pinwin_hide: 'Screen shotter',
}

// Detect platform
if (navigator.platform.startsWith('Win')) {
  isWindows.value = true
}

appWindow.isVisible().then( (visible)=>{
  if(visible == false) {
    appWindow.show()
    appWindow.setFocus()
  }
})

// General settings
const language = ref(0)
const theme = ref(0)
const powerBoot = ref(false)
const currentVersion = ref("Loading...")
const shortcutScreenshot = ref('Shift+C')
const shortcutSearch = ref('Shift+F')

// Screenshot settings
const shortcutPinwinClose = ref('Esc')
const shortcutPinwinSave = ref('S')
const shortcutPinwinCopy = ref('Enter')
const shortcutPinwinHide = ref('H')
const savePath = ref('')
const ifAutoChangeSavePath = ref(true)
const ifAskSavePath = ref(true)
const zoomDelta = ref(2)

// Quick settings
const quickActions = ref<QuickAction[]>([])

// Update modal states
const showUpdateModal = ref(false)
const showProgressModal = ref(false)
const updateVersion = ref<String>("null")
const updateProgress = ref(0)
const updateStatus = ref('')
const isUpdating = ref(false)
const isCheckingUpdate = ref(false)

let update_cache: Update | null = null

// Search settings
const searchExcludedDirs = ref('')
Promise.all([getAllConfig(), getQuickActions()]).then(async ([config, actions]) => {
  language.value = Number(config["language"])
  theme.value = Number(config["theme"])
  powerBoot.value = await isEnabled()

  // currentVerision.value = config["current_verision"]
  shortcutScreenshot.value = config["shortcut_screenshot"]
  shortcutSearch.value = config["shortcut_search"]

  // Screenshot settings
  shortcutPinwinClose.value = config["shortcut_pinwin_close"]
  shortcutPinwinSave.value = config["shortcut_pinwin_save"]
  shortcutPinwinCopy.value = config["shortcut_pinwin_copy"]
  shortcutPinwinHide.value = config["shortcut_pinwin_hide"]
  savePath.value = config["save_path"]
  ifAutoChangeSavePath.value = config["if_auto_change_save_path"] === "false" ? false : true
  ifAskSavePath.value = config["if_ask_save_path"] === "false" ? false : true
  zoomDelta.value = Number(config["zoom_delta"])
  searchExcludedDirs.value = config["search_excluded_dirs"]
  quickActions.value = actions
}).finally(() => {
  hasLoadedConfig.value = true
})

// Get app version
getAppVersion().then((version) => {
  currentVersion.value = version
})

function openGitHome() {
  openUrl("https://github.com/Horbin-Magician/rotor")
    .catch((error) => {
      console.error("Failed to open URL:", error)
    })
}

function checkUpdate() {
  isCheckingUpdate.value = true

  let options: CheckOptions = {
    timeout: 3000, // 3 seconds timeout
  }

  check(options).then(async (update) => {
    if (update) {
      update_cache = update
      updateVersion.value = update.version
      showUpdateModal.value = true
    } else {
      message.info(t('message.noUpdatesAvailable') + ', ' + t('message.latestVersion'))
    }
  }).catch((err) => {
    error(`Failed to check for updates: ${err}`)
    const displayErr = String(err).length > 50
      ? String(err).slice(0, 50) + '...'
      : String(err);

    message.error(() => h('div', [
        `${t('message.updateCheckFailed')}: ${displayErr}. ${t('message.manualDownloadSuggestion')}: `,
        h('a', {
          href: '#',
          style: {
            color: '#007bff',
            textDecoration: 'none',
          },
          onClick: (e: Event) => {
            e.preventDefault()
            openGitHome()
          }
        }, 'https://github.com/Horbin-Magician/rotor')
      ]))
  }).finally(() => {
    isCheckingUpdate.value = false
  })
}

async function confirmUpdate() {
  showUpdateModal.value = false
  showProgressModal.value = true
  isUpdating.value = true
  updateProgress.value = 0
  updateStatus.value = t('message.downloadingUpdate')
  
  info("Downloading and installing update...")
  
  // Download and install the update
  if (update_cache) {
    await update_cache.downloadAndInstall((event) => {
      switch (event.event) {
        case 'Started':
          updateStatus.value = t('message.updateStarted')
          updateProgress.value = 10
          info('Update started')
          break
        case 'Progress':
          // Calculate progress based on downloaded bytes (this is a rough estimate)
          updateProgress.value = Math.min(90, updateProgress.value + 5)
          const chunkLength = event.data?.chunkLength || 0
          updateStatus.value = `${t('message.downloading')}: ${chunkLength} bytes`
          info(`Downloaded ${chunkLength} bytes`)
          break
        case 'Finished':
          updateProgress.value = 100
          updateStatus.value = t('message.updateCompleted')
          info('Update downloaded')
          
          // Delay before relaunch to show completion
          setTimeout(async () => {
            try {
              await relaunch()
            } catch (err) {
              error(`Failed to relaunch: ${err}`)
            }
          }, 1000)
          break
      }
    })
  } else {
    error('Update object is invalid or missing downloadAndInstall method')
    showProgressModal.value = false
    isUpdating.value = false
    message.error(`${t('message.updateError')}: Update object is invalid`)
  }
}

function cancelUpdate() {
  showUpdateModal.value = false
}

function settingDisplayName(key: string) {
  const displayNames: Record<string, string> = {
    shortcut_screenshot: t('message.screenshot'),
    shortcut_search: t('message.search'),
    shortcut_pinwin_close: t('message.closePinwin'),
    shortcut_pinwin_save: t('message.savePinwin'),
    shortcut_pinwin_copy: t('message.completePinwin'),
    shortcut_pinwin_hide: t('message.hidePinwin'),
    search_excluded_dirs: t('message.searchExcludedDirs'),
  }

  if (key.startsWith('quick_action_')) {
    const actionId = key.replace('quick_action_', '')
    const action = quickActions.value.find((action) => action.id === actionId)
    return action?.name || t('message.quick')
  }

  return displayNames[key] ?? key
}

function showShortcutConflict(notice: ShortcutRegistrationNotice) {
  const targetTab = notice.key.startsWith('quick_action_')
    ? 'Quick'
    : shortcutTabByKey[notice.key]

  if (targetTab) {
    activeTab.value = targetTab
  }

  highlightedSetting.value = notice.key

  if (highlightTimer !== null) {
    window.clearTimeout(highlightTimer)
  }

  highlightTimer = window.setTimeout(() => {
    highlightedSetting.value = ''
    highlightTimer = null
  }, 6000)

  const details = notice.message ? ` ${notice.message}` : ''
  message.error(
    `${t('message.shortcutConflictPrefix')}${settingDisplayName(notice.key)}: ${notice.shortcut}.${details}`,
    { duration: 6000 }
  )
}

// Function to update a setting in the backend
async function updateSetting(key: string, value: any) {
  // Convert the value to string as required by the backend
  const stringValue = typeof value === 'boolean' ? value.toString() : String(value)
  
  // Invoke the set_cfg command to update the setting
  await setConfig(key, stringValue)
}

// Helper function to create watchers for settings
function createSettingWatcher<T>(settingRef: Ref<T>, key: string, callback?: (value: T) => void) {
  watch(settingRef, async (newValue, oldValue) => {
    if (!hasLoadedConfig.value || isRevertingSetting) {
      return
    }

    try {
      await updateSetting(key, newValue)
      if (callback) {
        callback(newValue)
      }
    } catch (err) {
      console.error(`Failed to update setting ${key}:`, err)
      isRevertingSetting = true
      settingRef.value = oldValue
      isRevertingSetting = false

      if (key.startsWith('shortcut_')) {
        showShortcutConflict({
          key,
          shortcut: String(newValue),
          message: String(err),
        })
      } else {
        message.error(`${t('message.settingUpdateFailed')}: ${settingDisplayName(key)}`)
      }
    }
  })
}

onMounted(async () => {
  unlistenShortcutConflict = await listen<ShortcutRegistrationNotice>(
    'shortcut-registration-conflict',
    (event) => showShortcutConflict(event.payload)
  )

  takeShortcutRegistrationNotices()
    .then((notices) => {
      notices.forEach(showShortcutConflict)
    })
    .catch((err) => {
      console.error('Failed to take shortcut registration notices:', err)
    })
})

onUnmounted(() => {
  if (unlistenShortcutConflict) {
    unlistenShortcutConflict()
  }

  if (highlightTimer !== null) {
    window.clearTimeout(highlightTimer)
  }
})

// Watch for changes in settings and update them in the backend
createSettingWatcher(language, "language", (newValue) => {
  // Update app language
  if (newValue === 0) {
    // System default - detect system language
    const systemLang = navigator.language || navigator.languages[0]
    locale.value = systemLang.startsWith('zh') ? 'zh-CN' : 'en-US'
  } else if (newValue === 1) {
    locale.value = 'zh-CN' // Chinese
  } else if (newValue === 2) {
    locale.value = 'en-US' // English
  }
})

createSettingWatcher(theme, "theme", (newValue) => { // Update app theme
  changeTheme(newValue)
})

createSettingWatcher(powerBoot, "power_boot", (newValue) => {
  if (newValue) { 
    enable().catch(e => console.error('Failed to enable autostart:', e))
  } else { 
    disable().catch(e => console.error('Failed to disable autostart:', e))
  }
})

// Global shortcuts
createSettingWatcher(shortcutScreenshot, "shortcut_screenshot")
createSettingWatcher(shortcutSearch, "shortcut_search")

// Screenshot settings
createSettingWatcher(shortcutPinwinClose, "shortcut_pinwin_close")
createSettingWatcher(shortcutPinwinSave, "shortcut_pinwin_save")
createSettingWatcher(shortcutPinwinCopy, "shortcut_pinwin_copy")
createSettingWatcher(shortcutPinwinHide, "shortcut_pinwin_hide")
createSettingWatcher(savePath, "save_path")
createSettingWatcher(ifAutoChangeSavePath, "if_auto_change_save_path")
createSettingWatcher(ifAskSavePath, "if_ask_save_path")
createSettingWatcher(zoomDelta, "zoom_delta")

// Search settings
createSettingWatcher(searchExcludedDirs, "search_excluded_dirs")

watch(quickActions, async (newValue, oldValue) => {
  if (!hasLoadedConfig.value || isRevertingSetting) {
    return
  }

  try {
    await setQuickActions(newValue)
  } catch (err) {
    console.error('Failed to update quick actions:', err)
    isRevertingSetting = true
    quickActions.value = oldValue
    isRevertingSetting = false
    message.error(`${t('message.settingUpdateFailed')}: ${t('message.quickActions')}`)
  }
})

async function runAction(id: string) {
  try {
    await runQuickAction(id)
  } catch (err) {
    message.error(`${t('message.quickActionRunFailed')}: ${String(err)}`)
  }
}

async function askSave() {
  const path = await open({
    multiple: false,
    directory: true,
  });
  if (typeof path === 'string') {
    savePath.value = path
  }
}
</script>


<style scoped>
.logo{
  width: 100%;
  display: flex;
  justify-content: center;
  cursor: pointer;
  transition: filter 0.3s;
  margin-top: 20px;
}

.logo :hover {
  filter: drop-shadow(0 0 2px #007bff);
}

.tab-wrapper {
  height: 100vh;
  overflow: hidden;
}

.sidebar{
  height: 100%;
}

.sidebar :deep(.n-tabs-pane-wrapper) {
  min-width: 0;
  overflow: hidden;
}

.sidebar :deep(.n-tab-pane) {
  min-width: 0;
}

.sidebar :deep(.n-tabs-nav .n-tabs-tab) {
  box-sizing: border-box;
  height: 28px;
  width: 100px;
  display: flex;
  justify-content: center;
  align-items: center;
  padding: 4px 16px;
  font-size: 14px;
  text-align: center;
}

.sidebar :deep(.n-tabs-nav .n-tabs-tab-pad) {
  height: 4px;
}

.sidebar :deep(.n-tabs-nav .n-tabs-tab.settings-sidebar-plugin-start) {
  position: relative;
  margin-top: 8px;
}

.sidebar :deep(.n-tabs-nav .n-tabs-tab.settings-sidebar-plugin-start::before) {
  content: "";
  position: absolute;
  top: -6px;
  left: 16px;
  right: 16px;
  height: 1px;
  background: var(--theme-border);
  pointer-events: none;
}

.tab-pane {
  margin-top: 30px;
}

.settings-container {
  padding-right: 20px;
  min-width: 0;
  max-width: 100%;
  box-sizing: border-box;
  overflow-x: hidden;
}

.settings-container :deep(.n-input:not(.n-input--textarea)) {
  --n-height: 30px !important;
}

.settings-container :deep(.n-input.n-input--textarea) {
  --n-padding-vertical: 5px !important;
}

.drag-region {
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  height: 30px;
}
</style>
