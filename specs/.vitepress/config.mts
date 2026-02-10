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

export default defineConfig({
  title: "AIVI Language Specification",
  description: "A high-integrity functional language with a Rust-first compilation pipeline.",
  base: resolveBase(),
  themeConfig: {
    search: {
      provider: 'local'
    },
    sidebar: [
      {
        text: 'Core Specification',
        collapsed: true,
        items: [
          { text: 'Introduction', link: '/01_introduction' }
        ]
      },
      {
        text: 'Roadmap',
        collapsed: true,
        items: [
          { text: 'Roadmap', link: '/roadmap/' }
        ]
      },
      {
        text: 'Syntax',
        collapsed: true,
        items: [
          { text: 'Concrete Syntax', link: '/02_syntax/00_grammar' },
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
          { text: 'Sigils', link: '/02_syntax/13_sigils' },
          { text: 'External Sources', link: '/02_syntax/12_external_sources' },
          { text: 'Decorators', link: '/02_syntax/14_decorators' },
          { text: 'Resources', link: '/02_syntax/15_resources' },
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
                { text: 'Units', link: '/05_stdlib/00_core/16_units' },
                { text: 'Regex', link: '/05_stdlib/00_core/24_regex' },
                { text: 'Testing', link: '/05_stdlib/00_core/27_testing' },
                { text: 'Collections', link: '/05_stdlib/00_core/28_collections' },
            ]
          },
          {
            text: 'Math & Science',
            collapsed: true,
            items: [
                { text: 'Vector', link: '/05_stdlib/01_math/05_vector' },
                { text: 'Matrix', link: '/05_stdlib/01_math/09_matrix' },
                { text: 'Complex', link: '/05_stdlib/01_math/10_complex' },
                { text: 'Quaternion', link: '/05_stdlib/01_math/11_quaternion' },
                { text: 'Rational & BigInt', link: '/05_stdlib/01_math/12_rational_bigint' },
                { text: 'Probability', link: '/05_stdlib/01_math/13_probability' },
                { text: 'FFT & Signal', link: '/05_stdlib/01_math/14_signal' },
                { text: 'Geometry', link: '/05_stdlib/01_math/15_geometry' },
                { text: 'Graph', link: '/05_stdlib/01_math/17_graph' },
                { text: 'Linear Algebra', link: '/05_stdlib/01_math/18_linear_algebra' },
            ]
          },
          {
            text: 'Chronos (Time)',
            collapsed: true,
            items: [
                { text: 'Calendar', link: '/05_stdlib/02_chronos/02_calendar' },
                { text: 'Duration', link: '/05_stdlib/02_chronos/03_duration' },
            ]
          },
          {
            text: 'System & IO',
            collapsed: true,
            items: [
                { text: 'File', link: '/05_stdlib/03_system/20_file' },
                { text: 'Console', link: '/05_stdlib/03_system/21_console' },
                { text: 'HTTP', link: '/05_stdlib/03_system/19_http' },
                { text: 'Database', link: '/05_stdlib/03_system/23_database' },
                { text: 'URL', link: '/05_stdlib/03_system/25_url' },
                { text: 'Crypto', link: '/05_stdlib/03_system/22_crypto' },
                { text: 'System', link: '/05_stdlib/03_system/25_system' },
                { text: 'Log', link: '/05_stdlib/03_system/26_log' },
            ]
          },
          {
            text: 'UI',
            collapsed: true,
            items: [
                { text: 'Color', link: '/05_stdlib/04_ui/04_color' },
            ]
          }
        ]
      },
      {
        text: 'Execution & Concurrency',
        collapsed: true,
        items: [
          { text: 'Concurrency', link: '/06_runtime/01_concurrency' },
          { text: 'Rustc Native Pipeline', link: '/06_runtime/02_rustc_native_pipeline' },
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
