<template>
  <WindowsTitlebar v-if="isWindows" />
  <div class="tab-wrapper">
    <n-tabs
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
      <n-tab-pane class="tab-pane" name="Base" :tab="t('message.base')">
        <n-scrollbar style="max-height: 100vh" trigger="none">
          <div class="settings-container">
            <GeneralSettings 
              v-model:language="language"
              v-model:theme="theme"
              v-model:power-boot="powerBoot"
              v-model:shortcut-screenshot="shortcutScreenshot"
              v-model:shortcut-search="shortcutSearch"
              :current-version="currentVersion"
              :is-checking-update="isCheckingUpdate"
              @check-update="checkUpdate"
            />
          </div>
        </n-scrollbar>
      </n-tab-pane>
      <n-tab-pane class="tab-pane" name="Screen shotter" :tab="t('message.screenShotter')">
        <n-scrollbar style="max-height: 100vh" trigger="none">
          <div class="settings-container">
            <PinwinSettings 
              v-model:shortcut-pinwin-close="shortcutPinwinClose"
              v-model:shortcut-pinwin-save="shortcutPinwinSave"
              v-model:shortcut-pinwin-copy="shortcutPinwinCopy"
              v-model:shortcut-pinwin-hide="shortcutPinwinHide"
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
import { ref, watch, h } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { enable, isEnabled, disable } from '@tauri-apps/plugin-autostart';
import { open } from '@tauri-apps/plugin-dialog';
import { check, Update, CheckOptions } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

import { useTheme } from '../composables/useTheme';

// Import setting components
import GeneralSettings from '../components/setting/GeneralSettings.vue'
import PinwinSettings from '../components/setting/PinwinSettings.vue'
import UpdateModals from '../components/setting/UpdateModals.vue'
import WindowsTitlebar from '../components/setting/WindowsTitlebar.vue'

import { useI18n } from 'vue-i18n'
const { t, locale } = useI18n()
const message = useMessage()
const { changeTheme } = useTheme()

import { getCurrentWindow } from '@tauri-apps/api/window'
import { info, error } from '@tauri-apps/plugin-log';

const appWindow = getCurrentWindow()
const isWindows = ref(false)

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
invoke("get_all_cfg").then(async (config: any) => {
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
})

// Get app version
invoke("get_app_version").then((version: any) => {
  currentVersion.value = version
})

function openGitHome() {
  invoke("open_url", { url: "https://github.com/Horbin-Magician/rotor" })
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

// Function to update a setting in the backend
function updateSetting(key: string, value: any) {
  // Convert the value to string as required by the backend
  const stringValue = typeof value === 'boolean' ? value.toString() : String(value)
  
  // Invoke the set_cfg command to update the setting
  invoke("set_cfg", { k: key, v: stringValue })
    .catch((error) => {
      console.error(`Failed to update setting ${key}:`, error)
    })
}

// Helper function to create watchers for settings
function createSettingWatcher(ref: any, key: string, callback?: (value: any) => void) {
  watch(ref, (newValue) => {
    updateSetting(key, newValue)
    if (callback) {
      callback(newValue)
    }
  })
}

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
}

.sidebar{
  height: 100%;
}

.sidebar :deep(.n-tabs-nav .n-tabs-tab) {
  height: 36px; /* 调整这里的值来改变标签页高度 */
  width: 100px;
  display: flex;
  justify-content: center;
  align-items: center;
  text-align: center;
}

.tab-pane {
  margin-top: 30px;
}

.settings-container {
  padding-right: 20px;
}

.drag-region {
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  height: 30px;
}
</style>
