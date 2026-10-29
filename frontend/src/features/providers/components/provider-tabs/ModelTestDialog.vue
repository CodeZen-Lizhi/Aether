<template>
  <!-- 模型测试统一工作台：左侧固定测试范围，右侧持续承载进度、结果与诊断详情。 -->
  <Dialog
    :open="open"
    size="5xl"
    :close-on-backdrop="false"
    :no-padding="true"
    opaque-backdrop
    @update:open="(val: boolean) => { if (!val) emit('close') }"
  >
    <template #header>
      <div class="model-test-header px-4 py-4 sm:px-6">
        <div class="flex items-center justify-between gap-3">
          <div class="flex min-w-0 items-center gap-3">
            <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-[0_10px_24px_rgba(37,99,235,0.22)]">
              <FlaskConical class="h-[18px] w-[18px]" />
            </div>
            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <h3 class="text-lg font-semibold leading-tight tracking-tight text-foreground">
                  模型测试
                </h3>
                <span class="hidden rounded-full border border-primary/15 bg-primary/[0.06] px-2 py-0.5 text-[10px] font-semibold tracking-wide text-primary sm:inline-flex">
                  连通诊断
                </span>
              </div>
              <p class="mt-1 truncate text-xs leading-5 text-muted-foreground">
                {{ dialogDescription }}
              </p>
            </div>
          </div>

          <div
            class="model-test-state"
            :data-state="recordState"
            role="status"
            aria-live="polite"
          >
            <span class="model-test-state-dot" />
            <span>{{ recordStateLabel }}</span>
            <span
              v-if="testing"
              class="border-l border-current/20 pl-2 font-mono text-[10px] opacity-70"
            >
              {{ liveProgressText }}
            </span>
          </div>
        </div>
      </div>
    </template>

    <div class="model-test-body">
      <div class="model-test-workbench">
        <aside
          class="model-test-panel config-panel"
          aria-labelledby="model-test-config-title"
        >
          <div class="panel-heading">
            <div class="min-w-0">
              <div class="panel-eyebrow">
                <SlidersHorizontal class="h-3 w-3" />
                测试范围
              </div>
              <h4
                id="model-test-config-title"
                class="panel-title"
              >
                测试配置
              </h4>
              <p
                class="panel-description truncate"
                :title="configDescription"
              >
                {{ configDescription }}
              </p>
            </div>
            <button
              type="button"
              class="panel-toggle"
              :aria-expanded="configExpanded"
              aria-controls="model-test-config-content"
              :aria-label="configExpanded ? '收起测试配置' : '展开测试配置'"
              @click="toggleConfig"
            >
              <ChevronDown
                class="h-4 w-4 transition-transform duration-200"
                :class="{ '-rotate-90': !configExpanded }"
              />
            </button>
          </div>

          <div
            v-show="configExpanded"
            id="model-test-config-content"
            class="config-content"
          >
            <div
              v-if="endpoints.length > 0"
              class="config-field"
            >
              <div class="field-label-row">
                <span class="field-label">测试端点</span>
                <span class="field-meta">{{ endpoints.length }} 个可用</span>
              </div>
              <div class="space-y-2">
                <button
                  v-for="endpoint in endpoints"
                  :key="endpoint.id"
                  type="button"
                  class="endpoint-option"
                  :class="{ 'is-selected': selectedEndpoint?.id === endpoint.id }"
                  :disabled="controlsDisabled"
                  @click="emit('selectEndpoint', endpoint.id)"
                >
                  <span class="endpoint-radio">
                    <span />
                  </span>
                  <span class="min-w-0 flex-1">
                    <span class="flex items-center gap-2">
                      <span class="truncate text-[13px] font-semibold text-foreground">
                        {{ formatApiFormat(endpoint.api_format) }}
                      </span>
                      <span
                        class="h-1.5 w-1.5 shrink-0 rounded-full"
                        :class="endpoint.is_active ? 'bg-emerald-500' : 'bg-muted-foreground/40'"
                      />
                    </span>
                    <span class="mt-1 block truncate font-mono text-[10px] text-muted-foreground">
                      {{ endpoint.base_url }}
                    </span>
                  </span>
                </button>
              </div>
            </div>

            <div
              v-if="endpoints.length > 0"
              class="config-field-grid"
            >
              <div class="config-field">
                <div class="field-label-row">
                  <span class="field-label">测试 Key</span>
                  <span
                    v-if="keyOptionsLoading"
                    class="field-meta"
                  >加载中</span>
                </div>
                <Select
                  :model-value="selectedKeyValue"
                  :disabled="controlsDisabled || (keyOptionsLoading && keyOptions.length === 0)"
                  @update:model-value="handleKeyValueChange"
                >
                  <SelectTrigger class="h-10 w-full rounded-xl border-border/70 bg-background text-xs shadow-none">
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

              <div class="config-field">
                <div class="field-label-row">
                  <span class="field-label">模型映射</span>
                </div>
                <Select
                  :model-value="selectedModelMappingValue"
                  :disabled="controlsDisabled"
                  @update:model-value="handleModelMappingValueChange"
                >
                  <SelectTrigger class="h-10 w-full rounded-xl border-border/70 bg-background text-xs shadow-none">
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

            <div class="config-field config-request-group">
              <div class="field-label-row">
                <span class="field-label">请求内容</span>
                <span class="field-meta">可选覆盖</span>
              </div>

              <div class="request-editor">
                <div class="request-editor-head">
                  <button
                    type="button"
                    class="request-editor-toggle"
                    :aria-expanded="requestHeadersExpanded"
                    aria-controls="model-test-request-headers"
                    @click="requestHeadersExpanded = !requestHeadersExpanded"
                  >
                    <ChevronRight
                      class="h-3.5 w-3.5 shrink-0 transition-transform duration-200"
                      :class="{ 'rotate-90': requestHeadersExpanded }"
                    />
                    <Code2 class="h-3.5 w-3.5 text-primary/70" />
                    <span>测试请求头</span>
                  </button>
                  <div
                    v-if="requestHeadersExpanded"
                    class="flex shrink-0 items-center gap-0.5"
                  >
                    <button
                      type="button"
                      class="editor-icon-button"
                      title="格式化请求头 JSON"
                      aria-label="格式化请求头 JSON"
                      :disabled="controlsDisabled"
                      @click="formatRequestHeadersDraft"
                    >
                      <Braces class="h-3.5 w-3.5" />
                    </button>
                    <button
                      type="button"
                      class="editor-icon-button"
                      title="重置请求头"
                      aria-label="重置请求头"
                      :disabled="controlsDisabled"
                      @click="resetRequestHeadersDraft"
                    >
                      <RotateCcw class="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
                <div
                  v-show="requestHeadersExpanded"
                  id="model-test-request-headers"
                  class="request-editor-content"
                >
                  <Textarea
                    :model-value="requestHeadersDraft"
                    class="h-[140px] min-h-[140px] font-mono text-[11px] leading-5"
                    :disabled="controlsDisabled"
                    placeholder="输入 JSON 请求头"
                    @update:model-value="emit('update:requestHeadersDraft', $event)"
                  />
                  <div
                    v-if="requestHeadersError"
                    class="field-error"
                  >
                    {{ requestHeadersError }}
                  </div>
                </div>
              </div>

              <div class="request-editor">
                <div class="request-editor-head">
                  <button
                    type="button"
                    class="request-editor-toggle"
                    :aria-expanded="requestBodyExpanded"
                    aria-controls="model-test-request-body"
                    @click="requestBodyExpanded = !requestBodyExpanded"
                  >
                    <ChevronRight
                      class="h-3.5 w-3.5 shrink-0 transition-transform duration-200"
                      :class="{ 'rotate-90': requestBodyExpanded }"
                    />
                    <Code2 class="h-3.5 w-3.5 text-primary/70" />
                    <span>测试请求体</span>
                  </button>
                  <div
                    v-if="requestBodyExpanded"
                    class="flex shrink-0 items-center gap-0.5"
                  >
                    <button
                      type="button"
                      class="editor-icon-button"
                      title="格式化请求体 JSON"
                      aria-label="格式化请求体 JSON"
                      :disabled="controlsDisabled"
                      @click="formatRequestBodyDraft"
                    >
                      <Braces class="h-3.5 w-3.5" />
                    </button>
                    <button
                      type="button"
                      class="editor-icon-button"
                      title="重置请求体"
                      aria-label="重置请求体"
                      :disabled="controlsDisabled"
                      @click="resetRequestBodyDraft"
                    >
                      <RotateCcw class="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
                <div
                  v-show="requestBodyExpanded"
                  id="model-test-request-body"
                  class="request-editor-content"
                >
                  <Textarea
                    :model-value="requestBodyDraft"
                    class="h-[180px] min-h-[180px] font-mono text-[11px] leading-5"
                    :disabled="controlsDisabled"
                    placeholder="输入 JSON 请求体"
                    @update:model-value="emit('update:requestBodyDraft', $event)"
                  />
                  <div
                    v-if="requestBodyError"
                    class="field-error"
                  >
                    {{ requestBodyError }}
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div
            v-if="!configExpanded"
            class="config-summary"
          >
            <div class="config-summary-row">
              <Server class="h-3.5 w-3.5 text-primary/70" />
              <span>端点</span>
              <strong>{{ selectedEndpointLabel }}</strong>
            </div>
            <div class="config-summary-row">
              <KeyRound class="h-3.5 w-3.5 text-primary/70" />
              <span>Key</span>
              <strong>{{ selectedKeyLabel }}</strong>
            </div>
            <div class="config-summary-row">
              <Layers3 class="h-3.5 w-3.5 text-primary/70" />
              <span>映射</span>
              <strong>{{ selectedMappingLabel }}</strong>
            </div>
            <p class="config-summary-note">
              {{ controlsDisabled ? '测试期间范围已锁定。' : '展开配置可调整本次测试范围。' }}
            </p>
          </div>
        </aside>

        <section
          class="model-test-panel record-panel"
          aria-labelledby="model-test-records-title"
        >
          <div class="panel-heading record-heading">
            <div class="min-w-0">
              <div class="panel-eyebrow">
                <Activity class="h-3 w-3" />
                执行与结果
              </div>
              <h4
                id="model-test-records-title"
                class="panel-title"
              >
                测试记录
              </h4>
              <p class="panel-description">
                {{ recordDescription }}
              </p>
            </div>
            <button
              v-if="requestId"
              type="button"
              class="request-id-chip"
              :title="`复制请求 ID：${requestId}`"
              @click="copyRequestId"
            >
              <span>请求 ID</span>
              <code>{{ requestId }}</code>
              <Copy class="h-3 w-3 shrink-0" />
            </button>
          </div>

          <div class="record-stage">
            <Transition
              name="record-swap"
              mode="out-in"
            >
              <div
                :key="recordState"
                class="record-state"
              >
                <div
                  v-if="testing || (stopped && trace)"
                  class="space-y-4"
                >
                  <div
                    class="run-card"
                    :class="{ 'is-stopped': stopped }"
                    role="status"
                    aria-live="polite"
                  >
                    <div class="flex items-start gap-3">
                      <div
                        class="run-icon"
                        :class="{ 'is-stopped': stopped }"
                      >
                        <Square
                          v-if="stopped"
                          class="h-4 w-4"
                        />
                        <Loader2
                          v-else
                          class="h-4 w-4 animate-spin"
                        />
                      </div>
                      <div class="min-w-0 flex-1">
                        <div class="flex flex-wrap items-start justify-between gap-3">
                          <div class="min-w-0">
                            <p class="text-sm font-semibold text-foreground">
                              {{ stopped ? '候选测试已停止' : '正在验证候选链路' }}
                            </p>
                            <p class="mt-1 truncate text-xs text-muted-foreground">
                              {{ selectedEndpoint ? `${formatApiFormat(selectedEndpoint.api_format)} · ${selectedEndpoint.base_url}` : '正在准备测试请求' }}
                            </p>
                          </div>
                          <div class="progress-fraction">
                            {{ liveProgressText }}
                          </div>
                        </div>

                        <div class="mt-3">
                          <div
                            class="progress-track"
                            :class="{ 'is-stopped': stopped }"
                            role="progressbar"
                            aria-label="模型测试进度"
                            aria-valuemin="0"
                            aria-valuemax="100"
                            :aria-valuenow="liveProgressPercent"
                          >
                            <div
                              class="progress-fill"
                              :class="{ 'is-stopped': stopped }"
                              :style="{ width: `${liveProgressPercent}%` }"
                            />
                          </div>
                        </div>

                        <div class="active-candidate">
                          <span class="active-label">
                            {{ stopped ? '停止于' : '当前' }}
                          </span>
                          <template v-if="activeTraceCandidate">
                            <span class="font-mono text-[10px] text-muted-foreground">
                              {{ formatTraceCandidateIndex(activeTraceCandidate) }}
                            </span>
                            <span class="truncate font-medium text-foreground">
                              {{ formatTraceCandidateAccount(activeTraceCandidate) }}
                            </span>
                            <span
                              v-if="traceCandidateMappingName(activeTraceCandidate)"
                              class="truncate text-muted-foreground"
                            >
                              → {{ traceCandidateMappingName(activeTraceCandidate) }}
                            </span>
                          </template>
                          <span
                            v-else
                            class="text-muted-foreground"
                          >
                            正在获取候选记录
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  <div v-if="liveRecentCandidates.length > 0">
                    <div class="section-heading">
                      <div>
                        <p class="section-title">
                          执行轨迹
                        </p>
                        <p class="section-subtitle">
                          最近 {{ liveRecentCandidates.length }} 个候选状态
                        </p>
                      </div>
                      <span class="live-indicator">
                        <span :class="stopped ? 'bg-amber-500' : 'bg-primary'" />
                        {{ stopped ? '已停止' : '实时更新' }}
                      </span>
                    </div>

                    <div class="execution-timeline">
                      <div
                        v-for="candidate in liveRecentCandidates"
                        :key="`${candidate.id}-${candidate.status}`"
                        class="timeline-item"
                      >
                        <div class="timeline-rail">
                          <span
                            class="timeline-dot"
                            :class="traceStatusDotClass(candidate.status, stopped)"
                          />
                        </div>
                        <div class="min-w-0 flex-1 pb-4">
                          <div class="flex flex-wrap items-center gap-2">
                            <span class="font-mono text-[10px] text-muted-foreground">
                              {{ formatTraceCandidateIndex(candidate) }}
                            </span>
                            <Badge
                              :variant="statusVariant(candidate.status)"
                              class="h-5 px-1.5 py-0 text-[10px]"
                            >
                              {{ statusDisplay(candidate, stopped) }}
                            </Badge>
                            <span class="truncate text-xs font-semibold text-foreground">
                              {{ formatTraceCandidateAccount(candidate) }}
                            </span>
                            <span
                              v-if="traceCandidateMappingName(candidate)"
                              class="truncate text-[11px] text-muted-foreground"
                            >
                              → {{ traceCandidateMappingName(candidate) }}
                            </span>
                            <span
                              v-if="candidate.latency_ms != null"
                              class="ml-auto font-mono text-[11px] font-medium text-sky-700 dark:text-sky-300"
                            >
                              {{ candidate.latency_ms }}ms
                            </span>
                          </div>
                          <p
                            v-if="traceCandidateDetail(candidate)"
                            class="mt-1 line-clamp-2 break-all text-[11px] leading-5 text-muted-foreground"
                          >
                            {{ traceCandidateDetail(candidate) }}
                          </p>
                        </div>
                      </div>
                    </div>
                  </div>

                  <div
                    v-else
                    class="mini-empty"
                  >
                    正在等待第一条候选记录…
                  </div>
                </div>

                <div
                  v-else-if="result"
                  class="space-y-4"
                >
                  <div
                    class="result-hero"
                    :class="result.success ? 'is-success' : 'is-failure'"
                  >
                    <div class="flex min-w-0 flex-1 items-start gap-3">
                      <div class="result-icon">
                        <CheckCircle2
                          v-if="result.success"
                          class="h-5 w-5"
                        />
                        <XCircle
                          v-else
                          class="h-5 w-5"
                        />
                      </div>
                      <div class="min-w-0 flex-1">
                        <p class="result-eyebrow">
                          {{ result.success ? '链路验证通过' : '候选均未通过' }}
                        </p>
                        <h5 class="mt-1 text-xl font-semibold tracking-tight text-foreground">
                          {{ result.success ? '模型测试成功' : '模型测试失败' }}
                        </h5>
                        <div
                          v-if="resultPrimaryAttempt"
                          class="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs"
                        >
                          <span class="font-semibold text-foreground">
                            {{ resultPrimaryAttempt.key_name || maskKey(resultPrimaryAttempt.key_id) }}
                          </span>
                          <span class="text-muted-foreground">·</span>
                          <span class="truncate text-muted-foreground">
                            {{ resultPrimaryAttempt.mapping_name || resultPrimaryAttempt.effective_model || result.model }}
                          </span>
                        </div>
                      </div>
                    </div>

                    <div class="result-metrics">
                      <div class="result-metric">
                        <span>HTTP</span>
                        <strong>{{ resultHttpText }}</strong>
                      </div>
                      <div class="result-metric">
                        <span>耗时</span>
                        <strong>{{ resultLatencyText }}</strong>
                      </div>
                    </div>

                    <div
                      v-if="!result.success && resultSummaryText"
                      class="result-error"
                    >
                      <TriangleAlert class="mt-0.5 h-4 w-4 shrink-0" />
                      <span>{{ resultSummaryText }}</span>
                    </div>
                  </div>

                  <div>
                    <div class="section-heading">
                      <div>
                        <p class="section-title">
                          执行明细
                        </p>
                        <p class="section-subtitle">
                          点击任意记录查看请求头和响应详情
                        </p>
                      </div>
                      <Button
                        v-if="shouldCollapseAttempts"
                        variant="ghost"
                        size="sm"
                        class="h-8 rounded-lg text-[11px]"
                        @click="showAllAttempts = !showAllAttempts"
                      >
                        {{ showAllAttempts ? '收起记录' : `展开全部 ${resultAttempts.length} 条` }}
                      </Button>
                    </div>

                    <div
                      v-if="resultAttempts.length > 0"
                      class="space-y-2.5 sm:hidden"
                    >
                      <div
                        v-for="(attempt, idx) in visibleAttempts"
                        :key="'m' + idx"
                        class="attempt-mobile"
                        :class="attemptRowClass(attempt.status, selectedInspectionKey === inspectionKey(attempt))"
                        :data-selected="selectedInspectionKey === inspectionKey(attempt) ? 'true' : 'false'"
                        role="button"
                        tabindex="0"
                        @click="toggleInspectionAttempt(attempt)"
                        @keydown.enter.prevent="toggleInspectionAttempt(attempt)"
                        @keydown.space.prevent="toggleInspectionAttempt(attempt)"
                      >
                        <div class="flex items-center justify-between gap-2">
                          <div class="flex min-w-0 items-center gap-2">
                            <span class="font-mono text-[10px] text-muted-foreground">
                              {{ formatAttemptIndex(attempt) }}
                            </span>
                            <Badge
                              :variant="statusVariant(attempt.status)"
                              class="h-5 px-1.5 py-0 text-[10px]"
                            >
                              {{ statusDisplay(attempt) }}
                            </Badge>
                            <span
                              v-if="attempt.latency_ms != null"
                              class="font-mono text-[11px] font-medium text-sky-700 dark:text-sky-300"
                            >
                              {{ attempt.latency_ms }}ms
                            </span>
                          </div>
                          <ChevronRight class="h-3.5 w-3.5 shrink-0 text-muted-foreground/60" />
                        </div>
                        <div class="mt-2 truncate text-xs font-semibold text-foreground">
                          {{ attempt.key_name || maskKey(attempt.key_id) }}
                        </div>
                        <div class="mt-0.5 truncate text-[11px] text-muted-foreground">
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
                            class="h-16 w-16 overflow-hidden rounded-lg border border-border/70 bg-muted/30 transition-colors hover:border-primary/60"
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
                        <p
                          v-else-if="attemptDetail(attempt) !== '-'"
                          class="mt-2 line-clamp-2 break-words text-[11px] leading-5 text-muted-foreground"
                        >
                          {{ attemptDetail(attempt) }}
                        </p>
                      </div>
                    </div>

                    <div
                      v-if="resultAttempts.length > 0"
                      class="result-table-wrap hidden sm:block"
                    >
                      <table class="w-full table-fixed text-xs">
                        <colgroup>
                          <col class="w-9">
                          <col class="w-[24%]">
                          <col class="w-[23%]">
                          <col class="w-16">
                          <col class="w-16">
                          <col>
                        </colgroup>
                        <thead>
                          <tr>
                            <th
                              scope="col"
                              class="text-left"
                            >
                              #
                            </th>
                            <th
                              scope="col"
                              class="text-left"
                            >
                              Key
                            </th>
                            <th
                              scope="col"
                              class="text-left"
                            >
                              模型
                            </th>
                            <th
                              scope="col"
                              class="text-left"
                            >
                              状态
                            </th>
                            <th
                              scope="col"
                              class="text-left"
                            >
                              耗时
                            </th>
                            <th
                              scope="col"
                              class="text-left"
                            >
                              详情
                            </th>
                          </tr>
                        </thead>
                        <tbody>
                          <tr
                            v-for="(attempt, idx) in visibleAttempts"
                            :key="idx"
                            class="attempt-row"
                            :class="attemptRowClass(attempt.status, selectedInspectionKey === inspectionKey(attempt))"
                            :data-status="attempt.status"
                            tabindex="0"
                            @click="toggleInspectionAttempt(attempt)"
                            @keydown.enter.prevent="toggleInspectionAttempt(attempt)"
                            @keydown.space.prevent="toggleInspectionAttempt(attempt)"
                          >
                            <td class="text-muted-foreground">
                              <span class="font-mono text-[10px]">{{ formatAttemptIndex(attempt) }}</span>
                            </td>
                            <td>
                              <div
                                class="truncate font-semibold text-foreground"
                                :title="attempt.key_name || attempt.key_id"
                              >
                                {{ attempt.key_name || maskKey(attempt.key_id) }}
                              </div>
                            </td>
                            <td>
                              <div
                                class="truncate text-muted-foreground"
                                :title="attempt.mapping_name || attempt.effective_model || '-'"
                              >
                                {{ attempt.mapping_name || attempt.effective_model || '-' }}
                              </div>
                            </td>
                            <td>
                              <Badge
                                :variant="statusVariant(attempt.status)"
                                class="h-5 px-1.5 py-0 text-[10px]"
                              >
                                {{ statusDisplay(attempt) }}
                              </Badge>
                            </td>
                            <td class="whitespace-nowrap">
                              <span class="font-mono text-[11px] font-medium text-sky-700 dark:text-sky-300">
                                {{ attempt.latency_ms != null ? attempt.latency_ms + 'ms' : '-' }}
                              </span>
                            </td>
                            <td class="text-muted-foreground">
                              <div
                                v-if="attemptImagePreviews(attempt).length > 0"
                                class="flex flex-wrap gap-2"
                              >
                                <button
                                  v-for="(preview, imageIndex) in attemptImagePreviews(attempt).slice(0, 4)"
                                  :key="`${preview.src}-${imageIndex}`"
                                  type="button"
                                  class="h-12 w-12 overflow-hidden rounded-lg border border-border/60 bg-muted/30 transition-colors hover:border-primary/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50"
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
                                  class="flex h-12 items-center rounded-lg border border-border/60 px-2 text-[10px] text-muted-foreground"
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
                      v-if="resultAttempts.length === 0"
                      class="mini-empty"
                    >
                      {{ resultEmptyMessage }}
                    </div>
                  </div>

                  <div
                    v-if="showDebugInspector && selectedInspectionAttempt"
                    id="model-test-inspector"
                    class="inspector"
                  >
                    <div class="inspector-heading">
                      <div class="min-w-0">
                        <div class="panel-eyebrow">
                          <ScanLine class="h-3 w-3" />
                          诊断详情
                        </div>
                        <div class="mt-1 flex flex-wrap items-center gap-2 text-xs">
                          <span class="font-mono font-semibold text-foreground">
                            {{ formatAttemptIndex(selectedInspectionAttempt) }}
                          </span>
                          <span class="font-medium text-foreground">
                            {{ selectedInspectionAttempt.key_name || maskKey(selectedInspectionAttempt.key_id) }}
                          </span>
                          <span class="text-muted-foreground">·</span>
                          <span class="text-muted-foreground">
                            {{ formatApiFormat(selectedInspectionAttempt.endpoint_api_format) }}
                          </span>
                        </div>
                      </div>
                      <Badge
                        variant="outline"
                        class="shrink-0"
                      >
                        已选中
                      </Badge>
                    </div>

                    <Card class="overflow-hidden rounded-xl border-border/70 shadow-none">
                      <Tabs
                        v-model="inspectionTab"
                        :default-value="inspectionTab"
                      >
                        <div class="inspector-tabs">
                          <button
                            v-for="tab in detailTabs"
                            :key="tab.name"
                            type="button"
                            class="inspector-tab"
                            :class="{ 'is-active': inspectionTab === tab.name }"
                            :aria-selected="inspectionTab === tab.name"
                            role="tab"
                            @click="inspectionTab = tab.name"
                          >
                            {{ tab.label }}
                          </button>
                        </div>

                        <div class="content-block overflow-hidden">
                          <div class="inspector-toolbar">
                            <span class="text-[10px] text-muted-foreground">JSON</span>
                            <div class="flex items-center gap-0.5">
                              <button
                                type="button"
                                class="editor-icon-button"
                                :title="inspectionExpandDepth === 0 ? '展开全部' : '收缩全部'"
                                :aria-label="inspectionExpandDepth === 0 ? '展开全部' : '收缩全部'"
                                @click="inspectionExpandDepth === 0 ? expandInspectionContent() : collapseInspectionContent()"
                              >
                                <Maximize2
                                  v-if="inspectionExpandDepth === 0"
                                  class="h-3.5 w-3.5"
                                />
                                <Minimize2
                                  v-else
                                  class="h-3.5 w-3.5"
                                />
                              </button>
                              <button
                                type="button"
                                class="editor-icon-button"
                                :title="inspectionCopiedStates[inspectionTab] ? '已复制' : '复制'"
                                :aria-label="inspectionCopiedStates[inspectionTab] ? '已复制' : '复制'"
                                @click="copyInspectionContent(inspectionTab)"
                              >
                                <Check
                                  v-if="inspectionCopiedStates[inspectionTab]"
                                  class="h-3.5 w-3.5 text-emerald-500"
                                />
                                <Copy
                                  v-else
                                  class="h-3.5 w-3.5"
                                />
                              </button>
                            </div>
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
                    </Card>
                  </div>
                </div>

                <div
                  v-else
                  class="idle-stage"
                  :class="{ 'is-stopped': stopped }"
                >
                  <div class="idle-visual">
                    <div class="idle-node">
                      <Server class="h-4 w-4" />
                      <span>端点</span>
                    </div>
                    <span class="idle-connector">
                      <ArrowRight class="h-4 w-4" />
                    </span>
                    <div class="idle-node">
                      <KeyRound class="h-4 w-4" />
                      <span>Key</span>
                    </div>
                    <span class="idle-connector">
                      <ArrowRight class="h-4 w-4" />
                    </span>
                    <div class="idle-node">
                      <Layers3 class="h-4 w-4" />
                      <span>映射</span>
                    </div>
                  </div>

                  <div class="mt-5 max-w-md text-center">
                    <div class="idle-icon">
                      <Square
                        v-if="stopped"
                        class="h-4 w-4"
                      />
                      <FlaskConical
                        v-else
                        class="h-5 w-5"
                      />
                    </div>
                    <h5 class="mt-3 text-base font-semibold text-foreground">
                      {{ stopped ? '测试已停止' : '等待开始测试' }}
                    </h5>
                    <p class="mt-1 text-xs leading-5 text-muted-foreground">
                      {{ stopped
                        ? '当前没有取得候选执行记录，可以调整配置后重新开始。'
                        : '开始后会按当前范围逐条验证候选，进度、失败原因和响应详情都会保留在这里。' }}
                    </p>
                  </div>

                  <div class="idle-note">
                    <Sparkles class="h-3.5 w-3.5 shrink-0" />
                    自动模式会按映射优先级和 Key 顺序执行，成功后停止。
                  </div>
                </div>
              </div>
            </Transition>
          </div>
        </section>
      </div>
    </div>

    <template #footer>
      <Button
        v-if="testing"
        variant="outline"
        class="border-amber-500/40 text-amber-700 hover:border-amber-500/60 hover:bg-amber-500/10 hover:text-amber-800 dark:text-amber-300 dark:hover:text-amber-200"
        @click="emit('cancel')"
      >
        <Square class="mr-2 h-3.5 w-3.5" />
        停止测试
      </Button>
      <Button
        v-else
        :disabled="startDisabled"
        @click="emit('start')"
      >
        <RotateCcw
          v-if="result"
          class="mr-2 h-4 w-4"
        />
        <Play
          v-else
          class="mr-2 h-4 w-4"
        />
        {{ result ? '重新测试' : '开始测试' }}
      </Button>
      <Button
        variant="outline"
        @click="emit('close')"
      >
        关闭
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
        <div class="text-lg font-semibold leading-tight text-foreground">
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
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import {
  Activity,
  ArrowRight,
  Braces,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Code2,
  Copy,
  FlaskConical,
  KeyRound,
  Layers3,
  Loader2,
  Maximize2,
  Minimize2,
  Play,
  RotateCcw,
  ScanLine,
  Server,
  SlidersHorizontal,
  Sparkles,
  Square,
  TriangleAlert,
  XCircle,
} from 'lucide-vue-next'
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
  /** 当前测试是否由用户主动停止；为 true 时保留已有追踪记录。 */
  stopped?: boolean
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
  cancel: []
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
const configExpanded = ref(true)
const requestHeadersDraft = computed(() => props.requestHeadersDraft ?? '')
const requestBodyDraft = computed(() => props.requestBodyDraft ?? '')
const traceCandidates = computed(() => props.trace?.candidates ?? [])
const controlsDisabled = computed(() => props.testing === true)
const { isDark } = useDarkMode()
const { copyToClipboard } = useClipboard()

function handleModelMappingValueChange(value: string) {
  emit('selectModelMapping', value === 'auto' ? '' : value)
}

function handleKeyValueChange(value: string) {
  emit('update:selectedKeyId', value === 'auto' ? 'auto' : value)
}

/** 展开或收起测试配置，保留同一弹窗内的记录可见性。 */
function toggleConfig() {
  configExpanded.value = !configExpanded.value
}

const dialogDescription = computed(() => {
  return props.selectingModelName
    ? `验证 ${props.selectingModelName} 在真实端点、Key 与映射组合下的可用性`
    : '验证真实候选链路的连通性、耗时与响应质量'
})

const selectedEndpointLabel = computed(() => (
  props.selectedEndpoint
    ? formatApiFormat(props.selectedEndpoint.api_format)
    : '未选择端点'
))

const selectedKeyLabel = computed(() => (
  selectedKeyValue.value === 'auto'
    ? '自动 Key'
    : (keyOptions.value.find(option => option.value === selectedKeyValue.value)?.label || '指定 Key')
))

const selectedMappingLabel = computed(() => (
  selectedModelMappingValue.value === 'auto'
    ? '自动映射'
    : selectedModelMappingValue.value
))

const configDescription = computed(() => {
  if (configExpanded.value) return '选择端点、Key 和映射；测试过程中配置会锁定。'
  return `${selectedEndpointLabel.value} · ${selectedKeyLabel.value} · ${selectedMappingLabel.value}`
})

const recordState = computed(() => {
  if (props.testing) return 'testing'
  if (props.result) return props.result.success ? 'success' : 'failure'
  if (props.stopped) return 'stopped'
  return 'idle'
})

const recordStateLabel = computed(() => ({
  idle: '待开始',
  testing: '测试中',
  success: '测试成功',
  failure: '测试失败',
  stopped: '已停止',
})[recordState.value])

const recordDescription = computed(() => {
  if (props.testing) return '候选正在按优先级执行，进度与最近结果会实时更新。'
  if (props.result) return '查看最终结论、候选执行顺序与完整请求响应。'
  if (props.stopped) return '测试已中止，已经取得的部分记录仍会保留。'
  return '一次完整验证，集中呈现候选进度、失败原因与响应质量。'
})

const resultAttempts = computed(() => props.result?.attempts ?? [])

const resultPrimaryAttempt = computed(() => {
  return resultAttempts.value.find(attempt => attempt.status === 'success')
    ?? resultAttempts.value.find(attempt => attempt.status === 'failed')
    ?? resultAttempts.value[0]
    ?? null
})

const resultHttpText = computed(() => {
  const attempt = resultPrimaryAttempt.value
  if (!attempt) return props.result?.success ? 'OK' : '-'
  return attempt.status_code != null ? String(attempt.status_code) : statusDisplay(attempt)
})

const resultLatencyText = computed(() => {
  const latency = resultPrimaryAttempt.value?.latency_ms
  return latency != null ? `${latency}ms` : '-'
})

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
let inspectionScrollTimer: ReturnType<typeof setTimeout> | null = null

watch(() => props.result, () => {
  showAllAttempts.value = false
  inspectionExpandDepth.value = 0
  inspectionCopiedStates.value = {}
  selectedInspectionKey.value = null
  inspectionTab.value = 'request-body'
})

watch(
  [
    () => props.open,
    () => props.testing,
    () => props.result,
    () => props.stopped,
    () => props.selectingModelName,
  ],
  ([open, testing, result, stopped]) => {
    if (!open || testing || result || stopped) return
    configExpanded.value = true
  },
  { immediate: true },
)

watch(
  [() => props.testing, () => props.result],
  ([testing, result]) => {
    if (testing || result) {
      configExpanded.value = false
    }
  },
  { immediate: true },
)

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
  if (status === 'success') return 'success-semantic' as const
  if (status === 'failed' || status === 'stream_interrupted') return 'destructive' as const
  if (status === 'cancelled') return 'warning' as const
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

function statusDisplay(
  item: { status: string; status_code?: number | null },
  forceStopped = false,
): string {
  if (forceStopped && ['pending', 'streaming', 'available'].includes(item.status)) {
    return '已停止'
  }
  const code = item.status_code
  const status = item.status
  if (!code) return statusLabel(status)
  if (status === 'failed' && code >= 200 && code < 300) {
    return `${code} 体内错误`
  }
  return String(code)
}

/** 为实时执行轨迹返回与结果状态一致的圆点颜色。 */
function traceStatusDotClass(status: string, forceStopped = false): string {
  if (forceStopped && ['pending', 'streaming', 'available'].includes(status)) {
    return 'bg-amber-500'
  }
  if (status === 'success') return 'bg-emerald-500'
  if (status === 'failed' || status === 'stream_interrupted') return 'bg-red-500'
  if (status === 'cancelled') return 'bg-amber-500'
  if (status === 'streaming' || status === 'pending') return 'animate-pulse bg-primary'
  return 'bg-muted-foreground/35'
}

function attemptRowClass(status: string, selected = false) {
  const statusClass = (() => {
    if (status === 'success') return 'bg-emerald-500/[0.035]'
    if (status === 'failed' || status === 'stream_interrupted') return 'bg-red-500/[0.035]'
    if (status === 'cancelled') return 'bg-amber-500/[0.04]'
    if (status === 'skipped' || status === 'unused') return 'bg-muted/20'
    return ''
  })()
  const interactionClass = 'cursor-pointer transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-primary/50'
  const selectedClass = selected ? 'bg-primary/[0.055] shadow-[inset_3px_0_0_hsl(var(--primary))]' : ''
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
  if (inspectionScrollTimer) clearTimeout(inspectionScrollTimer)
  inspectionScrollTimer = setTimeout(() => {
    const inspector = document.getElementById('model-test-inspector')
    const scroller = inspector?.closest('.dialog-body')
    if (!(inspector instanceof HTMLElement) || !(scroller instanceof HTMLElement)) return
    const targetTop = inspector.getBoundingClientRect().top
      - scroller.getBoundingClientRect().top
      + scroller.scrollTop
      - 8
    scroller.scrollTop = Math.max(0, targetTop)
    inspectionScrollTimer = null
  }, 50)
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

/** 复制当前测试请求 ID，便于在请求追踪页快速定位。 */
async function copyRequestId() {
  if (!props.requestId) return
  await copyToClipboard(props.requestId)
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

onBeforeUnmount(() => {
  if (inspectionScrollTimer) clearTimeout(inspectionScrollTimer)
})
</script>

<style scoped src="./ModelTestDialog.css"></style>
