<script setup>
import DefaultTheme from 'vitepress/theme'
const { Layout } = DefaultTheme
</script>

<template>
  <Layout>
    <template #home-hero-info-before>
      <img src="/aivi-logo.png" alt="AIVI" class="hero-logo" />
    </template>
  </Layout>
</template>

<style scoped>
.hero-logo {
  max-height: 96px;
  margin-bottom: 1.25rem;
  display: block;
}
</style>
