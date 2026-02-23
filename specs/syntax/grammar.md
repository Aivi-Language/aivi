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
as class do domain effect else export generate given hiding if
instance machine match module on or over patch recurse resource then use when with yield loop
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

<<< ../snippets/from_md/syntax/grammar/text_literals_and_interpolation.aivi{aivi}

Inside a `TextLit`, `{` starts interpolation and `}` ends it; braces must be balanced within the interpolated expression.
Each `{ Expr }` splice is treated as an expected-`Text` position, so the compiler may insert a `toText` coercion (see `ToText` in Types).

### Separators (layout)

Many constructs accept one or more newlines as a separator. The parser should treat consecutive newlines as one.

In addition, many comma-delimited forms allow `,` as an alternative separator.

We name these separators in the grammar:

```ebnf
Sep        := Newline { Newline }
FieldSep   := Sep | ","
```

### Ellipsis

- `...` is a single token (ellipsis) used for list rest patterns and spread entries.

## 0.2 Top level

```ebnf
Program        := { Decorator } ModuleDef
TopItem        := Definition

Decorator      := "@" lowerIdent [ DecoratorArg ] Sep
DecoratorArg   := Expr | RecordLit

Definition     := ValueSig
               | ValueBinding
               | OpaqueType
               | TypeAlias
               | TypeDef
               | DomainDef
               | ClassDef
               | InstanceDef

ValueSig       := lowerIdent ":" Type Sep
ValueBinding   := Pattern "=" Expr Sep

TypeAlias      := UpperIdent [ TypeParams ] "=" TypeRhs Sep
OpaqueType     := UpperIdent [ TypeParams ] Sep
TypeDef        := UpperIdent [ TypeParams ] "=" TypeRhs Sep
TypeParams     := UpperIdent { UpperIdent }
TypeRhs        := Type
               | RecordType
               | [ Sep? "|" ] ConDef { Sep? "|" ConDef }
ConDef         := UpperIdent { TypeAtom }

ModuleDef      := "module" ModulePath Sep ModuleBodyImplicit
ModulePath     := ModuleSeg { "." ModuleSeg }
ModuleSeg      := lowerIdent | UpperIdent
ModuleItem     := ExportStmt | UseStmt | Definition
ModuleBodyImplicit := { ModuleItem } EOF
(* `module` must be the first non-empty item in the file (after any module decorators). *)
ExportStmt     := "export" ( "*" | ExportList ) Sep
ExportList     := ExportItem { "," ExportItem }
ExportItem     := lowerIdent | UpperIdent | ("domain" UpperIdent)
UseStmt        := "use" ModulePath [ UseSpec ] Sep
UseSpec        := "as" UpperIdent
               | "(" ImportList ")"
               | "hiding" "(" ImportList ")"
ImportList     := ImportItem { "," ImportItem }
ImportItem     := (lowerIdent | UpperIdent | ("domain" UpperIdent)) [ "as" (lowerIdent | UpperIdent) ]

DomainDef      := "domain" UpperIdent "over" Type "=" "{" { DomainItem } "}" Sep
DomainItem     := OpaqueType | TypeAlias | TypeDef | ValueSig | ValueBinding | OpDef | DeltaLitBinding
OpDef          := "(" Operator ")" ":" Type Sep
               | "(" Operator ")" Pattern { Pattern } "=" Expr Sep
Operator       := "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" | "++" | "??"
               | "×"
               | "|" | "^" | "~" | "<<" | ">>"
DeltaLitBinding:= SuffixedNumberLit "=" Expr Sep

ClassDef       := "class" UpperIdent ClassParams "=" ClassRhs Sep
ClassParams    := ClassParam { ClassParam }
ClassParam     := UpperIdent
               | "(" UpperIdent "*" { "*" } ")"

(* Classes are records of methods ("dictionaries") with optional superclass composition.
   A class may also declare constraints on the type variables used in its member signatures. *)
ClassRhs       := [ ClassSupers ] [ ClassConstraints ] ClassMembers
ClassSupers    := UpperIdent { "," UpperIdent }
ClassConstraints := "given" "(" TypeVarConstraint { "," TypeVarConstraint } ")"
TypeVarConstraint := UpperIdent ":" UpperIdent
ClassMembers   := RecordType

InstanceDef    := "instance" UpperIdent InstanceHead "=" RecordLit Sep
InstanceHead   := "(" Type ")"
```

## 0.3 Expressions

```ebnf
Expr           := IfExpr

IfExpr         := "if" Expr "then" Expr "else" Expr
               | LambdaExpr

LambdaExpr     := LambdaArgs "=>" Expr
               | MatchExpr
LambdaArgs     := PatParam { PatParam }
PatParam       := lowerIdent [ "as" PatParam ]
               | "_"
               | RecordPat
               | TuplePat
               | ListPat
               | "(" PatParam ")"

MatchExpr      := PipeExpr [ "match" MatchArms ] [ OrFallback ]
MatchArms      := Sep? "|" Arm { Sep "|" Arm }
Arm            := Pattern [ "when" Expr ] "=>" Expr

OrFallback     := "or" ( Expr | OrArms )
OrArms         := Sep? "|" OrArm { Sep "|" OrArm }
OrArm          := Pattern [ "when" Expr ] "=>" Expr

PipeExpr       := CoalesceExpr { "|>" CoalesceExpr }

CoalesceExpr   := OrExpr { "??" OrExpr }
OrExpr         := AndExpr { "||" AndExpr }
AndExpr        := EqExpr { "&&" EqExpr }
EqExpr         := CmpExpr { ("==" | "!=") CmpExpr }
CmpExpr        := BitOrExpr { ("<" | "<=" | ">" | ">=") BitOrExpr }
BitOrExpr      := BitXorExpr { "|" BitXorExpr }
BitXorExpr     := ShiftExpr { "^" ShiftExpr }
ShiftExpr      := AddExpr { ("<<" | ">>") AddExpr }
AddExpr        := MulExpr { ("+" | "-" | "++") MulExpr }
MulExpr        := UnaryExpr { ("*" | "×" | "/" | "%") UnaryExpr }
UnaryExpr      := ("!" | "-" | "~" ) UnaryExpr
               | PatchExpr

PatchExpr      := AppExpr { "<|" PatchLit }

AppExpr        := PostfixExpr { PostfixExpr }
PostfixExpr    := Atom { "." lowerIdent }

Atom           := Literal
               | lowerIdent
               | UpperIdent
               | "." lowerIdent                 (* accessor sugar *)
               | SuffixedParens
               | "(" Expr ")"
               | TupleLit
               | ListLit
               | RecordLit
               | "patch" PatchLit
               | Block
               | DoBlock
               | GenerateBlock
               | ResourceBlock

SuffixedParens := "(" Expr ")" Suffix
Suffix         := lowerIdent | "%"

Block          := "{" { Stmt } "}"
DoBlock        := "do" UpperIdent "{" { DoStmt } "}"
GenerateBlock  := "generate" "{" { GenStmt } "}"
ResourceBlock  := "resource" "{" { ResStmt } "}"

Stmt           := BindStmt | ValueBinding | Expr Sep
BindStmt       := Pattern "<-" Expr [ OrFallback ] Sep

DoStmt         := BindStmt
               | ValueBinding
               | Expr Sep
               | "when" Expr "<-" Expr Sep
               | "unless" Expr "<-" Expr Sep
               | "given" Expr "or" Expr Sep
               | "on" PostfixExpr "=>" Expr Sep
               | "recurse" Expr Sep
               | "loop" Pattern "=" Expr "=>" Block Sep

GenStmt        := BindStmt
               | GuardStmt
               | ValueBinding
               | "yield" Expr Sep
               | "recurse" Expr Sep
               | "loop" Pattern "=" Expr "=>" "{" { GenStmt } "}" Sep
GuardStmt      := lowerIdent "->" Expr Sep

ResStmt        := ValueBinding
               | BindStmt
               | Expr Sep
               | "yield" Expr Sep

TupleLit       := "(" Expr "," Expr { "," Expr } ")"
ListLit        := "[" [ ListItem { FieldSep ListItem } ] "]"
ListItem       := "..." Expr
               | Range
               | Expr
Range          := Expr ".." Expr

RecordLit      := "{" { RecordEntry } "}"
RecordEntry    := RecordField | RecordSpread
RecordField    := lowerIdent ":" Expr [ FieldSep ]
RecordSpread   := "..." Expr [ FieldSep ]

MapLit         := "~map" "{" [ MapEntry { FieldSep MapEntry } ] "}"
SetLit         := "~set" "[" [ SetEntry { FieldSep SetEntry } ] "]"
MapEntry       := Spread | Expr "=>" Expr
SetEntry       := Spread | Expr
Spread         := "..." Expr

SigilLit       := MapLit | SetLit | RawSigilLit
RawSigilLit    := "~" lowerIdent SigilBody
SigilBody      := SigilParen | SigilBracket | SigilBrace | SigilRegex
SigilParen     := "(" SigilText ")"
SigilBracket   := "[" SigilText "]"
SigilBrace     := "{" SigilText "}"
SigilRegex     := "/" SigilRegexText "/" [ lowerIdent ]

Literal        := "True"
               | "False"
               | IntLit
               | FloatLit
               | TextLit
               | CharLit
               | IsoInstantLit
               | SuffixedNumberLit
               | SigilLit
```

**Notes**

- `{ ... }` is used for both record-shaped forms (`RecordLit`, `RecordType`, `RecordPat`, `PatchLit`, and module/domain bodies) *and* expression blocks.
- Parsing `{ ... }` should disambiguate **record literal vs block** by lookahead:
  - If the first non-newline token begins a record entry (`...` spread, or a field name followed by `:`), parse as `RecordLit`.
  - Otherwise parse as `Block`.
- `.field` is shorthand for `x => x.field` (a unary accessor function).
- `_` is *not* a value. It only appears in expressions as part of the placeholder-lambda sugar (see [Desugaring: Functions](../desugaring/functions.md)).
- `RawSigilLit` content (`SigilText` / `SigilRegexText`) is lexed as raw text until the matching delimiter; `~map{}` and `~set[]` are parsed as structured literals (`MapLit` / `SetLit`).
- `RecordSpread` (`...expr`) merges fields left-to-right; later fields override earlier ones.

## 0.4 Patching

```ebnf
PatchLit       := "{" { PatchEntry } "}"
PatchEntry     := Path ":" PatchInstr [ FieldSep ]
PatchInstr     := "-" | ":=" Expr | Expr

Path           := PathSeg { [ "." ] PathSeg }
PathSeg        := lowerIdent
               | UpperIdent "." lowerIdent
               | Select
Select         := "[" ( "*" | Expr ) "]"
```

**Notes**

- `PathSeg` is intentionally permissive in this draft: patch paths, traversal selectors, and prism-like focuses share syntax.
- A compiler should reject ill-typed or ill-scoped path forms with a targeted error (e.g. “predicate selector expects a `Bool` predicate”).

## 0.5 Multi-clause unary functions

A *unary* multi-clause function can be written using arms directly:

```ebnf
ValueBinding   := lowerIdent "=" FunArms Sep
FunArms        := "|" Arm { Sep "|" Arm }
```

This form desugars to a single-argument function that performs pattern matching (`match`) on its input (see [Desugaring: Patterns](../desugaring/patterns.md)).
In v0.1, multi-clause function definitions require an explicit type signature for the function name.

If you want multi-argument matching, match on a tuple:

<<< ../snippets/from_md/syntax/grammar/multi_clause_unary_functions.aivi{aivi}

## 0.6 Types

```ebnf
Type           := TypeArrow
TypeArrow      := TypeAnd [ "->" TypeArrow ]
TypeAnd        := TypePipe { "with" TypePipe }
TypePipe       := TypeApp { "|>" TypeApp }
TypeApp        := TypeAtom { TypeAtom }
TypeAtom       := UpperIdent
               | lowerIdent
               | "*"
               | "(" Type ")"
               | TupleType
               | RecordType

TupleType      := "(" Type "," Type { "," Type } ")"
RecordType     := "{" { RecordTypeField } "}"
RecordTypeField:= lowerIdent ":" Type { FieldDecorator } [ FieldSep ]
FieldDecorator := "@" lowerIdent [ DecoratorArg ]
```

## 0.7 Patterns

```ebnf
Pattern        := PatAtom [ "as" Pattern ]
PatAtom        := "_"
               | lowerIdent
               | UpperIdent
               | Literal
               | TuplePat
               | ListPat
               | RecordPat
               | ConPat

ConPat         := UpperIdent { PatAtom }
TuplePat       := "(" Pattern "," Pattern { "," Pattern } ")"
ListPat        := "[" [ Pattern { "," Pattern } [ "," "..." [ (lowerIdent | "_") ] ] ] "]"

RecordPat      := "{" { RecordPatField } "}"
RecordPatField := RecordPatKey [ (":" Pattern) | ("as" Pattern) | ("." "{" { RecordPatField } "}") ] [ FieldSep ]
RecordPatKey   := lowerIdent { "." lowerIdent }
```

## 0.8 Diagnostics (where the compiler should nag)

- **Arms without a `match`**: `| p => e` is only valid after `match` *or* directly after `=` in the multi-clause unary function form.
- **Multi-clause signature requirement**: when a function name has multiple `=` definitions (multi-clause form), emit an error unless an explicit type signature for that name is present.
- **`_` placeholder**: `_ + 1` is only legal where a unary function is expected; otherwise error and suggest `x => x + 1`.
- **Deep keys in record literals**: `a.b: 1` should be rejected in record literals (suggest patching with `<|` if the intent was a path).
