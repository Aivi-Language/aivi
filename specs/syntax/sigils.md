# Sigils

Sigils provide custom parsing for complex literals. They start with `~` followed by a tag and a delimiter.

<<< ../snippets/syntax/sigils/basic.aivi{aivi}

Domains define these sigils to validate and construct types at compile time.

Some sigils are compiler-provided and backed by stdlib domains, for example:

- `~u(https://example.com)` / `~url(https://example.com)` for `aivi.url.Url`
- `~path[/usr/local/bin]` for `aivi.path.Path`
- `~mat[...]` for matrix literals (`aivi.matrix.Mat2`, `aivi.matrix.Mat3`, `aivi.matrix.Mat4`)

## Structured sigils

Some domains parse sigils as **AIVI expressions** rather than raw text. The `Collections` domain defines:

<<< ../snippets/syntax/sigils/structured.aivi{aivi}

The `Matrix` domain defines a structured matrix literal sigil, `~mat[...]`; see [Matrix](../stdlib/math/matrix.md).

In addition, the UI layer defines a structured HTML sigil:

- `~<html>...</html>` for HTML literals to typed `aivi.ui.VNode` constructors and supports `{ expr }` splices.
- `~<gtk>...</gtk>` for GtkBuilder-style XML literals to typed `aivi.ui.gtk4.GtkNode` constructors.
  - `props={ { marginTop: 24, spacing: 24 } }` is sugar that lowers to normalized GTK property entries (`margin-top`, `spacing`).
  - `props` only accepts compile-time record literals in v0.1; non-literal values are diagnostics.

The exact meaning of a sigil is domain-defined (or compiler-provided for some stdlib features); see [Collections](../stdlib/core/collections.md) for `~map` and `~set`, and [UI](../stdlib/ui/html.md) for `~html`.
