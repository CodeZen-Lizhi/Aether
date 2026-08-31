import type { RouteRecordRaw } from 'vue-router'
import { view } from './helpers'

export const adminRoutes: RouteRecordRaw[] = [
  {
    path: '/admin',
    component: view(() => import('@/layouts/MainLayout.vue')),
    meta: { requiresAuth: true, requiresAdmin: true },
    children: [
      {
        path: 'dashboard',
        name: 'AdminDashboard',
        component: view(() => import('@/views/shared/Dashboard.vue'))
      },
      {
        path: 'users',
        name: 'Users',
        component: view(() => import('@/views/admin/Users.vue'))
      },
      {
        path: 'keys',
        name: 'ApiKeys',
        component: view(() => import('@/views/admin/ApiKeys.vue'))
      },
      {
        path: 'providers',
        name: 'ProviderManagement',
        component: view(() => import('@/views/admin/ProviderManagement.vue'))
      },
      {
        path: 'pool',
        name: 'PoolManagement',
        component: view(() => import('@/views/admin/PoolManagement.vue'))
      },
      {
        path: 'models',
        name: 'ModelManagement',
        component: view(() => import('@/views/admin/ModelManagement.vue'))
      },
      {
        path: 'routing',
        name: 'RoutingProfiles',
        component: view(() => import('@/views/admin/RoutingProfiles.vue'))
      },
      {
        path: 'routing/new',
        name: 'RoutingProfileCreate',
        component: view(() => import('@/views/admin/RoutingProfiles.vue'))
      },
      {
        path: 'routing/:groupId',
        name: 'RoutingProfileDetail',
        component: view(() => import('@/views/admin/RoutingProfiles.vue'))
      },
      {
        path: 'health-monitor',
        name: 'HealthMonitor',
        component: view(() => import('@/views/shared/HealthMonitor.vue'))
      },
      {
        path: 'usage',
        name: 'Usage',
        component: view(() => import('@/views/shared/Usage.vue'))
      },
      {
        path: 'system',
        name: 'SystemSettings',
        component: view(() => import('@/views/admin/SystemSettings.vue'))
      },
      {
        path: 'proxy-nodes',
        name: 'ProxyNodes',
        component: view(() => import('@/views/admin/ProxyNodes.vue'))
      }
    ]
  }
]
