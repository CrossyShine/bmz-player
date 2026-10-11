<script setup lang="ts">
const { sidebarMenuItems } = useAppNavigation()
const { open } = useAppSidebar()
const route = useRoute()
const localePath = useLocalePath()

// アカウントは常に開き、マニュアルは閲覧中だけ自動で開く。ユーザーが閉じた状態はそのまま保つ。
const isManualRoute = computed(() => {
  const base = localePath('/manual')
  return route.path === base || route.path.startsWith(`${base}/`)
})
const openItems = ref<string[]>([
  APP_NAV_ACCOUNT_VALUE,
  ...(isManualRoute.value ? [APP_NAV_MANUAL_VALUE] : []),
])
watch(isManualRoute, (inManual) => {
  if (inManual && !openItems.value.includes(APP_NAV_MANUAL_VALUE)) {
    openItems.value = [...openItems.value, APP_NAV_MANUAL_VALUE]
  }
})
</script>

<template>
  <USidebar
    v-model:open="open"
    collapsible="icon"
    :ui="{
      container: 'static h-full',
    }"
  >
    <template #default="{ state }">
      <UNavigationMenu
        v-model="openItems"
        :collapsed="state === 'collapsed'"
        :items="sidebarMenuItems"
        :popover="state === 'collapsed'"
        orientation="vertical"
        :ui="appNavigationMenuUi"
      />
    </template>
  </USidebar>
</template>
