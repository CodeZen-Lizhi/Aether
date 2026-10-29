const REQUEST_TYPE_LABELS: Readonly<Record<string, string>> = {
  chat: '对话',
  compact: '会话压缩',
  image: '图像',
  video: '视频',
  realtime: '实时会话',
  live: 'Live 会话',
  unknown: '未知类型',
}

/** 规范化请求类型标识；空值统一归入 unknown，便于稳定展示历史记录。 */
export function normalizeUsageRequestType(value: string | null | undefined): string {
  const normalized = value?.trim().toLowerCase()
  return normalized || 'unknown'
}

/** 将请求类型标识转换为可读文案；未知新类型保留原始标识。 */
export function formatUsageRequestType(value: string | null | undefined): string {
  const normalized = normalizeUsageRequestType(value)
  return REQUEST_TYPE_LABELS[normalized] ?? normalized
}
