# Concrete Syntax (EBNF Reference)

This page is the exact parsing reference for AIVI's surface language. Most readers will use it when implementing tooling, debugging a parse error, or checking how the compiler groups tokens.

If you are learning the language, read the guide-style syntax pages first and come back here when you need the precise grammar.

A quick reminder for the notation used below:

- `:=` means “is defined as”
- `[ ... ]` means “optional”
- `{ ... }` means “zero or more repetitions”
- comments in `(* ... *)` explain intent but are not part of the grammar

The grammar here is about **parsing**. Typing, elaboration, and runtime behavior are described in the other syntax and semantics chapters.

## 0.1 Lexical notes

> These rules are normative for parsing.

### Whitespace and comments

- Whitespace separates tokens and is otherwise insignificant. AIVI is not indentation-sensitive.
- Line comments start with `//` and run to the end of the line.
- Block comments start with `/*` and end with `*/`.

### Identifiers

- `lowerIdent` starts with a lowercase ASCII letter and is used for values, functions, and fields.
- `UpperIdent` starts with an uppercase ASCII letter and is used for types, constructors, modules, domains, and classes.
- After the first character, identifiers may contain ASCII letters, digits, and `_`.
- Keywords are reserved and cannot be reused as identifiers.

### Keywords

```text
as class do domain effect else export generate given hiding if in
instance machine match mock module on opaque or over patch recurse resource snapshot then unless use when with yield loop
```

(`True`, `False`, `None`, `Some`, `Ok`, and `Err` are ordinary constructors, not keywords.)

### Literals

The surface language includes these literal forms:

- `IntLit`: decimal digits such as `0` or `42`
- `FloatLit`: digits with a fractional part such as `3.14`
- `TextLit`: double-quoted text with escapes and interpolation
- `CharLit`: single-quoted character literals
- `IsoInstantLit`: ISO-8601 instant-like tokens such as `2024-05-21T12:00:00Z`
- `SuffixedNumberLit`: a number followed immediately by a suffix such as `10px`, `100%`, or `30s`

`SuffixedNumberLit` is a lexical form only. The lexer recognizes the token, but a domain decides what the suffix means.

### Text literals and interpolation

Text literals use `"..."` and support interpolation with `{ Expr }`.

<<< ../snippets/from_md/syntax/grammar/text_literals_and_interpolation.aivi{aivi}

Inside a text literal, `{` starts interpolation and `}` ends it. Braces must balance within the interpolated expression. If an interpolation appears where `Text` is expected, the compiler may insert a `toText` coercion.

### Separators (layout)

Many constructs accept one or more newlines as separators. Some comma-delimited forms also allow `,`.

The grammar names those separators like this:

```ebnf
Sep        := Newline { Newline }
FieldSep   := Sep | ","
```

### Ellipsis

- `...` is one token, used for list rest patterns and spread entries.

## 0.2 Top level

At the top level, a file starts with optional decorators and then a single module definition.

```ebnf
Program        := { Decorator } ModuleDef
TopItem        := Definition

Decorator      := "@" lowerIdent [ DecoratorArg ] Sep
DecoratorArg   := Expr | RecordLit

Definition     := ValueSig
               | ValueBinding
               | BrandedType
               | OpaqueType
               | TypeAlias
               | TypeDef
               | DomainDef
               | ClassDef
               | InstanceDef
               | MachineDef

ValueSig       := lowerIdent ":" Type Sep
ValueBinding   := Pattern "=" Expr Sep

BrandedType    := [ "opaque" ] UpperIdent [ TypeParams ] "=" Type "!" Sep
TypeAlias      := [ "opaque" ] UpperIdent [ TypeParams ] "=" TypeRhs Sep
OpaqueType     := UpperIdent [ TypeParams ] Sep
TypeDef        := [ "opaque" ] UpperIdent [ TypeParams ] "=" TypeRhs Sep
TypeParams     := UpperIdent { UpperIdent }
TypeRhs        := Type
               | RecordType
               | [ Sep? "|" ] ConDef { Sep? "|" ConDef }
ConDef         := UpperIdent { TypeAtom }

ModuleDef      := "module" ModulePath Sep ModuleBodyImplicit
ModulePath     := ModuleSeg { "." ModuleSeg }
ModuleSeg      := lowerIdent | UpperIdent
ModuleItem     := ExportStmt | ExportedDefinition | UseStmt | Definition
ModuleBodyImplicit := { ModuleItem } EOF
(* `module` must be the first non-empty item in the file (after any module decorators). *)
ExportedDefinition := "export" ExportableDefinition
ExportableDefinition := ValueSig
                    | ValueBinding
                    | BrandedType
                    | OpaqueType
                    | TypeAlias
                    | TypeDef
                    | DomainDef
                    | ClassDef
                    | InstanceDef
                    | MachineDef
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
DeltaLitBinding:= SuffixedNumberLit "=" Expr Sep

ClassDef       := "class" UpperIdent ClassParams "=" ClassRhs Sep
ClassParams    := ClassParam { ClassParam }
ClassParam     := TypeAtom

(* Classes are records of methods ("dictionaries") with optional superclass composition.
   A class may also declare constraints on the type variables used in its member signatures. *)
ClassRhs       := [ ClassSupers ] [ ClassConstraints ] ClassMembers
ClassSupers    := UpperIdent { "," UpperIdent }
ClassConstraints := "given" "(" TypeVarConstraint { "," TypeVarConstraint } ")"
TypeVarConstraint := UpperIdent ":" UpperIdent
ClassMembers   := RecordType

InstanceDef    := "instance" UpperIdent InstanceHead "=" RecordLit Sep
InstanceHead   := "(" Type ")"

MachineDef     := "machine" UpperIdent "=" "{" { MachineTransition } "}" Sep
MachineTransition := [ UpperIdent ] "->" UpperIdent ":" lowerIdent "{" { FieldDecl } "}"
FieldDecl      := lowerIdent ":" Type
```

## 0.3 Expressions

Expressions are where most everyday syntax lives: application, pipelines, `match`, blocks, and literals.

Useful reading rules:

- application is by whitespace
- field access is postfix (`value.field`)
- pipes have lower precedence than arithmetic and comparisons
- `do`, `generate`, and `resource` are expression forms

```ebnf
Expr           := WithCapsExpr

WithCapsExpr   := "with" CapabilitySet "in" Expr
               | IfExpr

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
CmpExpr        := AddExpr { ("<" | "<=" | ">" | ">=") AddExpr }
AddExpr        := MulExpr { ("+" | "-" | "++") MulExpr }
(* Note: bitwise operations (and, or, xor, shift, complement) are not language operators.
   They are provided by the `aivi.bits` standard library module. *)
MulExpr        := UnaryExpr { ("*" | "×" | "/" | "%") UnaryExpr }
UnaryExpr      := ("!" | "-") UnaryExpr
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

- `{ ... }` is used for both record-shaped forms (`RecordLit`, `RecordType`, `RecordPat`, `PatchLit`, and module/domain bodies) and expression blocks.
- Parsing `{ ... }` should disambiguate **record literal vs block** by lookahead:
  - if the first non-newline token begins a record entry (`...` spread, or a field name followed by `:`), parse as `RecordLit`
  - otherwise parse as `Block`
- `.field` is shorthand for `x => x.field`
- `_` is not a value; in expression position it appears only as placeholder-lambda sugar
- `RawSigilLit` content is lexed as raw text until the matching delimiter; `~map{}` and `~set[]` are structured literals
- `RecordSpread` (`...expr`) merges fields left to right, with later fields overriding earlier ones

## 0.4 Patching

Patching syntax updates record-shaped data by describing what to change instead of rebuilding the whole value manually.

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

- `PathSeg` is intentionally broad so path navigation, traversal selectors, and prism-like focuses can share one syntax family.
- The compiler should reject ill-typed or ill-scoped path forms with targeted diagnostics.

## 0.5 Multi-clause unary functions

A unary function can be written directly as a list of pattern arms.

```ebnf
ValueBinding   := lowerIdent "=" FunArms Sep
FunArms        := "|" Arm { Sep "|" Arm }
```

This form desugars to a one-argument function that performs a `match` on its input.

If you want matching on more than one value, match on a tuple:

<<< ../snippets/from_md/syntax/grammar/multi_clause_unary_functions.aivi{aivi}

## 0.6 Types

The type grammar covers function arrows, type application, tuple types, record types, and capability clauses.

```ebnf
Type           := TypeArrow [ CapabilityClause ]
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

CapabilityClause := "with" CapabilitySet
CapabilitySet    := "{" CapabilityPath { "," CapabilityPath } "}"
CapabilityPath   := lowerIdent { "." lowerIdent }

TupleType      := "(" Type "," Type { "," Type } ")"
RecordType     := "{" { RecordTypeField } "}"
RecordTypeField:= lowerIdent ":" Type [ FieldSep ]
```

Capability clauses matter only for effect-like types such as `Effect ...` and `Resource ...`. Likewise, `with { ... } in expr` narrows the capability scope; it does not install handlers by itself.

## 0.7 Patterns

Patterns are the shapes used by bindings, `match` arms, function clauses, and destructuring.

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
RecordPatField := RecordPatKey [ (":" Pattern) | ("as" Pattern) ] [ FieldSep ]
RecordPatKey   := lowerIdent { "." lowerIdent }
```

## 0.8 Diagnostics (where the compiler should nag)

Good syntax errors are part of the language experience. These are especially useful cases to diagnose clearly:

- **arms without a `match`**: `| p => e` is valid only after `match` or directly after `=` in the multi-clause unary form
- **multi-clause signature requirement**: when a function uses multiple pattern clauses, require an explicit type signature for that name
- **`_` placeholder misuse**: `_ + 1` is legal only where a unary function is expected; otherwise suggest `x => x + 1`
- **deep keys in record literals**: reject `a.b: 1` in record literals and suggest patching with `<|` when appropriate
