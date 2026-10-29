<template>
  <Dialog
    :open="open"
    size="3xl"
    :close-on-backdrop="false"
    @update:open="(val: boolean) => { if (!val) emit('close') }"
  >
    <template #header>
      <div class="border-b border-border px-6 py-4">
        <div class="space-y-3">
          <div class="text-lg font-semibold text-foreground leading-tight">
            {{ dialogTitle }}
          </div>
          <p
            v-if="dialogDescription && !showResult"
            class="text-xs text-muted-foreground"
          >
            {{ dialogDescription }}
          </p>
        </div>
      </div>
    </template>

    <div
      v-if="showSetup"
      class="space-y-3"
    >
      <div
        v-if="endpoints.length > 0"
        class="space-y-2"
      >
        <div class="text-sm font-medium text-foreground">
          选择测试端点
        </div>
        <div class="dialog-grid-2 gap-2">
          <button
            v-for="endpoint in endpoints"
            :key="endpoint.id"
            type="button"
            class="h-full w-full rounded-lg border px-3 py-2 text-left transition-colors"
            :class="selectedEndpoint?.id === endpoint.id
              ? 'border-primary bg-primary/5'
              : 'border-border/60 hover:bg-muted/40'"
            @click="emit('selectEndpoint', endpoint.id)"
          >
            <div class="flex items-center justify-between gap-3">
              <div class="min-w-0">
                <div class="text-sm font-medium">
                  {{ formatApiFormat(endpoint.api_format) }}
                </div>
                <div class="mt-1 text-xs text-muted-foreground break-all">
                  {{ endpoint.base_url }}
                </div>
              </div>
              <Badge :variant="selectedEndpoint?.id === endpoint.id ? 'success' : 'outline'">
                {{ selectedEndpoint?.id === endpoint.id ? '已选择' : (endpoint.is_active ? '可用' : '已禁用') }}
              </Badge>
            </div>
          </button>
        </div>
      </div>

      <div
        v-if="endpoints.length > 0"
        class="dialog-grid-2 gap-2"
      >
        <div class="space-y-2">
          <div class="text-sm font-medium text-foreground">
            测试 Key
          </div>
          <Select
            :model-value="selectedKeyValue"
            :disabled="keyOptionsLoading && keyOptions.length === 0"
            @update:model-value="handleKeyValueChange"
          >
            <SelectTrigger class="h-9 w-full border-border/60 text-xs">
              <SelectValue placeholder="自动" />
            </SelectTrigger>
            <SelectContent
              class="w-[var(--radix-select-trigger-width)]"
              :disable-portal="false"
            >
              <SelectItem
                value="auto"
                text-value="自动"
              >
                自动
              </SelectItem>
              <SelectItem
                v-for="option in keyOptions"
                :key="option.value"
                :value="option.value"
                :text-value="option.label"
              >
                {{ option.label }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-2">
          <div class="text-sm font-medium text-foreground">
            模型映射
          </div>
          <Select
            :model-value="selectedModelMappingValue"
            @update:model-value="handleModelMappingValueChange"
          >
            <SelectTrigger class="h-9 w-full border-border/60 text-xs">
              <SelectValue placeholder="自动" />
            </SelectTrigger>
            <SelectContent
              class="w-[var(--radix-select-trigger-width)]"
              :disable-portal="false"
              :searchable="false"
            >
              <SelectItem
                value="auto"
                text-value="自动"
              >
                自动
              </SelectItem>
              <SelectItem
                v-for="option in modelMappingOptions"
                :key="option.name"
                :value="option.name"
                :text-value="option.name"
              >
                {{ option.name }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <div class="dialog-grid-wide items-start gap-3">
        <div class="space-y-2">
          <div class="flex min-h-9 items-center justify-between gap-3 rounded-md border border-border/60 px-2">
            <button
              type="button"
              class="flex min-w-0 flex-1 items-center gap-2 py-2 text-left text-sm font-medium"
              :aria-expanded="requestHeadersExpanded"
              aria-controls="model-test-request-headers"
              @click="requestHeadersExpanded = !requestHeadersExpanded"
            >
              <ChevronRight
                class="h-4 w-4 shrink-0 text-muted-foreground transition-transform"
                :class="{ 'rotate-90': requestHeadersExpanded }"
              />
              <span>测试请求头</span>
            </button>
            <div
              v-if="requestHeadersExpanded"
              class="flex shrink-0 items-center gap-1"
            >
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 rounded-lg text-muted-foreground"
                title="格式化请求头 JSON"
                @click="formatRequestHeadersDraft"
              >
                <Code2 class="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 rounded-lg text-muted-foreground"
                title="重置请求头"
                @click="resetRequestHeadersDraft"
              >
                <RotateCcw class="h-4 w-4" />
              </Button>
            </div>
          </div>
          <div
            v-show="requestHeadersExpanded"
            id="model-test-request-headers"
            class="space-y-2"
          >
            <Textarea
              :model-value="requestHeadersDraft"
              class="h-[120px] min-h-[120px] font-mono text-xs"
              placeholder="输入 JSON 请求头"
              @update:model-value="emit('update:requestHeadersDraft', $event)"
            />
            <div
              v-if="requestHeadersError"
              class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive"
            >
              {{ requestHeadersError }}
            </div>
          </div>
        </div>

        <div class="space-y-2">
          <div class="flex min-h-9 items-center justify-between gap-3 rounded-md border border-border/60 px-2">
            <button
              type="button"
              class="flex min-w-0 flex-1 items-center gap-2 py-2 text-left text-sm font-medium"
              :aria-expanded="requestBodyExpanded"
              aria-controls="model-test-request-body"
              @click="requestBodyExpanded = !requestBodyExpanded"
            >
              <ChevronRight
                class="h-4 w-4 shrink-0 text-muted-foreground transition-transform"
                :class="{ 'rotate-90': requestBodyExpanded }"
              />
              <span>测试请求体</span>
            </button>
            <div
              v-if="requestBodyExpanded"
              class="flex shrink-0 items-center gap-1"
            >
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 rounded-lg text-muted-foreground"
                title="格式化请求体 JSON"
                @click="formatRequestBodyDraft"
              >
                <Code2 class="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 rounded-lg text-muted-foreground"
                title="重置请求体"
                @click="resetRequestBodyDraft"
              >
                <RotateCcw class="h-4 w-4" />
              </Button>
            </div>
          </div>
          <div
            v-show="requestBodyExpanded"
            id="model-test-request-body"
            class="space-y-2"
          >
            <Textarea
              :model-value="requestBodyDraft"
              class="h-[120px] min-h-[120px] font-mono text-xs"
              placeholder="输入 JSON 请求体"
              @update:model-value="emit('update:requestBodyDraft', $event)"
            />
            <div
              v-if="requestBodyError"
              class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive"
            >
              {{ requestBodyError }}
            </div>
          </div>
        </div>
      </div>

      <div class="flex justify-end">
        <Button
          :disabled="startDisabled"
          @click="emit('start')"
        >
          开始测试
        </Button>
      </div>
    </div>

    <div
      v-else-if="testing"
      class="space-y-4 py-6"
    >
      <div class="flex flex-col items-center justify-center gap-3 text-center">
        <Loader2 class="h-8 w-8 animate-spin text-primary" />
        <div class="space-y-1">
          <p class="text-sm font-medium">
            正在测试模型
          </p>
          <p class="text-xs text-muted-foreground">
            {{ selectingModelName || '-' }}
          </p>
          <p
            v-if="selectedEndpoint"
            class="text-xs text-muted-foreground"
          >
            端点：{{ formatApiFormat(selectedEndpoint.api_format) }} · {{ selectedEndpoint.base_url }}
          </p>
        </div>
      </div>

      <div class="rounded-lg border border-border/60 bg-muted/20 p-4 space-y-4">
        <div class="space-y-2">
          <div class="flex items-center justify-between gap-3 text-xs text-muted-foreground">
            <span>实时进度</span>
            <span>{{ liveProgressText }}</span>
          </div>
          <div class="h-2 overflow-hidden rounded-full bg-muted">
            <div
              class="h-full bg-primary transition-all duration-300"
              :style="{ width: `${liveProgressPercent}%` }"
            />
          </div>
        </div>

        <div class="flex min-w-0 items-center gap-2 text-sm font-medium">
          <Loader2 class="h-4 w-4 shrink-0 animate-spin text-primary" />
          <span
            v-if="activeTraceCandidate"
            class="truncate"
          >
            {{ formatTraceCandidateIndex(activeTraceCandidate) }}
            · {{ formatTraceCandidateAccount(activeTraceCandidate) }}
            <template v-if="traceCandidateMappingName(activeTraceCandidate)">
              · {{ traceCandidateMappingName(activeTraceCandidate) }}
            </template>
          </span>
          <span
            v-else
            class="text-muted-foreground"
          >
            正在准备测试请求
          </span>
        </div>

        <div
          v-if="requestId"
          class="break-all text-[11px] text-muted-foreground"
        >
          请求 ID：<code class="rounded bg-muted px-1 py-0.5">{{ requestId }}</code>
        </div>

        <div
          v-if="liveRecentCandidates.length > 0"
          class="space-y-2"
        >
          <div class="text-xs font-medium text-muted-foreground">
            最近
          </div>
          <div class="space-y-2">
            <div
              v-for="candidate in liveRecentCandidates"
              :key="`${candidate.id}-${candidate.status}`"
              class="flex items-start justify-between gap-3 rounded-md border border-border/50 bg-background/70 px-3 py-2 text-xs"
            >
              <div class="min-w-0 space-y-1">
                <div class="flex min-w-0 items-center gap-2">
                  <span class="shrink-0 text-muted-foreground">{{ formatTraceCandidateIndex(candidate) }}</span>
                  <Badge
                    :variant="statusVariant(candidate.status)"
                    class="shrink-0 px-1.5 py-0 text-[10px]"
                  >
                    {{ statusDisplay(candidate) }}
                  </Badge>
                  <span class="truncate font-medium">{{ formatTraceCandidateAccount(candidate) }}</span>
                  <span
                    v-if="traceCandidateMappingName(candidate)"
                    class="shrink-0 text-muted-foreground"
                  >
                    · {{ traceCandidateMappingName(candidate) }}
                  </span>
                </div>
                <div
                  v-if="traceCandidateDetail(candidate)"
                  class="break-all text-muted-foreground"
                >
                  {{ traceCandidateDetail(candidate) }}
                </div>
              </div>
              <div class="shrink-0 tabular-nums text-muted-foreground">
                {{ candidate.latency_ms != null ? `${candidate.latency_ms}ms` : '' }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div
      v-else-if="result"
      class="space-y-4"
    >
      <div class="rounded-lg border border-border/60 bg-muted/20 p-4">
        <div class="flex min-w-0 flex-wrap items-center gap-2">
          <Badge
            :variant="result.success ? 'success' : 'destructive'"
            class="px-2 py-0.5"
          >
            {{ result.success ? '测试成功' : '测试失败' }}
          </Badge>
          <span
            v-if="resultSummaryText"
            class="min-w-0 break-all text-xs text-muted-foreground"
          >
            {{ resultSummaryText }}
          </span>
        </div>
      </div>

      <div
        v-if="shouldCollapseAttempts"
        class="flex items-center justify-between gap-3 text-xs text-muted-foreground"
      >
        <span>仅展示前 {{ visibleAttempts.length }} 条，共 {{ resultAttempts.length }} 条</span>
        <Button
          variant="ghost"
          size="sm"
          @click="showAllAttempts = !showAllAttempts"
        >
          {{ showAllAttempts ? '收起详情' : `展开全部 ${resultAttempts.length} 条` }}
        </Button>
      </div>

      <div
        v-if="resultAttempts.length > 0"
        class="max-h-[360px] space-y-2 overflow-y-auto pr-1 sm:hidden"
      >
        <div
          v-for="(attempt, idx) in visibleAttempts"
          :key="'m' + idx"
          class="rounded-md border px-3 py-2 text-xs"
          :class="attemptRowClass(attempt.status, selectedInspectionKey === inspectionKey(attempt))"
          role="button"
          tabindex="0"
          @click="toggleInspectionAttempt(attempt)"
          @keydown.enter.prevent="toggleInspectionAttempt(attempt)"
          @keydown.space.prevent="toggleInspectionAttempt(attempt)"
        >
          <div class="flex items-center justify-between gap-2">
            <div class="flex min-w-0 items-center gap-1.5">
              <span class="shrink-0 text-muted-foreground">{{ formatAttemptIndex(attempt) }}</span>
              <Badge
                :variant="statusVariant(attempt.status)"
                class="shrink-0 px-1.5 py-0 text-[10px]"
              >
                {{ statusDisplay(attempt) }}
              </Badge>
              <span
                v-if="attempt.latency_ms != null"
                class="shrink-0 tabular-nums text-muted-foreground"
              >
                {{ attempt.latency_ms }}ms
              </span>
            </div>
          </div>
          <div class="mt-1.5 space-y-0.5">
            <div class="truncate font-medium">
              {{ attempt.key_name || maskKey(attempt.key_id) }}
            </div>
            <div class="truncate text-muted-foreground">
              {{ attempt.mapping_name || attempt.effective_model || '-' }}
            </div>
            <div
              v-if="attemptImagePreviews(attempt).length > 0"
              class="mt-2 flex flex-wrap gap-2"
            >
              <button
                v-for="(preview, imageIndex) in attemptImagePreviews(attempt).slice(0, 3)"
                :key="`${preview.src}-${imageIndex}`"
                type="button"
                class="h-16 w-16 overflow-hidden rounded-md border border-border/60 bg-muted/30 transition-colors hover:border-primary/60"
                :title="preview.label"
                @click.stop="openImagePreview(preview)"
              >
                <img
                  :src="preview.src"
                  :alt="preview.label"
                  class="h-full w-full object-contain"
                  loading="lazy"
                >
              </button>
            </div>
            <div
              v-else-if="attemptDetail(attempt) !== '-'"
              class="mt-1 break-words text-muted-foreground"
            >
              {{ attemptDetail(attempt) }}
            </div>
          </div>
        </div>
      </div>

      <div
        v-if="resultAttempts.length > 0"
        class="hidden max-h-[360px] overflow-y-auto rounded-md border sm:block"
      >
        <table class="w-full table-fixed text-xs">
          <colgroup>
            <col class="w-10">
            <col class="w-[30%]">
            <col class="w-[26%]">
            <col class="w-16">
            <col class="w-20">
            <col>
          </colgroup>
          <thead class="sticky top-0 z-10">
            <tr class="border-b bg-muted/30">
              <th class="py-2 pl-3 pr-1 text-left font-medium">
                #
              </th>
              <th class="px-3 py-2 text-left font-medium">
                Key
              </th>
              <th class="px-3 py-2 text-left font-medium">
                模型
              </th>
              <th class="px-3 py-2 text-left font-medium">
                状态
              </th>
              <th class="px-3 py-2 text-right font-medium">
                耗时
              </th>
              <th class="px-3 py-2 text-left font-medium">
                详情
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(attempt, idx) in visibleAttempts"
              :key="idx"
              class="last:border-b-0 align-top border-b"
              :class="attemptRowClass(attempt.status, selectedInspectionKey === inspectionKey(attempt))"
              tabindex="0"
              @click="toggleInspectionAttempt(attempt)"
              @keydown.enter.prevent="toggleInspectionAttempt(attempt)"
              @keydown.space.prevent="toggleInspectionAttempt(attempt)"
            >
              <td class="py-2 pl-3 pr-1 text-muted-foreground">
                {{ formatAttemptIndex(attempt) }}
              </td>
              <td class="px-3 py-2">
                <div
                  class="truncate font-medium"
                  :title="attempt.key_name || attempt.key_id"
                >
                  {{ attempt.key_name || maskKey(attempt.key_id) }}
                </div>
              </td>
              <td class="px-3 py-2">
                <div
                  class="truncate text-muted-foreground"
                  :title="attempt.mapping_name || attempt.effective_model || '-'"
                >
                  {{ attempt.mapping_name || attempt.effective_model || '-' }}
                </div>
              </td>
              <td class="px-3 py-2">
                <Badge
                  :variant="statusVariant(attempt.status)"
                  class="px-1.5 py-0 text-[10px]"
                >
                  {{ statusDisplay(attempt) }}
                </Badge>
              </td>
              <td class="whitespace-nowrap px-3 py-2 text-right tabular-nums text-muted-foreground">
                {{ attempt.latency_ms != null ? attempt.latency_ms + 'ms' : '-' }}
              </td>
              <td class="px-3 py-2 text-muted-foreground">
                <div
                  v-if="attemptImagePreviews(attempt).length > 0"
                  class="flex flex-wrap gap-2"
                >
                  <button
                    v-for="(preview, imageIndex) in attemptImagePreviews(attempt).slice(0, 4)"
                    :key="`${preview.src}-${imageIndex}`"
                    type="button"
                    class="h-14 w-14 overflow-hidden rounded-md border border-border/60 bg-muted/30 transition-colors hover:border-primary/60 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/70"
                    :title="preview.label"
                    @click.stop="openImagePreview(preview)"
                  >
                    <img
                      :src="preview.src"
                      :alt="preview.label"
                      class="h-full w-full object-contain"
                      loading="lazy"
                    >
                  </button>
                  <span
                    v-if="attemptImagePreviews(attempt).length > 4"
                    class="flex h-14 items-center rounded-md border border-border/60 px-2 text-xs text-muted-foreground"
                  >
                    +{{ attemptImagePreviews(attempt).length - 4 }}
                  </span>
                </div>
                <div
                  v-else
                  class="line-clamp-2 break-words"
                  :title="attemptDetail(attempt)"
                >
                  {{ attemptDetail(attempt) }}
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div
        v-else-if="resultAttempts.length === 0"
        class="py-4 text-center text-sm text-muted-foreground"
      >
        {{ resultEmptyMessage }}
      </div>

      <div
        v-if="showDebugInspector"
        class="space-y-3"
      >
        <div
          v-if="selectedInspectionAttempt"
          class="space-y-3"
        >
          <div
            class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
          >
            <span class="font-medium text-foreground">
              {{ formatAttemptIndex(selectedInspectionAttempt) }}
            </span>
            <span>{{ selectedInspectionAttempt.key_name || maskKey(selectedInspectionAttempt.key_id) }}</span>
            <span>·</span>
            <span>{{ formatApiFormat(selectedInspectionAttempt.endpoint_api_format) }}</span>
          </div>

          <Card>
            <div class="p-3 sm:p-4">
              <Tabs
                v-model="inspectionTab"
                :default-value="inspectionTab"
              >
                <div class="flex items-center border-b pb-2 mb-3">
                  <button
                    v-for="tab in detailTabs"
                    :key="tab.name"
                    class="px-2 sm:px-3 py-1.5 text-sm transition-colors border-b-2 -mb-[9px] whitespace-nowrap"
                    :class="inspectionTab === tab.name
                      ? 'border-primary text-foreground font-medium'
                      : 'border-transparent text-muted-foreground hover:text-foreground'"
                    @click="inspectionTab = tab.name"
                  >
                    {{ tab.label }}
                  </button>
                </div>

                <div class="content-block rounded-md border overflow-hidden">
                  <div class="flex items-center justify-end gap-0.5 px-3 py-1 border-b bg-muted/40">
                    <button
                      :title="inspectionExpandDepth === 0 ? '展开全部' : '收缩全部'"
                      class="p-1 rounded transition-colors text-muted-foreground hover:bg-muted"
                      @click="inspectionExpandDepth === 0 ? expandInspectionContent() : collapseInspectionContent()"
                    >
                      <Maximize2
                        v-if="inspectionExpandDepth === 0"
                        class="w-3.5 h-3.5"
                      />
                      <Minimize2
                        v-else
                        class="w-3.5 h-3.5"
                      />
                    </button>

                    <button
                      :title="inspectionCopiedStates[inspectionTab] ? '已复制' : '复制'"
                      class="p-1 rounded transition-colors text-muted-foreground hover:bg-muted"
                      @click="copyInspectionContent(inspectionTab)"
                    >
                      <Check
                        v-if="inspectionCopiedStates[inspectionTab]"
                        class="w-3.5 h-3.5 text-green-500"
                      />
                      <Copy
                        v-else
                        class="w-3.5 h-3.5"
                      />
                    </button>
                  </div>

                  <TabsContent value="request-headers">
                    <JsonContent
                      :data="selectedInspectionAttempt.request_headers"
                      view-mode="formatted"
                      :expand-depth="inspectionExpandDepth"
                      :is-dark="isDark"
                      empty-message="无请求头数据"
                    />
                  </TabsContent>

                  <TabsContent value="request-body">
                    <JsonContent
                      :data="asJsonDataValue(selectedInspectionAttempt.request_body)"
                      view-mode="formatted"
                      :expand-depth="inspectionExpandDepth"
                      :is-dark="isDark"
                      empty-message="无请求体数据"
                    />
                  </TabsContent>

                  <TabsContent value="response-headers">
                    <JsonContent
                      :data="selectedInspectionAttempt.response_headers"
                      view-mode="formatted"
                      :expand-depth="inspectionExpandDepth"
                      :is-dark="isDark"
                      empty-message="无响应头数据"
                    />
                  </TabsContent>

                  <TabsContent value="response-body">
                    <JsonContent
                      :data="asJsonDataValue(selectedInspectionAttempt.response_body)"
                      view-mode="formatted"
                      :expand-depth="inspectionExpandDepth"
                      :is-dark="isDark"
                      empty-message="无响应体数据"
                    />
                  </TabsContent>
                </div>
              </Tabs>
            </div>
          </Card>
        </div>
      </div>
    </div>

    <template #footer>
      <Button
        variant="outline"
        @click="emit('close')"
      >
        {{ showSetup ? '取消' : '关闭' }}
      </Button>
      <Button
        v-if="showResult"
        variant="outline"
        @click="emit('back')"
      >
        返回
      </Button>
    </template>
  </Dialog>

  <Dialog
    :open="Boolean(activeImagePreview)"
    size="6xl"
    :z-index="120"
    @update:open="(val: boolean) => { if (!val) activeImagePreview = null }"
  >
    <template #header>
      <div class="border-b border-border px-6 py-4">
        <div class="text-lg font-semibold text-foreground leading-tight">
          {{ activeImagePreview?.label || '图片预览' }}
        </div>
      </div>
    </template>

    <div class="dialog-image-preview rounded-md bg-muted/20 p-3">
      <img
        v-if="activeImagePreview"
        :src="activeImagePreview.src"
        :alt="activeImagePreview.label"
        class="min-h-0 max-h-full max-w-full rounded-md object-contain"
      >
    </div>

    <template #footer>
      <Button
        variant="outline"
        @click="activeImagePreview = null"
      >
        关闭
      </Button>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Check, ChevronRight, Code2, Copy, Loader2, Maximize2, Minimize2, RotateCcw } from 'lucide-vue-next'
import {
  Badge,
  Card,
  Dialog,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Tabs,
  TabsContent,
} from '@/components/ui'
import Button from '@/components/ui/button.vue'
import Textarea from '@/components/ui/textarea.vue'
import { formatApiFormat } from '@/api/endpoints/types/api-format'
import type { TestAttemptDetail, TestModelFailoverResponse } from '@/api/endpoints/providers'
import type { CandidateRecord, RequestTrace } from '@/api/requestTrace'
import JsonContent from '@/features/usage/components/RequestDetailDrawer/JsonContent.vue'
import { useClipboard } from '@/composables/useClipboard'
import { useDarkMode } from '@/composables/useDarkMode'
import {
  extractModelTestImagePreviews,
  extractModelTestResponsePreview,
  formatModelTestDiagnostic,
} from './model-test-request'
import type { ModelTestImagePreview } from './model-test-request'

type TestEndpointOption = {
  id: string
  api_format: string
  base_url: string
  is_active: boolean
}

type TestModelMappingOption = {
  name: string
  priority?: number
}

type TestKeyOption = {
  value: string
  label: string
}

const props = defineProps<{
  open: boolean
  result: TestModelFailoverResponse | null
  selectingModelName?: string | null
  endpoints?: TestEndpointOption[]
  selectedEndpoint?: TestEndpointOption | null
  testing?: boolean
  trace?: RequestTrace | null
  requestId?: string | null
  requestHeadersDraft?: string
  requestHeadersResetValue?: string
  requestHeadersError?: string | null
  requestBodyDraft?: string
  requestBodyResetValue?: string
  requestBodyError?: string | null
  modelMappingOptions?: TestModelMappingOption[]
  selectedModelMapping?: string | null
  keyOptions?: TestKeyOption[]
  selectedKeyId?: string
  keyOptionsLoading?: boolean
  startDisabled?: boolean
}>()

const emit = defineEmits<{
  close: []
  back: []
  start: []
  selectEndpoint: [endpointId: string]
  selectModelMapping: [modelName: string]
  'update:selectedKeyId': [value: string]
  'update:requestHeadersDraft': [value: string]
  'update:requestBodyDraft': [value: string]
}>()

const endpoints = computed(() => props.endpoints ?? [])
const modelMappingOptions = computed(() => props.modelMappingOptions ?? [])
const keyOptions = computed(() => props.keyOptions ?? [])
const keyOptionsLoading = computed(() => props.keyOptionsLoading === true)
const selectedModelMapping = computed(() => props.selectedModelMapping?.trim() || '')
const selectedModelMappingValue = computed(() => selectedModelMapping.value || 'auto')
const selectedKeyValue = computed(() => {
  const selected = props.selectedKeyId?.trim() || ''
  return selected && selected !== 'auto' ? selected : 'auto'
})
const requestHeadersExpanded = ref(false)
const requestBodyExpanded = ref(false)
const requestHeadersDraft = computed(() => props.requestHeadersDraft ?? '')
const requestBodyDraft = computed(() => props.requestBodyDraft ?? '')
const traceCandidates = computed(() => props.trace?.candidates ?? [])
const showSetup = computed(() => props.open && !props.testing && !props.result)
const showResult = computed(() => !!props.result)
const { isDark } = useDarkMode()
const { copyToClipboard } = useClipboard()

function handleModelMappingValueChange(value: string) {
  emit('selectModelMapping', value === 'auto' ? '' : value)
}

function handleKeyValueChange(value: string) {
  emit('update:selectedKeyId', value === 'auto' ? 'auto' : value)
}

const dialogTitle = computed(() => {
  if (props.result) return '模型测试结果'
  return '模型测试'
})

const dialogDescription = computed(() => {
  if (showSetup.value && props.selectingModelName) {
    return `为 ${props.selectingModelName} 选择端点并编辑测试请求头与请求体`
  }
  if (props.testing && props.selectedEndpoint) {
    return `正在通过 ${formatApiFormat(props.selectedEndpoint.api_format)} 测试 ${props.selectingModelName || '模型'}`
  }
  return ''
})

const resultAttempts = computed(() => props.result?.attempts ?? [])

const resultEmptyMessage = computed(() => {
  if (!props.result) return '没有可用的候选进行测试'
  if (typeof props.result.error === 'string' && props.result.error.trim()) {
    return formatModelTestDiagnostic(props.result.error)
  }
  const rawResult = props.result as TestModelFailoverResponse & { message?: string }
  if (typeof rawResult.message === 'string' && rawResult.message.trim()) {
    return formatModelTestDiagnostic(rawResult.message)
  }
  return '没有可用的候选进行测试'
})
const showAllAttempts = ref(false)
const inspectionTab = ref<'request-headers' | 'request-body' | 'response-headers' | 'response-body'>('request-body')
const selectedInspectionKey = ref<string | null>(null)
const inspectionExpandDepth = ref(0)
const inspectionCopiedStates = ref<Record<string, boolean>>({})
const activeImagePreview = ref<ModelTestImagePreview | null>(null)

watch(() => props.result, () => {
  showAllAttempts.value = false
  inspectionExpandDepth.value = 0
  inspectionCopiedStates.value = {}
  selectedInspectionKey.value = null
  inspectionTab.value = 'request-body'
})

const shouldCollapseAttempts = computed(() => resultAttempts.value.length > 20)

const visibleAttempts = computed(() => {
  if (!shouldCollapseAttempts.value || showAllAttempts.value) {
    return resultAttempts.value
  }
  return resultAttempts.value.slice(0, 20)
})

const liveTraceSummary = computed(() => {
  const summary = {
    total_candidates: traceCandidates.value.length,
    pending: 0,
    success: 0,
    failed: 0,
    skipped: 0,
    unused: 0,
    completed: 0,
    stop_reason: traceCandidates.value.length > 0 ? 'pending' : 'no_candidate',
  }

  for (const candidate of traceCandidates.value) {
    if (candidate.status === 'unused') summary.unused += 1
    if (candidate.status === 'pending' || candidate.status === 'streaming') summary.pending += 1
    if (candidate.status === 'success') summary.success += 1
    if (candidate.status === 'failed' || candidate.status === 'cancelled' || candidate.status === 'stream_interrupted') summary.failed += 1
    if (candidate.status === 'skipped') summary.skipped += 1
  }

  summary.completed = summary.success + summary.failed + summary.skipped + summary.unused
  if (summary.success > 0) summary.stop_reason = 'first_success'
  if (summary.success === 0 && summary.pending === 0 && summary.failed + summary.skipped > 0) summary.stop_reason = 'exhausted'
  return summary
})

const liveProgressText = computed(() => {
  if (liveTraceSummary.value.total_candidates <= 0) return '准备中'
  return `${liveTraceSummary.value.completed}/${liveTraceSummary.value.total_candidates}`
})

const liveProgressPercent = computed(() => {
  if (liveTraceSummary.value.total_candidates <= 0) return 8
  const raw = Math.round((liveTraceSummary.value.completed / liveTraceSummary.value.total_candidates) * 100)
  return Math.min(100, Math.max(raw, liveTraceSummary.value.pending > 0 ? 12 : 6))
})

const activeTraceCandidate = computed(() => {
  const preferredStatuses = ['pending', 'streaming', 'failed', 'success', 'skipped', 'cancelled']
  for (let index = traceCandidates.value.length - 1; index >= 0; index -= 1) {
    const candidate = traceCandidates.value[index]
    if (preferredStatuses.includes(candidate.status)) return candidate
  }
  return traceCandidates.value[0] ?? null
})

const liveRecentCandidates = computed(() => {
  return traceCandidates.value
    .filter(candidate => candidate.status !== 'available')
    .slice(-4)
    .reverse()
})

const resultSummaryText = computed(() => {
  const winner = resultAttempts.value.find(attempt => attempt.status === 'success')
  if (winner) {
    const parts = [
      winner.key_name || maskKey(winner.key_id),
      winner.mapping_name || winner.effective_model,
      winner.status_code != null ? `HTTP ${winner.status_code}` : '',
      winner.latency_ms != null ? `${winner.latency_ms}ms` : '',
    ].filter(Boolean)
    return parts.join(' · ')
  }
  return typeof props.result?.error === 'string' && props.result.error.trim()
    ? formatModelTestDiagnostic(props.result.error)
    : ''
})

const inspectableAttempts = computed(() => {
  return resultAttempts.value.filter(hasDebugData)
})

const showDebugInspector = computed(() => {
  return selectedInspectionKey.value !== null && inspectableAttempts.value.length > 0
})

const detailTabs = [
  { name: 'request-headers', label: '请求头' },
  { name: 'request-body', label: '请求体' },
  { name: 'response-headers', label: '响应头' },
  { name: 'response-body', label: '响应体' },
] as const

const selectedInspectionAttempt = computed(() => {
  const key = selectedInspectionKey.value
  if (key) {
    const fromKey = resultAttempts.value.find(attempt => inspectionKey(attempt) === key)
    if (fromKey) return fromKey
  }

  return inspectableAttempts.value[0] ?? resultAttempts.value[0] ?? null
})

// JsonContent 的 data 类型宽于接口返回值，这里做显式归一化
type JsonDataValue = Record<string, unknown> | unknown[] | string | number | boolean | null | undefined
function asJsonDataValue(value: unknown): JsonDataValue {
  return value as JsonDataValue
}


function statusVariant(status: string) {
  if (status === 'success') return 'success' as const
  if (status === 'failed' || status === 'stream_interrupted') return 'destructive' as const
  return 'secondary' as const
}

function statusLabel(status: string) {
  if (status === 'success') return '成功'
  if (status === 'failed') return '失败'
  if (status === 'skipped') return '跳过'
  if (status === 'pending') return '等待中'
  if (status === 'streaming') return '测试中'
  if (status === 'cancelled') return '已取消'
  if (status === 'stream_interrupted') return '流中断'
  if (status === 'available') return '待请求'
  if (status === 'unused') return '未使用'
  return status
}

function statusDisplay(item: { status: string; status_code?: number | null }): string {
  const code = item.status_code
  const status = item.status
  if (!code) return statusLabel(status)
  if (status === 'failed' && code >= 200 && code < 300) {
    return `${code} 体内错误`
  }
  return String(code)
}

function attemptRowClass(status: string, selected = false) {
  const statusClass = (() => {
    if (status === 'success') return 'bg-green-500/5'
    if (status === 'failed') return 'bg-red-500/5'
    if (status === 'cancelled') return 'bg-amber-500/5'
    if (status === 'skipped') return 'bg-muted/20'
    return ''
  })()
  const interactionClass = 'cursor-pointer transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/70'
  const selectedClass = selected ? 'ring-1 ring-primary/70 bg-primary/5' : ''
  return [statusClass, interactionClass, selectedClass].filter(Boolean).join(' ')
}

function maskKey(key: string): string {
  if (key.length <= 8) return key
  return `${key.slice(0, 4)}...${key.slice(-4)}`
}

function formatAttemptIndex(attempt: TestAttemptDetail): string {
  const retryIndex = attempt.retry_index ?? 0
  const candidateIndex = attempt.candidate_index + 1
  return retryIndex > 0 ? `#${candidateIndex}.${retryIndex}` : `#${candidateIndex}`
}

function formatTraceCandidateIndex(candidate: CandidateRecord): string {
  const candidateIndex = candidate.candidate_index + 1
  return candidate.retry_index > 0 ? `#${candidateIndex}.${candidate.retry_index}` : `#${candidateIndex}`
}

function formatTraceCandidateAccount(candidate: CandidateRecord): string {
  return candidate.key_account_label || candidate.key_name || candidate.key_preview || '-'
}

function traceCandidateMappingName(candidate: CandidateRecord): string {
  const adminData = candidate.extra_data?.admin_model_test
  if (!adminData || typeof adminData !== 'object' || Array.isArray(adminData)) return ''
  const value = (adminData as Record<string, unknown>).mapping_name
    ?? (adminData as Record<string, unknown>).effective_model
  return typeof value === 'string' ? value.trim() : ''
}

function traceCandidateDetail(candidate: CandidateRecord): string {
  if (candidate.skip_reason) return formatModelTestDiagnostic(candidate.skip_reason)
  if (candidate.error_message) return formatModelTestDiagnostic(candidate.error_message)
  if (candidate.status === 'unused') return '成功后未请求'
  if (candidate.status === 'available') return '待请求'
  if (candidate.endpoint_name) return `端点：${formatApiFormat(candidate.endpoint_name)}`
  return ''
}

function attemptDetail(attempt: TestAttemptDetail): string {
  if (attempt.status === 'cancelled') return '测试已取消'
  if (attempt.status === 'unused') return '-'
  if (attempt.skip_reason) return formatModelTestDiagnostic(attempt.skip_reason)
  if (attempt.error_message) return formatModelTestDiagnostic(attempt.error_message)
  if (attempt.status === 'success') {
    return extractModelTestResponsePreview(attempt.response_body)
      ?? (attempt.effective_model ? `请求模型：${attempt.effective_model}` : attempt.endpoint_base_url)
  }
  return '-'
}

function attemptImagePreviews(attempt: TestAttemptDetail): ModelTestImagePreview[] {
  return extractModelTestImagePreviews(attempt.response_body)
}

function openImagePreview(preview: ModelTestImagePreview) {
  activeImagePreview.value = preview
}

function inspectionKey(attempt: TestAttemptDetail): string {
  return `${attempt.candidate_index}:${attempt.retry_index ?? 0}:${attempt.key_id}`
}

function toggleInspectionAttempt(attempt: TestAttemptDetail) {
  const nextKey = inspectionKey(attempt)
  if (selectedInspectionKey.value === nextKey) {
    selectedInspectionKey.value = null
    return
  }
  selectedInspectionKey.value = nextKey
  inspectionTab.value = attemptImagePreviews(attempt).length > 0 ? 'response-body' : 'request-body'
}

function hasDebugData(attempt: TestAttemptDetail): boolean {
  return Boolean(
    attempt.request_url
    || attempt.request_headers
    || attempt.request_body != null
    || attempt.response_headers
    || attempt.response_body != null,
  )
}

function getInspectionTabData(
  tabName: typeof detailTabs[number]['name'],
  attempt: TestAttemptDetail | null,
): unknown {
  if (!attempt) return null
  switch (tabName) {
    case 'request-headers':
      return attempt.request_headers
    case 'request-body':
      return attempt.request_body
    case 'response-headers':
      return attempt.response_headers
    case 'response-body':
      return attempt.response_body
  }
}

function copyInspectionContent(tabName: typeof detailTabs[number]['name']) {
  const data = getInspectionTabData(tabName, selectedInspectionAttempt.value)
  if (data === null || data === undefined || data === '') return

  const text = typeof data === 'string' ? data : JSON.stringify(data, null, 2)
  copyToClipboard(text, false)
  inspectionCopiedStates.value[tabName] = true
  setTimeout(() => {
    inspectionCopiedStates.value[tabName] = false
  }, 2000)
}

function expandInspectionContent() {
  inspectionExpandDepth.value = 999
}

function collapseInspectionContent() {
  inspectionExpandDepth.value = 0
}

function formatRequestHeadersDraft() {
  formatJsonDraft(requestHeadersDraft.value, value => emit('update:requestHeadersDraft', value), '{}')
}

function formatRequestBodyDraft() {
  formatJsonDraft(requestBodyDraft.value, value => emit('update:requestBodyDraft', value))
}

function resetRequestHeadersDraft() {
  emit('update:requestHeadersDraft', props.requestHeadersResetValue ?? '{}')
}

function resetRequestBodyDraft() {
  emit('update:requestBodyDraft', props.requestBodyResetValue ?? '')
}

function formatJsonDraft(
  draft: string,
  onFormatted: (value: string) => void,
  emptyFallback?: string,
) {
  const normalized = draft.trim()
  if (!normalized) {
    if (emptyFallback !== undefined) {
      onFormatted(emptyFallback)
    }
    return
  }

  try {
    const parsed = JSON.parse(normalized)
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return
    onFormatted(JSON.stringify(parsed, null, 2))
  } catch {
    // keep user input untouched when JSON is invalid
  }
}
</script>

<style scoped>
.content-block :deep(.rounded-2xl) {
  border: none !important;
  border-radius: 0 !important;
  box-shadow: none !important;
}
</style>
