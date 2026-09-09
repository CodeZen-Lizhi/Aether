<script setup lang="ts">
import { CircleAlert, LoaderCircle, RotateCw } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'
import LanguageSwitcher from '@/components/common/LanguageSwitcher.vue'
import ThemeModeButton from '@/components/common/ThemeModeButton.vue'

defineProps<{ connecting: boolean; error: string }>()
defineEmits<{ retry: [] }>()
</script>

<template>
  <div class="flex min-h-screen flex-col bg-background text-foreground">
    <header class="flex items-center justify-between gap-4 px-6 py-5 sm:px-10">
      <div class="flex items-center gap-3">
        <img
          src="/aether_adaptive.svg"
          alt=""
          class="h-8 w-8 brightness-0 dark:invert"
          width="32"
          height="32"
        >
        <span class="text-lg font-semibold">Aether</span>
      </div>
      <div class="flex items-center gap-1">
        <LanguageSwitcher />
        <ThemeModeButton />
      </div>
    </header>
    <main class="flex flex-1 items-center justify-center px-6 pb-20">
      <section
        class="w-full max-w-md rounded-2xl border border-border bg-card p-8 sm:p-10"
        aria-labelledby="desktop-connection-title"
        :aria-busy="connecting"
      >
        <div class="mb-6 flex h-12 w-12 items-center justify-center rounded-xl bg-muted text-muted-foreground">
          <LoaderCircle
            v-if="connecting"
            class="h-6 w-6 animate-spin motion-reduce:animate-none"
            aria-hidden="true"
          />
          <CircleAlert
            v-else
            class="h-6 w-6 text-destructive"
            aria-hidden="true"
          />
        </div>
        <h1
          id="desktop-connection-title"
          class="text-2xl font-semibold tracking-tight"
        >
          {{ connecting ? '正在连接本机网关' : '连接本机网关' }}
        </h1>
        <p
          class="mt-3 text-sm leading-6 text-muted-foreground"
          :role="connecting ? 'status' : 'alert'"
          aria-live="polite"
        >
          {{ connecting ? '正在准备管理界面，连接成功后会自动进入。' : error }}
        </p>
        <template v-if="!connecting">
          <Button
            class="mt-6 gap-2 bg-[#2563eb] hover:bg-[#1d4ed8]"
            @click="$emit('retry')"
          >
            <RotateCw
              class="h-4 w-4"
              aria-hidden="true"
            />
            重新连接
          </Button>
          <p class="mt-5 text-xs leading-5 text-muted-foreground">
            可在 Aether 菜单栏中检查网关状态，启动后再次连接。
          </p>
        </template>
      </section>
    </main>
  </div>
</template>
