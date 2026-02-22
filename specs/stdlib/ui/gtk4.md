# `aivi.gtk4`
## Native GTK4 Runtime Bindings

<!-- quick-info: {"kind":"module","name":"aivi.gtk4"} -->
`aivi.gtk4` is the convenience module for GTK4-oriented native UI effects.
It exposes AIVI types/functions mapped directly to runtime native bindings.
<!-- /quick-info -->

<div class="import-badge">use aivi.gtk4</div>

## Public API

```aivi
AppId = Int
WindowId = Int
GtkError = Text

init : Unit -> Effect GtkError Unit
appNew : Text -> Effect GtkError AppId
windowNew : AppId -> Text -> Int -> Int -> Effect GtkError WindowId
windowSetTitle : WindowId -> Text -> Effect GtkError Unit
windowPresent : WindowId -> Effect GtkError Unit
appRun : AppId -> Effect GtkError Unit
```

## Native Mapping Table

| AIVI function | Native target |
| --- | --- |
| `init` | `gtk4.init` |
| `appNew` | `gtk4.appNew` |
| `windowNew` | `gtk4.windowNew` |
| `windowSetTitle` | `gtk4.windowSetTitle` |
| `windowPresent` | `gtk4.windowPresent` |
| `appRun` | `gtk4.appRun` |

## Example

```aivi
use aivi
use aivi.gtk4

main = do Effect {
  _ <- init Unit
  appId <- appNew "com.example.demo"
  winId <- windowNew appId "AIVI GTK4" 800 600
  _ <- windowPresent winId
  _ <- appRun appId
  pure Unit
}
```

## Compatibility

`aivi.ui.Gtk4` is still available and re-exports `aivi.gtk4` for compatibility.
