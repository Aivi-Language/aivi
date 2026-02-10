use std::fs;
use std::path::{Path, PathBuf};

use aivi::syntax;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::args()
        .nth(1)
        .ok_or("usage: gen_vscode_syntax <out-dir>")?;

    let out_dir = PathBuf::from(out_dir);
    fs::create_dir_all(&out_dir)?;

    write_json_if_changed(
        &out_dir.join("aivi.tmLanguage.json"),
        serde_json::to_string_pretty(&aivi_tmlanguage())? + "\n",
    )?;
    write_json_if_changed(
        &out_dir.join("ebnf.tmLanguage.json"),
        serde_json::to_string_pretty(&ebnf_tmlanguage())? + "\n",
    )?;

    Ok(())
}

fn write_json_if_changed(path: &Path, contents: String) -> Result<(), Box<dyn std::error::Error>> {
    let existing = fs::read_to_string(path).ok();
    if existing.as_deref() == Some(contents.as_str()) {
        return Ok(());
    }
    fs::write(path, contents)?;
    Ok(())
}

fn keyword_regex(keywords: &[&str]) -> String {
    format!("\\\\b({})\\\\b", keywords.join("|"))
}

fn regex_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' | '^' | '$' | '.' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{'
            | '}' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn operator_regex() -> String {
    let mut ops: Vec<String> = Vec::new();

    for (_, sym) in syntax::SYMBOLS_3 {
        ops.push((*sym).to_string());
    }
    for (_, sym) in syntax::SYMBOLS_2 {
        ops.push((*sym).to_string());
    }

    const SINGLE: &[char] = &[
        '?', '+', '-', '*', '/', '%', '=', ':', '.', ',', ';', '<', '>', '!', '~', '^', '&', '|',
    ];
    for &ch in SINGLE {
        if syntax::SYMBOLS_1.contains(&ch) {
            ops.push(ch.to_string());
        }
    }

    ops.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    ops.dedup();

    let escaped = ops.into_iter().map(|op| regex_escape(&op)).collect::<Vec<_>>();
    format!("({})", escaped.join("|"))
}

fn aivi_tmlanguage() -> serde_json::Value {
    let keyword_control = keyword_regex(syntax::KEYWORDS_CONTROL);
    let keyword_other = keyword_regex(syntax::KEYWORDS_OTHER);
    let boolean = keyword_regex(syntax::BOOLEAN_LITERALS);
    let constructors_common = keyword_regex(syntax::CONSTRUCTORS_COMMON);
    let operators = operator_regex();

    json!({
      "$schema": "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
      "name": "AIVI",
      "scopeName": "source.aivi",
      "patterns": [
        { "include": "#comment" },
        { "include": "#sigil" },
        { "include": "#string" },
        { "include": "#number" },
        { "include": "#decorator" },
        { "include": "#keyword" },
        { "include": "#boolean" },
        { "include": "#constructor" },
        { "include": "#type" },
        { "include": "#operator" }
      ],
      "repository": {
        "sigil": {
          "patterns": [
            {
              "name": "string.quoted.other.sigil.aivi",
              "begin": "(~[a-z][A-Za-z0-9_]*)(/)",
              "beginCaptures": {
                "1": { "name": "entity.name.function.sigil.aivi" },
                "2": { "name": "punctuation.definition.string.begin.aivi" }
              },
              "end": "(?<!\\\\)/(?:[a-zA-Z]*)",
              "endCaptures": {
                "0": { "name": "punctuation.definition.string.end.aivi" }
              },
              "patterns": [
                { "name": "constant.character.escape.aivi", "match": "\\\\." }
              ]
            },
            {
              "name": "string.quoted.other.sigil.aivi",
              "begin": "(~[a-z][A-Za-z0-9_]*)(\\\")",
              "beginCaptures": {
                "1": { "name": "entity.name.function.sigil.aivi" },
                "2": { "name": "punctuation.definition.string.begin.aivi" }
              },
              "end": "(?<!\\\\)\\\"(?:[a-zA-Z]*)",
              "endCaptures": {
                "0": { "name": "punctuation.definition.string.end.aivi" }
              },
              "patterns": [
                { "name": "constant.character.escape.aivi", "match": "\\\\." }
              ]
            },
            {
              "name": "string.quoted.other.sigil.aivi",
              "begin": "(~[a-z][A-Za-z0-9_]*)(\\()",
              "beginCaptures": {
                "1": { "name": "entity.name.function.sigil.aivi" },
                "2": { "name": "punctuation.definition.string.begin.aivi" }
              },
              "end": "(?<!\\\\)\\)(?:[a-zA-Z]*)",
              "endCaptures": {
                "0": { "name": "punctuation.definition.string.end.aivi" }
              },
              "patterns": [
                { "name": "constant.character.escape.aivi", "match": "\\\\." }
              ]
            },
            {
              "name": "string.quoted.other.sigil.aivi",
              "begin": "(~[a-z][A-Za-z0-9_]*)(\\[)",
              "beginCaptures": {
                "1": { "name": "entity.name.function.sigil.aivi" },
                "2": { "name": "punctuation.definition.string.begin.aivi" }
              },
              "end": "(?<!\\\\)\\](?:[a-zA-Z]*)",
              "endCaptures": {
                "0": { "name": "punctuation.definition.string.end.aivi" }
              },
              "patterns": [
                { "name": "constant.character.escape.aivi", "match": "\\\\." }
              ]
            },
            {
              "name": "string.quoted.other.sigil.aivi",
              "begin": "(~[a-z][A-Za-z0-9_]*)(\\{)",
              "beginCaptures": {
                "1": { "name": "entity.name.function.sigil.aivi" },
                "2": { "name": "punctuation.definition.string.begin.aivi" }
              },
              "end": "(?<!\\\\)\\}(?:[a-zA-Z]*)",
              "endCaptures": {
                "0": { "name": "punctuation.definition.string.end.aivi" }
              },
              "patterns": [
                { "name": "constant.character.escape.aivi", "match": "\\\\." }
              ]
            }
          ]
        },
        "comment": {
          "patterns": [
            {
              "name": "comment.line.double-slash.aivi",
              "match": "//.*$"
            },
            {
              "name": "comment.block.aivi",
              "begin": "/\\\\*",
              "end": "\\\\*/"
            }
          ]
        },
        "string": {
          "patterns": [
            {
              "name": "string.quoted.double.aivi",
              "begin": "\"",
              "end": "\"",
              "patterns": [
                {
                  "name": "constant.character.escape.aivi",
                  "match": "\\\\([\\\\\\\\\\\"nrt]|u\\\\{[0-9a-fA-F]+\\\\})"
                }
              ]
            },
            {
              "name": "string.quoted.other.backtick.aivi",
              "begin": "`",
              "end": "`"
            },
            {
              "name": "string.quoted.single.aivi",
              "begin": "'",
              "end": "'",
              "patterns": [
                {
                  "name": "constant.character.escape.aivi",
                  "match": "\\\\."
                }
              ]
            }
          ]
        },
        "number": {
          "patterns": [
            {
              "name": "constant.numeric.aivi",
              "match": "\\\\b\\\\d+(?:\\\\.\\\\d+)?\\\\b"
            }
          ]
        },
        "decorator": {
          "patterns": [
            {
              "name": "meta.annotation.decorator.aivi",
              "match": "@[a-z][A-Za-z0-9_]*\\\\b"
            }
          ]
        },
        "boolean": {
          "patterns": [
            {
              "name": "constant.language.boolean.aivi",
              "match": boolean
            }
          ]
        },
        "constructor": {
          "patterns": [
            {
              "name": "support.constant.aivi",
              "match": constructors_common
            }
          ]
        },
        "keyword": {
          "patterns": [
            {
              "name": "keyword.control.aivi",
              "match": keyword_control
            },
            {
              "name": "keyword.other.aivi",
              "match": keyword_other
            }
          ]
        },
        "type": {
          "patterns": [
            {
              "name": "entity.name.type.aivi",
              "match": "\\\\b[A-Z][A-Za-z0-9_]*\\\\b"
            }
          ]
        },
        "operator": {
          "patterns": [
            {
              "name": "keyword.operator.aivi",
              "match": operators
            }
          ]
        }
      }
    })
}

fn ebnf_tmlanguage() -> serde_json::Value {
    json!({
      "name": "EBNF",
      "scopeName": "source.ebnf",
      "patterns": [
        {
          "name": "comment.line.double-slash.ebnf",
          "match": "//.*$"
        },
        {
          "name": "comment.block.ebnf",
          "begin": "/\\\\*",
          "end": "\\\\*/"
        },
        {
          "name": "string.quoted.single.ebnf",
          "begin": "'",
          "end": "'",
          "patterns": [
            {
              "name": "constant.character.escape.ebnf",
              "match": "\\\\."
            }
          ]
        },
        {
          "name": "string.quoted.double.ebnf",
          "begin": "\"",
          "end": "\"",
          "patterns": [
            {
              "name": "constant.character.escape.ebnf",
              "match": "\\\\."
            }
          ]
        },
        {
          "name": "keyword.operator.definition.ebnf",
          "match": "::="
        },
        {
          "name": "keyword.operator.choice.ebnf",
          "match": "\\\\|"
        },
        {
          "name": "keyword.operator.quantifier.ebnf",
          "match": "[?*+]"
        },
        {
          "name": "punctuation.section.group.begin.ebnf",
          "match": "[\\\\[\\\\(\\\\{]"
        },
        {
          "name": "punctuation.section.group.end.ebnf",
          "match": "[\\\\]\\\\)\\\\}]"
        },
        {
          "name": "constant.numeric.ebnf",
          "match": "\\\\b\\\\d+\\\\b"
        },
        {
          "name": "entity.name.nonterminal.ebnf",
          "match": "\\\\b[A-Z][A-Za-z0-9_]*\\\\b"
        }
      ]
    })
}

