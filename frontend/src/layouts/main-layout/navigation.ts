import type { RouteLocationNormalizedLoaded } from 'vue-router'
import {
  Activity,
  BarChart3,
  Box,
  Cog,
  Database,
  FolderTree,
  Home,
  Key,
  Layers,
  Server,
  SlidersHorizontal,
  Users,
} from 'lucide-vue-next'
import type { NavigationGroup } from '@/components/layout/SidebarNav.vue'
import type { MessageKey } from '@/i18n'

export interface BreadcrumbItem {
  label: string
  href?: string
}

type Translate = (key: MessageKey) => string

export function buildNavigation(options: {
  canAccessAdmin: boolean
  t?: Translate
}): NavigationGroup[] {
  const { canAccessAdmin } = options
  const t = options.t ?? ((key: MessageKey) => key)

  if (!canAccessAdmin) {
    return [
      {
        title: t('nav.group.overview'),
        items: [
          { name: t('nav.dashboard'), href: '/dashboard', icon: Home },
          { name: t('nav.healthMonitor'), href: '/dashboard/endpoint-status', icon: Activity },
        ]
      },
      {
        title: t('nav.group.resources'),
        items: [
          { name: t('nav.modelCatalog'), href: '/dashboard/models', icon: Box },
          { name: t('nav.apiKeys'), href: '/dashboard/api-keys', icon: Key },
          { name: t('nav.usageStats'), href: '/dashboard/usage', icon: BarChart3 },
        ]
      }
    ]
  }

  return [
    {
      title: t('nav.group.overview'),
      items: [
        { name: t('nav.dashboard'), href: '/admin/dashboard', icon: Home },
        { name: t('nav.healthMonitor'), href: '/admin/health-monitor', icon: Activity },
        { name: t('nav.usageRecords'), href: '/admin/usage', icon: BarChart3 },
      ]
    },
    {
      title: t('nav.group.management'),
      items: [
        { name: t('nav.userManagement'), href: '/admin/users', icon: Users },
        { name: t('nav.providers'), href: '/admin/providers', icon: FolderTree },
        { name: t('nav.modelManagement'), href: '/admin/models', icon: Layers },
        { name: t('nav.routing'), href: '/admin/routing', icon: SlidersHorizontal },
        { name: t('nav.pool'), href: '/admin/pool', icon: Database },
        { name: t('nav.standaloneKeys'), href: '/admin/keys', icon: Key },
        { name: t('nav.proxyNodes'), href: '/admin/proxy-nodes', icon: Server },
      ]
    },
    {
      title: t('nav.group.system'),
      items: [
        { name: t('nav.systemSettings'), href: '/admin/system', icon: Cog },
      ]
    }
  ]
}

export function buildBreadcrumbs(options: {
  route: RouteLocationNormalizedLoaded
  navigation: NavigationGroup[]
  isNavActive: (href: string) => boolean
  t?: Translate
}): BreadcrumbItem[] {
  const { route, navigation, isNavActive } = options
  const t = options.t ?? ((key: MessageKey) => key)

  if (route.path === '/dashboard/settings') {
    return [
      { label: t('nav.group.account') },
      { label: t('breadcrumb.personalSettings') }
    ]
  }

  if (route.path.startsWith('/admin/routing/') && route.path !== '/admin/routing') {
    return [
      { label: t('nav.group.management') },
      { label: t('nav.routing'), href: '/admin/routing' },
      {
        label: route.name === 'RoutingProfileCreate'
          ? t('breadcrumb.routingCreate')
          : t('breadcrumb.routingConfig')
      }
    ]
  }

  for (const group of navigation) {
    const activeItem = group.items.find(item => isNavActive(item.href))
    if (activeItem) {
      return [
        { label: group.title || '' },
        { label: activeItem.name }
      ]
    }
  }

  return [{ label: t('nav.dashboard') }]
}
