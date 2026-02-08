# Idea: HTML Domains for DOM Manipulation

## Concept

In AIVI, **Domains** define semantics. We can define an `Html` domain where "addition" means composition or attribute merging.

## Domain Definition

```aivi
domain Html elements
domain Style css
```

## Usage

### Composition

```aivi
-- + concatenates children
list = ul [] +
  (li [] [`Item 1`]) +
  (li [] [`Item 2`])
```

### Attribute Merging

Deltas can represent style updates.

```aivi
baseBtn = button [ class `btn` ]

-- + merges attributes
primaryBtn = baseBtn + { class: `primary`, disabled: False }
```

### Units for CSS

CSS units as first-class Deltas.

```aivi
10px
2em
50pct
```

```aivi
style = {
  width: 100pct
  margin: 10px
  fontSize: 1.2em
}
```

## Advantages

*   **Type Safety**: `10px` + `5` is a type error.
*   **Composition**: Building UIs becomes algebraic.
*   **No "Stringly Typed" CSS**: Styles are typed records with unit validation.
