<template>
  <TableCard
    title="使用记录"
    class="responsive-list usage-records relative"
    :class="{ 'responsive-list--wide': visibleColumnCount > 9 }"
  >
    <template #actions>
      <!-- 时间范围筛选 -->
      <TimeRangePicker
        v-model="timeRangeModel"
        :show-granularity="false"
        class="responsive-list-desktop shrink-0"
      />

      <!-- 三维快捷筛选与时间范围共用同一条桌面操作带，每个筛选保持独立控件。 -->
      <div
        class="usage-quick-filters order-2 grid w-full min-w-0 grid-cols-3 gap-2 sm:order-none sm:flex sm:w-auto sm:flex-nowrap"
      >
        <Select
          :model-value="filterRequestStatus"
          @update:model-value="$emit('update:filterRequestStatus', $event)"
        >
          <SelectTrigger
            class="h-8 w-full min-w-0 px-2.5 text-xs sm:w-[6.5rem] sm:flex-none"
            aria-label="筛选请求状态"
          >
            <SelectValue placeholder="状态" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__">
              全部状态
            </SelectItem>
            <SelectItem value="active">
              活跃
            </SelectItem>
            <SelectItem value="failed">
              失败
            </SelectItem>
            <SelectItem value="cancelled">
              已取消
            </SelectItem>
          </SelectContent>
        </Select>

        <Select
          :model-value="filterTransport"
          @update:model-value="$emit('update:filterTransport', $event)"
        >
          <SelectTrigger
            class="h-8 w-full min-w-0 px-2.5 text-xs sm:w-[6.5rem] sm:flex-none"
            aria-label="筛选传输方式"
          >
            <SelectValue placeholder="传输" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__">
              全部传输
            </SelectItem>
            <SelectItem value="stream">
              HTTP 流式
            </SelectItem>
            <SelectItem value="standard">
              HTTP 标准
            </SelectItem>
            <SelectItem value="websocket">
              WebSocket
            </SelectItem>
          </SelectContent>
        </Select>

        <Select
          :model-value="filterAttempt"
          @update:model-value="$emit('update:filterAttempt', $event)"
        >
          <SelectTrigger
            class="h-8 w-full min-w-0 px-2.5 text-xs sm:w-[6.5rem] sm:flex-none"
            aria-label="筛选调度情况"
          >
            <SelectValue placeholder="调度" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__">
              全部调度
            </SelectItem>
            <SelectItem value="retry">
              发生重试
            </SelectItem>
            <SelectItem value="fallback">
              发生转移
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <Button
        variant="ghost"
        size="icon"
        class="absolute right-4 top-2.5 h-8 w-8 shrink-0 responsive-list-mobile"
        :class="autoRefresh ? 'text-primary' : ''"
        :title="autoRefresh ? '点击关闭自动刷新' : '点击开启自动刷新'"
        @click="$emit('update:autoRefresh', !autoRefresh)"
      >
        <RefreshCcw
          class="w-3.5 h-3.5"
          :class="autoRefresh ? 'animate-spin' : ''"
        />
      </Button>

      <div class="order-3 grid w-full grid-cols-[repeat(auto-fit,minmax(min(100%,10rem),1fr))] gap-2 responsive-list-mobile">
        <!-- 时间范围筛选 -->
        <TimeRangePicker
          v-model="timeRangeModel"
          :show-granularity="false"
          class="min-w-0"
          preset-trigger-class="!w-full"
        />

        <!-- 模型筛选 -->
        <Select
          :model-value="filterModel"
          @update:model-value="$emit('update:filterModel', $event)"
        >
          <SelectTrigger class="h-8 w-full min-w-0 text-xs border-border/60">
            <SelectValue placeholder="模型" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__">
              全部模型
            </SelectItem>
            <SelectItem
              v-for="model in availableModels"
              :key="model"
              :value="model"
            >
              {{ model.replace('claude-', '') }}
            </SelectItem>
          </SelectContent>
        </Select>

        <Select
          v-if="isColumnVisible('client_family') || filterClientFamily !== '__all__'"
          :model-value="filterClientFamily"
          @update:model-value="$emit('update:filterClientFamily', $event)"
        >
          <SelectTrigger class="h-8 w-full min-w-0 text-xs border-border/60">
            <SelectValue placeholder="客户端类型" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="option in clientFamilyFilterOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </SelectItem>
          </SelectContent>
        </Select>

        <!-- 提供商筛选（仅管理员可见） -->
        <Select
          v-if="isAdmin"
          :model-value="filterProvider"
          @update:model-value="$emit('update:filterProvider', $event)"
        >
          <SelectTrigger class="h-8 w-full min-w-0 text-xs border-border/60">
            <SelectValue placeholder="提供商" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__">
              全部提供商
            </SelectItem>
            <SelectItem
              v-for="provider in availableProviders"
              :key="provider"
              :value="provider"
            >
              {{ provider }}
            </SelectItem>
          </SelectContent>
        </Select>

        <!-- API格式筛选 -->
        <Select
          :model-value="filterApiFormat"
          @update:model-value="$emit('update:filterApiFormat', $event)"
        >
          <SelectTrigger class="h-8 w-full min-w-0 text-xs border-border/60">
            <SelectValue placeholder="格式" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__">
              全部格式
            </SelectItem>
            <SelectItem
              v-for="format in availableApiFormats"
              :key="format.value"
              :value="format.value"
            >
              {{ format.label }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <!-- 列显示配置（桌面端） -->
      <MultiSelect
        v-model="visibleColumnIds"
        class="max-w-full"
        :options="columnSelectOptions"
        placeholder="显示列"
        trigger-class="w-40 h-8 text-xs border-border/60"
        dropdown-min-width="14rem"
        :searchable="false"
      />

      <!-- 自动刷新按钮 -->
      <Button
        variant="ghost"
        size="icon"
        class="responsive-list-desktop h-8 w-8 shrink-0"
        :class="autoRefresh ? 'text-primary' : ''"
        :title="autoRefresh ? '点击关闭自动刷新' : '点击开启自动刷新'"
        @click="$emit('update:autoRefresh', !autoRefresh)"
      >
        <RefreshCcw
          class="w-3.5 h-3.5"
          :class="autoRefresh ? 'animate-spin' : ''"
        />
      </Button>
    </template>

    <!-- 紧凑卡片视图 -->
    <div class="responsive-list-cards">
      <div
        v-if="records.length === 0"
        class="text-center py-12 text-muted-foreground"
      >
        暂无请求记录
      </div>
      <div
        v-for="record in records"
        v-else
        :key="record.id"
        class="border-b border-border/40 px-3 py-2.5"
        :class="isAdmin ? 'cursor-pointer active:bg-muted/30 transition-colors' : ''"
        @click="isAdmin && emit('showDetail', record.id)"
      >
        <!-- 第一行：模型 + 费用 -->
        <div class="flex items-start justify-between gap-2">
          <div class="min-w-0 flex-1">
            <div class="flex min-w-0 flex-wrap items-center gap-1.5">
              <UsageModelDisplay
                :record="record"
                model-class="text-[15px] font-semibold leading-5"
                stack-full-width
              />
              <Badge
                variant="outline"
                data-usage-request-type="card"
                :class="`${getRequestTypeBadgeClass(record.request_type)} whitespace-normal text-[10px] px-1.5 h-4 leading-4 inline-flex items-center flex-shrink-0`"
              >
                {{ formatUsageRequestType(record.request_type) }}
              </Badge>
              <!-- 生命周期与传输方式作为独立次标签。 -->
              <Badge
                v-if="isUsageRecordFailed(record)"
                variant="outline"
                data-usage-lifecycle="card"
                class="whitespace-normal border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-800/70 dark:bg-rose-950/30 dark:text-rose-300 text-[10px] px-1.5 h-4 leading-4 inline-flex items-center flex-shrink-0"
              >
                失败
              </Badge>
              <Badge
                v-else-if="getDisplayStatus(record) === 'pending'"
                variant="outline"
                data-usage-lifecycle="card"
                class="whitespace-normal animate-pulse border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-800/70 dark:bg-amber-950/30 dark:text-amber-300 text-[10px] px-1.5 h-4 leading-4 inline-flex items-center flex-shrink-0"
              >
                等待
              </Badge>
              <Badge
                v-else-if="getDisplayStatus(record) === 'streaming'"
                variant="outline"
                data-usage-lifecycle="card"
                class="whitespace-normal animate-pulse border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-800/70 dark:bg-sky-950/30 dark:text-sky-300 text-[10px] px-1.5 h-4 leading-4 inline-flex items-center flex-shrink-0"
              >
                传输
              </Badge>
              <Badge
                v-else-if="record.status === 'cancelled'"
                variant="outline"
                data-usage-lifecycle="card"
                class="whitespace-normal border-orange-200 bg-orange-50 text-orange-700 dark:border-orange-800/70 dark:bg-orange-950/30 dark:text-orange-300 text-[10px] px-1.5 h-4 leading-4 inline-flex items-center flex-shrink-0"
              >
                取消
              </Badge>
              <Badge
                v-if="isUsageWebSocket(record)"
                variant="outline"
                data-usage-transport="websocket"
                :title="getWebSocketTransportTitle(record)"
                :class="`${getTransportBadgeClass(record)} whitespace-normal text-[10px] px-1.5 h-4 leading-4 inline-flex items-center flex-shrink-0`"
              >
                WS
              </Badge>
              <Badge
                v-else-if="getStreamModeSegments(record).hasConversion"
                variant="outline"
                data-usage-transport="http"
                :class="`${getTransportBadgeClass(record)} whitespace-normal text-[10px] px-1.5 h-4 leading-4 inline-flex items-center gap-0.5 flex-shrink-0`"
              >
                <span>{{ getStreamModeSegments(record).client }}</span>
                <span class="opacity-60">→</span>
                <span>{{ getStreamModeSegments(record).upstream }}</span>
              </Badge>
              <Badge
                v-else
                variant="outline"
                data-usage-transport="http"
                :class="`${getTransportBadgeClass(record)} whitespace-normal text-[10px] px-1.5 h-4 leading-4 inline-flex items-center flex-shrink-0`"
              >
                {{ getStreamModeLabel(record) }}
              </Badge>
            </div>
          </div>
          <div class="flex flex-col items-end flex-shrink-0">
            <span
              v-if="record.usage_available !== false && record.usage_pricing_available !== false"
              data-usage-cost-value="card"
              class="text-sm leading-5"
              :class="[getRecordCostClass(record.cost)]"
            >{{ formatRecordCost(record.cost) }}</span>
            <span
              v-else-if="record.usage_available === false"
              data-usage-unavailable="cost"
              class="text-sm text-muted-foreground font-medium leading-5"
              title="上游未提供可验证的 token/费用用量"
            >不可用</span>
            <span
              v-else
              data-usage-unpriced="cost"
              class="text-sm text-muted-foreground font-medium leading-5"
              title="token 用量可验证，但当前计价规则不支持该音频用量分项"
            >未计价</span>
            <span
              v-if="record.usage_available !== false && record.usage_pricing_available !== false && showActualCost && record.actual_cost !== undefined && record.rate_multiplier && record.rate_multiplier !== 1.0"
              class="text-[10px] text-muted-foreground"
            >{{ formatRecordCost(record.actual_cost) }}</span>
          </div>
        </div>

        <!-- 第二行：时间 + API格式 -->
        <div class="mt-1.5 flex min-w-0 flex-wrap items-center gap-1.5 text-xs leading-5 text-muted-foreground">
          <span class="shrink-0 tabular-nums text-foreground whitespace-normal">
            {{ formatRecordTime(record.created_at) }}
          </span>
          <span class="shrink-0 tabular-nums whitespace-normal">
            {{ formatRecordShortDate(record.created_at) }}
          </span>
          <template v-if="record.api_format">
            <span class="text-muted-foreground/40">·</span>
            <span class="min-w-0 [overflow-wrap:anywhere]">{{ formatApiFormat(record.api_format) }}</span>
          </template>
        </div>

        <!-- 第三行：提供商 -->
        <div
          v-if="isAdmin"
          class="mt-1 flex min-w-0 flex-wrap items-center gap-1.5 text-xs leading-5 text-muted-foreground"
        >
          <span class="min-w-0 [overflow-wrap:anywhere]">{{ formatRecordProviderSegment(record) }}</span>
        </div>

        <!-- 第四行：性能指标 -->
        <div class="mt-1.5 flex min-w-0 flex-wrap items-center gap-x-1.5 gap-y-1 text-xs leading-5 text-muted-foreground">
          <span
            class="min-w-0 [overflow-wrap:anywhere] whitespace-normal tabular-nums text-foreground"
            :title="getRecordPerformanceTitle(record)"
          >
            <span class="text-muted-foreground">耗时&amp;速度</span>
            <template v-if="getDisplayStatus(record) === 'pending' || getDisplayStatus(record) === 'streaming'">
              <span
                data-usage-performance-part="first-byte"
                class="ml-1 text-amber-700 dark:text-amber-300"
              >{{ formatRecordDurationSeconds(record.first_byte_time_ms) }}</span>
              <span class="text-muted-foreground"> / </span>
              <ElapsedTimeText
                data-usage-performance-part="total"
                class="text-blue-700 dark:text-blue-300"
                :created-at="record.created_at"
                :response-time-updated-at="record.response_time_updated_at ?? null"
                :status="getDisplayStatus(record)"
                :response-time-ms="record.response_time_ms ?? null"
              />
              <span class="text-muted-foreground"> / </span>
              <span
                data-usage-performance-part="speed"
                :class="getOutputRateClass(record)"
              >{{ formatOutputRate(getRecordDisplayOutputRate(record)) }}</span>
            </template>
            <template
              v-else-if="hasRecordDisplayLatency(record)"
            >
              <span
                data-usage-performance-part="first-byte"
                class="ml-1 text-amber-700 dark:text-amber-300"
              >{{ getRecordFirstByteLatency(record) }}</span>
              <span class="text-muted-foreground"> / </span>
              <span
                data-usage-performance-part="total"
                class="text-blue-700 dark:text-blue-300"
              >{{ getRecordTotalLatency(record) }}</span>
              <span class="text-muted-foreground"> / </span>
              <span
                data-usage-performance-part="speed"
                :class="getOutputRateClass(record)"
              >{{ formatOutputRate(getRecordDisplayOutputRate(record)) }}</span>
            </template>
            <span
              v-else
              class="ml-1 text-muted-foreground"
            >- / {{ formatOutputRate(getRecordDisplayOutputRate(record)) }}</span>
          </span>
          <span class="text-muted-foreground/40">·</span>
          <span
            class="min-w-0 [overflow-wrap:anywhere] whitespace-normal tabular-nums text-foreground"
            :title="hasRecordCacheTokens(record) ? getRecordCacheTokensTitle(record) : undefined"
          >
            <span class="text-muted-foreground">Tokens</span>
            <span
              v-if="record.usage_available !== false"
              class="ml-1"
              title="输入 Token / 输出 Token"
            >
              <span
                data-usage-token-part="input"
                class="text-sky-700 dark:text-sky-300"
              >{{ formatTokens(getRecordEffectiveInputTokens(record)) }}</span>
              <span class="text-muted-foreground"> / </span>
              <span
                data-usage-token-part="output"
                class="text-emerald-700 dark:text-emerald-300"
              >{{ formatTokens(record.output_tokens || 0) }}</span>
            </span>
            <span
              v-else
              data-usage-unavailable="tokens"
              class="ml-1 text-muted-foreground"
              title="上游未提供可验证的 token/费用用量"
            >不可用</span>
            <template v-if="record.usage_available !== false && hasRecordCacheTokens(record)">
              <span class="text-muted-foreground"> | </span>
              <span
                data-usage-token-part="cache-read"
                class="text-cyan-700 dark:text-cyan-300"
              >{{ formatOptionalTokens(getRecordCacheReadTokens(record)) }}</span>
              <span class="text-muted-foreground"> / </span>
              <span
                data-usage-cache-hit-rate="mobile"
                class="text-emerald-700 dark:text-emerald-300 tabular-nums font-medium"
              >
                {{ formatRecordCacheHitRate(record) }}
              </span>
            </template>
            <template v-if="record.usage_available !== false && ((record.input_audio_tokens || 0) > 0 || (record.output_audio_tokens || 0) > 0)">
              <span class="text-muted-foreground"> | 音频 </span>
              <span>{{ formatOptionalTokens(record.input_audio_tokens) }} / {{ formatOptionalTokens(record.output_audio_tokens) }}</span>
            </template>
          </span>
        </div>
        <dl
          v-if="isColumnVisible('client_family') || isColumnVisible('client_ip') || isColumnVisible('user_agent')"
          class="mt-2 grid min-w-0 grid-cols-[auto_minmax(0,1fr)] gap-x-2 gap-y-1 text-xs [overflow-wrap:anywhere]"
        >
          <template v-if="isColumnVisible('client_family')">
            <dt class="text-muted-foreground">
              客户端类型
            </dt>
            <dd>{{ formatClientFamily(record.client_family) }}</dd>
          </template>
          <template v-if="isColumnVisible('client_ip')">
            <dt class="text-muted-foreground">
              IP 地址
            </dt>
            <dd>{{ record.client_ip || '-' }}</dd>
          </template>
          <template v-if="isColumnVisible('user_agent')">
            <dt class="text-muted-foreground">
              User-Agent
            </dt>
            <dd>{{ record.user_agent || '-' }}</dd>
          </template>
        </dl>
      </div>
    </div>

    <!-- 宽屏表格视图 -->
    <Table
      ref="recordsTable"
      class="responsive-list-table usage-records-table [&_th]:px-1 [&_td]:px-1"
    >
      <colgroup>
        <col
          v-for="column in visibleColumns"
          :key="column.id"
          :style="{ width: getColumnWidth(column) }"
        >
      </colgroup>
      <TableHeader>
        <TableRow class="border-b border-border/60 hover:bg-transparent">
          <TableHead
            v-if="isColumnVisible('time')"
            class="h-12 font-semibold"
          >
            时间
          </TableHead>
          <TableHead
            v-if="!isAdmin && isColumnVisible('key')"
            class="h-12 font-semibold"
          >
            密钥
          </TableHead>
          <SortableTableHead
            v-if="isColumnVisible('model')"
            class="h-12 font-semibold"
            column-key="model"
            :sortable="false"
            :filter-active="filterModel !== '__all__'"
            filter-title="筛选模型"
            filter-content-class="w-64 p-1 rounded-2xl border-border bg-card text-foreground shadow-2xl backdrop-blur-xl"
          >
            模型
            <template #filter="{ close }">
              <TableFilterMenu
                :model-value="filterModel"
                :options="modelFilterOptions"
                @update:model-value="$emit('update:filterModel', $event)"
                @select="close"
              />
            </template>
          </SortableTableHead>
          <SortableTableHead
            v-if="isAdmin && isColumnVisible('provider')"
            class="h-12 font-semibold"
            column-key="provider"
            :sortable="false"
            :filter-active="filterProvider !== '__all__'"
            filter-title="筛选提供商"
            filter-content-class="w-48 p-1 rounded-2xl border-border bg-card text-foreground shadow-2xl backdrop-blur-xl"
          >
            提供商
            <template #filter="{ close }">
              <TableFilterMenu
                :model-value="filterProvider"
                :options="providerFilterOptions"
                @update:model-value="$emit('update:filterProvider', $event)"
                @select="close"
              />
            </template>
          </SortableTableHead>
          <SortableTableHead
            v-if="isColumnVisible('api_format')"
            class="h-12 font-semibold"
            column-key="api_format"
            :sortable="false"
            :filter-active="filterApiFormat !== '__all__'"
            filter-title="筛选 API 格式"
            filter-content-class="w-72 p-1 rounded-2xl border-border bg-card text-foreground shadow-2xl backdrop-blur-xl"
          >
            API格式
            <template #filter="{ close }">
              <TableFilterMenu
                :model-value="filterApiFormat"
                :options="apiFormatFilterOptions"
                @update:model-value="$emit('update:filterApiFormat', $event)"
                @select="close"
              />
            </template>
          </SortableTableHead>
          <TableHead
            v-if="isColumnVisible('status')"
            class="h-12 text-left font-semibold"
          >
            <span class="whitespace-nowrap">类型 / 状态</span>
          </TableHead>
          <TableHead
            v-if="isColumnVisible('tokens')"
            class="h-12 font-semibold text-left"
          >
            Tokens
          </TableHead>
          <TableHead
            v-if="isColumnVisible('cost')"
            class="h-12 font-semibold text-left"
          >
            费用
          </TableHead>
          <TableHead
            v-if="isColumnVisible('performance')"
            class="h-12 font-semibold text-left"
            :title="firstByteTimingDescription"
          >
            <div class="flex flex-col items-start text-[11px] leading-3">
              <span class="whitespace-normal">端到端</span>
              <span class="whitespace-normal">
                首字
                <span class="text-muted-foreground font-normal">/</span>
                总耗时
              </span>
              <span class="text-muted-foreground font-normal">输出速度</span>
            </div>
          </TableHead>
          <SortableTableHead
            v-if="isColumnVisible('client_family')"
            class="h-12 font-semibold"
            column-key="client_family"
            :sortable="false"
            :filter-active="filterClientFamily !== '__all__'"
            filter-title="筛选客户端"
            filter-content-class="w-44 p-1 rounded-2xl border-border bg-card text-foreground shadow-2xl backdrop-blur-xl"
          >
            客户端
            <template #filter="{ close }">
              <TableFilterMenu
                :model-value="filterClientFamily"
                :options="clientFamilyFilterOptions"
                @update:model-value="$emit('update:filterClientFamily', $event)"
                @select="close"
              />
            </template>
          </SortableTableHead>
          <TableHead
            v-if="isColumnVisible('client_ip')"
            class="h-12 font-semibold"
          >
            IP 地址
          </TableHead>
          <TableHead
            v-if="isColumnVisible('user_agent')"
            class="h-12 font-semibold"
          >
            User-Agent
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="records.length === 0">
          <TableCell
            :colspan="visibleColumnCount"
            class="text-center py-12 text-muted-foreground"
          >
            暂无请求记录
          </TableCell>
        </TableRow>
        <TableRow
          v-for="record in records"
          v-else
          :key="record.id"
          :class="isAdmin ? 'cursor-pointer border-b border-border/40 hover:bg-muted/30 transition-colors h-[72px]' : 'border-b border-border/40 hover:bg-muted/30 transition-colors h-[72px]'"
          @mousedown="handleRowMouseDown($event, record.id)"
          @click="handleRowClick($event, record.id)"
        >
          <TableCell
            v-if="isColumnVisible('time')"
            class="py-4 align-top"
          >
            <div class="flex w-full min-w-0 flex-col items-start gap-0.5 leading-tight">
              <span class="block w-full text-left text-xs text-foreground tabular-nums whitespace-nowrap">
                {{ formatRecordTime(record.created_at) }}
              </span>
              <span class="block w-full text-left text-[11px] text-muted-foreground tabular-nums whitespace-nowrap">
                {{ formatRecordDate(record.created_at) }}
              </span>
            </div>
          </TableCell>
          <!-- 用户页面的密钥列 -->
          <TableCell
            v-if="!isAdmin && isColumnVisible('key')"
            class="py-4"
            :title="record.api_key?.name || '-'"
          >
            <div class="flex flex-col text-xs gap-0.5">
              <span class="[overflow-wrap:anywhere]">{{ record.api_key?.name || '-' }}</span>
              <span
                v-if="record.api_key?.display"
                class="text-muted-foreground [overflow-wrap:anywhere]"
              >
                {{ record.api_key.display }}
              </span>
            </div>
          </TableCell>
          <TableCell
            v-if="isColumnVisible('model')"
            class="font-medium py-4"
            :title="getModelTooltip(record)"
          >
            <UsageModelDisplay
              :record="record"
              class="text-xs"
            />
          </TableCell>
          <TableCell
            v-if="isAdmin && isColumnVisible('provider')"
            class="py-4"
          >
            <div class="flex min-w-0 items-center gap-1">
              <div class="flex min-w-0 flex-col text-xs gap-0.5">
                <span class="[overflow-wrap:anywhere]">{{ record.provider }}</span>
                <span
                  v-if="record.provider_key_name"
                  class="text-muted-foreground [overflow-wrap:anywhere]"
                  :title="record.provider_key_name"
                >
                  {{ record.provider_key_name }}
                  <span
                    v-if="record.rate_multiplier && record.rate_multiplier !== 1.0"
                    class="text-foreground/60"
                  >({{ record.rate_multiplier }}x)</span>
                </span>
              </div>
              <Shuffle
                v-if="record.has_fallback"
                data-usage-attempt-marker="fallback"
                class="w-3.5 h-3.5 text-amber-600 dark:text-amber-400 flex-shrink-0"
                title="此请求发生了 Provider 故障转移"
                aria-label="发生 Provider 故障转移"
              />
              <RefreshCcw
                v-if="record.has_retry"
                data-usage-attempt-marker="retry"
                class="w-3.5 h-3.5 text-blue-600 dark:text-blue-400 flex-shrink-0"
                title="此请求发生了重试"
                aria-label="发生重试"
              />
            </div>
          </TableCell>
          <TableCell
            v-if="isColumnVisible('api_format')"
            class="py-4"
            :title="getApiFormatTooltip(record)"
          >
            <!-- 有格式转换或同族格式差异：两行显示 -->
            <div
              v-if="shouldShowFormatConversion(record)"
              class="flex flex-col text-xs gap-0.5"
            >
              <div class="flex items-center gap-1 whitespace-normal">
                <span>{{ formatApiFormat(record.api_format!) }}</span>
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 20 20"
                  fill="currentColor"
                  class="w-3 h-3 text-muted-foreground flex-shrink-0"
                >
                  <path
                    fill-rule="evenodd"
                    d="M3 10a.75.75 0 01.75-.75h10.638L10.23 5.29a.75.75 0 111.04-1.08l5.5 5.25a.75.75 0 010 1.08l-5.5 5.25a.75.75 0 11-1.04-1.08l4.158-3.96H3.75A.75.75 0 013 10z"
                    clip-rule="evenodd"
                  />
                </svg>
              </div>
              <span class="text-muted-foreground whitespace-normal">{{ formatApiFormat(record.endpoint_api_format!) }}</span>
            </div>
            <!-- 无格式转换：单行显示 -->
            <span
              v-else-if="record.api_format"
              class="text-xs whitespace-normal"
            >{{ formatApiFormat(record.api_format) }}</span>
            <span
              v-else
              class="text-muted-foreground text-xs"
            >-</span>
          </TableCell>
          <TableCell
            v-if="isColumnVisible('status')"
            class="usage-record-type text-left py-4 [&>div]:px-1.5"
          >
            <div class="flex flex-col items-start gap-1">
              <Badge
                variant="outline"
                data-usage-request-type="desktop"
                :class="getRequestTypeBadgeClass(record.request_type)"
              >
                {{ formatUsageRequestType(record.request_type) }}
              </Badge>
              <div class="flex flex-wrap items-center justify-start gap-1">
                <Badge
                  v-if="isUsageRecordFailed(record)"
                  variant="outline"
                  data-usage-lifecycle="desktop"
                  class="whitespace-nowrap border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-800/70 dark:bg-rose-950/30 dark:text-rose-300"
                >
                  失败
                </Badge>
                <Badge
                  v-else-if="getDisplayStatus(record) === 'pending'"
                  variant="outline"
                  data-usage-lifecycle="desktop"
                  class="whitespace-nowrap animate-pulse border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-800/70 dark:bg-amber-950/30 dark:text-amber-300"
                >
                  等待中
                </Badge>
                <Badge
                  v-else-if="getDisplayStatus(record) === 'streaming'"
                  variant="outline"
                  data-usage-lifecycle="desktop"
                  class="whitespace-nowrap animate-pulse border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-800/70 dark:bg-sky-950/30 dark:text-sky-300"
                >
                  传输中
                </Badge>
                <Badge
                  v-else-if="record.status === 'cancelled'"
                  variant="outline"
                  data-usage-lifecycle="desktop"
                  class="whitespace-nowrap border-orange-200 bg-orange-50 text-orange-700 dark:border-orange-800/70 dark:bg-orange-950/30 dark:text-orange-300"
                >
                  已取消
                </Badge>
                <Badge
                  v-if="isUsageWebSocket(record)"
                  variant="outline"
                  data-usage-transport="websocket"
                  :title="getWebSocketTransportTitle(record)"
                  :class="`${getTransportBadgeClass(record)} whitespace-nowrap`"
                >
                  WS
                </Badge>
                <Badge
                  v-else-if="getStreamModeSegments(record).hasConversion"
                  variant="outline"
                  data-usage-transport="http"
                  :class="`${getTransportBadgeClass(record)} whitespace-normal inline-flex max-w-full h-auto min-h-6 flex-wrap items-center gap-x-1 gap-y-0.5`"
                >
                  <span class="whitespace-nowrap">{{ getStreamModeSegments(record).client }}</span>
                  <span class="opacity-60">→</span>
                  <span class="whitespace-nowrap">{{ getStreamModeSegments(record).upstream }}</span>
                </Badge>
                <Badge
                  v-else
                  variant="outline"
                  data-usage-transport="http"
                  :class="`${getTransportBadgeClass(record)} whitespace-nowrap`"
                >
                  {{ getStreamModeLabel(record) }}
                </Badge>
              </div>
            </div>
          </TableCell>
          <TableCell
            v-if="isColumnVisible('tokens')"
            class="py-4"
          >
            <template v-if="record.usage_available !== false">
              <div
                class="flex w-full min-w-0 items-center justify-start gap-1 text-xs leading-tight tabular-nums whitespace-nowrap"
                title="输入 Token / 输出 Token"
              >
                <span
                  data-usage-token-part="input"
                  class="text-left text-sky-700 dark:text-sky-300"
                >
                  {{ formatTokens(getRecordEffectiveInputTokens(record)) }}
                </span>
                <span class="text-muted-foreground">
                  /
                </span>
                <span
                  data-usage-token-part="output"
                  class="text-left text-emerald-700 dark:text-emerald-300"
                >
                  {{ formatTokens(record.output_tokens || 0) }}
                </span>
              </div>
              <div
                class="mt-0.5 flex w-full min-w-0 items-center justify-start gap-1 text-xs leading-tight tabular-nums text-muted-foreground whitespace-nowrap"
                :title="getRecordCacheTokensTitle(record)"
              >
                <span
                  data-usage-token-part="cache-read"
                  class="text-left"
                  :class="[
                    hasPositiveTokens(getRecordCacheReadTokens(record))
                      ? 'text-cyan-700 dark:text-cyan-300'
                      : ''
                  ]"
                >
                  {{ formatOptionalTokens(getRecordCacheReadTokens(record)) }}
                </span>
                <span>
                  /
                </span>
                <span
                  data-usage-cache-hit-rate="desktop"
                  class="text-left"
                  :class="[
                    getRecordCacheHitRate(record) != null
                      ? 'text-emerald-700 dark:text-emerald-300 font-medium'
                      : ''
                  ]"
                >
                  {{ formatRecordCacheHitRate(record) }}
                </span>
              </div>
              <div
                v-if="(record.input_audio_tokens || 0) > 0 || (record.output_audio_tokens || 0) > 0"
                class="mt-0.5 text-left text-[10px] leading-tight tabular-nums text-muted-foreground whitespace-nowrap"
              >
                音频 {{ formatOptionalTokens(record.input_audio_tokens) }} / {{ formatOptionalTokens(record.output_audio_tokens) }}
              </div>
            </template>
            <div
              v-else
              data-usage-unavailable="tokens"
              class="text-left text-xs text-muted-foreground"
              title="上游未提供可验证的 token/费用用量"
            >
              不可用
            </div>
          </TableCell>
          <TableCell
            v-if="isColumnVisible('cost')"
            class="text-left py-4"
          >
            <div
              v-if="record.usage_available !== false && record.usage_pricing_available !== false"
              class="flex flex-col items-start text-xs gap-0.5 whitespace-nowrap"
            >
              <span
                data-usage-cost-value="desktop"
                class="whitespace-nowrap"
                :class="[getRecordCostClass(record.cost)]"
              >{{ formatRecordCost(record.cost) }}</span>
              <span
                v-if="showActualCost && record.actual_cost !== undefined && record.rate_multiplier && record.rate_multiplier !== 1.0"
                class="text-blue-600/70 dark:text-blue-300/70 whitespace-nowrap"
              >
                {{ formatRecordCost(record.actual_cost) }}
              </span>
            </div>
            <div
              v-else-if="record.usage_available === false"
              data-usage-unavailable="cost"
              class="text-xs text-muted-foreground"
              title="上游未提供可验证的 token/费用用量"
            >
              不可用
            </div>
            <div
              v-else
              data-usage-unpriced="cost"
              class="text-xs text-muted-foreground"
              title="token 用量可验证，但当前计价规则不支持该音频用量分项"
            >
              未计价
            </div>
          </TableCell>
          <TableCell
            v-if="isColumnVisible('performance')"
            class="text-left py-4"
          >
            <!-- pending/streaming 状态：首字与动态总耗时保留在同一行 -->
            <div
              v-if="getDisplayStatus(record) === 'pending' || getDisplayStatus(record) === 'streaming'"
              class="flex flex-col items-start text-xs gap-0.5"
              :title="getRecordPerformanceTitle(record)"
            >
              <span class="tabular-nums whitespace-normal">
                <span
                  data-usage-performance-part="first-byte"
                  class="text-amber-700 dark:text-amber-300"
                >{{ formatRecordDurationSeconds(record.first_byte_time_ms) }}</span>
                <span class="text-muted-foreground"> / </span>
                <ElapsedTimeText
                  data-usage-performance-part="total"
                  class="text-blue-700 dark:text-blue-300"
                  :created-at="record.created_at"
                  :response-time-updated-at="record.response_time_updated_at ?? null"
                  :status="getDisplayStatus(record)"
                  :response-time-ms="record.response_time_ms ?? null"
                />
              </span>
            </div>
            <!-- 已完成状态：首字 + 总耗时 -->
            <div
              v-else-if="hasRecordDisplayLatency(record)"
              class="flex flex-col items-start text-xs gap-0.5"
              :title="getRecordPerformanceTitle(record)"
            >
              <span class="tabular-nums whitespace-normal">
                <span
                  data-usage-performance-part="first-byte"
                  class="text-amber-700 dark:text-amber-300"
                >{{ getRecordFirstByteLatency(record) }}</span>
                <span class="text-muted-foreground"> / </span>
                <span
                  data-usage-performance-part="total"
                  class="text-blue-700 dark:text-blue-300"
                >{{ getRecordTotalLatency(record) }}</span>
              </span>
              <span
                data-usage-performance-part="speed"
                class="tabular-nums whitespace-normal"
                :class="[getOutputRateClass(record)]"
              >
                {{ formatOutputRate(getRecordDisplayOutputRate(record)) }}
              </span>
            </div>
            <span
              v-else
              class="text-muted-foreground"
            >-</span>
          </TableCell>
          <TableCell
            v-if="isColumnVisible('client_family')"
            class="py-4 text-xs"
            :title="formatClientFamily(record.client_family)"
          >
            <Badge
              variant="outline"
              class="w-fit max-w-full border-border/60 text-muted-foreground"
            >
              <span class="[overflow-wrap:anywhere]">{{ formatClientFamily(record.client_family) }}</span>
            </Badge>
          </TableCell>
          <TableCell
            v-if="isColumnVisible('client_ip')"
            class="py-4 text-xs [overflow-wrap:anywhere]"
            :title="record.client_ip || '-'"
          >
            {{ record.client_ip || '-' }}
          </TableCell>
          <TableCell
            v-if="isColumnVisible('user_agent')"
            class="py-4 text-xs [overflow-wrap:anywhere]"
            :title="record.user_agent || '-'"
          >
            {{ record.user_agent || '-' }}
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>

    <!-- 分页控件 -->
    <template #pagination>
      <Pagination
        v-if="totalRecords > 0"
        :current="currentPage"
        :total="totalRecords"
        :page-size="pageSize"
        :page-size-options="pageSizeOptions"
        cache-key="usage-records-page-size"
        @update:current="$emit('update:currentPage', $event)"
        @update:page-size="$emit('update:pageSize', $event)"
      />
    </template>
  </TableCard>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useElementSize, useLocalStorage } from '@vueuse/core'
import {
  TableCard,
  Badge,
  Button,
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
  Pagination,
  SortableTableHead,
  TableFilterMenu,
} from '@/components/ui'
import { RefreshCcw, Shuffle } from 'lucide-vue-next'
import { formatHitRate, formatTokens } from '@/utils/format'
import {
  getCacheCreationTokens,
  getCacheHitRate,
  getCacheReadTokens,
  getEffectiveInputTokens,
} from '../token-normalization'
import {
  formatOutputRate,
  formatOutputRateValue,
  getDisplayOutputRate,
  getGenerationTimeMs,
} from '../performance'
import {
  formatUsageStreamLabel,
  isUsageRecordFailed,
  isUsageWebSocket,
  resolveDisplayRequestStatus,
  resolveUsageStreamLabelSegments
} from '../utils/status'
import { useRowClick } from '@/composables/useRowClick'
import { API_FORMAT_ORDER, formatApiFormat } from '@/api/endpoints/types/api-format'
import { formatClientFamily } from '@/features/usage/utils/clientFamily'
import { formatServiceTierFact } from '../utils/service-tier'
import { isCyberPolicyError } from '../utils/cyberError'
import { formatUsageWebSocketTransportTitle as getWebSocketTransportTitle } from '../utils/websocketTransport'
import {
  formatUsageRequestType,
  normalizeUsageRequestType,
} from '../utils/requestType'
import type { DateRangeParams, UsageRecord } from '../types'
import { MultiSelect, TimeRangePicker } from '@/components/common'
import type { MultiSelectOption } from '@/components/common/MultiSelect.vue'
import ElapsedTimeText from './ElapsedTimeText.vue'
import UsageModelDisplay from './UsageModelDisplay.vue'


export interface UserOption {
  id: string
  username: string
  email: string
}

interface FilterOption {
  value: string
  label: string
  disabled?: boolean
}

type UsageRecordColumnId =
  | 'time'
  | 'key'
  | 'model'
  | 'provider'
  | 'api_format'
  | 'status'
  | 'tokens'
  | 'cost'
  | 'performance'
  | 'client_family'
  | 'client_ip'
  | 'user_agent'

interface UsageRecordColumnOption {
  id: UsageRecordColumnId
  label: string
  width: number
  adminOnly?: boolean
  userOnly?: boolean
}

const props = defineProps<{
  records: UsageRecord[]
  showActualCost: boolean
  loading: boolean
  // 时间范围
  timeRange: DateRangeParams
  // 筛选
  filterModel: string
  filterProvider: string
  filterApiFormat: string
  filterRequestStatus: string
  filterTransport: string
  filterAttempt: string
  filterClientFamily: string
  availableModels: string[]
  availableProviders: string[]
  availableClientFamilies: string[]
  // 分页
  currentPage: number
  pageSize: number
  totalRecords: number
  pageSizeOptions: number[]
  // 自动刷新
  autoRefresh: boolean
}>()

const emit = defineEmits<{
  'update:timeRange': [value: DateRangeParams]
  'update:filterModel': [value: string]
  'update:filterProvider': [value: string]
  'update:filterApiFormat': [value: string]
  'update:filterRequestStatus': [value: string]
  'update:filterTransport': [value: string]
  'update:filterAttempt': [value: string]
  'update:filterClientFamily': [value: string]
  'update:currentPage': [value: number]
  'update:pageSize': [value: number]
  'update:autoRefresh': [value: boolean]
  'refresh': []
  'showDetail': [id: string]
  'prefetchDetail': [id: string]
}>()

// 单用户化阶段4：使用记录表仅剩 admin 视角，isAdmin 由 prop 内化为常量 true。
const isAdmin = true

const firstByteTimingDescription = 'HTTP 流式首字：从本次执行开始到网关观察到首批响应体数据或流事件，可能包含启动或心跳事件；成功响应头到达和有效正文生成是不同时间点。端到端指标包含此前调度与重试等待，缺失时回退到单轮指标。'

const USAGE_RECORD_COLUMN_OPTIONS: UsageRecordColumnOption[] = [
  { id: 'time', label: '时间', width: 10 },
  { id: 'key', label: '密钥', width: 12, userOnly: true },
  { id: 'model', label: '模型', width: 16 },
  { id: 'provider', label: '提供商', width: 12, adminOnly: true },
  { id: 'api_format', label: 'API格式', width: 12 },
  { id: 'status', label: '类型/状态', width: 6 },
  { id: 'tokens', label: 'Tokens', width: 13 },
  { id: 'cost', label: '费用', width: 13 },
  { id: 'performance', label: '耗时/速度', width: 12 },
  { id: 'client_family', label: '客户端类型', width: 12 },
  { id: 'client_ip', label: 'IP 地址', width: 11 },
  { id: 'user_agent', label: 'User-Agent', width: 16 },
]

const DEFAULT_ADMIN_COLUMNS: UsageRecordColumnId[] = [
  'time',
  'model',
  'provider',
  'api_format',
  'status',
  'tokens',
  'cost',
  'performance',
]

/** 请求类型徽章色；颜色只辅助扫读，完整类型始终由文字表达。 */
const REQUEST_TYPE_BADGE_CLASSES: Readonly<Record<string, string>> = {
  chat: 'border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-800/70 dark:bg-sky-950/30 dark:text-sky-300',
  compact: 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-800/70 dark:bg-amber-950/30 dark:text-amber-300',
  image: 'border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-800/70 dark:bg-rose-950/30 dark:text-rose-300',
  video: 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-800/70 dark:bg-emerald-950/30 dark:text-emerald-300',
  realtime: 'border-cyan-200 bg-cyan-50 text-cyan-700 dark:border-cyan-800/70 dark:bg-cyan-950/30 dark:text-cyan-300',
  live: 'border-cyan-200 bg-cyan-50 text-cyan-700 dark:border-cyan-800/70 dark:bg-cyan-950/30 dark:text-cyan-300',
  unknown: 'border-slate-200 bg-slate-50 text-slate-600 dark:border-slate-700 dark:bg-slate-900/60 dark:text-slate-300',
}

/** 未识别的新请求类型使用中性色，避免把未知值伪装成已知业务类型。 */
const DEFAULT_REQUEST_TYPE_BADGE_CLASS = REQUEST_TYPE_BADGE_CLASSES.unknown

// 使用统一 API 格式枚举，避免使用记录筛选项和系统格式列表漂移。
const availableApiFormats = API_FORMAT_ORDER.map((value) => ({
  value,
  label: formatApiFormat(value),
}))

const adminVisibleColumnIds = useLocalStorage<UsageRecordColumnId[]>(
  'usage-records-visible-columns-admin',
  DEFAULT_ADMIN_COLUMNS,
)
const roleColumnOptions = computed(() => USAGE_RECORD_COLUMN_OPTIONS.filter(column => !column.userOnly))

const roleColumnIds = computed(() => new Set(roleColumnOptions.value.map(column => column.id)))

function sanitizeColumnIds(
  ids: readonly string[],
  fallback: readonly UsageRecordColumnId[],
): UsageRecordColumnId[] {
  const seen = new Set<UsageRecordColumnId>()
  const sanitized = ids.filter((id): id is UsageRecordColumnId => {
    if (!roleColumnIds.value.has(id as UsageRecordColumnId)) return false
    if (seen.has(id as UsageRecordColumnId)) return false
    seen.add(id as UsageRecordColumnId)
    return true
  })
  if (sanitized.length === 0) return [...fallback]
  // Add newly introduced feature column to existing saved layouts, keeping it
  // immediately before Tokens as the default presentation order.
  return sanitized
}

const visibleColumnIds = computed<UsageRecordColumnId[]>({
  get: () => sanitizeColumnIds(
    adminVisibleColumnIds.value,
    DEFAULT_ADMIN_COLUMNS,
  ),
  set: (value) => {
    adminVisibleColumnIds.value = sanitizeColumnIds(value, DEFAULT_ADMIN_COLUMNS)
  },
})

const visibleColumnSet = computed(() => new Set<UsageRecordColumnId>(visibleColumnIds.value))
const visibleColumnCount = computed(() => visibleColumnIds.value.length)
// Keep DOM column order and divide only among selected columns.
const visibleColumns = computed(() => roleColumnOptions.value.filter(column => isColumnVisible(column.id)))
const visibleColumnWeight = computed(() => visibleColumns.value.reduce((sum, column) => sum + column.width, 0))
const recordsTable = ref<InstanceType<typeof Table> | null>(null)
const { width: tableWidth } = useElementSize(recordsTable)
const flexibleColumnWeight = computed(() => visibleColumns.value
  .filter(column => column.id !== 'status')
  .reduce((sum, column) => sum + column.width, 0))

function getColumnWidth(column: UsageRecordColumnOption): string {
  if (!isColumnVisible('status') || visibleColumnCount.value === 1 || tableWidth.value === 0) {
    return `${column.width / visibleColumnWeight.value * 100}%`
  }
  // Table columns do not support mixed-unit calc widths, so resolve the reserved space first.
  const statusWidth = Math.min(88 / tableWidth.value * 100, 100)
  if (column.id === 'status') return `${statusWidth}%`
  return `${column.width / flexibleColumnWeight.value * (100 - statusWidth)}%`
}

const columnSelectOptions = computed<MultiSelectOption[]>(() => roleColumnOptions.value.map(column => ({
  value: column.id,
  label: column.label,
})))

function isColumnVisible(column: UsageRecordColumnId): boolean {
  return visibleColumnSet.value.has(column)
}

const modelFilterOptions = computed<FilterOption[]>(() => [
  { value: '__all__', label: '全部模型' },
  ...props.availableModels.map((model) => ({
    value: model,
    label: model.replace('claude-', ''),
  })),
])

const providerFilterOptions = computed<FilterOption[]>(() => [
  { value: '__all__', label: '全部提供商' },
  ...props.availableProviders.map((provider) => ({
    value: provider,
    label: provider,
  })),
])

const clientFamilyFilterOptions = computed<FilterOption[]>(() => {
  const families = new Set<string>(props.availableClientFamilies)
  props.records.forEach((record) => {
    const family = record.client_family?.trim()
    if (family) families.add(family)
  })
  return [
    { value: '__all__', label: '全部客户端' },
    ...Array.from(families).sort().map((family) => ({
      value: family,
      label: formatClientFamily(family),
    })),
  ]
})

const apiFormatFilterOptions = computed<FilterOption[]>(() => [
  { value: '__all__', label: '全部格式' },
  ...availableApiFormats.map((format) => ({
    value: format.value,
    label: format.label,
  })),
])

const timeRangeModel = computed({
  get: () => props.timeRange,
  set: (value: DateRangeParams) => emit('update:timeRange', value)
})

function getDisplayStatus(record: UsageRecord) {
  return resolveDisplayRequestStatus(record)
}

function getStreamModeLabel(record: UsageRecord): string {
  return formatUsageStreamLabel(record)
}

function getStreamModeSegments(record: UsageRecord) {
  return resolveUsageStreamLabelSegments(record)
}

function parseRecordDateTime(dateStr: string): Date {
  const utcDateStr = dateStr.includes('Z') || dateStr.includes('+') ? dateStr : `${dateStr}Z`
  return new Date(utcDateStr)
}

function formatRecordDate(dateStr: string): string {
  return formatRecordShortDate(dateStr)
}

function formatRecordShortDate(dateStr: string): string {
  const date = parseRecordDateTime(dateStr)
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${month}-${day}`
}

function formatRecordTime(dateStr: string): string {
  const date = parseRecordDateTime(dateStr)
  const hours = String(date.getHours()).padStart(2, '0')
  const minutes = String(date.getMinutes()).padStart(2, '0')
  const seconds = String(date.getSeconds()).padStart(2, '0')
  return `${hours}:${minutes}:${seconds}`
}

function formatRecordProviderSegment(record: UsageRecord): string {
  return `${record.provider || '-'} / ${record.provider_key_name || '-'}`
}

/** 返回请求类型对应的语义色徽章。 */
function getRequestTypeBadgeClass(value: string | null | undefined): string {
  return REQUEST_TYPE_BADGE_CLASSES[normalizeUsageRequestType(value)]
    ?? DEFAULT_REQUEST_TYPE_BADGE_CLASS
}

/** 返回客户端传输徽章色；WS、流式和标准请求使用不同但低饱和的色系。 */
function getTransportBadgeClass(record: UsageRecord): string {
  if (isUsageWebSocket(record)) {
    return 'border-indigo-200 bg-indigo-50 text-indigo-700 dark:border-indigo-800/70 dark:bg-indigo-950/30 dark:text-indigo-300'
  }
  const streamModes = getStreamModeSegments(record)
  if (streamModes.client === '流式' || streamModes.upstream === '流式') {
    return 'border-teal-200 bg-teal-50 text-teal-700 dark:border-teal-800/70 dark:bg-teal-950/30 dark:text-teal-300'
  }
  return 'border-slate-200 bg-slate-50 text-slate-600 dark:border-slate-700 dark:bg-slate-900/60 dark:text-slate-300'
}

// 使用复用的行点击逻辑
const { handleMouseDown, shouldTriggerRowClick } = useRowClick()

function handleRowMouseDown(event: MouseEvent, id: string) {
  handleMouseDown(event)
  if (event.button !== 0) return
  emit('prefetchDetail', id)
}

// 处理行点击，排除文本选择操作
function handleRowClick(event: MouseEvent, id: string) {
  if (!shouldTriggerRowClick(event)) return
  emit('showDetail', id)
}

function getRecordEffectiveInputTokens(record: UsageRecord): number {
  return getEffectiveInputTokens(record)
}

function getRecordCacheReadTokens(record: UsageRecord): number {
  return getCacheReadTokens(record)
}

function getRecordCacheCreationTokens(record: UsageRecord): number {
  return getCacheCreationTokens(record)
}

/** 返回单条记录的缓存命中率；缺少输入上下文时返回 null。 */
function getRecordCacheHitRate(record: UsageRecord): number | null {
  return getCacheHitRate(record)
}

/** 将缓存命中率格式化为两位小数百分比。 */
function formatRecordCacheHitRate(record: UsageRecord): string {
  return formatHitRate(getRecordCacheHitRate(record) ?? undefined)
}

function hasPositiveTokens(value: number | null | undefined): boolean {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

function formatOptionalTokens(value: number | null | undefined): string {
  return hasPositiveTokens(value) ? formatTokens(value) : '-'
}

function formatRecordCost(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return '$0.000000'
  return `$${value.toFixed(6)}`
}

/** 非零费用使用主色强调，零费用保持弱化以降低视觉噪声。 */
function getRecordCostClass(value: number | null | undefined): string {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
    ? 'font-semibold text-blue-700 dark:text-blue-300'
    : 'font-medium text-muted-foreground'
}

function hasRecordCacheTokens(record: UsageRecord): boolean {
  return hasPositiveTokens(getRecordCacheReadTokens(record)) || hasPositiveTokens(getRecordCacheCreationTokens(record))
}

/** 返回缓存读取、写入和命中率的完整说明，供列表悬停查看。 */
function getRecordCacheTokensTitle(record: UsageRecord): string {
  return [
    `缓存读取: ${formatOptionalTokens(getRecordCacheReadTokens(record))}`,
    `缓存写入: ${formatOptionalTokens(getRecordCacheCreationTokens(record))}`,
    `缓存命中率: ${formatRecordCacheHitRate(record)}`,
  ].join('\n')
}

/** 返回端到端首字耗时，缺失时回退到成功候选首字。 */
function getRecordFirstByteLatency(record: UsageRecord): string {
  return formatRecordDurationSeconds(
    record.end_to_end_first_byte_time_ms ?? record.first_byte_time_ms,
  )
}

/** 返回端到端总耗时，缺失时回退到成功候选耗时。 */
function getRecordTotalLatency(record: UsageRecord): string {
  return formatRecordDurationSeconds(record.end_to_end_time_ms ?? record.response_time_ms)
}

function formatRecordDurationSeconds(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return '-'
  return `${(ms / 1000).toFixed(2)}s`
}

function hasRecordDisplayLatency(record: UsageRecord): boolean {
  return record.end_to_end_time_ms != null
    || record.end_to_end_first_byte_time_ms != null
    || record.response_time_ms != null
    || record.first_byte_time_ms != null
}

function getRecordDisplayOutputRate(record: UsageRecord): number | null {
  return getDisplayOutputRate({
    output_tokens: record.output_tokens,
    response_time_ms: record.response_time_ms,
    first_byte_time_ms: record.first_byte_time_ms,
    is_stream: record.is_stream,
    upstream_is_stream: record.upstream_is_stream,
  })
}

/** 输出速度有值时使用绿色强调，缺失占位保持弱化。 */
function getOutputRateClass(record: UsageRecord): string {
  return getRecordDisplayOutputRate(record) == null
    ? 'text-muted-foreground'
    : 'font-medium text-emerald-700 dark:text-emerald-300'
}

function getRecordPerformanceTitle(record: UsageRecord): string {
  const outputRate = getRecordDisplayOutputRate(record)
  return [
    firstByteTimingDescription,
    `端到端首字: ${formatRecordDurationSeconds(record.end_to_end_first_byte_time_ms ?? record.first_byte_time_ms)}`,
    `端到端总耗时: ${formatRecordDurationSeconds(record.end_to_end_time_ms ?? record.response_time_ms)}`,
    `成功候选首字: ${formatRecordDurationSeconds(record.first_byte_time_ms)}`,
    `成功候选耗时: ${formatRecordDurationSeconds(record.response_time_ms)}`,
    `生成耗时: ${formatRecordDurationSeconds(getGenerationTimeMs(record))}`,
    `输出速度: ${formatOutputRateTokensPerSecond(outputRate)}`,
  ].join('\n')
}

function formatOutputRateTokensPerSecond(outputRate: number | null | undefined): string {
  const value = formatOutputRateValue(outputRate)
  if (value === '-') return value
  return `${value} tokens/s`
}


// useDebounceFn 自动处理清理，无需 onUnmounted

// 判断是否应该显示格式转换信息
// 包括：1. 跨格式转换（has_format_conversion=true）2. 同族格式差异
function shouldShowFormatConversion(record: UsageRecord): boolean {
  if (!record.api_format || !record.endpoint_api_format) {
    return false
  }
  // 跨格式转换
  if (record.has_format_conversion) {
    return true
  }
  // 同族格式差异（精确字符串比较，不区分大小写）
  return record.api_format.trim().toLowerCase() !== record.endpoint_api_format.trim().toLowerCase()
}

// 获取 API 格式的 tooltip（包含转换信息）
function getApiFormatTooltip(record: UsageRecord): string {
  if (!record.api_format) {
    return ''
  }
  const displayFormat = formatApiFormat(record.api_format)

  // 如果发生了格式转换或同族格式差异，显示详细信息
  if (shouldShowFormatConversion(record)) {
    const endpointApiFormat = record.endpoint_api_format ?? record.api_format
    const endpointDisplayFormat = formatApiFormat(endpointApiFormat)
    const conversionType = record.has_format_conversion ? '格式转换' : '格式兼容（无需转换）'
    return `用户请求格式: ${displayFormat}\n端点原生格式: ${endpointDisplayFormat}\n${conversionType}`
  }

  return displayFormat
}

// 获取实际使用的模型（优先 target_model，其次列表接口下发的 model_version）
// 只有当实际模型与请求模型不同时才返回，用于显示映射箭头
function getActualModel(record: UsageRecord): string | null {
  // 优先显示模型映射
  if (record.target_model && record.target_model !== record.model) {
    return record.target_model
  }
  // 其次显示 Provider 返回的实际版本（如 Gemini 的 modelVersion）
  if (record.model_version && record.model_version !== record.model) {
    return record.model_version
  }
  return null
}

function getReasoningEffort(record: UsageRecord): string | null {
  const requested = record.requested_reasoning_effort?.trim()
  const actual = record.reasoning_effort?.trim()
  if (requested && actual && requested.toLowerCase() !== actual.toLowerCase()) {
    return `${requested} -> ${actual}`
  }
  return actual || requested || null
}

function hasCyberPolicyError(record: UsageRecord): boolean {
  return isCyberPolicyError(record.error_message)
}

interface ServiceTierBadgePresentation {
  label: string
  className: string
  title: string
  ariaLabel: string
}

function normalizeServiceTier(value: string | null | undefined): string | null {
  const serviceTier = value?.trim().toLowerCase()
  return serviceTier || null
}

function canonicalServiceTier(value: string | null): string | null {
  if (value === 'auto' || value === 'default' || value === 'standard') {
    return 'standard'
  }
  if (value === 'fast') {
    return 'priority'
  }
  return value
}

function buildServiceTierBadgePresentation(
  requestedRaw: string | null,
): ServiceTierBadgePresentation {
  const titleLines: string[] = []
  const requestedLabel = formatServiceTierFact(requestedRaw)
  if (requestedLabel) titleLines.push(`上游请求档位：${requestedLabel}`)
  // Billing is resolved from the same final provider request tier. Keep it
  // explicit in the tooltip without consulting a response-side tier.
  if (requestedLabel) titleLines.push(`计费档位：${requestedLabel}`)
  const title = titleLines.join('\n')
  return {
    label: 'Fast',
    className: '!bg-transparent text-blue-500 dark:text-blue-300',
    title,
    ariaLabel: titleLines.join('，'),
  }
}

function getServiceTierBadge(record: UsageRecord): ServiceTierBadgePresentation | null {
  const requestedRaw = normalizeServiceTier(record.service_tier)
  const requested = canonicalServiceTier(requestedRaw)
  const requestedFast = requested === 'priority'
  if (!requestedFast) return null
  return buildServiceTierBadgePresentation(requestedRaw)
}

function getServiceTierTitle(record: UsageRecord): string {
  const badge = getServiceTierBadge(record)
  if (badge) return badge.title

  const requested = formatServiceTierFact(record.service_tier)
  return [
    requested ? `上游请求档位：${requested}` : null,
    requested ? `计费档位：${requested}` : null,
  ].filter((line): line is string => Boolean(line)).join('\n')
}

// 获取模型列的 tooltip
function getModelTooltip(record: UsageRecord): string {
  const actualModel = getActualModel(record)
  const reasoningEffort = getReasoningEffort(record)
  const serviceTierTitle = getServiceTierTitle(record)
  const tierSuffix = serviceTierTitle ? `\n${serviceTierTitle}` : ''
  const cyberSuffix = hasCyberPolicyError(record) ? '\nCyber Policy: blocked' : ''
  const suffix = `${reasoningEffort ? `\nReasoning: ${reasoningEffort}` : ''}${tierSuffix}${cyberSuffix}`
  if (actualModel) {
    return `${record.model} -> ${actualModel}${suffix}`
  }
  return `${record.model}${suffix}`
}
</script>
