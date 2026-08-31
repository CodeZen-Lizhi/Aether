import type { RouteRecordRaw } from 'vue-router'
import { view } from './helpers'

export const publicRoutes: RouteRecordRaw[] = [
  {
    path: '/',
    name: 'Login',
    component: view(() => import('@/views/public/Login.vue')),
    meta: { requiresAuth: false }
  }
]
