<template>
  <DefaultTheme.Layout />
  <ClientOnly>
    <CookieConsent />
  </ClientOnly>
</template>

<script setup lang="ts">
import DefaultTheme from 'vitepress/theme'
import CookieConsent from './cookie-consent.vue'
</script>
