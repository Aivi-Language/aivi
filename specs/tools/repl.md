# REPL

<img width="2878" height="1551" alt="image" src="https://github.com/user-attachments/assets/7e1860f3-8f42-46e0-8f4a-daebeedd68a4" />

The AIVI REPL (`aivi repl`) is the fastest way to try expressions, inspect inferred types, load files into a temporary session, and explore the standard library without scaffolding a full project first.

If you mainly want the command-line reference, see [CLI](cli.md#repl). This page focuses on the workflow and interactive behavior.

## Start here

If you are new to the REPL, this is the shortest useful path:

1. Run `aivi repl`.
2. Type `1 + 2` and press `Enter`.
3. Type `/use aivi.text` to bring a module into the session explicitly.
4. Type `/functions text` or `/explain aivi.text.isAlnum` to discover what's available.
5. Use `/load path/to/file.aivi` when you want to pull a scratch file into the current session.

## Mental model

The REPL keeps a live session in memory.

- the prelude is pre-loaded
- stdlib symbols are immediately in scope
- successful expression inputs are compiled and evaluated, and the transcript shows `value :: Type`
- top-level effect expressions autorun by default, so inputs such as `print "hi"` or `println "hi"` execute immediately
- session bindings, imports, and loaded files remain available until you exit or run `/reset`

If you want top-level effects to stay as inert values instead of executing, run `/autorun off`.

## Launching the REPL

```bash
aivi repl [--color] [--no-color] [--plain]
```

| Flag | Meaning |
| --- | --- |
| `--color` | Force ANSI color output even when stdout is not a terminal. |
| `--no-color` | Disable ANSI color output. |
| `--plain` | Use plain read-eval-print mode instead of the full-screen TUI. This is pipe-friendly and is selected automatically when stdin is not a terminal. |

With no flags, `aivi repl` opens the full-screen TUI.

## Runtime failures in the REPL

The REPL should present runtime failures using the same underlying runtime-diagnostic model as `aivi run`, including:

- a concise summary
- the best available source location
- notes and help text
- a call chain when one is available

In **plain mode**, the REPL renders that information as ordinary terminal output using the selected color mode.

In **TUI mode**, the REPL may style the same data with TUI widgets instead of raw ANSI escapes, but it should not throw away important details such as source frames or stack information simply because the user is in an interactive session.

See also [Runtime Diagnostics](runtime_diagnostics.md) for the shared contract.

## TUI mode vs plain mode

Use the default TUI when you want history navigation, inline suggestions, and the symbol pane.

Use `--plain` when you want a minimal transcript-oriented interface, when you are piping input, or when you are driving the REPL from another tool.

Both modes evaluate the same language and keep the same session semantics; the difference is presentation and interaction affordances.

## Common interactive workflows

### Try a small expression

Run:

```text
1 + 2
```

The REPL evaluates the expression immediately and shows both the result and its inferred type.

### Inspect symbols and documentation

Use slash commands to discover what is in scope:

```text
/functions text
/types
/explain aivi.text.isAlnum
```

`/explain` prints the indexed quick-info text together with the best available signature and the module where that symbol lives. When the same name exists in more than one module, use a qualified name such as `/explain aivi.text.isAlnum`.

### Bring more code into the session

Use:

```text
/use aivi.text
/load path/to/file.aivi
/modules
```

- `/use` adds a module import to the session and errors immediately if the module does not exist
- `/load` loads a `.aivi` file into the current session
- `/modules` shows which modules are currently loaded

### Control effect execution

Top-level effects autorun by default:

```text
println "hi"
```

If you want to inspect or build effect values without executing them on entry, turn autorun off:

```text
/autorun off
```

## Keyboard shortcuts (TUI mode)

| Key | Action |
| --- | --- |
| Enter | Submit the current input |
| Shift+Enter | Insert newline for multi-line input |
| ↑ / ↓ | Navigate history or inline suggestions |
| Ctrl+L | Clear the transcript |
| Ctrl+C | Cancel the current input |
| Ctrl+D | Exit when the input buffer is empty |
| Tab | Accept the highlighted suggestion, or toggle the symbol pane when no suggestion is shown |
| Esc | Close the symbol pane |

## Slash commands

| Command | Description |
| --- | --- |
| `/help` | Print command reference. |
| `/explain <name>` | Show quick info, the best available signature, and the module where that symbol lives. |
| `/use <module.path>` | Add an import to the session, for example `/use aivi.text`. |
| `/types [filter]` | List types in scope, with an optional substring filter. |
| `/values [filter]` | List session-defined values with their inferred types. |
| `/functions [filter]` | List functions in scope with their module names. |
| `/autorun [on\|off]` | Toggle whether top-level effect expressions execute automatically. |
| `/modules` | Show loaded modules in the session. |
| `/clear` | Clear the transcript while keeping session state. |
| `/reset` | Clear the transcript and reset all session state. |
| `/history [n]` | Show the last `n` inputs, defaulting to `20`. |
| `/load <path>` | Load a `.aivi` file into the session. |
| `/openapi file <path> [as <name>]` | Inject an OpenAPI spec file as a `@static` module. |
| `/openapi url <url> [as <name>]` | Inject an OpenAPI spec from a URL as a `@static` module. |

The `/openapi` commands inject typed API bindings into the session under the given module name, or under a name derived from the spec's `info.title` when you omit `as <name>`. They use the same `@static` OpenAPI mechanism as the language's compile-time external source support.

## Suggestions and completion

When your input starts with `/`, the TUI shows matching slash commands inline and filters them as you type.

For ordinary identifier input, the REPL suggests constructors, values, and functions that are currently in scope. `/functions <filter>` also suggests matching function names for the filter slot.

The suggestion popup shows five rows at a time, but `↑` and `↓` scroll through the full result set. `Tab` accepts the highlighted item, and `Enter` always runs the current input buffer.

## When to reach for the REPL

Use `aivi repl` when you want to:

- try syntax or stdlib calls quickly
- inspect inferred types without creating a test file
- experiment with imports, values, and helper functions in a temporary session
- load a local file and explore it interactively

Prefer other tools when your goal is different:

| Goal | Better command |
| --- | --- |
| Run a full project or entry file | `aivi run` |
| See diagnostics without executing code | `aivi check` |
| Run `@test` definitions | `aivi test` |
| Inspect compiler lowering stages | `aivi parse`, `aivi desugar`, `aivi kernel`, `aivi rust-ir` |
