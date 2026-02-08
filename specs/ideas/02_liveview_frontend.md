# Idea: Elixir LiveView-like Frontend via WASM

## Concept

Phoenix LiveView allows writing interactive web apps in Elixir by keeping state on the server and pushing DOM diffs over a WebSocket.

AIVI can adopt a similar model but **run the "server" logic in the browser via WASM**. This gives the "Single Page App" feel with the simplicity of a backend framework.

## Architecture

1.  **Model**: An immutable record representing the UI state.
2.  **View**: A pure function `Model => HtmlDomain`.
3.  **Update**: A pure function `Msg -> Model -> (Model, Effect)`.

## The "Live" Part

Instead of a Virtual DOM diffing in JS (React), or a server-side diff (LiveView):

1.  AIVI code produces a DOM tree description (using HTML Domain).
2.  The AIVI runtime (WASM) computes the diff against the previous version.
3.  WASM calls into JS only to apply the minimal patch.

This creates a **"No-JS" experience** for the developer. They write only AIVI code.

## Why AIVI fits this

*   **Generators**: Perfect for handling streams of events (clicks, inputs).
*   **Patching (`<=`)**: The deep update syntax makes state management trivial (Redux/Zustand replacement built-in).
*   **Domains**: HTML attributes can be modeled as domain operations (`style + { color: red }`).

## Example

```aivi
update = msg model =>
  msg ?
  | Inc => model <= { count: _ + 1 }
  | Dec => model <= { count: _ - 1 }

view = model =>
  div [
    button [onClick Dec] [- `1`]
    span [] [`Count: {model.count}`]
    button [onClick Inc] [+ `1`]
  ]
```
