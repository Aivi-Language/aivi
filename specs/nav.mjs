export const sidebar = [
  {
    text: 'Overview',
    collapsed: false,
    items: [
      { text: 'Spec Home', link: '/' },
      { text: 'Introduction', link: '/introduction' },
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
          { text: 'Bindings and Scope', link: '/syntax/bindings' },
          { text: 'Functions and Pipes', link: '/syntax/functions' },
          { text: 'Primitive Types', link: '/syntax/types/primitive_types' },
          { text: 'Algebraic Data Types', link: '/syntax/types/algebraic_data_types' },
          { text: 'Closed Records', link: '/syntax/types/closed_records' },
          { text: 'Record Row Transforms', link: '/syntax/types/record_row_transforms' },
          { text: 'Classes and HKTs', link: '/syntax/types/classes_and_hkts' },
          { text: 'Expected-Type Coercions', link: '/syntax/types/expected_type_coercions' },
          { text: 'Predicates', link: '/syntax/predicates' }
        ]
      },
      {
        text: 'Data & Flow',
        collapsed: true,
        items: [
          { text: 'Patching Records', link: '/syntax/patching' },
          { text: 'Domains, Units, and Deltas', link: '/syntax/domains' },
          { text: 'Generators', link: '/syntax/generators' },
          { text: 'Pattern Matching', link: '/syntax/pattern_matching' }
        ]
      },
      {
        text: 'Effects & Control',
        collapsed: true,
        items: [
          { text: 'Effects', link: '/syntax/effects' },
          { text: 'Machines', link: '/syntax/machines_runtime' },
          { text: 'Resources', link: '/syntax/resources' },
          { text: 'Generic `do` Notation', link: '/syntax/do_notation' }
        ]
      },
      {
        text: 'Modules & Interop',
        collapsed: true,
        items: [
          { text: 'Modules', link: '/syntax/modules' },
          { text: 'External Sources', link: '/syntax/external_sources' },
          {
            text: 'Source Integrations',
            collapsed: true,
            items: [
              { text: 'File Sources', link: '/syntax/external_sources/file' },
              { text: 'REST/HTTP Sources', link: '/syntax/external_sources/rest_http' },
              { text: 'Environment Sources', link: '/syntax/external_sources/environment' },
              { text: 'IMAP Email Sources', link: '/syntax/external_sources/imap_email' },
              { text: 'Image Sources', link: '/syntax/external_sources/image' },
              { text: 'Compile-Time Sources', link: '/syntax/external_sources/compile_time' }
            ]
          }
        ]
      },
      {
        text: 'Notation & Grammar',
        collapsed: true,
        items: [
          { text: 'Operators and Context', link: '/syntax/operators' },
          { text: 'Sigils', link: '/syntax/sigils' },
          { text: 'Decorators', link: '/syntax/decorators' },
          { text: 'Comments', link: '/syntax/comments' },
          { text: 'Concrete Syntax', link: '/syntax/grammar' }
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
          { text: 'Prelude', link: '/stdlib/core/prelude' },
          { text: 'Text', link: '/stdlib/core/text' },
          { text: 'Logic', link: '/stdlib/core/logic' },
          { text: 'Units', link: '/stdlib/core/units' },
          { text: 'Regex', link: '/stdlib/core/regex' },
          { text: 'Testing', link: '/stdlib/core/testing' },
          { text: 'Collections', link: '/stdlib/core/collections' },
          { text: 'I18n', link: '/stdlib/core/i18n' },
          { text: 'Generator', link: '/stdlib/core/generator' },
          { text: 'Validation', link: '/stdlib/core/validation' }
        ]
      },
      {
        text: 'Math & Science',
        collapsed: true,
        items: [
          { text: 'Math', link: '/stdlib/math/math' },
          { text: 'Vector', link: '/stdlib/math/vector' },
          { text: 'Matrix', link: '/stdlib/math/matrix' },
          { text: 'Numbers', link: '/stdlib/math/number' },
          { text: 'Probability', link: '/stdlib/math/probability' },
          { text: 'FFT & Signal', link: '/stdlib/math/signal' },
          { text: 'Geometry', link: '/stdlib/math/geometry' },
          { text: 'Graph', link: '/stdlib/math/graph' },
          { text: 'Linear Algebra', link: '/stdlib/math/linear_algebra' },
          { text: 'Tree', link: '/stdlib/math/tree' }
        ]
      },
      {
        text: 'Time',
        collapsed: true,
        items: [
          { text: 'Instant', link: '/stdlib/chronos/instant' },
          { text: 'Calendar', link: '/stdlib/chronos/calendar' },
          { text: 'Duration', link: '/stdlib/chronos/duration' },
          { text: 'TimeZone', link: '/stdlib/chronos/timezone' },
          { text: 'Scheduler', link: '/stdlib/chronos/scheduler' }
        ]
      },
      {
        text: 'System & IO',
        collapsed: true,
        items: [
          { text: 'File', link: '/stdlib/system/file' },
          { text: 'Console', link: '/stdlib/system/console' },
          { text: 'Crypto', link: '/stdlib/system/crypto' },
          { text: 'Database', link: '/stdlib/system/database' },
          { text: 'GOA', link: '/stdlib/system/goa' },
          { text: 'Secrets', link: '/stdlib/system/secrets' },
          { text: 'Path', link: '/stdlib/system/path' },
          { text: 'URL', link: '/stdlib/system/url' },
          { text: 'System', link: '/stdlib/system/system' },
          { text: 'Log', link: '/stdlib/system/log' },
          { text: 'Concurrency', link: '/stdlib/system/concurrency' }
        ]
      },
      {
        text: 'Network',
        collapsed: true,
        items: [
          { text: 'HTTP', link: '/stdlib/network/http' },
          { text: 'HTTPS', link: '/stdlib/network/https' },
          { text: 'HTTP Server', link: '/stdlib/network/http_server' },
          { text: 'Sockets', link: '/stdlib/network/sockets' },
          { text: 'Streams', link: '/stdlib/network/streams' }
        ]
      },
      {
        text: 'UI',
        collapsed: true,
        items: [
          { text: 'GTK4', link: '/stdlib/ui/gtk4' },
          { text: 'Layout', link: '/stdlib/ui/layout' },
          { text: 'VDOM', link: '/stdlib/ui/vdom' },
          { text: 'HTML Sigil', link: '/stdlib/ui/html' },
          { text: 'Color', link: '/stdlib/ui/color' },
          { text: 'ServerHtml', link: '/stdlib/ui/server_html' }
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
          { text: 'Core Terms', link: '/kernel/core_terms' },
          { text: 'Types', link: '/kernel/types' },
          { text: 'Records', link: '/kernel/records' },
          { text: 'Patterns', link: '/kernel/patterns' },
          { text: 'Predicates', link: '/kernel/predicates' },
          { text: 'Traversals', link: '/kernel/traversals' },
          { text: 'Generators', link: '/kernel/generators' },
          { text: 'Effects', link: '/kernel/effects' },
          { text: 'Classes', link: '/kernel/classes' },
          { text: 'Domains', link: '/kernel/domains' },
          { text: 'Patching', link: '/kernel/patching' },
          { text: 'Minimality Proof', link: '/kernel/minimality' }
        ]
      },
      {
        text: 'Desugaring (Surface -> Kernel)',
        collapsed: true,
        items: [
          { text: 'Bindings', link: '/desugaring/bindings' },
          { text: 'Functions', link: '/desugaring/functions' },
          { text: 'Records', link: '/desugaring/records' },
          { text: 'Patterns', link: '/desugaring/patterns' },
          { text: 'Predicates', link: '/desugaring/predicates' },
          { text: 'Generators', link: '/desugaring/generators' },
          { text: 'Effects', link: '/desugaring/effects' },
          { text: 'Classes', link: '/desugaring/classes' },
          { text: 'Domains and Operators', link: '/desugaring/domains' },
          { text: 'Patching', link: '/desugaring/patching' },
          { text: 'Resources', link: '/desugaring/resources' }
        ]
      }
    ]
  },
  {
    text: 'Runtime',
    collapsed: true,
    items: [
      { text: 'Concurrency', link: '/runtime/concurrency' },
      { text: 'Memory Management', link: '/runtime/memory_management' },
      { text: 'Package Manager (Cargo-backed)', link: '/runtime/package_manager' }
    ]
  },
  {
    text: 'Tooling',
    collapsed: true,
    items: [
      { text: 'CLI', link: '/tools/cli' },
      { text: 'LSP Server', link: '/tools/lsp_server' },
      { text: 'VSCode Extension', link: '/tools/vscode_extension' },
      { text: 'Packaging', link: '/tools/packaging' },
      { text: 'Spec Doc Markers', link: '/doc-markers-spec' }
    ]
  },
  {
    text: 'Compiler & Backend',
    collapsed: true,
    items: [
      { text: 'Typed Codegen Design', link: '/typed_codegen/design' }
    ]
  }
]
