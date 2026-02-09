# Concrete Syntax (EBNF draft)

This chapter is a **draft concrete grammar** for the surface language described in the Syntax section. It exists to make parsing decisions explicit and to highlight places where the compiler should emit helpful diagnostics.

This chapter is intentionally pragmatic: it aims to be complete enough to build a real lexer/parser/LSP for the current spec and repo examples, even though many parts of the language are still evolving.


## 0.1 Lexical notes

> These are **normative** for parsing. Typing/elaboration rules live elsewhere.

### Whitespace and comments

- Whitespace separates tokens and is otherwise insignificant (no indentation sensitivity in v0.1).
- Line comments start with `//` and run to the end of the line.
- Block comments start with `/*` and end with `*/` (nesting is not required).

### Identifiers

- `lowerIdent` starts with a lowercase ASCII letter: values, functions, fields.
- `UpperIdent` starts with an uppercase ASCII letter: types, constructors, modules, domains, classes.
- After the first character, identifiers may contain ASCII letters, digits, and `_`.
- Keywords are reserved and cannot be used as identifiers.

### Keywords (v0.1)

```text
as case class do domain effect else export generate hiding if
instance module over rec resource then type use when where yield
```

(`True`, `False`, `None`, `Some`, `Ok`, `Err` are ordinary constructors, not keywords.)

### Literals (minimal set for v0.1)

- `IntLit`: decimal digits (e.g. `0`, `42`).
- `FloatLit`: digits with a fractional part (e.g. `3.14`).
- `TextLit`: double-quoted with escapes and interpolation (see below).
- `CharLit`: single-quoted (optional in v0.1; many examples can use `Text` instead).
- `IsoInstantLit`: ISO-8601 instant-like token (e.g. `2024-05-21T12:00:00Z`), used by the `Calendar`/`Time` domains.
- `SuffixedNumberLit`: `IntLit` or `FloatLit` followed immediately by a suffix (e.g. `10px`, `100%`, `30s`, `1min`).

`SuffixedNumberLit` is *lexical*; its meaning is **domain-resolved** (see Domains). The lexer does not decide whether `1m` is “month” or “meter”.

### Text literals and interpolation

Text literals are delimited by `"` and support interpolation segments `{ Expr }`:

```aivi
"Hello"
"Count: {n}"
"{user.name}: {status}"
```

Inside a `TextLit`, `{` starts interpolation and `}` ends it; braces must be balanced within the interpolated expression.

### Separators (layout)

Many constructs accept either:
- one or more newlines, or
- `;`

as a separator. The parser should treat consecutive separators as one.

In addition, many comma-delimited forms allow `,` as an alternative separator.

We name these separators in the grammar:

```ebnf
Sep        := ( Newline | ";" ) { ( Newline | ";" ) } ;
FieldSep   := Sep | "," ;
```


## 0.2 Top level

```ebnf
Program        := { TopItem } ;
TopItem        := { Decorator } (ModuleDef | Definition) ;

Decorator      := "@" lowerIdent [ DecoratorArg ] Sep ;
DecoratorArg   := Expr | RecordLit ;

Definition     := ValueSig
               | ValueBinding
               | TypeAlias
               | TypeDef
               | DomainDef
               | ClassDef
               | InstanceDef ;

ValueSig       := lowerIdent ":" Type Sep ;
ValueBinding   := Pattern "=" Expr Sep ;

TypeAlias      := "type" UpperIdent [ TypeParams ] "=" TypeRhs Sep ;
TypeDef        := UpperIdent [ TypeParams ] "=" TypeRhs Sep ;
TypeParams     := UpperIdent { UpperIdent } ;
TypeRhs        := Type
               | RecordType
               | [ Sep? "|" ] ConDef { Sep? "|" ConDef } ;
ConDef         := UpperIdent { TypeAtom } ;

ModuleDef      := "module" ModulePath "=" ModuleBody Sep ;
ModulePath     := ModuleSeg { "." ModuleSeg } ;
ModuleSeg      := lowerIdent | UpperIdent ;
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
ImportItem     := (lowerIdent | UpperIdent | ("domain" UpperIdent)) [ "as" (lowerIdent | UpperIdent) ] ;

DomainDef      := "domain" UpperIdent "over" Type "=" "{" { DomainItem } "}" Sep ;
DomainItem     := TypeAlias | TypeDef | ValueSig | ValueBinding | OpDef | DeltaLitBinding ;
OpDef          := "(" Operator ")" ":" Type Sep
               | "(" Operator ")" Pattern { Pattern } "=" Expr Sep ;
Operator       := "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" | "++" | "??"
               | "&" | "|" | "^" | "~" | "<<" | ">>" ;
DeltaLitBinding:= SuffixedNumberLit "=" Expr Sep ;

ClassDef       := "class" UpperIdent ClassParams "=" Type Sep ;
ClassParams    := ClassParam { ClassParam } ;
ClassParam     := UpperIdent
               | "(" UpperIdent "*" { "*" } ")" ;

InstanceDef    := "instance" [ UpperIdent ":" ] UpperIdent InstanceHead "=" RecordLit Sep ;
InstanceHead   := "(" Type ")" ;
```


## 0.3 Expressions

```ebnf
Expr           := IfExpr ;

IfExpr         := "if" Expr "then" Expr "else" Expr
               | LambdaExpr ;

LambdaExpr     := LambdaArgs "=>" Expr
               | MatchExpr ;
LambdaArgs     := PatParam { PatParam } ;
PatParam       := lowerIdent
               | "_"
               | RecordPat
               | TuplePat
               | ListPat
               | "(" PatParam ")" ;

MatchExpr      := PipeExpr [ "?" MatchArms ] ;
MatchArms      := Sep? "|" Arm { Sep "|" Arm } ;
Arm            := Pattern [ "when" Expr ] "=>" Expr ;

PipeExpr       := CoalesceExpr { "|>" CoalesceExpr } ;

CoalesceExpr   := OrExpr { "??" OrExpr } ;
OrExpr         := AndExpr { "||" AndExpr } ;
AndExpr        := EqExpr { "&&" EqExpr } ;
EqExpr         := CmpExpr { ("==" | "!=") CmpExpr } ;
CmpExpr        := BitOrExpr { ("<" | "<=" | ">" | ">=") BitOrExpr } ;
BitOrExpr      := BitXorExpr { "|" BitXorExpr } ;
BitXorExpr     := BitAndExpr { "^" BitAndExpr } ;
BitAndExpr     := ShiftExpr { "&" ShiftExpr } ;
ShiftExpr      := AddExpr { ("<<" | ">>") AddExpr } ;
AddExpr        := MulExpr { ("+" | "-" | "++") MulExpr } ;
MulExpr        := UnaryExpr { ("*" | "/" | "%") UnaryExpr } ;
UnaryExpr      := ("!" | "-" | "~" ) UnaryExpr
               | PatchExpr ;

PatchExpr      := AppExpr { "<|" PatchLit } ;

AppExpr        := PostfixExpr { PostfixExpr } ;
PostfixExpr    := Atom { "." lowerIdent } ;

Atom           := Literal
               | lowerIdent
               | UpperIdent
               | "." lowerIdent                 (* accessor sugar *)
               | "(" Expr ")"
               | TupleLit
               | ListLit
               | RecordLit
               | Block
               | EffectBlock
               | GenerateBlock
               | ResourceBlock
               | JSXElement ;

Block          := "do" "{" { Stmt } "}" ;
EffectBlock    := "effect" "{" { Stmt } "}" ;
GenerateBlock  := "generate" "{" { GenStmt } "}" ;
ResourceBlock  := "resource" "{" { ResStmt } "}" ;

Stmt           := BindStmt | ValueBinding | Expr Sep ;
BindStmt       := Pattern "<-" Expr Sep ;

GenStmt        := BindStmt
               | GuardStmt
               | ValueBinding
               | "yield" Expr Sep
               | "loop" Pattern "=" Expr "=>" "{" { GenStmt } "}" Sep ;
GuardStmt      := lowerIdent "->" Expr Sep ;

ResStmt        := ValueBinding
               | BindStmt
               | Expr Sep
               | "yield" Expr Sep ;

TupleLit       := "(" Expr "," Expr { "," Expr } ")" ;
ListLit        := "[" [ Expr { FieldSep Expr } | Range ] "]" ;
Range          := Expr ".." Expr ;

RecordLit      := "{" { RecordField } "}" ;
RecordField    := lowerIdent [ ":" Expr ] [ FieldSep ] ;

Literal        := "True"
               | "False"
               | IntLit
               | FloatLit
               | TextLit
               | CharLit
               | IsoInstantLit
               | SuffixedNumberLit ;
```

**Notes**

- `{ ... }` is reserved for record-shaped forms (`RecordLit`, `RecordType`, `RecordPat`, `PatchLit`, and module/domain bodies).
- Multi-statement expression blocks use `do { ... }`, so the parser never needs to guess whether `{ ... }` is a record literal or a block.
- `.field` is shorthand for `x => x.field` (a unary accessor function).
- `_` is *not* a value. It only appears in expressions as part of the placeholder-lambda sugar (see `specs/04_desugaring/02_functions.md`).


## 0.4 Patching

```ebnf
PatchLit       := "{" { PatchEntry } "}" ;
PatchEntry     := Path ":" PatchInstr [ FieldSep ] ;
PatchInstr     := "-" | ":=" Expr | Expr ;

Path           := PathSeg { [ "." ] PathSeg } ;
PathSeg        := lowerIdent
               | UpperIdent "." lowerIdent
               | Select ;
Select         := "[" ( "*" | Expr ) "]" ;
```

**Notes**

- `PathSeg` is intentionally permissive in this draft: patch paths, traversal selectors, and prism-like focuses share syntax.
- A compiler should reject ill-typed or ill-scoped path forms with a targeted error (e.g. “predicate selector expects a `Bool` predicate”).


## 0.5 Multi-clause unary functions

A *unary* multi-clause function can be written using arms directly:

```ebnf
ValueBinding   := lowerIdent "=" FunArms Sep ;
FunArms        := "|" Arm { Sep "|" Arm } ;
```

This form desugars to a single-argument function that performs a `case` on its input (see `specs/04_desugaring/04_patterns.md`).

If you want multi-argument matching, match on a tuple:

```aivi
nextState =
  | (Idle, Start) => Running
  | (state, _)    => state
```

## 0.6 Types

```ebnf
Type           := TypeArrow ;
TypeArrow      := TypeAnd [ "->" TypeArrow ] ;
TypeAnd        := TypeApp { "&" TypeApp } ;
TypeApp        := TypeAtom { TypeAtom } ;
TypeAtom       := UpperIdent
               | lowerIdent
               | "*"
               | "(" Type ")"
               | TupleType
               | RecordType ;

TupleType      := "(" Type "," Type { "," Type } ")" ;
RecordType     := "{" { RecordTypeField } "}" ;
RecordTypeField:= lowerIdent ":" Type { FieldDecorator } [ FieldSep ] ;
FieldDecorator := "@" lowerIdent [ DecoratorArg ] ;
```

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

ConPat         := UpperIdent { PatAtom } ;
TuplePat       := "(" Pattern "," Pattern { "," Pattern } ")" ;
ListPat        := "[" [ Pattern { "," Pattern } [ "," "..." [ (lowerIdent | "_") ] ] ] "]" ;

RecordPat      := "{" { RecordPatField } "}" ;
RecordPatField := RecordPatKey [ (":" Pattern) | ("@" Pattern) ] [ FieldSep ] ;
RecordPatKey   := lowerIdent { "." lowerIdent } ;
```

## 0.8 JSX (parsing note)

JSX literals are a *secondary* syntax that is easiest to parse with a dedicated sub-parser once a `<` token is seen in an expression position.

This grammar document treats JSX as a single `JSXElement` atom (see `specs/02_syntax/13_jsx_literals.md` for surface rules and desugaring intent).


## 0.9 Diagnostics (where the compiler should nag)

- **Likely-missed `do`**: if `{ ... }` contains `=` bindings or statement separators, error and suggest `do { ... }` (since `{ ... }` is record-shaped).
- **Arms without a `?`**: `| p => e` is only valid after `?` *or* directly after `=` in the multi-clause unary function form.
- **`_` placeholder**: `_ + 1` is only legal where a unary function is expected; otherwise error and suggest `x => x + 1`.
- **Deep keys in record literals**: `a.b: 1` should be rejected in record literals (suggest patching with `<|` if the intent was a path).
