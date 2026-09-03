import { describe, expect, it } from 'vitest'

import type { ProviderWithEndpointsSummary } from '@/api/endpoints/types'
import { sortProvidersByActiveAndPriority } from '../providerPrioritySort'

function makeProvider(overrides: Partial<ProviderWithEndpointsSummary> = {}): ProviderWithEndpointsSummary {
  return {
    id: 'provider-1',
    name: 'Provider One',
    enable_format_conversion: true,
    is_active: true,
    total_endpoints: 0,
    active_endpoints: 0,
    total_keys: 0,
    active_keys: 0,
    total_models: 0,
    active_models: 0,
    global_model_ids: [],
    avg_health_score: 0,
    unhealthy_endpoints: 0,
    api_formats: [],
    endpoint_health_details: [],
    created_at: '2026-05-02T00:00:00Z',
    updated_at: '2026-05-02T00:00:00Z',
    ...overrides,
  }
}

function makeProviderSet() {
  return [
    makeProvider({ id: 'p-old', name: 'Old', created_at: '2026-01-01T00:00:00Z' }),
    makeProvider({ id: 'p-new', name: 'New', created_at: '2026-06-01T00:00:00Z' }),
    makeProvider({ id: 'p-inactive', name: 'Inactive', is_active: false, created_at: '2026-01-01T00:00:00Z' }),
  ]
}

describe('sortProvidersByActiveAndPriority', () => {
  it('sorts by priority ascending within same active status', () => {
    const sorted = sortProvidersByActiveAndPriority(makeProviderSet(), {
      'p-new': 2,
      'p-old': 1,
    })

    expect(sorted.map(provider => provider.id)).toEqual(['p-old', 'p-new', 'p-inactive'])
  })

  it('appends unconfigured providers after configured ones with same status', () => {
    const sorted = sortProvidersByActiveAndPriority(makeProviderSet(), {
      'p-new': 1,
    })

    expect(sorted.map(provider => provider.id)).toEqual(['p-new', 'p-old', 'p-inactive'])
  })

  it('falls back to created_at ascending when priorities tie or are missing', () => {
    const sorted = sortProvidersByActiveAndPriority(makeProviderSet(), {
      'p-old': 3,
      'p-new': 3,
    })

    expect(sorted.map(provider => provider.id)).toEqual(['p-old', 'p-new', 'p-inactive'])
  })

  it('ranks active providers before inactive ones regardless of priority', () => {
    const sorted = sortProvidersByActiveAndPriority(makeProviderSet(), {
      'p-inactive': 1,
      'p-old': 2,
      'p-new': 3,
    })

    expect(sorted.map(provider => provider.id)).toEqual(['p-old', 'p-new', 'p-inactive'])
  })

  it('degrades to active-then-created ordering when priorities are empty', () => {
    const sorted = sortProvidersByActiveAndPriority(makeProviderSet())

    expect(sorted.map(provider => provider.id)).toEqual(['p-old', 'p-new', 'p-inactive'])
  })
})
