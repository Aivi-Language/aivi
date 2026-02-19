import { readFileSync } from 'node:fs'
import { defineConfig } from 'vitepress'

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
      { icon: 'github', link: 'https://github.com/mendrik/aivi' }
    ],
    sidebar: [
      {
        text: 'Core Specification',
        collapsed: false,
        items: [
          { text: 'Introduction', link: '/01_introduction' },
          { text: 'Missing Features (v0.1)', link: '/missing_features_v0.1' }
        ]
      },
      {
        text: 'Syntax',
        collapsed: true,
        items: [
          { text: 'Bindings and Scope', link: '/02_syntax/01_bindings' },
          { text: 'Functions and Pipes', link: '/02_syntax/02_functions' },
          { text: 'The Type System', link: '/02_syntax/03_types' },
          { text: 'Predicates', link: '/02_syntax/04_predicates' },
          { text: 'Patching Records', link: '/02_syntax/05_patching' },
          { text: 'Domains, Units, and Deltas', link: '/02_syntax/06_domains' },
          { text: 'Generators', link: '/02_syntax/07_generators' },
          { text: 'Pattern Matching', link: '/02_syntax/08_pattern_matching' },
          { text: 'Effects', link: '/02_syntax/09_effects' },
          { text: 'Modules', link: '/02_syntax/10_modules' },
          { text: 'External Sources', link: '/02_syntax/12_external_sources' },
          { text: 'Sigils', link: '/02_syntax/13_sigils' },
          { text: 'Decorators', link: '/02_syntax/14_decorators' },
          { text: 'Resources', link: '/02_syntax/15_resources' },
          { text: 'Operators and Context', link: '/02_syntax/11_operators' },
          { text: 'Concrete Syntax', link: '/02_syntax/00_grammar' },
        ]
      },
      {
        text: 'Standard Library',
        collapsed: true,
        items: [
          {
            text: 'Core & Utils',
            collapsed: true,
            items: [
              { text: 'Prelude', link: '/05_stdlib/00_core/01_prelude' },
              { text: 'Text', link: '/05_stdlib/00_core/02_text' },
              { text: 'Logic', link: '/05_stdlib/00_core/03_logic' },
              { text: 'Units', link: '/05_stdlib/00_core/16_units' },
              { text: 'Regex', link: '/05_stdlib/00_core/24_regex' },
              { text: 'Testing', link: '/05_stdlib/00_core/27_testing' },
              { text: 'Collections', link: '/05_stdlib/00_core/28_collections' },
              { text: 'I18n', link: '/05_stdlib/00_core/29_i18n' },
            ]
          },
          {
            text: 'Math & Science',
            collapsed: true,
            items: [
              { text: 'Math', link: '/05_stdlib/01_math/01_math' },
              { text: 'Vector', link: '/05_stdlib/01_math/05_vector' },
              { text: 'Matrix', link: '/05_stdlib/01_math/09_matrix' },
              { text: 'Number (BigInt, Rational, Decimal, Complex, Quaternion)', link: '/05_stdlib/01_math/10_number' },
              { text: 'Probability', link: '/05_stdlib/01_math/13_probability' },
              { text: 'FFT & Signal', link: '/05_stdlib/01_math/14_signal' },
              { text: 'Geometry', link: '/05_stdlib/01_math/15_geometry' },
              { text: 'Graph', link: '/05_stdlib/01_math/17_graph' },
              { text: 'Linear Algebra', link: '/05_stdlib/01_math/18_linear_algebra' },
              { text: 'Tree', link: '/05_stdlib/01_math/19_tree' },
            ]
          },
          {
            text: 'Time',
            collapsed: true,
            items: [
              { text: 'Instant', link: '/05_stdlib/02_chronos/01_instant' },
              { text: 'Calendar', link: '/05_stdlib/02_chronos/02_calendar' },
              { text: 'Duration', link: '/05_stdlib/02_chronos/03_duration' },
              { text: 'TimeZone', link: '/05_stdlib/02_chronos/04_timezone' },
            ]
          },
          {
            text: 'System & IO',
            collapsed: true,
            items: [
              { text: 'File', link: '/05_stdlib/03_system/20_file' },
              { text: 'Console', link: '/05_stdlib/03_system/21_console' },
              { text: 'Crypto', link: '/05_stdlib/03_system/22_crypto' },
              { text: 'Database', link: '/05_stdlib/03_system/23_database' },
              { text: 'Path', link: '/05_stdlib/03_system/24_path' },
              { text: 'URL', link: '/05_stdlib/03_system/25_url' },
              { text: 'System', link: '/05_stdlib/03_system/26_system' },
              { text: 'Log', link: '/05_stdlib/03_system/27_log' },
              { text: 'Concurrency', link: '/05_stdlib/03_system/30_concurrency' },
            ]
          },
          {
            text: 'Network',
            collapsed: true,
            items: [
              { text: 'HTTP Utils', link: '/05_stdlib/03_network/01_http' },
              { text: 'HTTPS', link: '/05_stdlib/03_network/02_https' },
              { text: 'HTTP Server', link: '/05_stdlib/03_network/03_http_server' },
              { text: 'Sockets', link: '/05_stdlib/03_network/04_sockets' },
              { text: 'Streams', link: '/05_stdlib/03_network/05_streams' },
            ]
          },
          {
            text: 'UI',
            collapsed: true,
            items: [
              { text: 'Layout', link: '/05_stdlib/04_ui/01_layout' },
              { text: 'VDOM', link: '/05_stdlib/04_ui/02_vdom' },
              { text: 'HTML Sigil', link: '/05_stdlib/04_ui/03_html' },
              { text: 'Color', link: '/05_stdlib/04_ui/04_color' },
              { text: 'ServerHtml', link: '/05_stdlib/04_ui/05_server_html' },
            ]
          }
        ]
      },
      {
        text: 'Execution & Concurrency',
        collapsed: true,
        items: [
          { text: 'Concurrency', link: '/06_runtime/01_concurrency' },
          { text: 'Memory Management', link: '/06_runtime/02_memory_management' },
          { text: 'Package Manager (Cargo-backed)', link: '/06_runtime/03_package_manager' },
        ]
      },
      {
        text: 'Tools & Ecosystem',
        collapsed: true,
        items: [
          { text: 'CLI', link: '/07_tools/01_cli' },
          { text: 'LSP Server', link: '/07_tools/02_lsp_server' },
          { text: 'VSCode Extension', link: '/07_tools/03_vscode_extension' },
          { text: 'Packaging', link: '/07_tools/04_packaging' },
        ]
      },
      {
        text: 'Kernel (Core Calculus)',
        collapsed: true,
        items: [
          { text: 'Core Terms', link: '/03_kernel/01_core_terms' },
          { text: 'Types', link: '/03_kernel/02_types' },
          { text: 'Records', link: '/03_kernel/03_records' },
          { text: 'Patterns', link: '/03_kernel/04_patterns' },
          { text: 'Predicates', link: '/03_kernel/05_predicates' },
          { text: 'Traversals', link: '/03_kernel/06_traversals' },
          { text: 'Generators', link: '/03_kernel/07_generators' },
          { text: 'Effects', link: '/03_kernel/08_effects' },
          { text: 'Classes', link: '/03_kernel/09_classes' },
          { text: 'Domains', link: '/03_kernel/10_domains' },
          { text: 'Patching', link: '/03_kernel/11_patching' },
          { text: 'Minimality Proof', link: '/03_kernel/12_minimality' },
        ]
      },
      {
        text: 'Desugaring',
        collapsed: true,
        items: [
          { text: 'Bindings', link: '/04_desugaring/01_bindings' },
          { text: 'Functions', link: '/04_desugaring/02_functions' },
          { text: 'Records', link: '/04_desugaring/03_records' },
          { text: 'Patterns', link: '/04_desugaring/04_patterns' },
          { text: 'Predicates', link: '/04_desugaring/05_predicates' },
          { text: 'Generators', link: '/04_desugaring/06_generators' },
          { text: 'Effects', link: '/04_desugaring/07_effects' },
          { text: 'Classes', link: '/04_desugaring/08_classes' },
          { text: 'Domains and Operators', link: '/04_desugaring/09_domains' },
          { text: 'Patching', link: '/04_desugaring/10_patching' },
          { text: 'Resources', link: '/04_desugaring/11_resources' },
        ]
      }
    ]
  },
  markdown: {
    languages: [ebnfLanguage],
    languageAlias: {
      'aivi': 'rust'
    }
  }
})
