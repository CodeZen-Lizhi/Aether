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
  return { ArrowDown: Icon, ArrowUp: Icon, GripVertical: Icon }
})

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

async function settle() {
  for (let index = 0; index < 6; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

function dispatchPointer(target: EventTarget, type: string, clientY: number) {
  const event = new MouseEvent(type, { bubbles: true, clientX: 20, clientY })
  Object.defineProperty(event, 'pointerId', { value: 1 })
  target.dispatchEvent(event)
}

function setRowLayout(row: HTMLElement, top: number) {
  Object.defineProperty(row, 'offsetTop', { configurable: true, value: top })
  Object.defineProperty(row, 'offsetHeight', { configurable: true, value: 48 })
  vi.spyOn(row, 'getBoundingClientRect').mockReturnValue(DOMRect.fromRect({
    x: 10,
    y: 100 + top,
    width: 640,
    height: 48,
  }))
}

function setListLayout(list: HTMLElement) {
  vi.spyOn(list, 'getBoundingClientRect').mockReturnValue(DOMRect.fromRect({
    x: 10,
    y: 100,
    width: 640,
    height: 400,
  }))
}

function syncRowLayout(root: HTMLElement) {
  const list = root.querySelector<HTMLElement>('.provider-priority-list')
  expect(list).toBeTruthy()
  setListLayout(list!)
  Array.from(root.querySelectorAll<HTMLElement>('ul > li')).forEach((row, index) => {
    setRowLayout(row, index * 56)
  })
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
    { id: 'provider-d', name: 'D', is_active: true },
    { id: 'provider-e', name: 'E', is_active: true },
    { id: 'provider-f', name: 'F', is_active: true },
    { id: 'provider-g', name: 'G', is_active: true },
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
  it('allows one drag session to move across multiple providers in both directions', async () => {
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(RoutingProfiles)
    app.mount(root)
    mountedApps.push({ app, root })
    await settle()

    syncRowLayout(root)
    let rows = Array.from(root.querySelectorAll<HTMLElement>('ul > li'))
    const handle = rows[0]?.querySelector<HTMLElement>('.cursor-grab')
    expect(providerNames(root)).toEqual(['A', 'B', 'C', 'D', 'E', 'F', 'G'])
    expect(handle).toBeTruthy()
    expect(handle?.getAttribute('aria-label')).toBe('拖动 A 调整优先级')

    dispatchPointer(handle!, 'pointerdown', 24)
    await nextTick()

    expect(rows[0]?.classList.contains('provider-row--placeholder')).toBe(true)
    expect(document.body.querySelector('.provider-drag-preview')?.textContent).toContain('A')

    // 一次拖动连续跨过中间项，最终落到列表末尾。
    for (const clientY of [80, 150, 220, 290, 360, 470]) {
      dispatchPointer(window, 'pointermove', clientY)
      await nextTick()
      syncRowLayout(root)
    }
    dispatchPointer(window, 'pointerup', 470)
    await nextTick()

    expect(providerNames(root)).toEqual(['B', 'C', 'D', 'E', 'F', 'G', 'A'])
    expect(document.body.querySelector('.provider-drag-preview')).toBeNull()
    expect(root.querySelector('.provider-row--placeholder')).toBeNull()

    syncRowLayout(root)
    rows = Array.from(root.querySelectorAll<HTMLElement>('ul > li'))
    const upwardHandle = rows[6]?.querySelector<HTMLElement>('.cursor-grab')
    expect(upwardHandle?.getAttribute('aria-label')).toBe('拖动 A 调整优先级')
    dispatchPointer(upwardHandle!, 'pointerdown', 360)
    for (const clientY of [300, 240, 180, 120, 60, 40]) {
      dispatchPointer(window, 'pointermove', clientY)
      await nextTick()
      syncRowLayout(root)
    }
    dispatchPointer(window, 'pointerup', 40)
    await nextTick()

    expect(providerNames(root)).toEqual(['A', 'B', 'C', 'D', 'E', 'F', 'G'])
  })

  it('keeps accessible move buttons and disables them at list boundaries', async () => {
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(RoutingProfiles)
    app.mount(root)
    mountedApps.push({ app, root })
    await settle()

    const firstUp = root.querySelector<HTMLButtonElement>('[aria-label="上移 A"]')
    const firstDown = root.querySelector<HTMLButtonElement>('[aria-label="下移 A"]')
    expect(firstUp?.disabled).toBe(true)
    expect(firstDown?.disabled).toBe(false)

    firstDown?.click()
    await nextTick()
    expect(providerNames(root).slice(0, 3)).toEqual(['B', 'A', 'C'])

    const movedUp = root.querySelector<HTMLButtonElement>('[aria-label="上移 A"]')
    movedUp?.click()
    await nextTick()
    expect(providerNames(root).slice(0, 3)).toEqual(['A', 'B', 'C'])
  })

  it('moves rows live before drop and leaves no static insert marker', async () => {
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(RoutingProfiles)
    app.mount(root)
    mountedApps.push({ app, root })
    await settle()

    syncRowLayout(root)
    const rows = Array.from(root.querySelectorAll<HTMLElement>('ul > li'))
    const handle = rows[6]?.querySelector<HTMLElement>('.cursor-grab')
    expect(handle?.getAttribute('aria-label')).toBe('拖动 G 调整优先级')
    dispatchPointer(handle!, 'pointerdown', 360)
    for (const clientY of [300, 240, 180, 120, 60, 40]) {
      dispatchPointer(window, 'pointermove', clientY)
      await nextTick()
      syncRowLayout(root)
    }

    expect(providerNames(root)).toEqual(['G', 'A', 'B', 'C', 'D', 'E', 'F'])
    expect(root.querySelector('.provider-drop-indicator')).toBeNull()
    expect(root.querySelectorAll('.provider-row--placeholder')).toHaveLength(1)
    expect(document.body.querySelector('.provider-drag-preview')?.textContent).toContain('G')

    dispatchPointer(window, 'pointerup', 40)
    await nextTick()
    expect(providerNames(root)).toEqual(['G', 'A', 'B', 'C', 'D', 'E', 'F'])
    expect(root.querySelector('.provider-row--placeholder')).toBeNull()
    expect(document.body.querySelector('.provider-drag-preview')).toBeNull()
  })
})
