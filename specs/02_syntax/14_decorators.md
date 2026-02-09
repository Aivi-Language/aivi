# Decorators

Decorators provide **compile-time metadata** attached to definitions.

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
| `@static` | `@static x = file.read \`...\`` | Embed at compile time |
| `@inline` | `@inline f = ...` | Always inline function |
| `@deprecated` | `@deprecated msg` | Emit warning on use |

### Database (SQLite Domain)

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@table` | `@table \`users\`` | Bind type to SQL table |
| `@primary` | `id: Int @primary` | Primary key column |
| `@auto` | `id: Int @auto` | Auto-increment |
| `@unique` | `email: Text @unique` | Unique constraint |
| `@default` | `createdAt: Instant @default now` | Default value |
| `@migration` | `@migration \`001_name\`` | Database migration |

### Tooling (MCP)

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@mcp_tool` | `@mcp_tool fetchData = ...` | Expose as MCP tool |
| `@mcp_resource` | `@mcp_resource config = ...` | Expose as MCP resource |

### Pragmas (Module-level)

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@no_prelude` | `@no_prelude module M = ...` | Skip implicit prelude import |


## 14.3 Field Decorators

Decorators on record fields:

```aivi
@table "users"
User = {
  id: Int @primary @auto
  name: Text
  email: Text @unique
  role: Role @default Guest
  deletedAt: Option Instant
}
```

Multiple decorators stack left-to-right.


## 14.4 Custom Decorators

Decorators are extensible (future):

```aivi
decorator validate = { schema: JsonSchema } => ...
decorator cache = { ttl: Duration } => ...

@validate { schema: userSchema }
@cache { ttl: 5min }
fetchUser = id => ...
```


## 14.5 Decorator Desugaring

Decorators desugar to compile-time metadata:

| Surface | Desugared |
| :--- | :--- |
| `@table \`users\` User = {...}` | `User` + `TableMeta User "users"` |
| `@static x = file.read ...` | Compile-time evaluation |
| `@mcp_tool f = ...` | Register in MCP manifest |


## 14.6 Usage Examples

### Database Table

```aivi
@table "posts"
Post = {
  id: Int @primary @auto
  title: Text
  body: Text
  authorId: Int
  published: Bool @default False
  createdAt: Instant @default now
}
```

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
