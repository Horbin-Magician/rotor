import { createRouter, createWebHistory, RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
  { 
    path: '/', 
    component: () => import('../pages/Setting.vue') 
  },
  { 
    path: '/ScreenShotter/Mask', 
    component: () => import('../pages/ScreenShotter/Mask.vue') 
  },
  { 
    path: '/ScreenShotter/Pin', 
    component: () => import('../pages/ScreenShotter/Pin.vue') 
  },
  { 
    path: '/Searcher', 
    component: () => import('../pages/Searcher.vue') 
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

export default router
