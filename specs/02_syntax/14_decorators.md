# Decorators

Decorators provide **compile-time metadata** attached to definitions.

## Policy (Constraints)

Decorators are intentionally narrow:

- Decorators MUST NOT be used to model domain semantics (e.g. database schemas/ORM, SQL, HTTP, validation rules).
- Integration behavior belongs in **typed values** (e.g. `Source` configurations) and **types** (decoders), not hidden in decorators.
- Only the standard decorators listed here are allowed in v0.1. Unknown decorators are a compile error.
- User-defined decorators are not supported in v0.1.

## 14.1 Syntax

```aivi
@decorator_name
@decorator_name value
@decorator_name { key: value }
```

Decorators appear before the binding they annotate.


## 14.2 Standard Decorators

### Compile-Time

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@static` | `@static x = file.read "..."` | Embed at compile time |
| `@inline` | `@inline f = ...` | Always inline function |
| `@deprecated` | `@deprecated msg` | Emit warning on use |

### Tooling (MCP)

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@mcp_tool` | `@mcp_tool fetchData = ...` | Expose as MCP tool |
| `@mcp_resource` | `@mcp_resource config = ...` | Expose as MCP resource |

### Testing

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@test` | `@test add_is_commutative = ...` | Mark a definition as a test case |

### Pragmas (Module-level)
| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@no_prelude` | `@no_prelude module M = ...` | Skip implicit prelude import |
## 14.3 Decorator Desugaring

Decorators desugar to compile-time metadata:

| Surface | Desugared |
| :--- | :--- |
| `@static x = file.read ...` | Compile-time evaluation |
| `@mcp_tool f = ...` | Register in MCP manifest |


## 14.4 Usage Examples

### Compile-Time Embedding

```aivi
@static
version : Text
version = file.read "./VERSION"

@static
schema : JsonSchema
schema = file.json "./schema.json"
```

### MCP Tools

```aivi
@mcp_tool
searchDocs : Query -> Effect Http (List Document)
searchDocs query = http.get "https://api.example.com/search?q={query}"

@mcp_resource
appConfig : Source File Config
appConfig = file.json "./config.json"
```
