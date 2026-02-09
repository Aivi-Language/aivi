# URL Domain

The `Url` domain provides a structured, type-safe way to handle **Uniform Resource Locators**.

## What is a URL?

A URL is the address of a resource on the internet. It's not just a string of text; it has distinct components:
*   **Protocol**: How to talk (e.g., `https`, `mailto`).
*   **Host**: Who has the data (e.g., `api.github.com`).
*   **Path**: Where it is (e.g., `/users/1`).
*   **Query**: Extra parameters (e.g., `?sort=desc`).

## Why this exists

Manipulating URLs as raw strings is brittle. Concatenating strings often leads to duplicate slashes (`//api//v1`), missing `?` or `&` in queries, or unescaped characters causing bugs.
This domain treats URLs as **Records**, allowing you to modify specific parts (like adding a query parameter) safely without breaking the rest of the address.

## Module

```aivi
module aivi.std.system.url = {
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

```aivi
parse : String -> Result Url Error
parse str = // ... implementation ...

toString : Url -> String
toString url = // ... reconstruct string ...
```

## Usage Examples

```aivi
use aivi.std.system.url

// Create using the ~u sigil
let base = ~u(https://api.example.com/v1/search)

// Add parameter: "?q=aivi"
let search = base + ("q", "aivi")

// Add another: "?q=aivi&sort=desc"
let sorted = search + ("sort", "desc")

// Change protocol or path using record update
let secure_login = base <| { 
  path: "/v1/login",
  protocol: "wss" 
}
```
