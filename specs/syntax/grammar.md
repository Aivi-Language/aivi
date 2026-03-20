# Concrete Syntax (EBNF Reference)

This page is the concrete-syntax reference for AIVI's surface language. Most readers will use it when implementing tooling, debugging a parse error, or checking how the compiler groups tokens.

If you are learning the language, read the guide-style syntax pages first and come back here when you need the precise grammar. Where the reader-facing spec pages and the current parser are still being aligned, this page calls that out explicitly instead of pretending the mismatch does not exist.

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
- `Ident` in the EBNF below means either `lowerIdent` or `UpperIdent`.

### Keywords

```text
as class do domain effect else export generate given hiding if in
instance match mock module opaque or over patch recurse resource snapshot then unless use when with yield loop
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

At the top level, a file starts with optional module decorators and then a single module definition. For the practical reader-facing guides, see [Modules](modules.md), [Bindings and Scope](bindings.md), [Domains](domains.md), and [Type Classes & HKTs](types/classes_and_hkts.md).

```ebnf
Program        := { Decorator } ModuleDef
TopItem        := Definition

Decorator      := "@" lowerIdent [ DecoratorArg ] Sep
DecoratorArg   := Expr | RecordLit
Ident          := lowerIdent | UpperIdent

Definition     := ValueSig
               | ValueBinding
               | BrandedType
               | OpaqueType
               | TypeAlias
               | TypeDef
               | DomainDef
               | ClassDef
               | InstanceDef

ValueSig       := lowerIdent ":" Type Sep
ValueBinding   := Pattern "=" BindingRhs Sep
BindingRhs     := Expr | FunArms

BrandedType    := [ "opaque" ] UpperIdent [ TypeParams ] "=" Type "!" Sep
TypeAlias      := [ "opaque" ] UpperIdent [ TypeParams ] "=" Type Sep
OpaqueType     := UpperIdent [ TypeParams ] Sep
TypeDef        := [ "opaque" ] UpperIdent [ TypeParams ] "=" TypeConstructors Sep
TypeParams     := UpperIdent { UpperIdent }
TypeConstructors := [ Sep? "|" ] ConDef { Sep? "|" ConDef }
ConDef         := UpperIdent { TypeAtom }

ModuleDef      := "module" ModulePath Sep ModuleBodyImplicit
ModulePath     := lowerIdent { "." lowerIdent }
ModuleItem     := ExportStmt
               | UseStmt
               | DecoratedDefinition
ModuleBodyImplicit := { ModuleItem } EOF
(* `module` must be the first non-empty item in the file (after any module decorators). *)
DecoratedDefinition := { Decorator } ( ExportedDefinition | Definition )
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
     ExportStmt     := "export" ExportList Sep
ExportList     := ExportItem { "," ExportItem }
ExportItem     := lowerIdent | UpperIdent | ("domain" UpperIdent)
UseStmt        := "use" ModulePath [ UseSpec ] Sep
               | "use" ModulePath "(" GroupedImportList ")" Sep
UseSpec        := "as" Ident
               | "(" ImportList ")"
               | "hiding" "(" ImportList ")"
GroupedImportList := GroupedImportItem { (Sep | ",") GroupedImportItem }
GroupedImportItem := lowerIdent "(" ImportList ")"
ImportList     := ImportItem { ("," | Sep) ImportItem }
ImportItem     := (lowerIdent | UpperIdent | ("domain" UpperIdent)) [ "as" Ident ]

DomainDef      := "domain" UpperIdent "over" Type "=" "{" { DomainItem } "}" Sep
DomainItem     := { Decorator } DomainEntry
DomainEntry    := TypeDef
               | ValueSig
               | ValueBinding
               | OpDef
               | DeltaLitSig
               | DeltaLitBinding
OpDef          := "(" Operator ")" ":" Type Sep
               | "(" Operator ")" Pattern { Pattern } "=" Expr Sep
Operator       := "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" | "++" | "??"
               | "×"
DeltaLitSig    := SuffixedNumberLit ":" Type Sep
DeltaLitBinding:= SuffixedNumberLit "=" Expr Sep

ClassDef       := "class" UpperIdent ClassParams "=" ClassRhs Sep
ClassParams    := ClassParam { ClassParam }
ClassParam     := TypeAtom

(* Classes are records of methods ("dictionaries") with optional superclass composition.
   A class may also declare constraints on the type variables used in its member signatures. *)
ClassRhs       := [ ClassPrelude ] [ ClassMembers ]
ClassPrelude   := ClassPreludeItem { "," ClassPreludeItem }
ClassPreludeItem := UpperIdent | ClassConstraints
ClassConstraints := "given" "(" TypeVarConstraint { "," TypeVarConstraint } ")"
TypeVarConstraint := UpperIdent ":" UpperIdent
ClassMembers   := RecordType

InstanceDef    := "instance" UpperIdent InstanceHead "=" [ InstanceConstraints ] "{" { InstanceItem } "}" Sep
InstanceHead   := TypeAtom { TypeAtom }
InstanceConstraints := "given" "(" TypeVarConstraint { "," TypeVarConstraint } ")"
InstanceItem   := lowerIdent ":" Expr Sep
               | ValueBinding

```

Notes:

- `OpaqueType` is the bare abstract form: a line such as `Token` or `Token A` declares a name without exposing a representation.
- `TypeAlias` covers transparent aliases such as `User = { name: Text }` or `Handler = Req -> Effect Err Res`.
- `TypeDef` is the constructor-list form (`Foo = A | B | C`) used for ADTs.
- Decorators may appear before ordinary declarations and before inline `export` declarations, but not before standalone `use` or export-list items.
- `hiding (...)` excludes the listed exports from an otherwise wildcard-like module import.
- A grouped import `use a.b (c (...), d (...))` desugars to `use a.b.c (...)` + `use a.b.d (...)` during parsing. The resolver and typechecker only see flat `UseDecl` values.
- In the broader docs, “binding” is sometimes used more loosely for destructuring `=` forms. This grammar keeps the parser-facing distinction explicit and uses `BindingRhs` to show where the arm form from §0.5 fits.

## 0.3 Expressions

Expressions are where most everyday syntax lives: application, pipelines, `match`, blocks, and literals.

Useful reading rules:

- application is by whitespace
- field access is postfix (`value.field`)
- pipes have lower precedence than arithmetic and comparisons
- `do`, `generate`, and `resource` are expression forms

```ebnf
Expr           := IfExpr

IfExpr         := "if" Expr "then" Expr "else" Expr
               | LambdaExpr

LambdaExpr     := LambdaHead Sep? "=>" Expr
               | MatchExpr
LambdaHead     := LambdaParam { Sep? LambdaParam }
LambdaParam    := PatParam
               | PatchedParam
PatchedParam   := lowerIdent "<|" PipeArg
LambdaArgs     := PatParam { PatParam }
PatParam       := lowerIdent [ "as" PatParam ]
               | "_"
               | RecordPat
               | TuplePat
               | ListPat
               | "(" PatParam ")"
(* `name <| updater => body` is argument-patch sugar. It desugars to a lambda whose
   body starts by shadowing `name` with `name |> updater`. The updater uses the same
   grammar as a pipe RHS (`PipeArg`), so placeholder transforms like `_ + 1`,
   accessor sugar like `.field`, explicit lambdas, and bare matcher blocks all work.
   In v0.1, only simple identifier parameters may use the `<|` head form. *)

MatchExpr      := PipeExpr [ "match" MatchArms ] [ OrFallback ]
MatchArms      := Sep? "|" Arm { Sep "|" Arm }
Arm            := Pattern [ MatchGuard ] "=>" Expr
MatchGuard     := ( "when" | "unless" ) Expr

OrFallback     := "or" ( Expr | OrArms )
OrArms         := Sep? "|" OrArm { Sep "|" OrArm }
OrArm          := Pattern [ MatchGuard ] "=>" Expr

PipeExpr       := CoalesceExpr { ("|>" | "->>") PipeArg }
PipeArg        := LambdaArgs "=>" Expr
               | MatchArms
               | CoalesceExpr
(* `|>` is the regular pipe. `->>` is the signal-derive pipe; the LHS must be
   a Signal A and the result is another Signal. Both share precedence 2. *)

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

PatchExpr      := AppExpr { ("<|" | "<<-") PatchArg }
PatchArg       := PatchLit | AppExpr
(* `<|` is record-patch. When the left-hand side elaborates to a database
   selector from `aivi.database`, `<| { ... }` updates the selected rows.
   Database row deletion uses `db.delete` / `db.deleteOn`. `<<-` is the
   signal-write operator; the LHS must be a Signal A. Both share precedence 10.
   Signal-write semantics:
     signal <<- value   → set signal value
     signal <<- fn      → update signal fn
     signal <<- { ... } → update signal (patch { ... }) *)

AppExpr        := PostfixExpr { PostfixExpr }
PostfixExpr    := Atom { PostfixOp }
PostfixOp      := "." lowerIdent
               | "[" Expr "]"

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
               | MockExpr
               | Block
               | DoBlock
               | EffectBlock
               | GenerateBlock
               | ResourceBlock

SuffixedParens := "(" Expr ")" Suffix
Suffix         := lowerIdent | "%"

Block          := "{" { Stmt } "}"
DoBlock        := "do" UpperIdent "{" { DoStmt } "}"
EffectBlock    := "effect" "{" { DoStmt } "}"   (* deprecated alias for `do Effect { ... }` *)
GenerateBlock  := "generate" "{" { GenStmt } "}"
ResourceBlock  := "resource" "{" { ResStmt } "}"

MockExpr       := "mock" MockBinding { Sep? "mock" MockBinding } "in" Expr
MockBinding    := [ "snapshot" ] MockPath [ "=" Expr ]
MockPath       := lowerIdent { "." lowerIdent }

Stmt           := BindStmt | ValueBinding | Expr Sep
BindStmt       := Pattern "<-" Expr [ OrFallback ] Sep

DoStmt         := BindStmt
               | ValueBinding
               | Expr Sep
               | "when" Expr "<-" Expr Sep
               | "unless" Expr "<-" Expr Sep
               | "given" Expr "or" ( Expr | OrArms ) Sep
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
RecordField    := RecordKey ":" Expr [ FieldSep ]
               | lowerIdent [ FieldSep ]       (* shorthand field *)
RecordKey      := lowerIdent { "." lowerIdent }
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
  - if the first non-newline token can start a record entry (`...`, `name`, or `name.path:`), parse as `RecordLit`
  - otherwise parse as `Block`
- `RecordTypeSpread` (`...Type`) merges record-type entries left to right, with later entries overriding earlier ones
- a `RecordTypeSpread` target must elaborate to a closed record type
- `.field` is shorthand for `x => x.field`
- `_` is not a value; in expression position it appears only as placeholder-lambda sugar
- `mock snapshot some.binding` records the real binding result for later replay; see [@test — Test Declarations](decorators/test.md#mock-expressions)
- `RawSigilLit` content is lexed as raw text until the matching delimiter; `~map{}` and `~set[]` are structured literals, and HTML/GTK angle sigils are documented in [Operators and Context](operators.md#118-sigils)
- `RecordSpread` (`...expr`) merges fields left to right, with later fields overriding earlier ones
- `name.path: value` inside a record literal builds nested record-shaped data from scratch. If you meant “update an existing nested value”, use `<|` / `patch`.
- Postfix brackets reuse one syntax family for list indexing, map indexing, and database row selectors such as `users[id == userId]`. Which meaning applies is decided during elaboration from the left-hand side type.

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
- `-` inside `PatchLit` still means field removal. Database row deletion is separate and uses `db.delete table[pred]`.

## 0.5 Multi-clause unary functions

A unary function can be written directly as a list of pattern arms.

```ebnf
FunArms        := "|" Arm { Sep "|" Arm }
```

This form is one possible `BindingRhs` from §0.2. It desugars to a one-argument function that performs a `match` on its input.

If you want matching on more than one value, match on a tuple:

<<< ../snippets/from_md/syntax/grammar/multi_clause_unary_functions.aivi{aivi}

## 0.6 Types

The type grammar covers function arrows, type application, tuple types, and record types.

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
RecordType      := "{" { RecordTypeEntry } "}"
RecordTypeEntry := RecordTypeSpread
                 | lowerIdent ":" Type [ FieldSep ]
RecordTypeSpread := "..." Type [ FieldSep ]
```

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

- **arms without a `match`**: `| p => e` is valid only after `match`, directly after `=` in the multi-clause unary form, or as the right-hand side of `|>` / `->>`
- **multi-clause signature requirement**: when a function uses multiple pattern clauses, require an explicit type signature for that name
- **`_` placeholder misuse**: `_ + 1` is legal only where a unary function is expected; otherwise suggest `x => x + 1`
- **deep keys in record literals**: if `a.b: 1` appears where an update was intended, suggest `<|` / `patch`; in a record literal it builds nested data rather than patching an existing value
