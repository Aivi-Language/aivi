# Modules

## 10.1 Module Definitions

Modules are the primary unit of code organization, encapsulation, and reuse in AIVI. They define a closed scope and explicitly export symbols for public use.

Modules can be written in a **flat** form that keeps file indentation shallow. The module body runs until end-of-file:

<<< ../snippets/from_md/02_syntax/10_modules/block_01.aivi{aivi}

In v0.1, there is exactly one module per file. The `module` declaration must be the first non-empty item in the file (after any module decorators), and its body extends to EOF.


## 10.2 Module Pathing (Dot Separator)

Modules are identified by hierarchical paths using common **dot notation**. This separates logical namespaces. By convention:
- `aivi.*`   Standard library
- `vendor.name.*`   Foreign libraries
- `user.app.*`   Application-specific logic

Module resolution is static and determined at compile time based on the project manifest.


## 10.3 Importing and Scope

Use the `use` keyword to bring symbols from another module into the current scope.

### Basic Import

<<< ../snippets/from_md/02_syntax/10_modules/block_02.aivi{aivi}

### Selective / Selective Hiding

<<< ../snippets/from_md/02_syntax/10_modules/block_03.aivi{aivi}

### Renaming / Aliasing

<<< ../snippets/from_md/02_syntax/10_modules/block_04.aivi{aivi}

Compiler checks:

- Importing a missing module or symbol is a compile-time error.
- Unused imports produce a warning (suppressed if importing solely for a domain side-effect in v0.1).


## 10.4 Domain Exports

Modules are the primary vehicle for delivering **Domains**. Exporting a domain automatically exports its carrier type, delta types, and operators.

<<< ../snippets/from_md/02_syntax/10_modules/block_05.aivi{aivi}

When another module imports the domain (for example: `use geo.vector (domain Vector)`), it gains the ability to use domain-resolved operators like `+` on `Vec2` records.

In v0.1, importing a module does not automatically activate its domains; domains must be imported explicitly via `use ... (domain Name)` (or by importing specific operator/template bindings as values).


## 10.5 File-Scoped Modules (No Nesting)

In v0.1, modules are **file-scoped**. There is no `{ ... }` module body syntax and no nested module declarations.

Use dot notation to define hierarchy: `module my.app.api` lives in a file such as `my/app/api.aivi` (project layout is determined by the manifest and tooling).

### Module Re-exports
A module can aggregate other modules, acting as a facade.

<<< ../snippets/from_md/02_syntax/10_modules/block_07.aivi{aivi}


## 10.6 The Prelude

Every AIVI module implicitly starts with `use aivi.prelude`. This provides access to the core language types and the most common domains without boilerplate.

To opt-out of this behavior (mandatory for the core stdlib itself):

<<< ../snippets/from_md/02_syntax/10_modules/block_08.aivi{aivi}


## 10.7 Circular Dependencies

Circular module dependencies are **strictly prohibited** at the import level. The compiler enforces a Directed Acyclic Graph (DAG) for module resolution. For mutually recursive types or functions, they must reside within the same module or be decoupled via higher-order abstractions.
## 10.8 Expressive Module Orchestration

Modules allow for building clean, layered architectures where complex internal implementations are hidden behind simple, expressive facades.

### Clean App Facade

<<< ../snippets/from_md/02_syntax/10_modules/block_09.aivi{aivi}

### Domain Extension Pattern

<<< ../snippets/from_md/02_syntax/10_modules/block_10.aivi{aivi}

### Context-Specific Environments (Static Injection)

This pattern allows you to **statically swap** entire module implementations for different build contexts (e.g., Test vs. Prod). This is not for runtime configuration (see below), but for compile-time substitution of logic.

<<< ../snippets/from_md/02_syntax/10_modules/block_11.aivi{aivi}

<<< ../snippets/from_md/02_syntax/10_modules/block_11_test.aivi{aivi}

To use the test environment, your test entry point (`tests/main.aivi`) simply imports the test module instead of the production one:

<<< ../snippets/from_md/02_syntax/10_modules/block_12.aivi{aivi}

## 10.9 Runtime Configuration (Env Vars)

For values that change between deployments (like API URLs or DB passwords) without changing code, use **Runtime Configuration** via the `Env` source.

Do not use module swapping for this. Instead, inject the configuration as data.

See [12.4 Environment Sources](12_external_sources.md#124-environment-sources-env) for details.

<<< ../snippets/from_md/02_syntax/10_modules/block_13.aivi{aivi}
