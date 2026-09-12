<template>
  <div class="px-4 py-3 sm:py-3.5 border-b border-border/50">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <!-- 左侧：标题 -->
      <h3 class="text-sm sm:text-base font-semibold text-foreground shrink-0">
        {{ legacyT('提供商管理') }}
      </h3>

      <!-- 右侧：操作区 -->
      <div class="provider-toolbar flex flex-1 flex-wrap items-center justify-end gap-2">
        <!-- 搜索框 -->
        <div class="provider-toolbar__search relative">
          <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground/70 z-10 pointer-events-none" />
          <Input
            id="provider-search"
            :model-value="searchQuery"
            type="text"
            :placeholder="legacyT('搜索提供商...')"
            class="w-full pl-8 pr-3 h-8 text-sm bg-muted/30 border-border/50 focus:border-primary/50 transition-colors"
            @update:model-value="$emit('update:searchQuery', $event)"
          />
        </div>

        <!-- 状态筛选 -->
        <div class="provider-toolbar__filter">
          <Select
            :model-value="filterStatus"
            @update:model-value="$emit('update:filterStatus', $event)"
          >
            <SelectTrigger class="w-full px-2.5 h-8 text-xs border-border/60">
              <SelectValue :placeholder="legacyT('全部状态')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="status in statusFilters"
                :key="status.value"
                :value="status.value"
              >
                {{ legacyT(status.label) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <!-- API 格式筛选 -->
        <div class="provider-toolbar__filter">
          <Select
            :model-value="filterApiFormat"
            @update:model-value="$emit('update:filterApiFormat', $event)"
          >
            <SelectTrigger class="w-full px-2.5 h-8 text-xs border-border/60">
              <SelectValue :placeholder="legacyT('全部格式')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="fmt in apiFormatFilters"
                :key="fmt.value"
                :value="fmt.value"
              >
                {{ legacyT(fmt.label) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <!-- 模型筛选 -->
        <div class="provider-toolbar__filter provider-toolbar__filter--model">
          <Select
            :model-value="filterModel"
            @update:model-value="$emit('update:filterModel', $event)"
          >
            <SelectTrigger class="w-full px-2.5 h-8 text-xs border-border/60">
              <SelectValue :placeholder="legacyT('全部模型')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="model in modelFilters"
                :key="model.value"
                :value="model.value"
              >
                {{ legacyT(model.label) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <!-- 重置筛选 -->
        <Button
          v-if="hasActiveFilters"
          variant="ghost"
          size="icon"
          class="h-8 w-8 shrink-0"
          :title="legacyT('重置筛选')"
          @click="$emit('resetFilters')"
        >
          <FilterX class="w-3.5 h-3.5" />
        </Button>

        <div class="hidden sm:block h-4 w-px shrink-0 bg-border" />

        <!-- 操作按钮 -->
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8 shrink-0"
          :title="legacyT('新增提供商')"
          @click="$emit('addProvider')"
        >
          <Plus class="w-3.5 h-3.5" />
        </Button>
        <RefreshButton
          class="shrink-0"
          :loading="loading"
          @click="$emit('refresh')"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Search, Plus, FilterX } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'
import Input from '@/components/ui/input.vue'
import Select from '@/components/ui/select.vue'
import SelectTrigger from '@/components/ui/select-trigger.vue'
import SelectValue from '@/components/ui/select-value.vue'
import SelectContent from '@/components/ui/select-content.vue'
import SelectItem from '@/components/ui/select-item.vue'
import RefreshButton from '@/components/ui/refresh-button.vue'
import type { FilterOption } from '@/features/providers/composables/useProviderFilters'
import { useI18n } from '@/i18n'

defineProps<{
  searchQuery: string
  filterStatus: string
  filterApiFormat: string
  filterModel: string
  statusFilters: FilterOption[]
  apiFormatFilters: FilterOption[]
  modelFilters: FilterOption[]
  hasActiveFilters: boolean
  loading: boolean
}>()

defineEmits<{
  'update:searchQuery': [value: string]
  'update:filterStatus': [value: string]
  'update:filterApiFormat': [value: string]
  'update:filterModel': [value: string]
  'resetFilters': []
  'addProvider': []
  'refresh': []
}>()

const { legacyT } = useI18n()
</script>

<style scoped>
.provider-toolbar {
  min-width: min(100%, 32rem);
}

.provider-toolbar__search {
  flex: 1 1 7rem;
  min-width: 7rem;
  max-width: 11rem;
}

.provider-toolbar__filter {
  flex: 0 0 5.5rem;
  min-width: 0;
}

.provider-toolbar__filter--model {
  flex-basis: 6.5rem;
}

@container provider-list (min-width: 64rem) {
  .provider-toolbar__filter {
    flex-basis: 7rem;
  }

  .provider-toolbar__filter--model {
    flex-basis: 9rem;
  }
}
</style>
