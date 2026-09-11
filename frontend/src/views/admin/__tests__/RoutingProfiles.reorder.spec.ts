import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, nextTick, type App } from 'vue'

import RoutingProfiles from '../RoutingProfiles.vue'

const apiMocks = vi.hoisted(() => ({
  listRoutingGroups: vi.fn(),
  listAdminProviders: vi.fn(),
}))

vi.mock('@/api/routing-profiles', () => ({
  listRoutingGroups: apiMocks.listRoutingGroups,
  createRoutingGroup: vi.fn(),
  updateRoutingGroup: vi.fn(),
  publishRoutingGroup: vi.fn(),
}))

vi.mock('@/api/endpoints/providers', () => ({
  listAdminProviders: apiMocks.listAdminProviders,
}))

vi.mock('@/components/layout', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    PageContainer: defineComponent({
      name: 'PageContainerStub',
      setup(_, { slots }) {
        return () => h('main', slots.default?.())
      },
    }),
  }
})

vi.mock('@/components/ui', async () => {
  const { defineComponent, h } = await import('vue')
  const passthrough = (name: string, tag = 'div') => defineComponent({
    name,
    inheritAttrs: false,
    setup(_, { attrs, slots }) {
      return () => h(tag, attrs, slots.default?.())
    },
  })
  return {
    Badge: passthrough('BadgeStub', 'span'),
    Button: passthrough('ButtonStub', 'button'),
    TableCard: defineComponent({
      name: 'TableCardStub',
      setup(_, { slots }) {
        return () => h('section', [slots.header?.(), slots.default?.()])
      },
    }),
  }
})

vi.mock('lucide-vue-next', () => {
  const Icon = defineComponent({
    name: 'IconStub',
    setup(_, { attrs }) {
      return () => h('span', attrs)
    },
  })
  return { ChevronDown: Icon, ChevronUp: Icon, GripVertical: Icon }
})

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

async function settle() {
  for (let index = 0; index < 6; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

function dispatchPointer(target: Element, type: string, clientY: number) {
  const event = new MouseEvent(type, { bubbles: true, clientX: 20, clientY })
  Object.defineProperty(event, 'pointerId', { value: 1 })
  target.dispatchEvent(event)
}

function setRowRect(row: HTMLElement, top: number) {
  vi.spyOn(row, 'getBoundingClientRect').mockReturnValue(DOMRect.fromRect({
    x: 10,
    y: top,
    width: 300,
    height: 48,
  }))
}

function providerNames(root: HTMLElement): string[] {
  return Array.from(root.querySelectorAll('ul > li')).map(row => (
    row.querySelector<HTMLElement>('.font-medium')?.textContent?.trim() ?? ''
  ))
}

beforeEach(() => {
  apiMocks.listRoutingGroups.mockReset()
  apiMocks.listAdminProviders.mockReset()
  apiMocks.listRoutingGroups.mockResolvedValue({ items: [], total: 0 })
  apiMocks.listAdminProviders.mockResolvedValue([
    { id: 'provider-a', name: 'A', is_active: true },
    { id: 'provider-b', name: 'B', is_active: true },
    { id: 'provider-c', name: 'C', is_active: true },
  ])
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
  document.body.innerHTML = ''
  vi.restoreAllMocks()
})

describe('RoutingProfiles provider reorder', () => {
  it('reorders providers when the drag handle moves down or up', async () => {
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(RoutingProfiles)
    app.mount(root)
    mountedApps.push({ app, root })
    await settle()

    const rows = Array.from(root.querySelectorAll<HTMLElement>('ul > li'))
    const handle = rows[0]?.querySelector<HTMLElement>('.cursor-grab')
    rows.forEach((row, index) => setRowRect(row, index * 56))
    expect(providerNames(root)).toEqual(['A', 'B', 'C'])
    expect(handle).toBeTruthy()
    expect(handle?.getAttribute('aria-label')).toBe('拖动 A 调整优先级')

    Object.defineProperty(document, 'elementFromPoint', {
      configurable: true,
      value: vi.fn(() => rows[1]),
    })
    dispatchPointer(handle!, 'pointerdown', 24)
    await nextTick()

    expect(rows[0]?.classList.contains('provider-row--placeholder')).toBe(true)
    expect(document.body.querySelector('.provider-drag-preview')?.textContent).toContain('A')

    dispatchPointer(handle!, 'pointermove', 90)
    dispatchPointer(handle!, 'pointerup', 90)
    await nextTick()

    expect(providerNames(root)).toEqual(['B', 'A', 'C'])
    expect(document.body.querySelector('.provider-drag-preview')).toBeNull()
    expect(root.querySelector('.provider-row--placeholder')).toBeNull()

    const reorderedRows = Array.from(root.querySelectorAll<HTMLElement>('ul > li'))
    const upwardHandle = reorderedRows[2]?.querySelector<HTMLElement>('.cursor-grab')
    expect(upwardHandle?.getAttribute('aria-label')).toBe('拖动 C 调整优先级')
    vi.mocked(document.elementFromPoint).mockReturnValue(reorderedRows[0])
    dispatchPointer(upwardHandle!, 'pointerdown', 136)
    dispatchPointer(upwardHandle!, 'pointermove', 10)
    dispatchPointer(upwardHandle!, 'pointerup', 10)
    await nextTick()

    expect(providerNames(root)).toEqual(['C', 'B', 'A'])
  })
})
