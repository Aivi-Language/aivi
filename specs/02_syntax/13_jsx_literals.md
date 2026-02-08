# JSX Literals

AIVI supports **JSX-like syntax** as domain sugar for the Html domain.

---

## 13.1 Basic Syntax

```aivi
header = <div class=`header`>
  <span>AIVI</span>
</div>
```

Desugars to:

```aivi
header = div [ class `header` ] (
  span [] (TextNode `AIVI`)
)
```

---

## 13.2 Attributes

```aivi
<a href=`/home` class=`link` target=`_blank`>Home</a>
```

Attribute syntax:
- `name=value` — value is any AIVI expression
- `name` alone — shorthand for `name=True`
- `{...record}` — spread attributes from a record

```aivi
attrs = { class: `btn`, disabled: True }
button = <button {...attrs}>Click</button>
```

---

## 13.3 Expression Interpolation

Use `{expr}` to embed AIVI expressions:

```aivi
greeting = name => <h1>Hello, {name}!</h1>

userAge = user => <p>Age: {user.age ?? `Unknown`}</p>
```

---

## 13.4 Conditionals

```aivi
badge = user => <div>
  {user.verified ? <span class=`verified`>✓</span> : <></>}
  <span>{user.name}</span>
</div>
```

The `<></>` is an empty fragment.

---

## 13.5 Lists and Iteration

```aivi
nav = links => <ul class=`nav`>
  {links |> map (link => <li>
    <a href={link.url}>{link.title}</a>
  </li>)}
</ul>
```

Pipe expressions work naturally inside interpolation.

---

## 13.6 Components

Functions returning elements are components:

```aivi
-- Component definition
Card = { title, children } => <div class=`card`>
  <h2>{title}</h2>
  <div class=`card-body`>{children}</div>
</div>

-- Usage
page = <div>
  <Card title=`Welcome`>
    <p>This is the card content.</p>
  </Card>
</div>
```

Components are just functions — no special syntax beyond JSX.

---

## 13.7 Fragments

Group elements without a wrapper:

```aivi
items = <>
  <li>First</li>
  <li>Second</li>
  <li>Third</li>
</>
```

Desugars to:

```aivi
items = fragment [
  li [] (TextNode `First`)
  li [] (TextNode `Second`)
  li [] (TextNode `Third`)
]
```

---

## 13.8 Self-Closing Tags

```aivi
<input type=`text` placeholder=`Enter name` />
<img src=`/logo.png` alt=`Logo` />
<br />
```

---

## 13.9 Desugaring Rules

| JSX | Desugars To |
| :--- | :--- |
| `<div>` | `div []` |
| `<div class=x>` | `div [ class x ]` |
| `<div>text</div>` | `div [] (TextNode \`text\`)` |
| `<div>{expr}</div>` | `div [] expr` |
| `<div>{a}{b}</div>` | `div [] (fragment [a, b])` |
| `<><A/><B/></>` | `fragment [A, B]` |
| `<Comp x=y>` | `Comp { x: y }` |

---

## 13.10 Full Example

```aivi
use aivi/std/html

UserCard = user => <div class=`user-card`>
  <img src={user.avatar} alt={user.name} />
  <h3>{user.name}</h3>
  <p class=`bio`>{user.bio ?? `No bio provided`}</p>
  {user.verified ? <span class=`badge`>Verified</span> : <></>}
  <ul class=`stats`>
    {user.stats |> map (s => <li>
      <strong>{s.label}:</strong> {s.value}
    </li>)}
  </ul>
</div>

App = users => <main>
  <h1>Users</h1>
  <div class=`user-grid`>
    {users |> map UserCard}
  </div>
</main>
```

This is clean, readable, and fully type-safe — the Html domain validates element structure at compile time.
