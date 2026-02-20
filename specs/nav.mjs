export const sidebar = [
  {
    text: 'Overview',
    collapsed: false,
    items: [
      { text: 'Spec Home', link: '/' },
      { text: 'Introduction', link: '/01_introduction' },
      { text: 'Missing Features (v0.1)', link: '/missing_features_v0.1' }
    ]
  },
  {
    text: 'Syntax',
    collapsed: true,
    items: [
      {
        text: 'Core Forms',
        collapsed: true,
        items: [
          { text: 'Bindings and Scope', link: '/02_syntax/01_bindings' },
          { text: 'Functions and Pipes', link: '/02_syntax/02_functions' },
          { text: 'The Type System', link: '/02_syntax/03_types' },
          { text: 'Predicates', link: '/02_syntax/04_predicates' }
        ]
      },
      {
        text: 'Data & Flow',
        collapsed: true,
        items: [
          { text: 'Patching Records', link: '/02_syntax/05_patching' },
          { text: 'Domains, Units, and Deltas', link: '/02_syntax/06_domains' },
          { text: 'Generators', link: '/02_syntax/07_generators' },
          { text: 'Pattern Matching', link: '/02_syntax/08_pattern_matching' }
        ]
      },
      {
        text: 'Effects & Control',
        collapsed: true,
        items: [
          { text: 'Effects', link: '/02_syntax/09_effects' },
          { text: 'Resources', link: '/02_syntax/15_resources' },
          { text: 'Generic `do` Notation', link: '/02_syntax/16_do_notation' }
        ]
      },
      {
        text: 'Modules & Interop',
        collapsed: true,
        items: [
          { text: 'Modules', link: '/02_syntax/10_modules' },
          { text: 'External Sources', link: '/02_syntax/12_external_sources' }
        ]
      },
      {
        text: 'Notation & Grammar',
        collapsed: true,
        items: [
          { text: 'Operators and Context', link: '/02_syntax/11_operators' },
          { text: 'Sigils', link: '/02_syntax/13_sigils' },
          { text: 'Decorators', link: '/02_syntax/14_decorators' },
          { text: 'Comments', link: '/02_syntax/17_comments' },
          { text: 'Concrete Syntax', link: '/02_syntax/00_grammar' }
        ]
      }
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
          { text: 'Generator', link: '/05_stdlib/00_core/30_generator' },
          { text: 'MutableMap', link: '/05_stdlib/00_core/31_mutable_map' }
        ]
      },
      {
        text: 'Math & Science',
        collapsed: true,
        items: [
          { text: 'Math', link: '/05_stdlib/01_math/01_math' },
          { text: 'Vector', link: '/05_stdlib/01_math/05_vector' },
          { text: 'Matrix', link: '/05_stdlib/01_math/09_matrix' },
          { text: 'Numbers', link: '/05_stdlib/01_math/10_number' },
          { text: 'Probability', link: '/05_stdlib/01_math/13_probability' },
          { text: 'FFT & Signal', link: '/05_stdlib/01_math/14_signal' },
          { text: 'Geometry', link: '/05_stdlib/01_math/15_geometry' },
          { text: 'Graph', link: '/05_stdlib/01_math/17_graph' },
          { text: 'Linear Algebra', link: '/05_stdlib/01_math/18_linear_algebra' },
          { text: 'Tree', link: '/05_stdlib/01_math/19_tree' }
        ]
      },
      {
        text: 'Time (Chronos)',
        collapsed: true,
        items: [
          { text: 'Instant', link: '/05_stdlib/02_chronos/01_instant' },
          { text: 'Calendar', link: '/05_stdlib/02_chronos/02_calendar' },
          { text: 'Duration', link: '/05_stdlib/02_chronos/03_duration' },
          { text: 'TimeZone', link: '/05_stdlib/02_chronos/04_timezone' }
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
          { text: 'Concurrency', link: '/05_stdlib/03_system/30_concurrency' }
        ]
      },
      {
        text: 'Network',
        collapsed: true,
        items: [
          { text: 'HTTP', link: '/05_stdlib/03_network/01_http' },
          { text: 'HTTPS', link: '/05_stdlib/03_network/02_https' },
          { text: 'HTTP Server', link: '/05_stdlib/03_network/03_http_server' },
          { text: 'Sockets', link: '/05_stdlib/03_network/04_sockets' },
          { text: 'Streams', link: '/05_stdlib/03_network/05_streams' }
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
          { text: 'ServerHtml', link: '/05_stdlib/04_ui/05_server_html' }
        ]
      }
    ]
  },
  {
    text: 'Semantics',
    collapsed: true,
    items: [
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
          { text: 'Minimality Proof', link: '/03_kernel/12_minimality' }
        ]
      },
      {
        text: 'Desugaring (Surface -> Kernel)',
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
          { text: 'Resources', link: '/04_desugaring/11_resources' }
        ]
      }
    ]
  },
  {
    text: 'Runtime',
    collapsed: true,
    items: [
      { text: 'Concurrency', link: '/06_runtime/01_concurrency' },
      { text: 'Memory Management', link: '/06_runtime/02_memory_management' },
      { text: 'Package Manager (Cargo-backed)', link: '/06_runtime/03_package_manager' }
    ]
  },
  {
    text: 'Tooling',
    collapsed: true,
    items: [
      { text: 'CLI', link: '/07_tools/01_cli' },
      { text: 'LSP Server', link: '/07_tools/02_lsp_server' },
      { text: 'VSCode Extension', link: '/07_tools/03_vscode_extension' },
      { text: 'Packaging', link: '/07_tools/04_packaging' },
      { text: 'Spec Doc Markers', link: '/doc-markers-spec' }
    ]
  },
  {
    text: 'Compiler & Backend',
    collapsed: true,
    items: [
      { text: 'Typed Codegen Design', link: '/08_typed_codegen/01_design' }
    ]
  }
]
