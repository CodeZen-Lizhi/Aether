import type { RouteLocationNormalizedLoaded } from 'vue-router'
import {
  BarChart3,
  Cog,
  FolderTree,
  Home,
  Key,
  Layers,
  SlidersHorizontal,
} from 'lucide-vue-next'
import type { NavigationGroup } from '@/components/layout/SidebarNav.vue'
import type { MessageKey } from '@/i18n'

export interface BreadcrumbItem {
  label: string
  href?: string
}

type Translate = (key: MessageKey) => string

export function buildNavigation(options: {
  t?: Translate
}): NavigationGroup[] {
  const t = options.t ?? ((key: MessageKey) => key)

  return [
    {
      title: t('nav.group.overview'),
      items: [
        { name: t('nav.dashboard'), href: '/admin/dashboard', icon: Home },
        { name: t('nav.usageRecords'), href: '/admin/usage', icon: BarChart3 },
      ]
    },
    {
      title: t('nav.group.management'),
      items: [
        { name: t('nav.providers'), href: '/admin/providers', icon: FolderTree },
        { name: t('nav.modelManagement'), href: '/admin/models', icon: Layers },
        { name: t('nav.routing'), href: '/admin/routing', icon: SlidersHorizontal },
        { name: t('nav.standaloneKeys'), href: '/admin/keys', icon: Key },
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
