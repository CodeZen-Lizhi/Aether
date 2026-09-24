import { ref, computed } from 'vue'
import type { ProviderSummaryQuery } from '@/api/endpoints'
import { API_FORMAT_ORDER, formatApiFormat } from '@/api/endpoints/types/api-format'
import { useI18n } from '@/i18n'

export interface FilterOption {
  value: string
  label: string
}

/** 供应商摘要接口单次返回上限，避免较小页尺寸隐藏列表项。 */
const PROVIDER_LIST_PAGE_SIZE = 10_000

/** 创建供应商列表筛选状态，并固定请求接口支持的最大页尺寸。 */
export function useProviderFilters(
  globalModels: () => { id: string; name: string }[],
) {
  const { legacyT } = useI18n()
  // 搜索与筛选
  const searchQuery = ref('')
  const filterStatus = ref('all')
  const filterApiFormat = ref('all')
  const filterModel = ref('all')

  const statusFilters = computed<FilterOption[]>(() => [
    { value: 'all', label: legacyT('全部状态') },
    { value: 'active', label: legacyT('活跃') },
    { value: 'inactive', label: legacyT('停用') },
  ])

  const apiFormatFilters = computed<FilterOption[]>(() => [
    { value: 'all', label: legacyT('全部格式') },
    ...API_FORMAT_ORDER.map(value => ({ value, label: formatApiFormat(value) })),
  ])

  const modelFilters = computed<FilterOption[]>(() => {
    const items = globalModels()
      .map(m => ({ value: m.id, label: m.name }))
      .sort((a, b) => a.label.localeCompare(b.label))
    return [{ value: 'all', label: legacyT('全部模型') }, ...items]
  })

  const hasActiveFilters = computed(() => {
    return (
      searchQuery.value !== '' ||
      filterStatus.value !== 'all' ||
      filterApiFormat.value !== 'all' ||
      filterModel.value !== 'all'
    )
  })

  // 服务端筛选参数；使用最大页尺寸一次加载列表。
  const queryParams = computed<ProviderSummaryQuery>(() => ({
    page: 1,
    page_size: PROVIDER_LIST_PAGE_SIZE,
    search: searchQuery.value.trim() || undefined,
    status: filterStatus.value !== 'all' ? filterStatus.value : undefined,
    api_format: filterApiFormat.value !== 'all' ? filterApiFormat.value : undefined,
    model_id: filterModel.value !== 'all' ? filterModel.value : undefined,
  }))

  function resetFilters() {
    searchQuery.value = ''
    filterStatus.value = 'all'
    filterApiFormat.value = 'all'
    filterModel.value = 'all'
  }

  return {
    searchQuery,
    filterStatus,
    filterApiFormat,
    filterModel,
    statusFilters,
    apiFormatFilters,
    modelFilters,
    hasActiveFilters,
    queryParams,
    resetFilters,
  }
}
