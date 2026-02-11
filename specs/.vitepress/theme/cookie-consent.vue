<template>
  <div v-if="visible" class="cc-backdrop" role="dialog" aria-modal="true" aria-label="Cookie consent">
    <div class="cc-card">
      <div class="cc-title">Cookies</div>
      <div class="cc-text">
        We use analytics cookies to understand traffic and improve the site.
      </div>

      <div class="cc-actions">
        <button type="button" class="cc-btn" @click="reject">Reject</button>
        <button type="button" class="cc-btn cc-btn-primary" @click="accept">Accept analytics</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'

type ConsentValue = 'accepted' | 'rejected'
type GaApi = {
  accept: () => void
  reject: () => void
  pageView: (path: string, title: string) => void
}

const STORAGE_KEY = 'cookie-consent.v1'
const visible = ref(false)

const readConsent = (): ConsentValue | null => {
  const v = localStorage.getItem(STORAGE_KEY)
  return v === 'accepted' || v === 'rejected' ? v : null
}

const writeConsent = (v: ConsentValue): void => {
  localStorage.setItem(STORAGE_KEY, v)
}

const getGaApi = (): GaApi | null => {
  const w = window as unknown as { __ga?: GaApi }
  return w.__ga ?? null
}

const apply = (v: ConsentValue): void => {
  const ga = getGaApi()
  if (!ga) return

  if (v === 'accepted') ga.accept()
  if (v === 'rejected') ga.reject()
}

const accept = (): void => {
  writeConsent('accepted')
  apply('accepted')
  visible.value = false
}

const reject = (): void => {
  writeConsent('rejected')
  apply('rejected')
  visible.value = false
}

onMounted(() => {
  // Only run in production (avoid counting local dev)
  if (!import.meta.env.PROD) return

  const existing = readConsent()
  if (existing) {
    apply(existing)
    visible.value = false
    return
  }

  visible.value = true
})
</script>

<style scoped>
.cc-backdrop {
  position: fixed;
  inset: 0;
  display: grid;
  place-items: center;
  background: rgba(0,0,0,0.5);
  z-index: 9999;
}
.cc-card {
  width: min(520px, 92vw);
  background: white;
  border-radius: 12px;
  padding: 16px;
  font-family: system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
}
.cc-title { font-size: 18px; font-weight: 600; margin-bottom: 8px; color: #111; }
.cc-text { line-height: 1.35; margin-bottom: 12px; color: #333; }
.cc-actions { display: flex; gap: 8px; justify-content: flex-end; }
.cc-btn { padding: 8px 10px; border-radius: 8px; border: 1px solid #ccc; background: #fff; cursor: pointer; color: #111; }
.cc-btn-primary { border-color: #111; background: #111; color: #fff; }
</style>
