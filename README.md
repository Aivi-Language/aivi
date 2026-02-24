# AIVI

> [!CAUTION]
> This is a vibe coded project. Do not use for anything serious. I'm still rewriting a lot of fundamentals.

AIVI is a functional programming language and toolchain for building strongly typed applications on a Rust-powered foundation. It’s for developers who want the confidence of static types and the clarity of explicit effects, without needing to write Rust as their day‑to‑day language.

AIVI is opinionated in a practical way: it tries to make “the right thing” feel normal. You can keep business logic small and readable, model errors instead of hand-waving them away, and let the compiler do a lot of the heavy lifting when the codebase grows.

## A language you can live in

A big part of choosing a language is whether it keeps up with you after the first week. AIVI is built with tooling as part of the product. The language server powers autocomplete, go-to definition, rename, and real-time diagnostics, and it also makes type information easy to reach when you need it. In VS Code, the extension brings formatting and language intelligence together so your editor stays in sync with how the language actually works.

Instead of treating formatting and “type help” as separate worlds, AIVI leans into the feedback loop: you write a little, you get fast answers, you keep moving. The goal is not just correctness on paper, but a smoother day-to-day experience.

## UI without switching ecosystems

AIVI also has a clear story for building user interfaces without hopping between stacks. If you want interactive web UIs, `aivi.ui.ServerHtml` lets the server render typed view trees to HTML and handle user events in a structured way. There’s a small browser client in `ui-client/` that handles the browser-side wiring (events, patches, and a few platform capabilities) and gets synced into the Rust runtime crates, so the whole system stays cohesive.

If you’d rather ship a native desktop app, `aivi.ui.gtk4` gives you a GTK4 path with AIVI types and functions mapped to runtime bindings. You can keep the same language and modeling approach while targeting a very different UI surface.

## Domains that make math feel safer

AIVI’s “domains” feature is a quiet superpower: it helps you express real-world concepts directly in code. Instead of turning everything into plain numbers and hoping conventions hold, domains can give meaning to operators and literals, including unit suffixes and delta values. Writing things like `10ms` or `20deg` can stay readable while still being checked and interpreted in a way that matches the problem space.

This pairs naturally with the standard library’s math modules (vectors, matrices, linear algebra, geometry, probability, and signal work), so you can do serious calculations without rebuilding foundations in every project.

## Specs and AI-friendly workflows

AIVI keeps its specs close to the implementation, and the toolchain can serve them to other tools. In v0.1, `aivi mcp serve` exposes the language specifications as MCP resources, which makes it easier to plug AIVI into modern AI-assisted workflows and internal developer tooling.

## Getting started

If you have Rust installed, you can install the CLI from this repo and explore what it can do:

```bash
cargo install --path crates/aivi

aivi --help
```

From there, you can format and check AIVI code, run the language server (for editors that use LSP), and explore the `specs/` folder for the language and standard library documentation.
