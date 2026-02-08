# Effects

## 9.1 Effect annotation

```aivi
load : Source K A => A ! source
```

Effects are tracked in the type system but are distinct from the return type.

---

## 9.2 `effect` blocks

```aivi
main = effect {
  cfg = load (file.json `config.json`)
  print `loaded`
}
```

This is syntax sugar for monadic binding (see Desugaring section).

---

## 9.3 Effects and patching

```aivi
user = fetchUser 123

authorized = user <= {
  roles: _ ++ [`Admin`]
  lastLogin: now
}
```

Automatic lifting handles `Result` and other effect functors seamlessly.
