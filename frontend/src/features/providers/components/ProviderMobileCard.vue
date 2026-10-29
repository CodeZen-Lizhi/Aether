<template>
  <div
    class="p-4 space-y-3 hover:bg-muted/20 transition-colors cursor-pointer"
    @click="$emit('viewDetail', provider.id)"
  >
    <!-- 第一行：名称 + 状态 + 操作 -->
    <div class="flex items-start justify-between gap-3">
      <div class="flex-1 min-w-0 space-y-0.5">
        <div class="flex items-center gap-1.5">
          <span class="font-medium text-foreground truncate">{{ provider.name }}</span>
          <a
            v-if="provider.website"
            :href="provider.website"
            target="_blank"
            rel="noopener noreferrer"
            class="text-muted-foreground hover:text-primary transition-colors shrink-0"
            :title="provider.website"
            @click.stop
          >
            <ExternalLink class="w-3.5 h-3.5" />
          </a>
          <Badge
            :variant="provider.is_active ? 'success' : 'secondary'"
            class="text-xs shrink-0"
          >
            {{ legacyT(provider.is_active ? '活跃' : '停用') }}
          </Badge>
        </div>
      </div>
      <div
        class="flex items-center gap-0.5 shrink-0"
        @click.stop
      >
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7"
          :title="legacyT('编辑')"
          @click="$emit('editProvider', provider)"
        >
          <Edit class="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7"
          :title="legacyT('用户认证')"
          data-testid="provider-open-auth"
          @click="$emit('openOpsConfig', provider)"
        >
          <KeyRound class="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7"
          @click="$emit('toggleStatus', provider)"
        >
          <Power class="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7"
          @click="$emit('deleteProvider', provider)"
        >
          <Trash2 class="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>

    <!-- 第二行：计费类型 + 余额/配额 + 资源统计 -->
    <div class="flex flex-wrap items-center gap-3 text-xs">
      <ProviderBalanceCell
        :provider="provider"
        :is-balance-loading="isBalanceLoading"
        :get-provider-balance="getProviderBalance"
        :get-provider-balance-breakdown="getProviderBalanceBreakdown"
        :get-provider-balance-error="getProviderBalanceError"
        :get-provider-checkin="getProviderCheckin"
        :get-provider-balance-extra="getProviderBalanceExtra"
        :format-balance-display="formatBalanceDisplay"
        :format-reset-countdown="formatResetCountdown"
        :get-quota-used-color-class="getQuotaUsedColorClass"
        class="min-w-0 max-w-full"
      />
      <Badge
        variant="outline"
        class="text-xs font-normal border-border/50"
      >
        {{ formatBillingType(provider.billing_type || 'pay_as_you_go') }}
      </Badge>
      <span class="text-muted-foreground">
        {{ legacyT('端点') }} {{ provider.active_endpoints }}/{{ provider.total_endpoints }}
      </span>
      <span class="text-muted-foreground">
        {{ getCredentialLabel(provider) }} {{ provider.active_keys }}/{{ provider.total_keys }}
      </span>
      <span class="text-muted-foreground">
        {{ legacyT('模型') }} {{ provider.active_models }}/{{ provider.total_models }}
      </span>
    </div>

    <!-- 第三行：端点健康 -->
    <div
      v-if="provider.endpoint_health_details && provider.endpoint_health_details.length > 0"
      class="grid grid-cols-3 gap-x-3 gap-y-2 max-w-[240px]"
    >
      <div
        v-for="endpoint in sortEndpoints(provider.endpoint_health_details)"
        :key="endpoint.api_format"
        class="flex flex-col gap-1.5"
        :title="getEndpointTooltip(endpoint, locale)"
      >
        <!-- 上排：缩写 + 百分比 -->
        <div class="flex items-center justify-between text-[10px] leading-none">
          <span class="font-medium text-muted-foreground/80">
            {{ formatApiFormatShort(endpoint.api_format) }}
          </span>
          <span class="font-medium text-muted-foreground/80">
            {{ isEndpointAvailable(endpoint) ? `${(endpoint.health_score * 100).toFixed(0)}%` : '-' }}
          </span>
        </div>

        <!-- 下排：进度条 -->
        <div class="h-1.5 w-full bg-border dark:bg-border/80 rounded-full overflow-hidden">
          <div
            class="h-full rounded-full transition-all duration-300"
            :class="getEndpointDotColor(endpoint)"
            :style="{ width: isEndpointAvailable(endpoint) ? `${Math.max(endpoint.health_score * 100, 5)}%` : '100%' }"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  Edit,
  Trash2,
  Power,
  KeyRound,
  ExternalLink,
} from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'
import Badge from '@/components/ui/badge.vue'
import { type ProviderWithEndpointsSummary, formatApiFormatShort } from '@/api/endpoints'
import { formatBillingType } from '@/utils/format'
import ProviderBalanceCell from './ProviderBalanceCell.vue'
import type { BalanceExtraItem } from '@/features/providers/auth-templates'
import { sortEndpoints, isEndpointAvailable, getEndpointDotColor, getEndpointTooltip } from '@/features/providers/composables/useEndpointStatus'
import { useI18n } from '@/i18n'

defineProps<{
  provider: ProviderWithEndpointsSummary
  isBalanceLoading: (providerId: string) => boolean
  getProviderBalance: (providerId: string) => { available: number | null; currency: string } | null
  getProviderBalanceBreakdown: (providerId: string) => { balance: number; points: number; currency: string } | null
  getProviderBalanceError: (providerId: string) => { status: string; message: string } | null
  getProviderCheckin: (providerId: string) => { success: boolean | null; message: string } | null
  getProviderBalanceExtra: (providerId: string, architectureId?: string) => BalanceExtraItem[]
  formatBalanceDisplay: (balance: { available: number | null; currency: string } | null) => string
  formatResetCountdown: (resetsAt: number) => string
  getQuotaUsedColorClass: (provider: ProviderWithEndpointsSummary) => string
}>()

defineEmits<{
  'viewDetail': [providerId: string]
  'editProvider': [provider: ProviderWithEndpointsSummary]
  'openOpsConfig': [provider: ProviderWithEndpointsSummary]
  'toggleStatus': [provider: ProviderWithEndpointsSummary]
  'deleteProvider': [provider: ProviderWithEndpointsSummary]
}>()

const { legacyT, locale } = useI18n()

function getCredentialLabel(_provider: ProviderWithEndpointsSummary): string {
  return legacyT('密钥')
}
</script>
