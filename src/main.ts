import { createApp } from "vue";
import App from "./App.vue";

import { createRouter, createWebHistory } from 'vue-router'
import naive from 'naive-ui'

import Setting from './pages/Setting.vue'
import SSMask from './pages/SSMask.vue'

const routes = [
  { path: '/', component: Setting },
  { path: '/ssmask', component: SSMask },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

createApp(App)
  .use(router)
  .use(naive)
  .mount("#app");

// for hide initial white screen
import { getCurrentWindow } from '@tauri-apps/api/window'
const appWindow = getCurrentWindow()
appWindow.isVisible().then( (visible)=>{
  if(visible == false) {
    appWindow.show()
    appWindow.setFocus()
  }
})
