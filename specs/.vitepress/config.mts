import { defineConfig } from "vitepress";

function resolveBase(): string {
  const repo = process.env.GITHUB_REPOSITORY;
  if (!repo) return "/";
  const [, repoName] = repo.split("/");
  return repoName ? `/${repoName}/` : "/";
}

function resolveRepoUrl(): string | undefined {
  const repo = process.env.GITHUB_REPOSITORY;
  return repo ? `https://github.com/${repo}` : undefined;
}

export default defineConfig({
  lang: "en-US",
  title: "AIVI",
  description: "AIVI Language Specification",
  base: process.env.CI ? resolveBase() : "/",
  cleanUrls: false,
  lastUpdated: true,
  themeConfig: {
    nav: [
      { text: "Spec", link: "/" },
      { text: "Syntax", link: "/02_syntax/00_grammar" },
      { text: "Kernel", link: "/03_kernel/01_core_terms" }
    ],
    sidebar: [
      {
        text: "Specification",
        items: [
          { text: "Introduction", link: "/01_introduction" }
        ]
      },
      {
        text: "Syntax",
        collapsed: false,
        items: [
          { text: "Concrete Syntax (EBNF draft)", link: "/02_syntax/00_grammar" },
          { text: "Bindings and Scope", link: "/02_syntax/01_bindings" },
          { text: "Functions and Pipes", link: "/02_syntax/02_functions" },
          { text: "The Type System", link: "/02_syntax/03_types" },
          { text: "Predicates", link: "/02_syntax/04_predicates" },
          { text: "Patching Records", link: "/02_syntax/05_patching" },
          { text: "Domains, Units, and Deltas", link: "/02_syntax/06_domains" },
          { text: "Generators", link: "/02_syntax/07_generators" },
          { text: "Pattern Matching", link: "/02_syntax/08_pattern_matching" },
          { text: "Effects", link: "/02_syntax/09_effects" },
          { text: "Modules", link: "/02_syntax/10_modules" },
          { text: "Domain Definitions", link: "/02_syntax/11_domain_definition" },
          { text: "External Sources", link: "/02_syntax/12_external_sources" },
          { text: "JSX Literals", link: "/02_syntax/13_jsx_literals" },
          { text: "Decorators", link: "/02_syntax/14_decorators" },
          { text: "Resources", link: "/02_syntax/15_resources" }
        ]
      },
      {
        text: "Kernel",
        collapsed: true,
        items: [
          { text: "Core Terms", link: "/03_kernel/01_core_terms" },
          { text: "Types", link: "/03_kernel/02_types" },
          { text: "Records", link: "/03_kernel/03_records" },
          { text: "Patterns", link: "/03_kernel/04_patterns" },
          { text: "Predicates", link: "/03_kernel/05_predicates" },
          { text: "Traversals", link: "/03_kernel/06_traversals" },
          { text: "Generators", link: "/03_kernel/07_generators" },
          { text: "Effects", link: "/03_kernel/08_effects" },
          { text: "Classes", link: "/03_kernel/09_classes" },
          { text: "Domains", link: "/03_kernel/10_domains" },
          { text: "Patching", link: "/03_kernel/11_patching" },
          { text: "Minimality Proof", link: "/03_kernel/12_minimality" }
        ]
      },
      {
        text: "Desugaring",
        collapsed: true,
        items: [
          { text: "Bindings", link: "/04_desugaring/01_bindings" },
          { text: "Functions", link: "/04_desugaring/02_functions" },
          { text: "Records", link: "/04_desugaring/03_records" },
          { text: "Patterns", link: "/04_desugaring/04_patterns" },
          { text: "Predicates", link: "/04_desugaring/05_predicates" },
          { text: "Generators", link: "/04_desugaring/06_generators" },
          { text: "Effects", link: "/04_desugaring/07_effects" },
          { text: "Classes", link: "/04_desugaring/08_classes" },
          { text: "Domains and Operators", link: "/04_desugaring/09_domains" },
          { text: "Patching", link: "/04_desugaring/10_patching" }
        ]
      },
      {
        text: "Standard Library",
        collapsed: true,
        items: [
          { text: "Prelude", link: "/05_stdlib/01_prelude" },
          { text: "Calendar", link: "/05_stdlib/02_calendar" },
          { text: "Duration", link: "/05_stdlib/03_duration" },
          { text: "Color", link: "/05_stdlib/04_color" },
          { text: "Vector", link: "/05_stdlib/05_vector" },
          { text: "HTML", link: "/05_stdlib/06_html" },
          { text: "Style", link: "/05_stdlib/07_style" },
          { text: "SQLite", link: "/05_stdlib/08_sqlite" }
        ]
      },
      {
        text: "Runtime",
        collapsed: true,
        items: [{ text: "Concurrency", link: "/06_runtime/01_concurrency" }]
      },
      {
        text: "Guides",
        collapsed: true,
        items: [
          { text: "From TypeScript", link: "/guides/01_from_typescript" },
          { text: "From Haskell", link: "/guides/02_from_haskell" }
        ]
      },
      {
        text: "Ideas",
        collapsed: true,
        items: [
          { text: "WASM Target", link: "/ideas/01_wasm_target" },
          { text: "LiveView Frontend", link: "/ideas/02_liveview_frontend" },
          { text: "HTML Domains", link: "/ideas/03_html_domains" },
          { text: "Meta-Domain", link: "/ideas/04_meta_domain" },
          { text: "Tooling", link: "/ideas/05_tooling" }
        ]
      },
      {
        text: "Meta",
        collapsed: true,
        items: [
          { text: "TODO", link: "/TODO" },
          { text: "Open Questions", link: "/OPEN_QUESTIONS" }
        ]
      }
    ],
    search: {
      provider: "local"
    },
    socialLinks: resolveRepoUrl() ? [{ icon: "github", link: resolveRepoUrl()! }] : []
  }
});
