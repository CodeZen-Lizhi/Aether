import type { RouteRecordRaw } from 'vue-router'
import { adminRoutes } from './admin'
import { publicRoutes } from './public'

export const routes: RouteRecordRaw[] = [
  ...publicRoutes,
  ...adminRoutes,
]
