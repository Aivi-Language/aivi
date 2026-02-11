import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import Layout from './Layout.vue'
import './style.css'

type GaApi = {
  pageView: (path: string, title: string) => void
}

const getGaApi = (): GaApi | null => {
  const w = window as unknown as { __ga?: GaApi }
  return w.__ga ?? null
}

const theme: Theme = {
  extends: DefaultTheme,
  Layout,
  enhanceApp: ({ router }) => {
    if (typeof window === 'undefined') return
    if (!import.meta.env.PROD) return

    router.onAfterRouteChange = (to) => {
      const ga = getGaApi()
      ga?.pageView(to, document.title)
    }
  }
}

export default theme
