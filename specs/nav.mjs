export const sidebar = [
  {
    text: 'Start Here',
    collapsed: false,
    items: [
      { text: 'Language at a Glance', link: '/language-overview' },
      { text: 'Introduction', link: '/introduction' }
    ]
  },
  {
    text: 'Learn AIVI',
    collapsed: false,
    items: [
      {
        text: 'Basics',
        collapsed: false,
        items: [
          { text: 'Bindings & Scope', link: '/syntax/bindings' },
          { text: 'Functions & Pipes', link: '/syntax/functions' }
        ]
      },
      {
        text: 'Data & Types',
        collapsed: true,
        items: [
          { text: 'Type System Overview', link: '/syntax/types' },
          { text: 'Primitive Types', link: '/syntax/types/primitive_types' },
          { text: 'Custom Data Types (ADTs)', link: '/syntax/types/algebraic_data_types' },
          { text: 'Records', link: '/syntax/types/closed_records' },
          { text: 'Extending & Reshaping Records', link: '/syntax/types/record_row_transforms' },
          { text: 'Pattern Matching', link: '/syntax/pattern_matching' },
          { text: 'Predicates', link: '/syntax/predicates' },
          { text: 'Updating Records', link: '/syntax/patching' },
          { text: 'Helpful Type Conversions', link: '/syntax/types/expected_type_coercions' },
          { text: 'Opaque Types', link: '/syntax/types/opaque_types' },
          { text: 'Domains & Units', link: '/syntax/domains' },
        ]
      },
      {
        text: 'Effects & Workflows',
        collapsed: true,
        items: [
          { text: 'Flow Syntax', link: '/syntax/flows' },
          { text: 'Effects', link: '/syntax/effects' },
          { text: 'Cleanup & Lifetimes', link: '/syntax/resources' },
          { text: 'Fan-out & Collection Shaping', link: '/syntax/generators' }
        ]
      },
      {
        text: 'Modules & External Data',
        collapsed: true,
        items: [
          { text: 'Modules', link: '/syntax/modules' },
          {
            text: 'External Sources',
            collapsed: true,
            items: [
              { text: 'Overview', link: '/syntax/external_sources' },
              { text: 'Define Sources from Schemas', link: '/syntax/external_sources/schema_first' },
              { text: 'Combine Sources', link: '/syntax/external_sources/composition' },
              { text: 'File Sources', link: '/syntax/external_sources/file' },
              { text: 'REST / HTTP Sources', link: '/syntax/external_sources/rest_http' },
              { text: 'Environment Sources', link: '/syntax/external_sources/environment' },
              { text: 'IMAP Email Sources', link: '/syntax/external_sources/imap_email' },
              { text: 'Image Sources', link: '/syntax/external_sources/image' },
              { text: 'Compile-Time Sources', link: '/syntax/external_sources/compile_time' }
            ]
          }
        ]
      },
      {
        text: 'Advanced Features',
        collapsed: true,
        items: [
          { text: 'Reusable Type Patterns (Classes & HKTs)', link: '/syntax/types/classes_and_hkts' },
          {
            text: 'Decorators',
            collapsed: true,
            items: [
              { text: 'Overview', link: '/syntax/decorators/' },
              { text: '@static', link: '/syntax/decorators/static' },
              { text: '@native', link: '/syntax/decorators/native' },
              { text: '@deprecated', link: '/syntax/decorators/deprecated' },
              { text: '@debug', link: '/syntax/decorators/debug' },
              { text: '@test', link: '/syntax/decorators/test' },
              { text: '@no_prelude', link: '/syntax/decorators/no_prelude' }
            ]
          },
          { text: 'Operators & Context', link: '/syntax/operators' },
          { text: 'Grammar Reference', link: '/syntax/grammar' }
        ]
      }
    ]
  },
  {
    text: 'Build Native Apps',
    collapsed: true,
    items: [
      { text: 'Mental Model', link: '/stdlib/ui/gtk4/mental_model' },
      { text: 'First Window', link: '/stdlib/ui/gtk4/first_window' },
      { text: 'State & Patches', link: '/stdlib/ui/gtk4/state_patches' },
      { text: 'GTK Sugar', link: '/stdlib/ui/gtk4/gtk_sugar' },
      { text: 'Callbacks', link: '/stdlib/ui/gtk4/callbacks' },
      { text: 'Events', link: '/stdlib/ui/gtk4/events' },
      { text: 'Structure', link: '/stdlib/ui/gtk4/structure' },
      { text: 'Lifecycle', link: '/stdlib/ui/gtk4/lifecycle' },
      { text: 'MCP Debugging', link: '/stdlib/ui/gtk4/mcp_debugging' },
      {
        text: 'Supporting Docs',
        collapsed: true,
        items: [
          { text: 'Signals', link: '/stdlib/ui/reactive_signals' },
          { text: 'Reactive Dataflow', link: '/stdlib/ui/reactive_dataflow' },
          { text: 'Forms', link: '/stdlib/ui/forms' },
          { text: 'Layout', link: '/stdlib/ui/layout' },
          { text: 'Color', link: '/stdlib/ui/color' },
          { text: 'HTML Sigil', link: '/stdlib/ui/html' },
          { text: 'Virtual DOM', link: '/stdlib/ui/vdom' }
        ]
      }
    ]
  },
  {
    text: 'Standard Library',
    collapsed: true,
    items: [
      {
        text: 'Core Building Blocks',
        collapsed: false,
        items: [
          { text: 'Prelude', link: '/stdlib/core/prelude' },
          { text: 'Option', link: '/stdlib/core/option' },
          { text: 'Result', link: '/stdlib/core/result' },
          { text: 'Logic', link: '/stdlib/core/logic' },
          { text: 'Collections', link: '/stdlib/core/collections' },
          { text: 'Generator', link: '/stdlib/core/generator' },
          { text: 'Validation', link: '/stdlib/core/validation' }
        ]
      },
      {
        text: 'Text & Data',
        collapsed: true,
        items: [
          { text: 'Text', link: '/stdlib/core/text' },
          { text: 'Regex', link: '/stdlib/core/regex' },
          { text: 'I18n', link: '/stdlib/core/i18n' },
          { text: 'JSON', link: '/stdlib/data/json' },
          { text: 'Bits', link: '/stdlib/data/bits' },
        ]
      },
      {
        text: 'Math, Units & Models',
        collapsed: true,
        items: [
          { text: 'Units', link: '/stdlib/core/units' },
          { text: 'Math', link: '/stdlib/math/math' },
          { text: 'Numbers', link: '/stdlib/math/number' },
          { text: 'Vector', link: '/stdlib/math/vector' },
          { text: 'Matrix', link: '/stdlib/math/matrix' },
          { text: 'Linear Algebra', link: '/stdlib/math/linear_algebra' },
          { text: 'Geometry', link: '/stdlib/math/geometry' },
          { text: 'Graph', link: '/stdlib/math/graph' },
          { text: 'Tree', link: '/stdlib/math/tree' }
        ]
      },
      {
        text: 'Time & Scheduling',
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
        text: 'Files, System & Security',
        collapsed: true,
        items: [
          { text: 'File', link: '/stdlib/system/file' },
          { text: 'Console', link: '/stdlib/system/console' },
          { text: 'Path', link: '/stdlib/system/path' },
          { text: 'URL', link: '/stdlib/system/url' },
          { text: 'System', link: '/stdlib/system/system' },
          { text: 'Log', link: '/stdlib/system/log' },
          { text: 'Concurrency', link: '/stdlib/system/concurrency' },
          { text: 'Crypto', link: '/stdlib/system/crypto' },
          { text: 'Secrets', link: '/stdlib/system/secrets' },
          { text: 'GNOME Online Accounts', link: '/stdlib/system/gnome_online_accounts' }
        ]
      },
      {
        text: 'Network, Services & Storage',
        collapsed: true,
        items: [
          { text: 'HTTP & HTTPS', link: '/stdlib/network/http' },
          { text: 'HTTP Server', link: '/stdlib/network/http_server' },
          { text: 'REST', link: '/stdlib/network/rest' },
          { text: 'Sockets', link: '/stdlib/network/sockets' },
          { text: 'Streams', link: '/stdlib/network/streams' },
          { text: 'Database', link: '/stdlib/system/database' },
          { text: 'Email', link: '/stdlib/system/email' }
        ]
      }
    ]
  },
  {
    text: 'Testing & Tooling',
    collapsed: true,
    items: [
      {
        text: 'Testing',
        collapsed: false,
        items: [
          { text: 'Testing Module', link: '/stdlib/core/testing' },
          { text: 'Test Decorator & Mocking', link: '/syntax/decorators/test' }
        ]
      },
      {
        text: 'Developer Tools',
        collapsed: true,
        items: [
          { text: 'CLI', link: '/tools/cli' },
          { text: 'Runtime Diagnostics', link: '/tools/runtime_diagnostics' },
          { text: 'REPL', link: '/tools/repl' },
          { text: 'MCP Server', link: '/tools/mcp' },
          { text: 'Package Manager', link: '/tools/package_manager' },
          { text: 'LSP Server', link: '/tools/lsp_server' },
          { text: 'VSCode Extension', link: '/tools/vscode_extension' },
          { text: 'Incremental Compilation', link: '/tools/incremental_compilation' }
        ]
      }
    ]
  },
  {
    text: 'Internals',
    collapsed: true,
    items: [
      { text: 'Compiler & Backend', link: '/typed_codegen/design' },
      { text: 'Minimality Proof', link: '/typed_codegen/minimality' },
      { text: 'Spec Doc Markers', link: '/doc-markers-spec' }
    ]
  }
]
