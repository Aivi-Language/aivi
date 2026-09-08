# aivi.gnome.settings

Types for working with GNOME settings (GSettings).

GSettings is the desktop settings system used for values such as the color scheme, text
scaling, and many other GNOME preferences.

This module currently exports the schema type, key type, setting value type, and task alias
used by GSettings integrations. The stdlib comments also document the watcher source shape.

## Import

```aivi
use aivi.gnome.settings (
    SettingsError
    SettingsSchema
    SettingsKey
    SettingValue
    SettingsTask
)
```

## Overview

| Item | Type | Description |
|------|------|-------------|
| `SettingsError` | type | Things that can go wrong when resolving or decoding a setting |
| `SettingsSchema` | domain over `Text` | Checked schema identifier |
| `SettingsKey` | domain over `Text` | Wrapped key name |
| `SettingValue` | type | Generic setting value |
| `SettingsTask A` | `Task SettingsError A` | Generic settings task alias |
| `gsettings.watch` | source | Documented watcher source shape |

## Types

### SettingsError

```aivi
type SettingsError =
  | SchemaNotFound Text
  | KeyNotFound Text
  | TypeMismatch Text
  | SettingsUnavailable
```

These variants describe the usual GSettings failure cases.

- `SchemaNotFound` — the schema ID does not exist
- `KeyNotFound` — the schema exists, but the key does not
- `TypeMismatch` — the key exists, but not with the type you expected
- `SettingsUnavailable` — GSettings access is not available in the current runtime

### SettingsSchema

```aivi
use aivi.gnome.settings (SettingsError)

domain SettingsSchema over Text = {
    type parse : Text -> Result SettingsError SettingsSchema
}
```

Schema identifier such as `"org.gnome.desktop.interface"`. The module currently publishes the
domain declaration, but it does not export a runtime schema-validation function.

An integration receives a `SettingsSchema` from code that owns schema validation; consumers keep
the nominal type intact:

```aivi
use aivi.gnome.settings (SettingsSchema)

type SettingsSchema -> SettingsSchema
func retainSchema = schema =>
    schema
```

### SettingsKey

```aivi
domain SettingsKey over Text = {
    type make : Text -> SettingsKey
}
```

Wrapped key name such as `"color-scheme"`.

The domain declaration names a future `make` member, but the current module does not export that
member. Construct the domain value explicitly for now.

```aivi
use aivi.gnome.settings (SettingsKey)

type SettingsKey -> SettingsKey
func retainKey = key =>
    key
```

### SettingValue

```aivi
type SettingValue =
  | SettingBool Bool
  | SettingInt Int
  | SettingFloat Float
  | SettingText Text
  | SettingList (List Text)
```

Generic setting value for code that needs to handle several setting types in one place.

```aivi
use aivi.gnome.settings (
    SettingValue
    SettingBool
    SettingInt
    SettingFloat
    SettingText
    SettingList
)

type SettingValue -> Text
func settingKind = value => value
 ||> SettingBool b  -> "bool"
 ||> SettingInt n   -> "int"
 ||> SettingFloat x -> "float"
 ||> SettingText t  -> "text"
 ||> SettingList xs -> "list"
```

### SettingsTask

```aivi
use aivi.gnome.settings (SettingsError)

type SettingsTask A = (Task SettingsError A)
```

Generic alias for settings-related tasks.

## Documented source shapes

The stdlib module comments document the following watcher patterns:

```aivi
use aivi.gnome.settings (SettingsError)

@source gsettings.watch "org.gnome.desktop.interface" "color-scheme"
signal colorScheme : Signal (Result SettingsError Text)

@source gsettings.watch "org.gnome.desktop.interface" "text-scaling-factor"
signal textScale : Signal (Result SettingsError Float)
```

The concrete signal payload depends on the key you watch. This module does not currently
export direct read or write helpers.
