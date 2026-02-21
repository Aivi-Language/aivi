# Standard Library: JSON Parsing & Validation

<!-- quick-info: {"kind":"module","name":"aivi.json"} -->
The `aivi.json` module defines parsing strategies that connect raw data sources to typed structures via the `Validation` Applicative. 
<!-- /quick-info -->

<div class="import-badge">use aivi.json</div>

## 1. Type-Driven Parsing 

AIVI utilizes the type expected by the assignment to drive validation. Behind the scenes, `parse` takes a raw string (or dynamically parsed JSON Document) and checks it against an implicit AST dictionary of the expected shape.

<<< ../../snippets/from_md/05_stdlib/01_data/01_json/block_01.aivi{aivi}

Because the output is `Validation (List DecodeError) A`, the caller never gets a malformed `User` structure that crashes deep within the logic phase. AIVI accumulates all structural errors instead of failing upon discovering the missing `age` key.

## 2. Integrating Decode with External Sources

A large part of the AIVI vision is that `Source` declarations automatically perform this validation when accessed via `<-` inside an `Effect` block. The implementation bridges `file.read` with `json.parse`.

<<< ../../snippets/from_md/05_stdlib/01_data/01_json/block_02.aivi{aivi}

## 3. Custom Decoders for Enums / Complex Types

Developers can supply custom decoders for types that cannot be structurally derived automatically. A decoder is any function returning a `Validation (List DecodeError) A`.

<<< ../../snippets/from_md/05_stdlib/01_data/01_json/block_03.aivi{aivi}
