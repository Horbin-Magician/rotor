<template>
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
            <div class="settings-card">
              <div class="settings-card-title">{{ t('message.common') }}</div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.language') }}</span>
                <n-select v-model:value="language" :options="languageOptions" />
              </div>
              
              <div class="setting-item">
                <span class="setting-label">{{ t('message.theme') }}</span>
                <n-select v-model:value="theme" :options="themeOptions" />
              </div>

              <div class="setting-item">
                <span class="setting-label">{{ t('message.powerBoot') }}</span>
                <n-switch v-model:value="powerBoot" />
              </div>

              <div class="setting-item">
                <span class="setting-label">{{ t('message.currentVersion') }}{{ currentVersion }}</span>
                <n-button @click="checkUpdate" :loading="isCheckingUpdate" :disabled="isCheckingUpdate">
                  {{ t('message.checkUpdate') }}
                </n-button>
              </div>
            </div>
            <div class="settings-card">
              <div class="settings-card-title">{{ t('message.globalShortcuts') }}</div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.screenshot') }}</span>
                <ShortcutInput v-model:shortcut="shortcutScreenshot" />
              </div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.search') }}</span>
                <ShortcutInput v-model:shortcut="shortcutSearch" />
              </div>
            </div>
          </div>
        </n-scrollbar>
      </n-tab-pane>
      <n-tab-pane class="tab-pane" name="Screen shotter" :tab="t('message.screenShotter')">
        <n-scrollbar style="max-height: 100vh" trigger="none">
          <div class="settings-container">
            <div class="settings-card">
              <div class="settings-card-title">{{ t('message.shortcuts') }}</div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.closePinwin') }}</span>
                <ShortcutInput v-model:shortcut="shortcutPinwinClose"/>
              </div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.savePinwin') }}</span>
                <ShortcutInput v-model:shortcut="shortcutPinwinSave"/>
              </div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.completePinwin') }}</span>
                <ShortcutInput v-model:shortcut="shortcutPinwinCopy"/>
              </div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.hidePinwin') }}</span>
                <ShortcutInput v-model:shortcut="shortcutPinwinHide"/>
              </div>
            </div>
            <div class="settings-card">
              <div class="settings-card-title">{{ t('message.pinwin') }}</div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.defaultSavePath') }}</span>
                <div class="path-selector">
                  <n-input v-model:value="savePath" placeholder="" readonly />
                  <n-button  @click="askSave">{{ t('message.browse') }}</n-button>
                </div>
              </div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.autoChangeSavePath') }}</span>
                <n-switch v-model:value="ifAutoChangeSavePath" />
              </div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.askSavePath') }}</span>
                <n-switch v-model:value="ifAskSavePath" />
              </div>
              <div class="setting-item">
                <span class="setting-label">{{ t('message.zoomDelta') }}</span>
                <n-slider v-model:value="zoomDelta" :step="1" :max="10" :min="1" />
              </div>
            </div>
          </div>
        </n-scrollbar>
      </n-tab-pane>
      <n-tab-pane class="tab-pane" name="Search" :tab="t('message.search')">
      </n-tab-pane>
    </n-tabs>
  </div>
  <div class="drag-region"  data-tauri-drag-region></div>

  <!-- Update Confirmation Modal -->
  <n-modal v-model:show="showUpdateModal" preset="dialog" :title="t('message.updateAvailable')" style="width: 400px;">
    <div class="update-modal-content">
      <p>{{ t('message.newVersionAvailable') }} <strong>{{ updateVersion }}</strong> {{ t('message.isAvailable') }}</p>
      <p>{{ t('message.installNow') }}</p>
    </div>
    <template #action>
      <div class="modal-actions">
        <n-button @click="cancelUpdate" class="cancel-btn">
          {{ t('message.cancel') }}
        </n-button>
        <n-button type="primary" @click="confirmUpdate" class="confirm-btn">
          {{ t('message.install') }}
        </n-button>
      </div>
    </template>
  </n-modal>

  <!-- Update Progress Modal -->
  <n-modal v-model:show="showProgressModal" :closable="false" :mask-closable="false">
    <n-card style="width: 400px" :title="t('message.updatingApp')">
      <div class="progress-content">
        <n-progress 
          type="line" 
          :percentage="updateProgress" 
          indicator-placement="inside"
          :color="themeVars.primaryColor"
          status="success"
        />
        <p class="progress-status">{{ updateStatus }}</p>
      </div>
    </n-card>
  </n-modal>
</template>

<script setup lang="ts">
import { NTabs, NTabPane, NScrollbar, NSlider, NSwitch, NButton, NInput, NSelect, NModal, NCard, NProgress, useNotification, useThemeVars } from 'naive-ui'
import { ref, watch, h } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { enable, isEnabled, disable } from '@tauri-apps/plugin-autostart';
import { open } from '@tauri-apps/plugin-dialog';
import { check, Update, CheckOptions } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

import ShortcutInput from '../components/ShortcutInput.vue';

import { useI18n } from 'vue-i18n'
const { t, locale } = useI18n()
const notification = useNotification()
const themeVars = useThemeVars()

import { getCurrentWindow } from '@tauri-apps/api/window'
import { info, error } from '@tauri-apps/plugin-log';
const appWindow = getCurrentWindow()
appWindow.isVisible().then( (visible)=>{
  if(visible == false) {
    appWindow.show()
    appWindow.setFocus()
  }
})

const languageOptions = ref([
  { label: t('message.systemDefault'), value: 0 },
  { label: t('message.chinese'), value: 1 },
  { label: t('message.english'), value: 2 }
])

const themeOptions = ref([
  { label: t('message.followSystem'), value: 0 },
  { label: t('message.light'), value: 1 },
  { label: t('message.dark'), value: 2 }
])

// Update options when language changes
watch(locale, () => {
  languageOptions.value = [
    { label: t('message.systemDefault'), value: 0 },
    { label: t('message.chinese'), value: 1 },
    { label: t('message.english'), value: 2 }
  ]
  
  themeOptions.value = [
    { label: t('message.followSystem'), value: 0 },
    { label: t('message.light'), value: 1 },
    { label: t('message.dark'), value: 2 }
  ]
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
      console.log(update)
      info(`Update available: ${update.version}`)
      update_cache = update
      updateVersion.value = update.version
      showUpdateModal.value = true
    } else {
      info("You're already on the latest version")
      notification.info({
        title: t('message.noUpdatesAvailable'),
        content: t('message.latestVersion'),
        duration: 3000
      })
    }
  }).catch((err) => {
    error(`Failed to check for updates: ${err}`)
    notification.error({
      title: t('message.updateError'),
      content: () => h('div', [
        `${t('message.updateCheckFailed')}: ${err}. ${t('message.manualDownloadSuggestion')}: `,
        h('a', {
          href: '#',
          style: {
            color: '#007bff',
            textDecoration: 'underline',
            cursor: 'pointer'
          },
          onClick: (e: Event) => {
            e.preventDefault()
            openGitHome()
          }
        }, 'https://github.com/Horbin-Magician/rotor')
      ]),
      duration: 5000
    })
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
    notification.error({
      title: t('message.updateError'),
      content: 'Update object is invalid',
      duration: 5000
    })
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
    // Chinese
    locale.value = 'zh-CN'
  } else if (newValue === 2) {
    // English
    locale.value = 'en-US'
  }
})
createSettingWatcher(theme, "theme", (newValue) => {
  // Update app theme
  if ((window as any).updateAppTheme) {
    (window as any).updateAppTheme(newValue)
  }
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

.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 34px;
  margin-bottom: 10px;
  margin-left: 20px;
}

.setting-label {
  min-width: 130px;
}

.settings-card-title {
  font-weight: bold;
  font-size: 16px;
  height: 30px;
  border-bottom: 1px solid gray;
  margin-bottom: 10px;
}

.path-selector {
  display: flex;
  gap: 8px;
  flex: 1;
}

.path-selector .n-input {
  flex: 1;
}

.drag-region {
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  height: 30px;
}

/* Update Modal Styles */
.update-modal-content p {
  margin: 6px 0;
}

.modal-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.cancel-btn {
  min-width: 80px;
}

.confirm-btn {
  min-width: 80px;
}

.progress-status {
  margin-top: 10px;
  text-align: center;
  color: #666;
}
</style>
