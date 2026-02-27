; AIVI tree-sitter highlight queries for Zed

; ─── Comments ────────────────────────────────────────────────────────────────

(comment) @comment

; ─── Module / imports ────────────────────────────────────────────────────────

"module" @keyword
"use"    @keyword
"export" @keyword
"as"     @keyword
"hiding" @keyword

(module_declaration path: (module_path) @namespace)
(use_declaration    path: (module_path) @namespace)
(use_declaration    alias: (upper_identifier) @namespace)

(module_path (upper_identifier) @namespace)

; ─── Type definitions ────────────────────────────────────────────────────────

(type_definition name:  (upper_identifier) @type.definition)
(type_definition param: (lower_identifier) @type.parameter)
(type_constructor name:  (upper_identifier) @constructor)
(record_type_field name: (lower_identifier) @property)

; ─── Type annotations ────────────────────────────────────────────────────────

; Annotated name treated as function definition
(type_annotation name: (lower_identifier) @function)

(type_annotation (upper_identifier) @type)
(_type_expr       (upper_identifier) @type)
(_simple_type     (upper_identifier) @type)

; ─── Bindings ────────────────────────────────────────────────────────────────

(binding name: (lower_identifier) @function.definition)

; ─── Decorators ──────────────────────────────────────────────────────────────

(decorator "@" @attribute)
(decorator name: (lower_identifier) @attribute)

; ─── Keywords ────────────────────────────────────────────────────────────────

[
  "if" "then" "else" "when" "unless"
  "match" "given" "or"
] @keyword

[
  "do" "effect" "generate" "resource"
  "yield" "recurse"
] @keyword.control.return

(loop_keyword) @keyword.control.repeat

[
  "domain" "class" "instance"
  "over" "patch" "with"
  "machine" "on"
] @keyword

; ─── Arrow / lambda operators ────────────────────────────────────────────────

"=>" @operator
"<-" @operator
"->" @operator

; ─── Pipe operators ──────────────────────────────────────────────────────────

(pipe_operator) @operator.special

; ─── Binary operators ────────────────────────────────────────────────────────

[
  "+" "-" "*" "/" "%"
  "==" "!=" "<" ">" "<=" ">="
  "&&" "||" "??" "++" "::" ".."
] @operator

; ─── Accessor sugar ──────────────────────────────────────────────────────────

(accessor "." @punctuation.delimiter)
(accessor (lower_identifier) @property)

; ─── Record fields ───────────────────────────────────────────────────────────

(record_field      key: (lower_identifier) @property)
(record_pattern    (lower_identifier) @property)
(_record_field_pat key: (lower_identifier) @property)

; ─── Match arms ──────────────────────────────────────────────────────────────

(match_arm "when" @keyword)
(match_arm "|" @punctuation.delimiter)

; ─── Literals ────────────────────────────────────────────────────────────────

(boolean) @constant.builtin

(constructor) @constructor

(integer)     @number
(float)       @number
(unit_literal) @number.unit

(color_literal) @constant.color

; ─── Strings ─────────────────────────────────────────────────────────────────

(string)         @string
(backtick_string) @string
(char_literal)   @character

(escape_sequence)     @string.escape
(string_interpolation ["{" "}"] @string.special)
(string_content) @string

(sigil) @string.special

; ─── Wildcards / placeholders ────────────────────────────────────────────────

(wildcard) @variable.special

; ─── Identifiers ─────────────────────────────────────────────────────────────

(lower_identifier) @variable
(upper_identifier) @type

; ─── Punctuation ─────────────────────────────────────────────────────────────

["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ":"]                  @punctuation.delimiter
