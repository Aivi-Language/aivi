# Sigils

Sigils provide custom parsing for complex literals. They start with `~` followed by a tag and a delimiter.

<<< ../snippets/02_syntax/13_sigils/basic.aivi{aivi}

Domains define these sigils to validate and construct types at compile time.

## Structured sigils

Some domains parse sigils as **AIVI expressions** rather than raw text. The `Collections` domain defines:

<<< ../snippets/02_syntax/13_sigils/structured.aivi{aivi}

In addition, the UI layer defines a structured HTML sigil:

- `~html{ <div>{ expr }</div> }` lowers to typed `aivi.ui.VNode` constructors and supports `{ expr }` splices.

The exact meaning of a sigil is domain-defined (or compiler-provided for some stdlib features); see [Collections](../05_stdlib/00_core/28_collections.md) for `~map` and `~set`, and [UI](../05_stdlib/04_ui/03_html.md) for `~html`.
