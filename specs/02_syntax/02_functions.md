# Functions and Pipes

## 2.1 Application

* Functions are **curried by default**
* Application is by whitespace

```aivi
add 5 10
```


## 2.2 Lambdas

`_` denotes a **single-argument lambda**.

```aivi
inc = _ + 1
```

Multi-argument lambdas must be explicit:

```aivi
add = x y => x + y
```


## 2.3 Pipes

Pipelines use `|>`.

```aivi
xs |> map inc |> filter (_ > 0)
```


## 2.4 Usage Examples

### Basic Functions

```aivi
// Identity
id = x => x

// Constant: returns a function that ignores its input and always returns x
const = x _ => x

// Flip arguments
flip = f => x y => f y x

// Function composition is most common via the pipe operator:
processName = name => name |> trim |> lowercase |> capitalize
result = processName "  HELLO  "
```


### Higher-Order Functions

```aivi
// Apply function twice
twice = f => x => f (f x)

increment = _ + 1
addTwo = twice increment

// Result: addTwo 5 = 7
```

### Partial Application

```aivi
add = x y => x + y
add5 = add 5

// add5 10 = 15

// With pipes
numbers = [1, 2, 3]
result = numbers |> map (add 10)
// [11, 12, 13]
```

### Block Pipelines


Pipelines allow building complex data transformations without nested function calls.

```aivi
users = [
  { name: "Alice", age: 30, active: True }
  { name: "Bob", age: 25, active: False }
  { name: "Carol", age: 35, active: True }
]

// Data processing pipeline
activeNames = users
  |> filter active
  |> map .name
  |> sort
  |> join ", "
// "Alice, Carol"

// Mathematical series
sigma = [1..100]
  |> filter (_ % 2 == 0)
  |> map (pow _ 2) // or map _²
  |> sum
```

### Expressive Logic: Point-Free Style

Functions can be combined to form new functions without naming their arguments, leading to very concise code.

```aivi
// Boolean predicate composition
isAdmin : User => Bool
isAdmin = role == Admin

isOwner : User => Bool
isOwner = id == ownerId

canDelete : User => Bool
canDelete = u => isAdmin u || isOwner u

// Validation chains
isEmail = contains "@"
isLongEnough = s => s |> len |> (_ > 8)
isValidPassword = s => isEmail s && isLongEnough s

// Usage
passwords |> filter isValidPassword
```

### Lambda Shorthand

```aivi
// Single arg with _
double = _ * 2
isEven = _ % 2 == 0 
getName = .name // Accessor shorthand for x => x.name

// Equivalent explicit forms
double = x => x * 2
isEven = x => x % 2 == 0
getName = user => user.name

// Boolean fields can be used directly as predicates:
// filter active is a shortcut for filter (x => x.active)

// Complexity can be handled inline via explicit record deconstruction:
// map { name, id } => if id > 10 then name else "Anonymous"
```
