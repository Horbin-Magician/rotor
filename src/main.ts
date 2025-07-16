import { createApp } from "vue";
import App from "./App.vue";

import { createRouter, createWebHistory } from 'vue-router'
import naive from 'naive-ui'

const Setting = () => import('./pages/Setting.vue')
const ScreenShotterMask = () => import('./pages/ScreenShotter/Mask.vue')
const ScreenShotterPin = () => import('./pages/ScreenShotter/Pin.vue')

const routes = [
  { path: '/', component: Setting },
  { path: '/ScreenShotter/Mask', component: ScreenShotterMask },
  { path: '/ScreenShotter/Pin', component: ScreenShotterPin },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

createApp(App)
  .use(router)
  .use(naive)
  .mount("#app");

