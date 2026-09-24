import { describe, expect, it, vi } from 'vitest'

vi.mock('@/i18n', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/i18n')>()
  return {
    ...actual,
    useI18n: () => ({ legacyT: (value: string) => value }),
  }
})

import { useProviderFilters } from '../useProviderFilters'

describe('useProviderFilters', () => {
  it('requests all matching providers while retaining server-side filters', () => {
    const filters = useProviderFilters(() => [])

    expect(filters.queryParams.value).toMatchObject({
      page: 1,
      page_size: 10_000,
    })

    filters.searchQuery.value = 'openai'
    filters.filterStatus.value = 'active'

    expect(filters.queryParams.value).toMatchObject({
      page: 1,
      page_size: 10_000,
      search: 'openai',
      status: 'active',
    })
  })
})
