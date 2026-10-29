import type { UsageRecord } from '../types'

/** 使用记录请求类型筛选项；值对应后端持久化的 UsageRecord.request_type。 */
export interface UsageRequestTypeOption {
  /** 归一化后的请求类型值。 */
  value: string
  /** 面向当前界面语言的展示文案。 */
  label: string
}

const REQUEST_TYPE_LABELS: Readonly<Record<string, string>> = {
  chat: '对话',
  compact: '会话压缩',
  image: '图像',
  video: '视频',
  realtime: '实时会话',
  live: 'Live 会话',
  unknown: '未知类型',
}

/** 已知请求类型的展示顺序；未出现的类型不会主动加入筛选项。 */
const REQUEST_TYPE_ORDER = [
  'chat',
  'compact',
  'image',
  'video',
  'realtime',
  'live',
  'unknown',
]

/** 规范化请求类型标识；空值统一归入 unknown，便于筛选缺失历史记录。 */
export function normalizeUsageRequestType(value: string | null | undefined): string {
  const normalized = value?.trim().toLowerCase()
  return normalized || 'unknown'
}

/** 将请求类型标识转换为可读文案；未知新类型保留原始标识。 */
export function formatUsageRequestType(value: string | null | undefined): string {
  const normalized = normalizeUsageRequestType(value)
  return REQUEST_TYPE_LABELS[normalized] ?? normalized
}

/** 合并服务端聚合结果、当前页记录和当前选择，生成稳定且不丢历史值的筛选项。 */
export function buildUsageRequestTypeOptions(
  availableRequestTypes: readonly string[],
  records: readonly Pick<UsageRecord, 'request_type'>[] = [],
  selected: string | null | undefined = null,
): UsageRequestTypeOption[] {
  const values = new Set<string>()
  availableRequestTypes.forEach(value => values.add(normalizeUsageRequestType(value)))
  records.forEach(record => values.add(normalizeUsageRequestType(record.request_type)))
  if (selected && selected !== '__all__') {
    values.add(normalizeUsageRequestType(selected))
  }

  return Array.from(values)
    .sort((left, right) => {
      const leftIndex = REQUEST_TYPE_ORDER.indexOf(left)
      const rightIndex = REQUEST_TYPE_ORDER.indexOf(right)
      if (leftIndex === -1 && rightIndex === -1) return left.localeCompare(right)
      if (leftIndex === -1) return 1
      if (rightIndex === -1) return -1
      return leftIndex - rightIndex
    })
    .map(value => ({
      value,
      label: REQUEST_TYPE_LABELS[value] ?? value,
    }))
}
