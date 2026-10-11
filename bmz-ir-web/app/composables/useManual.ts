export function useManual() {
  const { locale } = useI18n()
  const localePath = useLocalePath()

  const manualLocale = computed(() => manualLocaleFor(locale.value))
  const pages = computed(() => manualPages(manualLocale.value))
  const showsFallback = computed(() => manualLocale.value !== locale.value)
  const pagePath = (slug: string) => localePath(`/manual/${slug}`)

  return { pages, showsFallback, pagePath }
}
