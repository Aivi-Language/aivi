# Modules

Modules are how AIVI organizes code, controls visibility, and groups related definitions. If you are used to files plus imports in Rust, TypeScript, or Python, the role is familiar even though the syntax is more explicit.

## 10.1 Module Definitions

A module declaration names the file's module and starts the module body.

Modules use a flat form, so most files stay readable without an extra indentation level.

<<< ../snippets/from_md/syntax/modules/module_definitions.aivi{aivi}

Practical rules:

- there is exactly one module per file
- the `module` declaration is the first non-empty item in the file, after any module decorators
- the module body continues to end-of-file

## 10.2 Module Pathing (Dot Separator)

Module names use dot-separated paths. The path is the module's logical namespace, not just a filename.

Common conventions:

- `aivi.…` for the standard library
- `vendor.name.…` for third-party libraries
- `user.app.…` for application code

Module resolution is static and is determined by the project manifest and tooling.

## 10.3 Importing and Scope

Use `use` to bring exported names from another module into scope.

### Basic Import

This is the direct form: import a module so you can use its exported names.

<<< ../snippets/from_md/syntax/modules/basic_import.aivi{aivi}

### Selective / Selective Hiding

Use selective imports when you want to make dependencies obvious, or `hiding` when you want almost everything except a few names.

<<< ../snippets/from_md/syntax/modules/selective_selective_hiding.aivi{aivi}

### Renaming / Aliasing

Aliasing helps when a module name is long or when two imports would otherwise collide.

```aivi
use company.project.analytics as Analytics // shorter name inside this file
```

<<< ../snippets/from_md/syntax/modules/renaming_aliasing.aivi{aivi}

Compiler checks:

- importing a missing module or symbol is a compile-time error
- unused imports produce a warning, unless the import is needed only for a domain side effect

## 10.4 Domain Exports

Modules are also the unit used to publish **domains**. If you are new to domains, think of them as packages of type-directed operator and literal behavior.

Exporting a domain makes its carrier type, delta types, and operators available to importers.

<<< ../snippets/from_md/syntax/modules/domain_exports.aivi{aivi}

A consuming module must import the domain explicitly when it wants the domain-resolved operators:

- `use geo.vector (domain Vector)` imports the domain behavior
- ordinary `use geo.vector` does not automatically activate that domain's operators

## 10.4.1 Inline export declarations

You can put `export` directly in front of a declaration when the file mostly defines public items.

<<< ../snippets/from_md/syntax/modules/block_02.aivi{aivi}


The export-list form is still useful for facade modules and re-exports.

## 10.5 File-Scoped Modules (No Nesting)

Modules are file-scoped. There is no nested `module` syntax inside another module body.

Use a dotted path to express hierarchy instead:

- `module my.app.api` lives in a file such as `my/app/api.aivi`
- related modules form a tree through their paths, not by nesting declarations

### Module Re-exports

A module can act as a small public facade that re-exports a curated API from several implementation modules.

<<< ../snippets/from_md/syntax/modules/module_re_exports.aivi{aivi}

## 10.6 The Prelude

Every module implicitly starts with `use aivi.prelude`. This saves boilerplate for the most common types, functions, and default domains.

If you are writing low-level code and want a completely explicit environment, opt out with:

<<< ../snippets/from_md/syntax/modules/the_prelude.aivi{aivi}

## 10.7 Circular Dependencies

Circular module dependencies are not allowed. The compiler requires the module import graph to be acyclic.

If two parts of the codebase appear to need each other, common fixes are:

- move the mutually recursive parts into one module
- extract a smaller shared module
- depend on functions or interfaces rather than concrete implementations

## 10.8 Practical module organization patterns

These examples show common ways to keep public APIs small while letting implementation modules stay focused.

### Clean App Facade

<<< ../snippets/from_md/syntax/modules/clean_app_facade.aivi{aivi}

### Domain Extension Pattern

<<< ../snippets/from_md/syntax/modules/domain_extension_pattern.aivi{aivi}

### Context-Specific Environments (Static Injection)

This pattern swaps one module implementation for another at compile time. It is useful for test doubles, local development wiring, or alternate backends.

<<< ../snippets/from_md/syntax/modules/context_specific_environments_static_injection_01.aivi{aivi}

<<< ../snippets/from_md/syntax/modules/context_specific_environments_static_injection_02.aivi{aivi}

A test entry point can simply import the test module instead of the production one:

<<< ../snippets/from_md/syntax/modules/context_specific_environments_static_injection_03.aivi{aivi}

## 10.9 Runtime Configuration (Env Vars)

Use module swapping for code structure. Use runtime configuration for values that differ between deployments, such as URLs, credentials, or feature flags.

That configuration should be injected as data, often through the `Env` source.

See [12.4 Environment Sources](external_sources.md#124-environment-sources-env) for details.

<<< ../snippets/from_md/syntax/modules/runtime_configuration_env_vars.aivi{aivi}
