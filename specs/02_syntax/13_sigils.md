# Sigils

Sigils provide custom parsing for complex literals. They start with `~` followed by a tag and a delimiter.

<<< ../snippets/02_syntax/13_sigils/basic.aivi{aivi}

Domains define these sigils to validate and construct types at compile time.

## Structured sigils

Some domains parse sigils as **AIVI expressions** rather than raw text. For v1.0, the `Collections` domain defines:

<<< ../snippets/02_syntax/13_sigils/structured.aivi{aivi}

The exact meaning of a sigil is domain-defined; see [Collections](../05_stdlib/00_core/28_collections.md) for `~map` and `~set`.
