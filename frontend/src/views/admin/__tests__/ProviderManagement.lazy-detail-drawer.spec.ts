import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const source = readFileSync(
  resolve(process.cwd(), 'src/views/admin/ProviderManagement.vue'),
  'utf8',
)

describe('ProviderManagement detail drawer loading', () => {
  it('keeps the heavy detail drawer out of the initial route chunk', () => {
    expect(source).not.toContain(
      "import ProviderDetailDrawer from '@/features/providers/components/ProviderDetailDrawer.vue'",
    )
    expect(source).toContain(
      "() => import('@/features/providers/components/ProviderDetailDrawer.vue')",
    )
  })

  it('does not resolve the async drawer until it is opened', () => {
    const drawerTemplate = source
      .split('<ProviderDetailDrawer')[1]
      ?.split('/>')[0]

    expect(drawerTemplate).toBeTruthy()
    expect(drawerTemplate).toContain('v-if="providerDrawerMounted"')
    expect(source).toContain('providerDrawerMounted.value = true')
  })
})

describe('ProviderManagement provider list', () => {
  it('does not render pagination controls', () => {
    const template = source.split('<script setup lang="ts">')[0]

    expect(template).not.toContain('<Pagination')
  })

  it('keeps the provider list in table layout at narrow widths', () => {
    const layout = readFileSync(
      resolve(process.cwd(), 'src/views/admin/provider-list-layout.css'),
      'utf8',
    )

    expect(layout).toContain('.responsive-list.provider-list > .responsive-list-table { display: block; }')
    expect(layout).toContain('.responsive-list.provider-list > .responsive-list-cards { display: none; }')
    expect(layout).not.toContain('@container provider-list (max-width: 57.999rem)')
  })
})
