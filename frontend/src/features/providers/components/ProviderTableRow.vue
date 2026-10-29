<template>
  <TableRow
    class="border-b border-border/30 hover:bg-muted/20 transition-colors cursor-pointer"
    @mousedown="$emit('mousedown', $event)"
    @click="$emit('rowClick', $event, provider.id)"
  >
    <TableCell class="py-3.5">
      <div class="space-y-0.5">
        <div class="flex items-center gap-1.5">
          <span class="min-w-0 text-sm font-medium text-foreground [overflow-wrap:anywhere]">{{ provider.name }}</span>
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
        </div>
      </div>
    </TableCell>
    <TableCell class="py-3.5">
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
      />
    </TableCell>
    <TableCell class="py-3.5 text-center">
      <div class="grid w-fit grid-cols-[auto_auto] justify-start gap-x-1.5 gap-y-0.5 text-left text-xs">
        <span class="text-muted-foreground/70">{{ legacyT('端点:') }}</span>
        <span class="tabular-nums text-right"><span class="font-medium text-foreground/90">{{ provider.active_endpoints }}</span><span class="text-muted-foreground/50"> /{{ provider.total_endpoints }}</span></span>

        <span class="text-muted-foreground/70">{{ `${getCredentialLabel(provider)}:` }}</span>
        <span class="tabular-nums text-right"><span class="font-medium text-foreground/90">{{ provider.active_keys }}</span><span class="text-muted-foreground/50"> /{{ provider.total_keys }}</span></span>

        <span class="text-muted-foreground/70">{{ legacyT('模型:') }}</span>
        <span class="tabular-nums text-right"><span class="font-medium text-foreground/90">{{ provider.active_models }}</span><span class="text-muted-foreground/50"> /{{ provider.total_models }}</span></span>
      </div>
    </TableCell>
    <TableCell class="py-3.5 align-middle">
      <div
        v-if="provider.endpoint_health_details && provider.endpoint_health_details.length > 0"
        class="grid grid-cols-[repeat(auto-fit,minmax(min(100%,3.75rem),1fr))] gap-x-3 gap-y-2 max-w-[240px]"
      >
        <div
          v-for="endpoint in sortEndpoints(provider.endpoint_health_details)"
          :key="endpoint.api_format"
          class="flex flex-col gap-1.5"
          :title="getEndpointTooltip(endpoint, locale)"
        >
          <!-- 上排：缩写 + 百分比 -->
          <div class="flex items-center justify-between gap-1 whitespace-nowrap text-[10px] leading-none">
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
      <span
        v-else
        class="text-xs text-muted-foreground/50"
      >{{ legacyT('暂无端点') }}</span>
    </TableCell>
    <TableCell class="py-3.5 text-center">
      <Badge
        :variant="provider.is_active ? 'success' : 'secondary'"
        class="px-1.5 text-xs"
      >
        {{ legacyT(provider.is_active ? '活跃' : '停用') }}
      </Badge>
    </TableCell>
    <TableCell
      class="py-3.5"
      @click.stop
    >
      <div class="flex items-center justify-center gap-0.5 [&>button]:shrink-0">
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7 text-muted-foreground/70 hover:text-foreground"
          :title="legacyT('编辑提供商')"
          @click="$emit('editProvider', provider)"
        >
          <Edit class="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7 text-muted-foreground/70 hover:text-foreground"
          :title="legacyT('用户认证')"
          data-testid="provider-open-auth"
          @click="$emit('openOpsConfig', provider)"
        >
          <KeyRound class="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7 text-muted-foreground/70 hover:text-foreground"
          :title="legacyT(provider.is_active ? '停用提供商' : '启用提供商')"
          @click="$emit('toggleStatus', provider)"
        >
          <Power class="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7 text-muted-foreground/70 hover:text-destructive"
          :title="legacyT('删除提供商')"
          @click="$emit('deleteProvider', provider)"
        >
          <Trash2 class="h-3.5 w-3.5" />
        </Button>
      </div>
    </TableCell>
  </TableRow>
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
import TableRow from '@/components/ui/table-row.vue'
import TableCell from '@/components/ui/table-cell.vue'
import ProviderBalanceCell from './ProviderBalanceCell.vue'
import { type ProviderWithEndpointsSummary, formatApiFormatShort } from '@/api/endpoints'
import { sortEndpoints, isEndpointAvailable, getEndpointDotColor, getEndpointTooltip } from '@/features/providers/composables/useEndpointStatus'
import type { BalanceExtraItem } from '@/features/providers/auth-templates'
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
  'mousedown': [event: MouseEvent]
  'rowClick': [event: MouseEvent, providerId: string]
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
