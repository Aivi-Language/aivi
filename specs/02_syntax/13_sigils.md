# Sigils

Sigils provide custom parsing for complex literals. They start with `~` followed by a tag and a delimiter.

```aivi
// Regex
pattern = ~r/\w+@\w+\.\w+/

// URL
endpoint = ~u(https://api.example.com)

// Date
birthday = ~d(1990-12-31)
```

Domains define these sigils to validate and construct types at compile time.

## Structured sigils

Some domains parse sigils as **AIVI expressions** rather than raw text. For v1.0, the `Collections` domain defines:

```aivi
// Map literal (entries use =>, spread with ...)
users = ~map{
  "id-1" => { name: "Alice" }
  "id-2" => { name: "Bob" }
}

// Set literal (spread with ...)
tags = ~set[...baseTags, "hot", "new"]
```

The exact meaning of a sigil is domain-defined; see [Collections](../05_stdlib/00_core/28_collections.md) for `~map` and `~set`.
