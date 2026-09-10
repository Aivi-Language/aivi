# aivi.gnome.settings

Application data types for settings. This module supplies vocabulary only: it does not load data, watch sources, or implement the task aliases it defines.

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
| `SettingsSchema` | `Text` alias | Schema identifier text |
| `SettingsKey` | `Text` alias | Key name text |
| `SettingValue` | type | Generic setting value |
| `SettingsTask A` | `Task SettingsError A` | Generic settings task alias |

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

### Settings identifiers

`SettingsSchema` and `SettingsKey` are `Text` aliases. They do not validate installed schemas or keys. This module supplies data types only.

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
