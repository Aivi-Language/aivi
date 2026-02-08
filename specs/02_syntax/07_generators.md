# Generators

## 7.1 Concept

Generators are **pure, pull-based sequence producers**.

They:

* do not perform effects
* do not suspend execution stacks
* model finite or infinite data

---

## 7.2 Generator type

```aivi
Generator A
```

---

## 7.3 Generator expressions

```aivi
gen = generate {
  yield 1
  yield 2
  yield 3
}
```

### From Python/JavaScript
Similar to `yield` syntax, but purely functional (no mutable iterator state).

---

## 7.4 Guards and predicates

```aivi
generate {
  for x in xs
  when price > 80
  yield x
}
```

Predicate rules are identical to `filter`.

---

## 7.5 Relationship to collections

Arrays, ranges, and comprehensions are syntax sugar over generators.
