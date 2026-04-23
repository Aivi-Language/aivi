import type { DefaultTheme } from 'vitepress'

type DocLink = {
  text: string
  link: string
}

type DocSection = {
  text: string
  collapsed?: boolean
  items: DocNode[]
}

type DocNode = DocLink | DocSection

function isSection(item: DocNode): item is DocSection {
  return 'items' in item
}

function sidebarItem(item: DocNode): DefaultTheme.SidebarItem {
  if (!isSection(item)) {
    return { text: item.text, link: item.link }
  }

  return {
    text: item.text,
    collapsed: item.collapsed,
    items: item.items.map(sidebarItem),
  }
}

function group(section: DocSection): DefaultTheme.SidebarItem {
  return sidebarItem(section)
}

const tutorialsSection: DocSection = {
  text: 'Tutorials',
  collapsed: false,
  items: [
    { text: 'Overview', link: '/tutorials/' },
    { text: 'Start Here', link: '/guide/getting-started' },
    { text: 'Build a Small Task Tracker', link: '/guide/your-first-app' },
    { text: 'Optional Deep Example: Snake', link: '/guide/building-snake' },
  ],
}

const howToSection: DocSection = {
  text: 'How-to Guides',
  collapsed: false,
  items: [
    { text: 'Overview', link: '/how-to/' },
    { text: 'Fetch HTTP Data', link: '/how-to/fetch-http-data' },
    { text: 'Model Loading and Error States', link: '/how-to/loading-and-error-states' },
    { text: 'Work with Timers', link: '/how-to/work-with-timers' },
    { text: 'Organize Modules', link: '/how-to/organize-modules' },
    { text: 'Launch Multiple Entries', link: '/how-to/launch-multiple-entries' },
    { text: 'OpenAPI-backed Services', link: '/guide/openapi-source' },
    { text: 'Integration Patterns', link: '/guide/integrations' },
  ],
}

const referenceSection: DocSection = {
  text: 'Reference',
  collapsed: false,
  items: [
    { text: 'Overview', link: '/reference/' },
    {
      text: 'Core language',
      collapsed: false,
      items: [
        { text: 'Values & Functions', link: '/guide/values-and-functions' },
        { text: 'Types', link: '/guide/types' },
        { text: 'Pipes & Operators', link: '/guide/pipes' },
        { text: 'Pattern Matching', link: '/guide/pattern-matching' },
        { text: 'Record Patterns', link: '/guide/record-patterns' },
        { text: 'Predicates & Selectors', link: '/guide/predicates' },
        { text: 'Domains', link: '/guide/domains' },
      ],
    },
    {
      text: 'Reactivity and UI',
      collapsed: false,
      items: [
        { text: 'Signals', link: '/guide/signals' },
        { text: 'Sources', link: '/guide/sources' },
        { text: 'Built-in Source Catalog', link: '/guide/source-catalog' },
        { text: 'Markup & UI', link: '/guide/markup' },
      ],
    },
    {
      text: 'Abstractions and structure',
      collapsed: true,
      items: [
        { text: 'Classes', link: '/guide/classes' },
        { text: 'Typeclasses & HKTs', link: '/guide/typeclasses' },
        { text: 'Class Laws & Boundaries', link: '/guide/class-laws' },
        { text: 'Modules', link: '/guide/modules' },
      ],
    },
  ],
}

const explanationSection: DocSection = {
  text: 'Explanation',
  collapsed: false,
  items: [
    { text: 'Overview', link: '/explanation/' },
    { text: 'Why AIVI?', link: '/guide/why-aivi' },
    { text: 'If You Are New to Functional Programming', link: '/explanation/functional-programming-bridge' },
    { text: 'Thinking in AIVI', link: '/guide/thinking-in-aivi' },
  ],
}

const examplesSection: DocSection = {
  text: 'Examples',
  collapsed: true,
  items: [
    { text: 'Snake', link: '/guide/building-snake' },
  ],
}

const stdlibStartHere: DocSection = {
  text: 'Start Here',
  collapsed: false,
  items: [
    { text: 'Overview', link: '/stdlib/' },
    { text: 'Prelude', link: '/stdlib/prelude' },
    { text: 'Default Values', link: '/stdlib/defaults' },
  ],
}

const stdlibSections: DocSection[] = [
  {
    text: 'Core Values & Collections',
    collapsed: true,
    items: [
      { text: 'Async Tracker', link: '/stdlib/async' },
      { text: 'Boolean Logic', link: '/stdlib/bool' },
      { text: 'Optional Values', link: '/stdlib/option' },
      { text: 'Result Values', link: '/stdlib/result' },
      { text: 'Validation', link: '/stdlib/validation' },
      { text: 'Either Values', link: '/stdlib/either' },
      { text: 'Lists', link: '/stdlib/list' },
      { text: 'Matrices', link: '/stdlib/matrix' },
      { text: 'Non-Empty Lists', link: '/stdlib/nonEmpty' },
      { text: 'Pairs', link: '/stdlib/pair' },
      { text: 'Ordering & Comparison', link: '/stdlib/order' },
      { text: 'Dictionaries', link: '/stdlib/dict' },
      { text: 'Sets', link: '/stdlib/set' },
      { text: 'Ranges', link: '/stdlib/range' },
      { text: 'Function Helpers', link: '/stdlib/fn' },
    ],
  },
  {
    text: 'Numbers, Text & Data',
    collapsed: true,
    items: [
      { text: 'Arithmetic Intrinsics', link: '/stdlib/arithmetic' },
      { text: 'Bitwise Intrinsics', link: '/stdlib/bits' },
      { text: 'Math', link: '/stdlib/math' },
      { text: 'Floating-Point Numbers', link: '/stdlib/float' },
      { text: 'Big Integers', link: '/stdlib/bigint' },
      { text: 'JSON', link: '/stdlib/json' },
      { text: 'Text Processing', link: '/stdlib/text' },
      { text: 'Regular Expressions', link: '/stdlib/regex' },
      { text: 'Byte Buffers', link: '/stdlib/bytes' },
    ],
  },
  {
    text: 'Time, Randomness & Scheduling',
    collapsed: true,
    items: [
      { text: 'Dates & Calendar', link: '/stdlib/date' },
      { text: 'Durations', link: '/stdlib/duration' },
      { text: 'Time', link: '/stdlib/time' },
      { text: 'Timers', link: '/stdlib/timer' },
      { text: 'Randomness', link: '/stdlib/random' },
    ],
  },
  {
    text: 'Files, Environment & Processes',
    collapsed: true,
    items: [
      { text: 'File System', link: '/stdlib/fs' },
      { text: 'Paths', link: '/stdlib/path' },
      { text: 'Environment Variables', link: '/stdlib/env' },
      { text: 'Standard I/O', link: '/stdlib/stdio' },
      { text: 'Logging', link: '/stdlib/log' },
      { text: 'Processes', link: '/stdlib/process' },
    ],
  },
  {
    text: 'Network & Services',
    collapsed: true,
    items: [
      { text: 'URLs', link: '/stdlib/url' },
      { text: 'HTTP', link: '/stdlib/http' },
      { text: 'API Vocabulary', link: '/stdlib/api' },
      { text: 'Authentication', link: '/stdlib/auth' },
      { text: 'Databases', link: '/stdlib/db' },
      { text: 'IMAP', link: '/stdlib/imap' },
      { text: 'SMTP', link: '/stdlib/smtp' },
    ],
  },
  {
    text: 'Desktop, UI & GNOME',
    collapsed: true,
    items: [
      { text: 'Application Framework', link: '/stdlib/app' },
      { text: 'Application Lifecycle', link: '/stdlib/lifecycle' },
      { text: 'XDG Directories', link: '/stdlib/xdg' },
      { text: 'Portals', link: '/stdlib/portal' },
      { text: 'D-Bus', link: '/stdlib/dbus' },
      { text: 'GNOME Settings', link: '/stdlib/settings' },
      { text: 'Online Accounts', link: '/stdlib/onlineAccounts' },
      { text: 'Desktop Notifications', link: '/stdlib/notifications' },
      { text: 'Clipboard', link: '/stdlib/clipboard' },
      { text: 'Colors', link: '/stdlib/color' },
      { text: 'Images', link: '/stdlib/image' },
      { text: 'GResources', link: '/stdlib/gresource' },
      { text: 'Internationalization', link: '/stdlib/i18n' },
    ],
  },
]

const stdlibGuideSection: DocSection = {
  text: 'Standard Library',
  collapsed: true,
  items: [
    stdlibStartHere,
    ...stdlibSections,
  ],
}

export const nav: DefaultTheme.NavItem[] = [
  { text: 'Tutorials', link: '/tutorials/' },
  { text: 'How-to', link: '/how-to/' },
  { text: 'Reference', link: '/reference/' },
  { text: 'Explanation', link: '/explanation/' },
  { text: 'Standard Library', link: '/stdlib/' },
]

const manualSidebar = [
  group(tutorialsSection),
  group(howToSection),
  group(referenceSection),
  group(explanationSection),
  group(examplesSection),
  group(stdlibGuideSection),
]

export const sidebar: DefaultTheme.SidebarMulti = {
  '/tutorials/': manualSidebar,
  '/how-to/': manualSidebar,
  '/reference/': manualSidebar,
  '/explanation/': manualSidebar,
  '/guide/': manualSidebar,
  '/stdlib/': [
    group({
      text: 'Manual',
      collapsed: false,
      items: [
        { text: 'Tutorials', link: '/tutorials/' },
        { text: 'How-to Guides', link: '/how-to/' },
        { text: 'Reference', link: '/reference/' },
        { text: 'Explanation', link: '/explanation/' },
      ],
    }),
    group(stdlibStartHere),
    ...stdlibSections.map(group),
  ],
  '/': [
    group({
      text: 'Manual',
      collapsed: false,
      items: [
        { text: 'Tutorials', link: '/tutorials/' },
        { text: 'How-to Guides', link: '/how-to/' },
        { text: 'Reference', link: '/reference/' },
        { text: 'Explanation', link: '/explanation/' },
        { text: 'Standard Library', link: '/stdlib/' },
      ],
    }),
  ],
}
