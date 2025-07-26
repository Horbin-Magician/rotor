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
          <img src="/assets/logo.svg" width="60px" @click="openGitHome" draggable="false"/>
        </div>
      </template>
      <n-tab-pane class="tab-pane" name="Base" tab="基础">
        <n-scrollbar style="max-height: 100vh" trigger="none">
          <div class="settings-container">
            <div class="settings-card">
              <div class="settings-card-title">通常</div>
              <div class="setting-item">
                <span class="setting-label">语言</span>
                <n-select v-model:value="language" :options="languageOptions" disabled="true" />
              </div>
              
              <div class="setting-item">
                <span class="setting-label">主题</span>
                <n-select v-model:value="theme" :options="themeOptions" />
              </div>

              <div class="setting-item">
                <span class="setting-label">开机自启</span>
                <n-switch v-model:value="powerBoot" />
              </div>

              <div class="setting-item">
                <span class="setting-label">当前版本：2.0.0</span>
                <n-button  disabled="true">检查更新</n-button>
              </div>
            </div>
            <div class="settings-card">
              <div class="settings-card-title">全局快捷键</div>
              <div class="setting-item">
                <span class="setting-label">截图</span>
                <ShortcutInput v-model:shortcut="shortcutScreenshot" />
              </div>
              <div class="setting-item">
                <span class="setting-label">搜索</span>
                <ShortcutInput v-model:shortcut="shortcutSearch" />
              </div>
            </div>
          </div>
        </n-scrollbar>
      </n-tab-pane>
      <n-tab-pane class="tab-pane" name="Screen shotter" tab="截图">
        <n-scrollbar style="max-height: 100vh" trigger="none">
          <div class="settings-container">
            <div class="settings-card">
              <div class="settings-card-title">快捷键</div>
              <div class="setting-item">
                <span class="setting-label">关闭贴图</span>
                <ShortcutInput v-model:shortcut="shortcutPinwinClose"/>
              </div>
              <div class="setting-item">
                <span class="setting-label">保存贴图</span>
                <ShortcutInput v-model:shortcut="shortcutPinwinSave"/>
              </div>
              <div class="setting-item">
                <span class="setting-label">完成贴图</span>
                <ShortcutInput v-model:shortcut="shortcutPinwinCopy"/>
              </div>
              <div class="setting-item">
                <span class="setting-label">隐藏贴图</span>
                <ShortcutInput v-model:shortcut="shortcutPinwinHide"/>
              </div>
            </div>
            <div class="settings-card">
              <div class="settings-card-title">贴图</div>
              <div class="setting-item">
                <span class="setting-label">默认保存路径</span>
                <div class="path-selector">
                  <n-input v-model:value="savePath" placeholder="" readonly />
                  <n-button  @click="askSave">浏览</n-button>
                </div>
              </div>
              <div class="setting-item">
                <span class="setting-label">自动更改保存路径</span>
                <n-switch v-model:value="ifAutoChangeSavePath" />
              </div>
              <div class="setting-item">
                <span class="setting-label">每次询问保存路径</span>
                <n-switch v-model:value="ifAskSavePath" />
              </div>
              <div class="setting-item">
                <span class="setting-label">缩放增量</span>
                <n-slider v-model:value="zoomDelta" :step="1" :max="10" :min="1" />
              </div>
            </div>
          </div>
        </n-scrollbar>
      </n-tab-pane>
      <n-tab-pane class="tab-pane" name="Search" tab="搜索">
      </n-tab-pane>
    </n-tabs>
  </div>
  <div class="drag-region"  data-tauri-drag-region></div>
</template>

<script setup lang="ts">
// import type { TabsProps, SelectOption } from 'naive-ui'
import { ref, watch } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import { invoke } from '@tauri-apps/api/core'
import { enable, isEnabled, disable } from '@tauri-apps/plugin-autostart';
import { open } from '@tauri-apps/plugin-dialog';

import ShortcutInput from '../components/ShortcutInput.vue';

import { getCurrentWindow } from '@tauri-apps/api/window'
const appWindow = getCurrentWindow()
appWindow.isVisible().then( (visible)=>{
  if(visible == false) {
    appWindow.show()
    appWindow.setFocus()
  }
})

const languageOptions = [
  { label: '系统默认', value: 0 },
  { label: '中文', value: 1 },
  { label: '英文', value: 2 }
]

const themeOptions = [
  { label: '跟随系统', value: 0 },
  { label: '浅色', value: 1 },
  { label: '深色', value: 2 }
]

// General settings
const language = ref(0)
const theme = ref(0)
const powerBoot = ref(false)
// const currentVerision = ref("0.0.0")
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

// Search settings
invoke("get_all_cfg").then(async (config: any) => {
  language.value = Number(config["language"])
  theme.value = Number(config["theme"])
  powerBoot.value = await isEnabled()
  console.log(isEnabled())
  // currentVerision.value = config["current_verision"]
  shortcutScreenshot.value = config["shortcut_screenshot"]
  shortcutSearch.value = config["shortcut_search"]

  // Screenshot settings
  shortcutPinwinClose.value = config["shortcut_pinwin_close"]
  shortcutPinwinSave.value = config["shortcut_pinwin_save"]
  shortcutPinwinCopy.value = config["shortcut_pinwin_copy"]
  shortcutPinwinHide.value = config["shortcut_pinwin_hide"]
  savePath.value = config["save_path"]
  ifAutoChangeSavePath.value = Boolean(config["if_auto_change_save_path"])
  ifAskSavePath.value = Boolean(config["if_ask_save_path"])
  zoomDelta.value = Number(config["zoom_delta"])
})

function openGitHome() {
  openUrl("https://github.com/Horbin-Magician/rotor")
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

// Watch for changes in settings and update them in the backend
watch(language, (newValue) => updateSetting("language", newValue))
watch(theme, (newValue) => {
  updateSetting("theme", newValue)
  // 更新应用主题
  if ((window as any).updateAppTheme) {
    (window as any).updateAppTheme(newValue)
  }
})
watch(powerBoot, (newValue) => { if(newValue) { enable() } else { disable() }})
watch(shortcutScreenshot, (newValue) => updateSetting("shortcut_screenshot", newValue))
watch(shortcutSearch, (newValue) => updateSetting("shortcut_search", newValue))

// Watch screenshot settings
watch(shortcutPinwinClose, (newValue) => updateSetting("shortcut_pinwin_close", newValue))
watch(shortcutPinwinSave, (newValue) => updateSetting("shortcut_pinwin_save", newValue))
watch(shortcutPinwinCopy, (newValue) => updateSetting("shortcut_pinwin_copy", newValue))
watch(shortcutPinwinHide, (newValue) => updateSetting("shortcut_pinwin_hide", newValue))
watch(savePath, (newValue) => updateSetting("save_path", newValue))
watch(ifAutoChangeSavePath, (newValue) => updateSetting("if_auto_change_save_path", newValue))
watch(ifAskSavePath, (newValue) => updateSetting("if_ask_save_path", newValue))
watch(zoomDelta, (newValue) => updateSetting("zoom_delta", newValue))

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
</style>
