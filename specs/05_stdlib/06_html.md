# Standard Library: Html Domain

## Module

```aivi
module aivi/std/html = {
  export domain Html
  export Element, Attribute, Children
  export div, span, p, a, ul, li, button, input, form
}
```

## Types

```aivi
Element = {
  tag: Text
  attrs: List Attribute
  children: Children
}

Attribute = { name: Text, value: Text }

Children = TextNode Text | Elements (List Element) | Empty
```

## Domain Definition

```aivi
domain Html over Element = {
  -- Concatenate children
  (+) : Element -> Element -> Element
  (+) parent child = { parent | children: append parent.children child }
  
  -- Merge attributes
  (+) : Element -> Attribute -> Element
  (+) el attr = { el | attrs: el.attrs + [attr] }
  
  -- Merge attribute records
  (+) : Element -> { ... } -> Element
  (+) el rec = { el | attrs: mergeAttrs el.attrs rec }
}
```

## Constructors

```aivi
div : List Attribute -> Children -> Element
div attrs children = { tag: `div`, attrs, children }

span : List Attribute -> Children -> Element
span attrs children = { tag: `span`, attrs, children }

p : List Attribute -> Children -> Element
p attrs children = { tag: `p`, attrs, children }

a : List Attribute -> Children -> Element
a attrs children = { tag: `a`, attrs, children }

ul : List Attribute -> Children -> Element
ul attrs children = { tag: `ul`, attrs, children }

li : List Attribute -> Children -> Element
li attrs children = { tag: `li`, attrs, children }

button : List Attribute -> Children -> Element
button attrs children = { tag: `button`, attrs, children }

input : List Attribute -> Element
input attrs = { tag: `input`, attrs, children: Empty }

form : List Attribute -> Children -> Element
form attrs children = { tag: `form`, attrs, children }
```

## Usage Examples

With [JSX literals](../02_syntax/13_jsx_literals.md), HTML becomes natural:

```aivi
use aivi/std/html

header = <div class=`header`>
  <span>AIVI</span>
</div>

nav = <ul class=`nav`>
  <li><a href=`/home`>Home</a></li>
  <li><a href=`/about`>About</a></li>
</ul>

page = <div class=`container`>
  {header}
  {nav}
</div>
```

### Dynamic Content

```aivi
UserList = users => <ul>
  {users |> map (u => <li>
    <a href={`/user/{u.id}`}>{u.name}</a>
  </li>)}
</ul>

Dashboard = { user, posts } => <main>
  <h1>Welcome, {user.name}!</h1>
  {posts |> isEmpty ? 
    <p>No posts yet.</p> : 
    <PostList posts={posts} />}
</main>
```

### Underlying Function Syntax

JSX desugars to these constructors (rarely used directly):

```aivi
header = div [ class `header` ] (
  span [] (TextNode `AIVI`)
)
```

