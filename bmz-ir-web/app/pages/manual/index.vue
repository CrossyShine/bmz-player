<script setup lang="ts">
const { t } = useI18n()
const { pages, showsFallback, pagePath } = useManual()

useSeoMeta({ title: () => t('manual.title') })
</script>

<template>
  <main>
    <section class="mx-auto w-full max-w-4xl px-5 py-10">
      <div class="mb-8">
        <p class="mb-2 text-sm font-medium text-primary-300">BMZ Player</p>
        <h1 class="text-3xl font-semibold">{{ t('manual.title') }}</h1>
        <p class="mt-2 text-sm text-neutral-300">{{ t('manual.description') }}</p>
      </div>

      <UAlert
        v-if="showsFallback"
        class="mb-6"
        color="neutral"
        icon="i-lucide-languages"
        :title="t('manual.englishOnly')"
        variant="subtle"
      />

      <ol class="grid gap-3">
        <li v-for="(page, index) in pages" :key="page.slug">
          <ULink
            :to="pagePath(page.slug)"
            class="block rounded-lg border border-default p-4 transition-colors hover:bg-elevated"
          >
            <p class="text-xs text-muted">{{ index + 1 }}</p>
            <p class="font-semibold text-highlighted">{{ page.title }}</p>
            <p class="mt-1 text-sm text-neutral-300">{{ page.summary }}</p>
          </ULink>
        </li>
      </ol>
    </section>
  </main>
</template>
