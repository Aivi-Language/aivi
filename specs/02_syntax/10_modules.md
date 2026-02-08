# Modules and External Sources

## 10.1 External Sources

```aivi
Kind = File | Http | Db | Email | Llm | Image | ...
Source K A
```

## 10.2 Modules

```aivi
module aivi/app/main = {
  export main
  use aivi/std/core
}
```

Modules are first-class citizens but resolved statically.
