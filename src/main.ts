import { createApp } from "vue";
import App from "./App.vue";

import { createRouter, createWebHistory } from 'vue-router'
import naive from 'naive-ui'

const Setting = () => import('./pages/Setting.vue')
const SSMask = () => import('./pages/SSMask.vue')
const SSPin = () => import('./pages/SSPin.vue')

const routes = [
  { path: '/', component: Setting },
  { path: '/ssmask', component: SSMask },
  { path: '/sspin', component: SSPin },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

createApp(App)
  .use(router)
  .use(naive)
  .mount("#app");

