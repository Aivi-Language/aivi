import { readFileSync } from 'node:fs'
import { defineConfig } from 'vitepress'
import { sidebar } from '../nav.mjs'

const ebnfGrammar = JSON.parse(
  readFileSync(new URL('../../vscode/syntaxes/ebnf.tmLanguage.json', import.meta.url), 'utf-8')
)

const ebnfLanguage = {
  ...ebnfGrammar,
  name: 'ebnf',
  displayName: 'EBNF'
}

function normalizeBase(base: string): string {
  if (!base.startsWith('/')) base = `/${base}`
  if (!base.endsWith('/')) base = `${base}/`
  return base
}

function resolveBase(): string {
  const explicit = process.env.BASE_PATH || process.env.BASE_URL
  if (explicit) return normalizeBase(explicit)

  const repo = process.env.GITHUB_REPOSITORY?.split('/')[1]
  if (repo && !repo.endsWith('.github.io')) return normalizeBase(repo)

  return '/'
}

const base = resolveBase()

export default defineConfig({
  title: "AIVI",
  description: "A high-integrity functional language with a Rust-first compilation pipeline.",
  base,
  head: [
    ['link', { rel: 'icon', href: `${base}favicon.png` }],
    ['link', { rel: 'preconnect', href: 'https://fonts.googleapis.com' }],
    ['link', { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' }],
    ['link', { rel: 'stylesheet', href: 'https://fonts.googleapis.com/css2?family=Fira+Code:wght@300..700&display=swap' }],
    [
      'script',
      { id: 'ga-consent-bootstrap' },
      `
;(() => {
  const GA_ID = 'G-S11CQ296S5'

  window.dataLayer = window.dataLayer || []
  function gtag(){ window.dataLayer.push(arguments) }
  window.gtag = window.gtag || gtag

  // Default: no analytics cookies until user opts in
  window.gtag('consent', 'default', { analytics_storage: 'denied' })

  const ensureGaLoaded = () => {
    if (window.__gaLoaded) return
    window.__gaLoaded = true
    const s = document.createElement('script')
    s.async = true
    s.src = 'https://www.googletagmanager.com/gtag/js?id=' + encodeURIComponent(GA_ID)
    document.head.appendChild(s)

    window.gtag('js', new Date())
    window.gtag('config', GA_ID, { send_page_view: false })
  }

  window.__ga = {
    accept: () => {
      window.gtag('consent', 'update', { analytics_storage: 'granted' })
      ensureGaLoaded()
    },
    reject: () => {
      window.gtag('consent', 'update', { analytics_storage: 'denied' })
    },
    pageView: (path, title) => {
      if (!window.__gaLoaded) return
      window.gtag('event', 'page_view', {
        page_path: path,
        page_title: title
      })
    }
  }
})()
      `
    ]
  ],
  themeConfig: {
    search: {
      provider: 'local'
    },
    socialLinks: [
      { icon: 'github', link: 'https://github.com/Aivi-Language/aivi' }
    ],
    sidebar
  },
  markdown: {
    languages: [ebnfLanguage],
    languageAlias: {
      'aivi': 'rust'
    }
  }
})
