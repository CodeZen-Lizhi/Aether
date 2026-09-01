import { describe, expect, it } from 'vitest'
import type { RouteLocationNormalizedLoaded } from 'vue-router'

import { buildBreadcrumbs, buildNavigation } from '@/layouts/main-layout/navigation'
import type { MessageKey } from '@/i18n'

const translate = (key: MessageKey) => `tx:${key}`

function route(path: string, name?: string, meta: Record<string, unknown> = {}): RouteLocationNormalizedLoaded {
  return {
    path,
    fullPath: path,
    query: {},
    hash: '',
    name,
    params: {},
    matched: [],
    meta,
    redirectedFrom: undefined,
  } as RouteLocationNormalizedLoaded
}

describe('main layout navigation builder', () => {
  it('builds user navigation from translation keys', () => {
    const navigation = buildNavigation({
      canAccessAdmin: false,
      t: translate,
    })

    expect(navigation.map(group => group.title)).toEqual([
      'tx:nav.group.overview',
      'tx:nav.group.resources',
    ])
    expect(navigation.flatMap(group => group.items.map(item => item.name))).toEqual([
      'tx:nav.dashboard',
      'tx:nav.modelCatalog',
      'tx:nav.apiKeys',
      'tx:nav.usageStats',
    ])
  })

  it('builds admin navigation', () => {
    const navigation = buildNavigation({
      canAccessAdmin: true,
      t: translate,
    })

    expect(navigation.map(group => group.title)).toEqual([
      'tx:nav.group.overview',
      'tx:nav.group.management',
      'tx:nav.group.system',
    ])
    const managementItems = navigation.find(group => group.title === 'tx:nav.group.management')?.items ?? []
    expect(managementItems.map(item => item.name)).toEqual([
      'tx:nav.providers',
      'tx:nav.modelManagement',
      'tx:nav.routing',
      'tx:nav.standaloneKeys',
    ])
  })

  it('builds translated breadcrumbs for settings and routing detail pages', () => {
    const navigation = buildNavigation({
      canAccessAdmin: true,
      t: translate,
    })

    expect(buildBreadcrumbs({
      route: route('/dashboard/settings'),
      navigation,
      isNavActive: () => false,
      t: translate,
    })).toEqual([
      { label: 'tx:nav.group.account' },
      { label: 'tx:breadcrumb.personalSettings' },
    ])

    expect(buildBreadcrumbs({
      route: route('/admin/routing/new', 'RoutingProfileCreate'),
      navigation,
      isNavActive: href => href === '/admin/routing',
      t: translate,
    })).toEqual([
      { label: 'tx:nav.group.management' },
      { label: 'tx:nav.routing', href: '/admin/routing' },
      { label: 'tx:breadcrumb.routingCreate' },
    ])
  })
})
