import { describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h } from 'vue'

import ProviderKeyDetailPopover from '@/features/providers/components/ProviderKeyDetailPopover.vue'
import type { EndpointAPIKey } from '@/api/endpoints'
import { createI18n } from '@/i18n'

vi.mock('lucide-vue-next', async () => {
  const { defineComponent, h } = await import('vue')
  const Icon = defineComponent({
    setup() {
      return () => h('span')
    },
  })

  return { Info: Icon }
})

vi.mock('@/components/ui/button.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      setup(_, { attrs, slots }) {
        return () => h('button', attrs, slots.default?.())
      },
    }),
  }
})

vi.mock('@/components/ui', async () => {
  const { defineComponent, h } = await import('vue')
  const passthrough = (name: string) => defineComponent({
    name,
    setup(_, { slots }) {
      return () => h('div', slots.default?.())
    },
  })

  return {
    Popover: passthrough('PopoverStub'),
    PopoverTrigger: passthrough('PopoverTriggerStub'),
    PopoverContent: passthrough('PopoverContentStub'),
  }
})

function createProviderKey(overrides: Partial<EndpointAPIKey> = {}): EndpointAPIKey {
  return {
    id: 'provider-key-1',
    provider_id: 'provider-1',
    api_formats: [],
    api_key_masked: 'sk-***',
    auth_type: 'api_key',
    name: 'Primary key',
    cache_ttl_minutes: 5,
    max_probe_interval_minutes: 32,
    health_score: 100,
    consecutive_failures: 0,
    request_count: 0,
    success_count: 0,
    error_count: 0,
    success_rate: 1,
    avg_response_time_ms: 0,
    is_active: true,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

function mount(apiKey: EndpointAPIKey) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(defineComponent({
    setup() {
      return () => h(ProviderKeyDetailPopover, { apiKey })
    },
  }))
  app.use(createI18n())
  app.mount(root)

  return {
    root,
    unmount: () => {
      app.unmount()
      root.remove()
    },
  }
}

describe('ProviderKeyDetailPopover RPM display', () => {
  it('ignores a legacy learned value when no manual RPM cap is configured', () => {
    const legacyApiKey = Object.assign(createProviderKey({ rpm_limit: null }), {
      is_adaptive: true,
      learned_rpm_limit: 5,
      effective_limit: 5,
    })
    const { root, unmount } = mount(legacyApiKey)

    const rpmLabel = [...root.querySelectorAll('dt')]
      .find(label => label.textContent?.trim() === 'RPM 限制')

    expect(rpmLabel?.nextElementSibling?.textContent?.trim()).toBe('无限制')
    expect(root.textContent).not.toContain('自适应')

    unmount()
  })

  it('shows the manually configured RPM cap', () => {
    const { root, unmount } = mount(createProviderKey({ rpm_limit: 18 }))
    const rpmLabel = [...root.querySelectorAll('dt')]
      .find(label => label.textContent?.trim() === 'RPM 限制')

    expect(rpmLabel?.nextElementSibling?.textContent?.trim()).toBe('18')

    unmount()
  })
})
