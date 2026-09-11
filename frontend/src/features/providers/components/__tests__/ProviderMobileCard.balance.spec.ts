import { createApp } from 'vue'
import { describe, expect, it } from 'vitest'

import ProviderMobileCard from '@/features/providers/components/ProviderMobileCard.vue'
import type { ProviderWithEndpointsSummary } from '@/api/endpoints'
import { createI18n } from '@/i18n'

function createProvider(overrides: Partial<ProviderWithEndpointsSummary> = {}): ProviderWithEndpointsSummary {
  return {
    id: 'provider-1',
    name: 'input',
    provider_type: 'openai',
    description: undefined,
    website: undefined,
    is_active: true,
    billing_type: 'pay_as_you_go',
    active_endpoints: 1,
    total_endpoints: 1,
    active_keys: 1,
    total_keys: 1,
    active_models: 1,
    total_models: 1,
    endpoint_health_details: [],
    ops_configured: true,
    ops_architecture_id: 'test',
    monthly_used_usd: 0,
    monthly_quota_usd: 0,
    ...overrides,
  } as ProviderWithEndpointsSummary
}

function mountCard(
  provider: ProviderWithEndpointsSummary,
  getProviderBalance: (providerId: string) => { available: number | null; currency: string } | null = () => ({ available: 12.34, currency: 'USD' }),
) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(ProviderMobileCard, {
    provider,
    editingDescriptionId: null,
    isBalanceLoading: () => false,
    getProviderBalance,
    getProviderBalanceBreakdown: () => null,
    getProviderBalanceError: () => null,
    getProviderCheckin: () => null,
    getProviderBalanceExtra: () => [],
    formatBalanceDisplay: (balance: { available: number | null; currency: string } | null) => (
      balance ? `$${balance.available?.toFixed(2) ?? '-'}` : '-'
    ),
    formatResetCountdown: () => '',
    getQuotaUsedColorClass: () => 'text-foreground',
  })
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

describe('ProviderMobileCard balance display', () => {
  it('keeps the operations balance visible in the compact card layout', () => {
    const { root, unmount } = mountCard(createProvider())

    expect(root.textContent).toContain('$12.34')

    unmount()
  })

  it('keeps a local monthly quota visible when operations are not configured', () => {
    const { root, unmount } = mountCard(
      createProvider({
        ops_configured: false,
        billing_type: 'monthly_quota',
        monthly_used_usd: 2.5,
        monthly_quota_usd: 10,
      }),
      () => null,
    )

    expect(root.textContent).toContain('$2.50')
    expect(root.textContent).toContain('$10.00')

    unmount()
  })
})
