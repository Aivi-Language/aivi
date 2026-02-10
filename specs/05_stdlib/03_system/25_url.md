# URL Domain

The `Url` domain handles **Uniform Resource Locators** without the string-mashing headaches.

A URL isn't just text; it's a structured address with protocols, hosts, and queries. Concatenating strings to build URLs leads to bugs (missing `/`, double `?`, unescaped spaces). This domain treats URLs as safe, structured records, letting you modify protocols or add query parameters without breaking the address.

## Module

```aivi
module aivi.url = {
  export domain Url
  export Url
  export parse, toString
}
```

## Types

```aivi
Url = {
  protocol: String,
  host: String,
  port: Option Int,
  path: String,
  query: List (String, String),
  hash: Option String
}
```

## Domain Definition

```aivi
domain Url over Url = {
  // Add a query parameter
  (+) : Url -> (String, String) -> Url
  (+) url (key, value) = { 
    ...url, 
    query: url.query ++ [(key, value)] 
  }
  
  // Remove a query parameter by key
  (-) : Url -> String -> Url
  (-) url key = { 
    ...url, 
    query: filter (\(k, _) -> k != key) url.query 
  }
  
  // Update record fields (standard record update syntax)
  // url <| { protocol: "https" }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **parse** text<br><pre><code>`String -> Result Url Error`</code></pre> | Converts a URL string into a structured `Url`. |
| **toString** url<br><pre><code>`Url -> String`</code></pre> | Renders a `Url` back into its string form. |

## Usage Examples

```aivi
use aivi.url

// Create using the ~u sigil
base = ~u(https://api.example.com/v1/search)

// Add parameter: "?q=aivi"
search = base + ("q", "aivi")

// Add another: "?q=aivi&sort=desc"
sorted = search + ("sort", "desc")

// Change protocol or path using record update
secure_login = base <| { 
  path: "/v1/login",
  protocol: "wss" 
}
```
