# Idea: Meta-Domain for Domain Authoring

## Concept

A **meta-domain** allows AIVI to express domain definitions as first-class data structures. This enables:
- Programmatic domain construction
- Domain composition and extension
- Testing domain implementations within AIVI

## The Domain Domain

```aivi
module aivi/meta/domain = {
  export domain Domain
  export DomainDef, Rule, Operator
  
  Operator = Add | Sub | Mul | Div | Custom Text
  
  Rule = {
    operator: Operator
    carrier: Type
    delta: Type
    impl: (carrier -> delta -> carrier)
  }
  
  DomainDef = {
    name: Text
    carriers: List Type
    deltas: List (Text, Type)
    rules: List Rule
  }
  
  domain Domain over DomainDef = {
    (+) : DomainDef -> Rule -> DomainDef
    (+) def rule = { def | rules: def.rules + [rule] }
    
    (++) : DomainDef -> DomainDef -> DomainDef
    (++) base ext = {
      name: ext.name
      carriers: base.carriers + ext.carriers
      deltas: base.deltas + ext.deltas
      rules: base.rules + ext.rules
    }
  }
}
```

## Usage: Domain Composition

```aivi
use aivi/meta/domain

-- Define a base numeric domain
numericDomain = {
  name: `Numeric`
  carriers: [Int, Float]
  deltas: []
  rules: [
    { operator: Add, carrier: Int, delta: Int, impl: intAdd }
    { operator: Sub, carrier: Int, delta: Int, impl: intSub }
  ]
}

-- Extend with comparison
orderedDomain = numericDomain ++ {
  name: `Ordered`
  carriers: []
  deltas: []
  rules: [
    { operator: Custom `<`, carrier: Int, delta: Int, impl: intLt }
  ]
}
```

## Usage: Domain Testing

```aivi
testCalendarDomain : Test
testCalendarDomain = describe `Calendar domain` [
  it `adds days correctly` (
    { year: 2025, month: 2, day: 28 } + Day 1
    `shouldEqual`
    { year: 2025, month: 3, day: 1 }
  )
  
  it `handles leap years` (
    { year: 2024, month: 2, day: 28 } + Day 1
    `shouldEqual`
    { year: 2024, month: 2, day: 29 }
  )
]
```

## Bootstrap Question

The meta-domain is self-referential: it's a domain that describes domains.

Compiler bootstrap options:
1. **Hard-coded kernel domain** — The compiler has a built-in Domain domain
2. **Interpreted bootstrap** — A minimal interpreter evaluates meta-domain definitions
3. **Two-stage compilation** — First stage compiles meta-domain, second uses it

## Relationship to Type Classes

| Concept | Haskell | AIVI |
| :--- | :--- | :--- |
| Polymorphic operations | Type class | — |
| Domain-specific operations | — | Domain |
| Meta-level definitions | Template Haskell | Meta-domain |
| Compile-time resolution | Instances | Domain rules |

## Advantages

- **Self-hosting**: AIVI can define its own extension mechanisms
- **Testability**: Domain logic is ordinary AIVI code
- **Composition**: Domains can inherit and extend each other
- **Introspection**: Programs can inspect domain definitions at compile time
