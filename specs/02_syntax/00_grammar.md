# Concrete Syntax (EBNF draft)

This chapter is a **draft concrete grammar** for the surface language described in the Syntax section. It exists to make parsing decisions explicit and to highlight places where the compiler should emit helpful diagnostics.

This is *not* intended as a complete lexical specification; it focuses on the constructs used throughout the spec examples.

---

## 0.1 Lexical notes

### Identifiers

- `lowerIdent` starts with a lowercase letter: values, functions, fields.
- `UpperIdent` starts with an uppercase letter: types, constructors, domains, classes.
- Keywords are reserved and cannot be used as identifiers.

### Separators (layout)

Many constructs accept either:
- one or more newlines, or
- `;`

as a separator. The parser should treat consecutive separators as one.

In addition, many comma-delimited forms allow `,` as an alternative separator.

---

## 0.2 Top level

```ebnf
Program        := { TopItem } ;
TopItem        := { Decorator } (Definition | ModuleDef) ;

Decorator      := "@" lowerIdent [ DecoratorArg ] Sep ;
DecoratorArg   := Expr | RecordLit ;

Definition     := TypeSig | Binding ;
TypeSig        := lowerIdent ":" Type Sep ;
Binding        := Pattern "=" Expr Sep ;

ModuleDef      := "module" ModulePath "=" ModuleBody Sep ;
ModulePath     := lowerIdent { "." lowerIdent } ;
ModuleBody     := "{" { ModuleItem } "}" ;
ModuleItem     := ExportStmt | UseStmt | Definition | ModuleDef ;
ExportStmt     := "export" ( "*" | ExportList ) Sep ;
ExportList     := ExportItem { "," ExportItem } ;
ExportItem     := lowerIdent | UpperIdent | ("domain" UpperIdent) ;
UseStmt        := "use" ModulePath [ UseSpec ] Sep ;
UseSpec        := "as" UpperIdent
               | "(" ImportList ")"
               | "hiding" "(" ImportList ")" ;
ImportList     := ImportItem { "," ImportItem } ;
ImportItem     := (lowerIdent | UpperIdent) [ "as" (lowerIdent | UpperIdent) ] ;
```

---

## 0.3 Expressions

```ebnf
Expr           := IfExpr ;

IfExpr         := "if" Expr "then" Expr "else" Expr
               | MatchExpr ;

MatchExpr      := PatchExpr [ "?" MatchArms ] ;
MatchArms      := Sep? "|" Arm { Sep "|" Arm } ;
Arm            := Pattern [ "when" Expr ] "=>" Expr ;

PatchExpr      := PipeExpr { "<|" PatchLit } ;
PipeExpr       := AppExpr { "|>" AppExpr } ;

AppExpr        := Atom { Atom } ;

Atom           := Literal
               | lowerIdent
               | UpperIdent
               | "." lowerIdent
               | "(" Expr ")"
               | TupleLit
               | ListLit
               | RecordLit
               | Block
               | EffectBlock ;

Block          := "do" "{" { Stmt } "}" ;
EffectBlock    := "effect" "{" { Stmt } "}" ;
Stmt           := BindStmt | Binding | Expr Sep ;
BindStmt       := Pattern "<-" Expr Sep ;

TupleLit       := "(" Expr "," Expr { "," Expr } ")" ;
ListLit        := "[" [ Expr { "," Expr } | Range ] "]" ;
Range          := Expr ".." Expr ;

RecordLit      := "{" { RecordField } "}" ;
RecordField    := lowerIdent ":" Expr [ FieldSep ] ;
FieldSep       := Sep | "," ;
```

**Notes**

- `{ ... }` is reserved for record-shaped forms (`RecordLit`, `RecordType`, `RecordPat`, `PatchLit`, and module/domain bodies).
- Multi-statement expression blocks use `do { ... }`, so the parser never needs to guess whether `{ ... }` is a record literal or a block.
- `.field` is shorthand for `x => x.field` (a unary accessor function).
- `_` is *not* a value. It only appears in expressions as part of the placeholder-lambda sugar (see `specs/04_desugaring/02_functions.md`).

---

## 0.4 Patching

```ebnf
PatchLit       := "{" { PatchEntry } "}" ;
PatchEntry     := Path ":" PatchInstr [ FieldSep ] ;
PatchInstr     := "-" | ":=" Expr | Expr ;

Path           := PathSeg { "." PathSeg } ;
PathSeg        := lowerIdent
               | UpperIdent "." lowerIdent
               | Select ;
Select         := "[" ( "*" | Expr ) "]" ;
```

**Notes**

- `PathSeg` is intentionally permissive in this draft: patch paths, traversal selectors, and prism-like focuses share syntax.
- A compiler should reject ill-typed or ill-scoped path forms with a targeted error (e.g. “predicate selector expects a `Bool` predicate”).

---

## 0.5 Multi-clause unary functions

A *unary* multi-clause function can be written using arms directly:

```ebnf
Binding        := lowerIdent "=" FunArms Sep ;
FunArms        := "|" Arm { Sep "|" Arm } ;
```

This form desugars to a single-argument function that performs a `case` on its input (see `specs/04_desugaring/04_patterns.md`).

If you want multi-argument matching, match on a tuple:

```aivi
nextState =
  | (Idle, Start) => Running
  | (state, _)    => state
```

---

## 0.6 Types (minimal)

```ebnf
Type           := TypeArrow ;
TypeArrow      := TypeApp [ "->" TypeArrow ] ;
TypeApp        := TypeAtom { TypeAtom } ;
TypeAtom       := UpperIdent
               | lowerIdent
               | "(" Type ")"
               | TupleType
               | RecordType ;

TupleType      := "(" Type "," Type { "," Type } ")" ;
RecordType     := "{" { RecordTypeField } "}" ;
RecordTypeField:= lowerIdent ":" Type [ FieldSep ] ;
```

---

## 0.7 Patterns

```ebnf
Pattern        := PatAtom [ "@" Pattern ] ;
PatAtom        := "_"
               | lowerIdent
               | UpperIdent
               | Literal
               | TuplePat
               | ListPat
               | RecordPat
               | ConPat ;

ConPat         := UpperIdent Pattern ;
TuplePat       := "(" Pattern "," Pattern { "," Pattern } ")" ;
ListPat        := "[" [ Pattern { "," Pattern } [ "," "..." lowerIdent ] ] "]" ;

RecordPat      := "{" { RecordPatField } "}" ;
RecordPatField := RecordPatKey [ ":" Pattern ] [ FieldSep ] ;
RecordPatKey   := lowerIdent { "." lowerIdent } ;
```

**Notes**

- `v@p` binds the whole matched value to `v` while also matching `p`.
- Record patterns permit dotted keys for deep destructuring (e.g. `{ data.user.profile@{ name } }`).

---

## 0.8 Diagnostics (where the compiler should nag)

- **Likely-missed `do`**: if `{ ... }` contains `=` bindings or statement separators, error and suggest `do { ... }` (since `{ ... }` is record-shaped).
- **Arms without a `?`**: `| p => e` is only valid after `?` *or* directly after `=` in the multi-clause unary function form.
- **`_` placeholder**: `_ + 1` is only legal where a unary function is expected; otherwise error and suggest `x => x + 1`.
- **Deep keys in record literals**: `a.b: 1` should be rejected in record literals (suggest patching with `<|` if the intent was a path).
