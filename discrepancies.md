# AIVI Discrepancy Report

This report documents discrepancies, broken features, and misleading documentation found by comparing the `specs/` (Source of Truth) with the `crates/aivi/` (Implementation).

## 1. Structured Sigils (`~map`, `~set`) are Unreachable
**Severity:** Critical (Feature broken)

*   **Spec:** `specs/02_syntax/13_sigils.md` and `00_grammar.md` define structured parsing for `~map{...}` and `~set[...]` where the content is parsed as AIVI expressions.
*   **Code:** `crates/aivi/src/surface/parser.rs` contains methods `parse_structured_sigil`, `parse_map_literal`, and `parse_set_literal` (lines 1857-1970).
*   **Issue:** `parse_primary` (lines 1746-1855) **never calls** `parse_structured_sigil`. It only checks for `consume_sigil()` (which returns a `Literal::Sigil` string) or standard tokens. If the lexer emits `~` as a symbol, parsing fails. If it emits `~map{}` as a Sigil token, it becomes a string literal, not a structured map.
*   **Consequence:** `~map` and `~set` literals defined in the Spec do not work in the current implementation.
-> implement structured sigils

## 2. Standard Library Argument Order
**Severity:** Major (Breaks Guidelines/Pipelines)

The AIVI guidelines (`AGENTS.md`) emphasize pipelines (`data |> transform`). This requires functions to be "Function First, Data Last" (e.g., `map f list`).

### Logic Module (`aivi.logic`)
*   **Spec:** `map: F A -> (A -> B) -> F B` (Data First - **Wrong for pipelines**)
*   **Code:** `map: (A -> B) -> F A -> F B` (Function First - **Correct for pipelines**)
*   **Status:** Code is correct/idiomatic, Spec is outdated or incorrect.
-> go with map: (A -> B) -> F A -> F

### Text Module (`aivi.text`)
*   **Spec:** `contains haystack needle` (`Text -> Text -> Bool`)
*   **Code:** `contains haystack needle` (`Text -> Text -> Bool`)
*   **Examples:** `text.md` shows `slug |> replaceAll " " "-"`.
*   **Issue:** Both Spec and Code define functions as **Data First** (e.g., `contains haystack needle`). This makes idiomatic pipelining impossible (e.g., `text |> contains "x"` would evaluate to `contains "x" text` which expects `haystack` as first arg, so it receives `"x"` as haystack).
*   **Consequence:** The Usage Examples in `text.md` are valid syntax but semantically wrong or type-checking failures given the current definitions. The library design contradicts the `|>` guideline.
-> switch order to allow pipe usage

## 3. Class Inheritance & Monad Definition
**Severity:** Moderate (Implementation Detail/Hack)

*   **Spec:** `logic.md` defines classes using intersection/inheritance:
    ```aivi
    class Ord A = Setoid A & { lte: ... }
    class Monad (M *) = Applicative (M *) & Chain (M *)
    ```
*   **Code:** `logic.rs` defines independent classes:
    ```aivi
    class Ord A = { lte: ... } // Missing Setoid superclass
    class Monad (M *) = { __monad: Unit } // Marker class hack
    ```
*   **Comment in Code:** "// Without class inheritance syntax, we model Monad as a marker class..."
*   **Issue:** The parser *does* appear to support `&` in class declarations (`parse_class_decl` flattens intersections), so the comment might be outdated, or the typechecker doesn't support the resulting structure yet.
-> let's check this and replace "&" combinator with a dedicated "with" keyword
-> Check for valid typechecker implementation

## 4. Decorators Support
**Severity:** Moderate (Incomplete Feature)

*   **Spec:** `grammar.md` allows decorators on any `Definition` (`class`, `instance`, `type`, etc.).
*   **Code:** `parser.rs` explicitly emits error `E1507` ("decorators are not supported ... yet") for `class`, `instance`, `domain`, `type`, `export`, and `use`. Only `module` and `def` (value bindings) support decorators.
-> add support for decorators where appropriate, since we have only a fixed set of decorators.

## 5. Named Instances
**Severity:** Minor (Syntax Gap)

*   **Spec:** `grammar.md` allows named instances: `instance [Name :] Class ...`.
*   **Code:** `parse_instance_decl` assumes the first identifier is the Class name. It does not check for a `:` separator to parse an optional Instance Name.
*   **Consequence:** Syntax `instance MyShow : Show Int = ...` will likely verify fail or parse incorrectly.
-> remove named instances from spec

## 6. Text Examples Validity
**Severity:** Minor (Documentation Error)

*   **File:** `specs/05_stdlib/00_core/02_text.md`
*   **Snippet:** `slug |> trim |> toLower |> replaceAll " " "-"`
*   **Issue:** `replaceAll` is defined as `text -> needle -> replacement`. In a pipeline `text |> replaceAll " " "-"`, the call becomes `(replaceAll " " "-") text`. This passes `" "` as `text`, `"-"` as `needle`, and `text` (the slug) as `replacement`. This is semantically backwards. The function should be `needle -> replacement -> text`.
-> make it: needle -> replacement -> text`
