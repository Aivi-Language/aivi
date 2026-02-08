# Functions and Pipes

## 2.1 Application

* Functions are **curried by default**
* Application is by whitespace

```aivi
add 5 10
```

---

## 2.2 Lambdas

`_` denotes a **single-argument lambda**.

```aivi
inc = _ + 1
```

Multi-argument lambdas must be explicit:

```aivi
add = x y => x + y
```

---

## 2.3 Pipes

Pipelines use `|>`.

```aivi
xs |> map inc |> filter (_ > 0)
```

---

## 2.4 Usage Examples

### Basic Functions

```aivi
-- Identity
id = x => x

-- Constant
const = x _ => x

-- Flip arguments
flip = f => x y => f y x
```

### Composition

```aivi
-- Function composition
compose = f g => x => f (g x)

-- Or with operator
(>>) = f g => x => g (f x)

-- Usage
processName = trim >> lowercase >> capitalize
result = processName `  HELLO  `
```

### Higher-Order Functions

```aivi
-- Apply function twice
twice = f => x => f (f x)

increment = _ + 1
addTwo = twice increment

-- Result: addTwo 5 = 7
```

### Partial Application

```aivi
add = x y => x + y
add5 = add 5

-- add5 10 = 15

-- With pipes
numbers = [1, 2, 3]
result = numbers |> map (add 10)
-- [11, 12, 13]
```

### Complex Pipelines

```aivi
users = [
  { name: `Alice`, age: 30, active: True }
  { name: `Bob`, age: 25, active: False }
  { name: `Carol`, age: 35, active: True }
]

activeNames = users
  |> filter _.active
  |> map _.name
  |> sort
  |> join `, `
-- `Alice, Carol`
```

### Lambda Shorthand

```aivi
-- Single arg with _
double = _ * 2
isEven = _ % 2 == 0
getName = _.name

-- Equivalent explicit forms
double = x => x * 2
isEven = x => x % 2 == 0
getName = user => user.name
```
