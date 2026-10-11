<script setup lang="ts">
const route = useRoute()
const localePath = useLocalePath()
const { t } = useI18n()
const { pages, showsFallback, pagePath } = useManual()

const slug = computed(() => String(route.params.slug))
const index = computed(() => pages.value.findIndex((page) => page.slug === slug.value))
const page = computed(() => pages.value[index.value])
const previous = computed(() => (index.value > 0 ? pages.value[index.value - 1] : undefined))
const next = computed(() => pages.value[index.value + 1])

if (!page.value) {
  throw createError({ statusCode: 404, statusMessage: 'Not Found' })
}

const html = computed(() => (page.value ? renderManualMarkdown(page.value.markdown, pagePath) : ''))

// 原稿内のページ間リンクは、ページを読み直さずに移動する。
function onContentClick(event: MouseEvent) {
  if (event.defaultPrevented || event.button !== 0) return
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
  const link = (event.target as Element | null)?.closest('a[data-manual-link]')
  const href = link?.getAttribute('href')
  if (!href) return
  event.preventDefault()
  navigateTo(href)
}

useSeoMeta({
  title: () => (page.value ? `${page.value.title} | ${t('manual.title')}` : t('manual.title')),
  description: () => page.value?.summary ?? '',
})
</script>

<template>
  <main>
    <div
      v-if="page"
      class="mx-auto flex w-full max-w-6xl flex-col gap-8 px-5 py-10 lg:flex-row lg:items-start"
    >
      <nav class="shrink-0 lg:sticky lg:top-6 lg:w-56" :aria-label="t('manual.contents')">
        <ULink
          :to="localePath('/manual')"
          class="mb-3 inline-flex items-center gap-1 text-sm text-muted hover:text-highlighted"
        >
          <UIcon name="i-lucide-book-open" />
          {{ t('manual.title') }}
        </ULink>
        <ol class="flex flex-col gap-1 text-sm">
          <li v-for="item in pages" :key="item.slug">
            <ULink
              :to="pagePath(item.slug)"
              class="block rounded px-2 py-1"
              :class="
                item.slug === page.slug
                  ? 'bg-elevated font-semibold text-highlighted'
                  : 'text-muted hover:text-highlighted'
              "
            >
              {{ item.title }}
            </ULink>
          </li>
        </ol>
      </nav>

      <article class="min-w-0 flex-1">
        <UAlert
          v-if="showsFallback"
          class="mb-6"
          color="neutral"
          icon="i-lucide-languages"
          :title="t('manual.englishOnly')"
          variant="subtle"
        />
        <h1 class="mb-6 text-3xl font-semibold">{{ page.title }}</h1>
        <!-- eslint-disable-next-line vue/no-v-html -- リポジトリ内の原稿だけを表示する -->
        <div class="manual-content" @click="onContentClick" v-html="html" />

        <div class="mt-12 flex flex-col gap-3 border-t border-default pt-6 sm:flex-row">
          <UButton
            v-if="previous"
            color="neutral"
            icon="i-lucide-arrow-left"
            :to="pagePath(previous.slug)"
            variant="subtle"
          >
            {{ t('manual.previous', { title: previous.title }) }}
          </UButton>
          <UButton
            v-if="next"
            class="sm:ml-auto"
            color="neutral"
            trailing-icon="i-lucide-arrow-right"
            :to="pagePath(next.slug)"
            variant="subtle"
          >
            {{ t('manual.next', { title: next.title }) }}
          </UButton>
        </div>
      </article>
    </div>
  </main>
</template>
