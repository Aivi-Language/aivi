/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

/**
 * Tree-sitter grammar for the AIVI language.
 *
 * AIVI is a statically-typed, purely-functional, expression-oriented language.
 * Identifiers: lowerCamelCase for values/functions, UpperCamelCase for types.
 * No mutation, no null — uses Option/Result and recursion instead.
 */

module.exports = grammar({
  name: "aivi",

  extras: ($) => [/\s+/, $.comment],

  word: ($) => $.lower_identifier,

  conflicts: ($) => [
    // type_constructor params vs _simple_type in type body
    [$.type_constructor, $._simple_type],
    // lambda params (wildcards) vs wildcard atoms
    [$.lambda, $._atom],
    // standalone decorator (_definition) vs decorator before a binding
    [$._definition, $.binding],
  ],

  rules: {
    source_file: ($) => repeat($._definition),

    _definition: ($) =>
      choice(
        $.module_declaration,
        $.use_declaration,
        $.type_definition,
        $.type_annotation,
        $.binding,
        $.decorator,
      ),

    // ─── Comments ────────────────────────────────────────────────────────────

    comment: ($) =>
      token(
        choice(
          seq("//", /.*/),
          seq("/*", /[^*]*\*+([^/*][^*]*\*+)*/, "/"),
        ),
      ),

    // ─── Module / imports ────────────────────────────────────────────────────

    module_declaration: ($) =>
      seq("module", field("path", $.module_path)),

    use_declaration: ($) =>
      seq(
        choice("use", "export"),
        field("path", $.module_path),
        optional(seq("as", field("alias", choice($.upper_identifier, $.lower_identifier)))),
        optional(
          seq("hiding", "(", commaSep($.lower_identifier), ")"),
        ),
      ),

    module_path: ($) =>
      seq(
        choice($.upper_identifier, $.lower_identifier),
        repeat(seq(".", $.lower_identifier)),
      ),

    // ─── Type definitions ────────────────────────────────────────────────────

    // e.g.  Option A = None | Some A
    //       Color    = Red | Green | Blue
    //       Email    = Text!
    //       Pair A B = (A, B)
    type_definition: ($) =>
      seq(
        field("name", $.upper_identifier),
        repeat(field("param", $.upper_identifier)),  // type params use UpperCamelCase
        "=",
        field("body", $._type_body),
      ),

    _type_body: ($) =>
      choice(
        // branded nominal type: `Text!`
        seq($._type_expr, "!"),
        // ADT variants
        seq(
          optional("|"),
          $.type_constructor,
          repeat(seq("|", $.type_constructor)),
        ),
        // type alias or single-constructor
        $._type_expr,
      ),

    type_constructor: ($) =>
      prec.left(
        seq(
          field("name", $.upper_identifier),
          repeat(field("param", $._simple_type)),
        ),
      ),

    _type_expr: ($) =>
      prec.right(
        seq($._type_app, repeat(seq("->", $._type_app))),
      ),

    // Type application: Pair A B, Maybe Int, Result E A
    _type_app: ($) =>
      prec.right(seq($._simple_type, repeat($._simple_type))),

    _simple_type: ($) =>
      choice(
        $.upper_identifier,
        seq("[", $._type_expr, "]"),
        seq("(", commaSep($._type_expr), ")"),
        seq("{", commaSep($.record_type_field), "}"),
      ),

    record_type_field: ($) =>
      seq(field("name", $.lower_identifier), ":", $._type_expr),

    // ─── Type annotation ─────────────────────────────────────────────────────

    // e.g.  add : Int -> Int -> Int
    type_annotation: ($) =>
      seq(field("name", $.lower_identifier), ":", $._type_expr),

    // ─── Bindings ────────────────────────────────────────────────────────────

    // e.g.  x     = 42
    //       add   = a b => a + b
    //       greet name = "Hello, {name}!"
    //       describe =
    //         | 0 => "zero"
    //         | _ => "many"
    binding: ($) =>
      seq(
        field("name", $.lower_identifier),
        "=",
        field("value", choice($.multi_clause_function, $._expression)),
      ),

    // Multi-clause function: only valid as the direct RHS of a binding.
    multi_clause_function: ($) =>
      prec.left(repeat1($.match_arm)),

    decorator: ($) =>
      seq("@", field("name", $.lower_identifier)),

    // ─── Expressions ─────────────────────────────────────────────────────────

    _expression: ($) =>
      choice(
        $.lambda,
        $.if_expression,
        $.match_expression,
        $.do_block,
        $.effect_block,
        $.generate_block,
        $.resource_block,
        $.pipe_expression,
        $.binary_expression,
        $.application,
        $._atom,
      ),

    // e.g.  a b => a + b
    lambda: ($) =>
      prec.right(
        seq(
          repeat1(choice($.lower_identifier, $.wildcard)),
          "=>",
          $._expression,
        ),
      ),

    if_expression: ($) =>
      prec.right(
        seq(
          "if",
          $._expression,
          "then",
          $._expression,
          optional(seq("else", $._expression)),
        ),
      ),

    // e.g.  value match | Ok x => x | Err _ => 0
    match_expression: ($) =>
      prec.left(
        seq($._atom, "match", repeat1($.match_arm)),
      ),

    match_arm: ($) =>
      seq(
        "|",
        field("pattern", $._pattern),
        optional(seq("when", field("guard", $._atom))),
        "=>",
        field("body", $._expression),
      ),

    // do Effect { x <- action; y = 1; expr }
    do_block: ($) => seq("do", optional($.upper_identifier), "{", repeat($._do_statement), "}"),

    _do_statement: ($) =>
      choice(
        seq($.lower_identifier, "<-", $._expression),
        seq($.lower_identifier, "=", $._expression),
        $._expression,
      ),

    // effect { x <- action }
    effect_block: ($) =>
      seq("effect", "{", repeat($._effect_statement), "}"),

    _effect_statement: ($) =>
      choice(
        seq($.lower_identifier, "<-", $._expression),
        $._expression,
      ),

    // generate { yield expr }
    generate_block: ($) =>
      seq("generate", "{", repeat($._gen_statement), "}"),

    _gen_statement: ($) =>
      choice(
        seq("yield", $._expression),
        seq($.loop_keyword, $._expression),
        seq($.lower_identifier, "=", $._expression),
        $._expression,
      ),

    loop_keyword: (_$) => "loop",

    resource_block: ($) =>
      seq("resource", "{", repeat($._do_statement), "}"),

    pipe_expression: ($) =>
      prec.left(
        1,
        seq(
          $._atom,
          field("operator", $.pipe_operator),
          $._expression,
        ),
      ),

    binary_expression: ($) =>
      prec.left(
        2,
        seq(
          field("left", $._atom),
          field("operator", $._binary_op),
          field("right", $._atom),
        ),
      ),

    _binary_op: ($) =>
      choice(
        "+",  "-",  "*",  "/",  "%",
        "==", "!=", "<",  ">",  "<=", ">=",
        "&&", "||", "??", "++", "::", "..",
      ),

    application: ($) =>
      prec.left(10, seq($._atom, repeat1($._atom))),

    // ─── Atoms (primary expressions) ─────────────────────────────────────────

    _atom: ($) =>
      choice(
        $.boolean,
        $.constructor,
        $.wildcard,
        $.unit_literal,
        $.float,
        $.integer,
        $.color_literal,
        $.string,
        $.backtick_string,
        $.char_literal,
        $.sigil,
        $.list,
        $.record,
        $.tuple_or_group,
        $.accessor,
        $.lower_identifier,
        $.upper_identifier,
      ),

    // .fieldName — accessor sugar: shorthand for x => x.fieldName
    accessor: ($) => seq(".", $.lower_identifier),

    // ─── Patterns ────────────────────────────────────────────────────────────

    _pattern: ($) =>
      choice(
        $.wildcard,
        $.boolean,
        $.constructor,
        $.integer,
        $.float,
        $.string,
        $.char_literal,
        $.constructor_pattern,
        $.record_pattern,
        $.tuple_pattern,
        $.list_pattern,
        $.lower_identifier,
      ),

    constructor_pattern: ($) =>
      seq(
        field("name", $.upper_identifier),
        repeat(field("arg", $._simple_pattern)),
      ),

    _simple_pattern: ($) =>
      choice(
        $.lower_identifier,
        $.wildcard,
        $.integer,
        $.string,
        $.boolean,
        $.constructor,
        seq("(", $._pattern, ")"),
      ),

    record_pattern: ($) =>
      seq("{", commaSep($._record_field_pat), "}"),

    _record_field_pat: ($) =>
      choice(
        seq(
          field("key", $.lower_identifier),
          ":",
          field("value", $._pattern),
        ),
        // shorthand: `{ name }` binds name by name
        $.lower_identifier,
      ),

    tuple_pattern: ($) =>
      seq("(", $._pattern, ",", commaSep1($._pattern), ")"),

    list_pattern: ($) =>
      seq(
        "[",
        commaSep($._pattern),
        optional(seq(",", "...", field("rest", $.lower_identifier))),
        "]",
      ),

    // ─── Literals ────────────────────────────────────────────────────────────

    boolean: (_$) =>
      token(prec(1, choice("True", "False"))),

    constructor: (_$) =>
      token(prec(1, choice("None", "Some", "Ok", "Err"))),

    wildcard: (_$) => token(prec(1, "_")),

    // unit_literal must be tried before float/integer (longest match wins in tree-sitter,
    // but we also set higher precedence just in case)
    unit_literal: (_$) =>
      token(prec(1, /[0-9]+(?:\.[0-9]+)?[a-z][A-Za-z0-9_]*/)),

    float: (_$) => token(/[0-9]+\.[0-9]+/),

    integer: (_$) => token(/[0-9]+/),

    color_literal: (_$) => token(/#[0-9a-fA-F]{6}/),

    // "hello { name }" — double-quoted strings with interpolation
    string: ($) =>
      seq(
        '"',
        repeat(
          choice(
            $.escape_sequence,
            $.string_interpolation,
            alias(token.immediate(/[^"\\{]+/), $.string_content),
          ),
        ),
        '"',
      ),

    escape_sequence: (_$) =>
      token.immediate(
        seq("\\", choice(/[nrt"\\]/, seq("u{", /[0-9a-fA-F]+/, "}"))),
      ),

    string_interpolation: ($) =>
      seq(token.immediate("{"), $._expression, "}"),

    // `raw string` — no interpolation
    backtick_string: (_$) => token(seq("`", /[^`]*/, "`")),

    char_literal: (_$) =>
      token(seq("'", choice(/[^'\\]/, seq("\\", /[nrt'\\]/)), "'")),

    // Sigils: ~name/content/  ~name"content"  ~<html>...</html>
    // Note: tree-sitter does not support lookahead; HTML/GTK sigils match
    // single-line content only (multi-line requires an external scanner).
    sigil: (_$) =>
      token(
        choice(
          seq("~<html>", /[^<]*/, "</html>"),
          seq("~<gtk>",  /[^<]*/, "</gtk>"),
          seq("~", /[a-z][A-Za-z0-9_]*/, "/",  /[^\/]*/,  "/",  /[a-zA-Z]*/),
          seq("~", /[a-z][A-Za-z0-9_]*/, '"',  /[^"]*/,   '"',  /[a-zA-Z]*/),
          seq("~", /[a-z][A-Za-z0-9_]*/, "(",  /[^)]*/,   ")",  /[a-zA-Z]*/),
          seq("~", /[a-z][A-Za-z0-9_]*/, "[",  /[^\]]*/,  "]",  /[a-zA-Z]*/),
          seq("~", /[a-z][A-Za-z0-9_]*/, "{",  /[^}]*/,   "}",  /[a-zA-Z]*/),
        ),
      ),

    list: ($) => seq("[", commaSep($._expression), "]"),

    tuple_or_group: ($) =>
      seq(
        "(",
        $._expression,
        optional(seq(",", commaSep1($._expression))),
        ")",
      ),

    record: ($) => seq("{", commaSep($.record_field), "}"),

    record_field: ($) =>
      seq(
        field("key", $.lower_identifier),
        optional(seq(":", field("value", $._expression))),
      ),

    // ─── Operators ───────────────────────────────────────────────────────────

    pipe_operator: (_$) => token(choice("|>", "<|")),

    // ─── Identifiers ─────────────────────────────────────────────────────────

    lower_identifier: (_$) => /[a-z_][A-Za-z0-9_]*/,
    upper_identifier: (_$) => /[A-Z][A-Za-z0-9_]*/,
  },
});

// ─── Helpers ─────────────────────────────────────────────────────────────────

function commaSep(rule) {
  return optional(commaSep1(rule));
}

function commaSep1(rule) {
  return seq(rule, repeat(seq(",", rule)));
}
