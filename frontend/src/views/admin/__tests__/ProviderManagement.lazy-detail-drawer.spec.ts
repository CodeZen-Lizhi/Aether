import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const source = readFileSync(
  resolve(process.cwd(), 'src/views/admin/ProviderManagement.vue'),
  'utf8',
)
const tableRowSource = readFileSync(
  resolve(process.cwd(), 'src/features/providers/components/ProviderTableRow.vue'),
  'utf8',
)
const mobileCardSource = readFileSync(
  resolve(process.cwd(), 'src/features/providers/components/ProviderMobileCard.vue'),
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

  it('opens details from the row without a redundant eye action', () => {
    const rowTemplate = source
      .split('<ProviderTableRow')[1]
      ?.split('/>')[0]

    expect(rowTemplate).toBeTruthy()
    expect(rowTemplate).toContain('@row-click="handleRowClick"')
    expect(rowTemplate).not.toContain('@view-detail=')
    expect(tableRowSource).not.toContain("legacyT('查看详情')")
    expect(tableRowSource).not.toContain('<Eye')
    expect(mobileCardSource).not.toContain("legacyT('查看详情')")
    expect(mobileCardSource).not.toContain('<Eye')
    expect(mobileCardSource).toContain("@click=\"$emit('viewDetail', provider.id)\"")
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

  it('lays out provider table columns proportionally so resizing rescales every column', () => {
    const layout = readFileSync(
      resolve(process.cwd(), 'src/views/admin/provider-list-layout.css'),
      'utf8',
    )

    // 六列都必须按百分比分配：固定像素会让某一列独吞富余宽度、其余列挤在一侧。
    const ratios: Record<string, number> = {}
    for (const column of ['info', 'balance', 'resources', 'health', 'status', 'actions']) {
      const matched = layout.match(new RegExp(`\\.provider-list__${column}-column \\{ width: ([\\d.]+)%; \\}`))
      expect(matched, `${column} 列必须声明百分比宽度`).not.toBeNull()
      ratios[column] = Number(matched?.[1])
    }
    // 比例合计必须为 100%，否则 table-layout:fixed 会留下一列独吞差额。
    const total = Object.values(ratios).reduce((sum, value) => sum + value, 0)
    expect(total).toBeCloseTo(100, 5)
    // 不允许任何一列退化为固定像素或 auto。
    expect(layout).not.toMatch(/\.provider-list__\w+-column \{ width: (auto|[\d.]+rem); \}/)
    // 比例分配后不再按容器宽度分档。
    expect(layout).not.toContain('@container provider-list (min-width: 70rem)')
  })
})
