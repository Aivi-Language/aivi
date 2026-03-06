use std::collections::BTreeMap;
use std::fmt;

use aivi_core::cg_type::CgType;

use serde::{Deserialize, Serialize};

/// Lightweight type schema for validating JSON values at source boundaries.
///
/// Derived from `CgType` at codegen time and attached to `SourceValue` so that
/// `json_to_runtime` can validate the parsed JSON against the expected AIVI type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum JsonSchema {
    Int,
    Float,
    Text,
    Bool,
    DateTime,
    List(Box<JsonSchema>),
    Tuple(Vec<JsonSchema>),
    Record(BTreeMap<String, JsonSchema>),
    Option(Box<JsonSchema>),
    /// All-nullary ADT: accepts a JSON string matching one of the constructor names.
    Enum(Vec<String>),
    /// No validation — accepts any JSON value.
    Any,
}

impl fmt::Display for JsonSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonSchema::Int => write!(f, "Int"),
            JsonSchema::Float => write!(f, "Float"),
            JsonSchema::Text => write!(f, "Text"),
            JsonSchema::Bool => write!(f, "Bool"),
            JsonSchema::DateTime => write!(f, "DateTime"),
            JsonSchema::List(elem) => write!(f, "List {elem}"),
            JsonSchema::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            JsonSchema::Record(fields) => {
                write!(f, "{{ ")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, " }}")
            }
            JsonSchema::Option(inner) => write!(f, "Option {inner}"),
            JsonSchema::Enum(variants) => {
                write!(f, "Enum(")?;
                for (i, v) in variants.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            JsonSchema::Any => write!(f, "Any"),
        }
    }
}

/// A single type mismatch found during JSON validation.
#[derive(Debug, Clone)]
pub(crate) struct JsonMismatch {
    /// JSON path, e.g. `$.user.age` or `$[0].name`
    pub(crate) path: String,
    /// Human-readable expected type
    pub(crate) expected: String,
    /// Human-readable description of what was found
    pub(crate) got: String,
    /// The raw JSON fragment that caused the mismatch
    pub(crate) fragment: String,
}

/// Validates a `serde_json::Value` against a `JsonSchema`, collecting all mismatches.
pub(crate) fn validate_json(
    value: &serde_json::Value,
    schema: &JsonSchema,
    path: &str,
    errors: &mut Vec<JsonMismatch>,
) {
    use serde_json::Value as JV;
    match (schema, value) {
        (JsonSchema::Any, _) => {}

        (JsonSchema::Int, JV::Number(n)) if n.is_i64() => {}
        (JsonSchema::Int, v) => errors.push(JsonMismatch {
            path: path.to_string(),
            expected: "Int".to_string(),
            got: json_type_name(v),
            fragment: fragment_str(v),
        }),

        (JsonSchema::Float, JV::Number(_)) => {}
        (JsonSchema::Float, v) => errors.push(JsonMismatch {
            path: path.to_string(),
            expected: "Float".to_string(),
            got: json_type_name(v),
            fragment: fragment_str(v),
        }),

        (JsonSchema::Text, JV::String(_)) => {}
        (JsonSchema::Text, v) => errors.push(JsonMismatch {
            path: path.to_string(),
            expected: "Text".to_string(),
            got: json_type_name(v),
            fragment: fragment_str(v),
        }),

        (JsonSchema::Bool, JV::Bool(_)) => {}
        (JsonSchema::Bool, v) => errors.push(JsonMismatch {
            path: path.to_string(),
            expected: "Bool".to_string(),
            got: json_type_name(v),
            fragment: fragment_str(v),
        }),

        (JsonSchema::DateTime, JV::String(_)) => {}
        (JsonSchema::DateTime, v) => errors.push(JsonMismatch {
            path: path.to_string(),
            expected: "DateTime".to_string(),
            got: json_type_name(v),
            fragment: fragment_str(v),
        }),

        (JsonSchema::Option(_), JV::Null) => {}
        (JsonSchema::Option(inner), v) => validate_json(v, inner, path, errors),

        (JsonSchema::Enum(variants), JV::String(s)) => {
            if !variants.contains(s) {
                errors.push(JsonMismatch {
                    path: path.to_string(),
                    expected: format!(
                        "one of {}",
                        variants
                            .iter()
                            .map(|v| format!("\"{v}\""))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    got: format!("Text (\"{s}\")"),
                    fragment: fragment_str(value),
                });
            }
        }
        (JsonSchema::Enum(variants), v) => errors.push(JsonMismatch {
            path: path.to_string(),
            expected: format!(
                "one of {}",
                variants
                    .iter()
                    .map(|v| format!("\"{v}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            got: json_type_name(v),
            fragment: fragment_str(v),
        }),

        (JsonSchema::List(elem), JV::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                validate_json(item, elem, &format!("{path}[{i}]"), errors);
            }
        }
        (JsonSchema::List(_), v) => errors.push(JsonMismatch {
            path: path.to_string(),
            expected: "List".to_string(),
            got: json_type_name(v),
            fragment: fragment_str(v),
        }),

        (JsonSchema::Tuple(schemas), JV::Array(items)) => {
            if items.len() != schemas.len() {
                errors.push(JsonMismatch {
                    path: path.to_string(),
                    expected: format!("Tuple of {} elements", schemas.len()),
                    got: format!("Array of {} elements", items.len()),
                    fragment: fragment_str(value),
                });
            } else {
                for (i, (item, schema)) in items.iter().zip(schemas.iter()).enumerate() {
                    validate_json(item, schema, &format!("{path}[{i}]"), errors);
                }
            }
        }
        (JsonSchema::Tuple(_), v) => errors.push(JsonMismatch {
            path: path.to_string(),
            expected: "Tuple (Array)".to_string(),
            got: json_type_name(v),
            fragment: fragment_str(v),
        }),

        (JsonSchema::Record(fields), JV::Object(map)) => {
            for (key, field_schema) in fields {
                match map.get(key) {
                    Some(v) => {
                        validate_json(v, field_schema, &format!("{path}.{key}"), errors);
                    }
                    None => {
                        if !matches!(field_schema, JsonSchema::Option(_)) {
                            errors.push(JsonMismatch {
                                path: format!("{path}.{key}"),
                                expected: format!("{field_schema}"),
                                got: "missing field".to_string(),
                                fragment: String::new(),
                            });
                        }
                    }
                }
            }
        }
        (JsonSchema::Record(_), v) => errors.push(JsonMismatch {
            path: path.to_string(),
            expected: "Record (Object)".to_string(),
            got: json_type_name(v),
            fragment: fragment_str(v),
        }),
    }
}

fn json_type_name(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "Null".to_string(),
        serde_json::Value::Bool(b) => format!("Bool ({b})"),
        serde_json::Value::Number(n) => {
            if n.is_i64() {
                format!("Int ({n})")
            } else {
                format!("Float ({n})")
            }
        }
        serde_json::Value::String(s) => {
            let display = if s.len() > 30 {
                format!("{}…", &s[..27])
            } else {
                s.clone()
            };
            format!("Text (\"{display}\")")
        }
        serde_json::Value::Array(items) => format!("Array ({} elements)", items.len()),
        serde_json::Value::Object(map) => format!("Object ({} fields)", map.len()),
    }
}

fn fragment_str(v: &serde_json::Value) -> String {
    let s = serde_json::to_string(v).unwrap_or_default();
    if s.len() > 40 {
        format!("{}…", &s[..37])
    } else {
        s
    }
}

/// Formats validation errors as a pretty ANSI-colored JSON snippet with inline annotations.
///
/// Produces output like:
/// ```text
/// failed to parse source [File]
///
///   {
///     "user": {
///       "id": 123,
///       "age": "twenty",
///              ^^^^^^^^ expected Int, got Text ("twenty")
///       "name": "Alice"
///     }
///   }
/// ```
pub(crate) fn format_json_validation_errors(
    raw_json: &str,
    kind: &str,
    errors: &[JsonMismatch],
) -> String {
    if errors.is_empty() {
        return String::new();
    }

    // Pretty-print the JSON for display
    let pretty = match serde_json::from_str::<serde_json::Value>(raw_json) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| raw_json.to_string()),
        Err(_) => raw_json.to_string(),
    };

    let lines: Vec<&str> = pretty.lines().collect();

    // Build a map: line_index → Vec<(col_start, col_end, message)>
    let mut annotations: BTreeMap<usize, Vec<(usize, usize, String)>> = BTreeMap::new();

    for err in errors {
        if err.got == "missing field" {
            continue; // handled separately below
        }
        // Find the fragment in the pretty-printed JSON
        if let Some((line_idx, col_start, col_end)) = find_fragment_location(&lines, &err.path, &err.fragment) {
            let msg = format!("expected {}, got {}", err.expected, err.got);
            annotations
                .entry(line_idx)
                .or_default()
                .push((col_start, col_end, msg));
        }
    }

    // Build header
    let mut out = format!(
        "\x1b[1;31mfailed to parse source\x1b[0m [{kind}]\n"
    );

    // Collect missing field errors
    let missing: Vec<&JsonMismatch> = errors
        .iter()
        .filter(|e| e.got == "missing field")
        .collect();
    for m in &missing {
        out.push_str(&format!(
            "\x1b[31m  missing field\x1b[0m at \x1b[36m{}\x1b[0m (expected {})\n",
            m.path, m.expected
        ));
    }

    if annotations.is_empty() && missing.is_empty() {
        // Fallback: just list errors textually
        for err in errors {
            out.push_str(&format!(
                "  at \x1b[36m{}\x1b[0m: expected \x1b[32m{}\x1b[0m, got \x1b[31m{}\x1b[0m\n",
                err.path, err.expected, err.got
            ));
        }
        return out;
    }

    out.push('\n');

    // Determine which lines to show (annotated lines ± 1 context line)
    let mut visible: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for &line_idx in annotations.keys() {
        let start = line_idx.saturating_sub(1);
        let end = (line_idx + 2).min(lines.len());
        for i in start..end {
            visible.insert(i);
        }
    }
    // If few enough lines (≤ 20), show all
    if lines.len() <= 20 {
        for i in 0..lines.len() {
            visible.insert(i);
        }
    }

    let gutter_width = format!("{}", lines.len()).len();
    let mut prev_idx: Option<usize> = None;

    for &idx in &visible {
        // Ellipsis for gaps
        if let Some(prev) = prev_idx {
            if idx > prev + 1 {
                out.push_str(&format!(
                    "\x1b[2m{:>gutter_width$} |\x1b[0m ...\n",
                    "",
                    gutter_width = gutter_width
                ));
            }
        }
        prev_idx = Some(idx);

        let line_num = idx + 1;
        let line_text = lines[idx];

        // Print the line
        out.push_str(&format!(
            "\x1b[2m{line_num:>gutter_width$} |\x1b[0m {line_text}\n",
            gutter_width = gutter_width
        ));

        // Print annotations for this line
        if let Some(anns) = annotations.get(&idx) {
            for (col_start, col_end, msg) in anns {
                let width = if col_end > col_start {
                    col_end - col_start
                } else {
                    1
                };
                let carets = "^".repeat(width);
                out.push_str(&format!(
                    "\x1b[2m{:>gutter_width$} |\x1b[0m {}\x1b[1;33m{carets}\x1b[0m \x1b[1;31m{msg}\x1b[0m\n",
                    "",
                    " ".repeat(*col_start),
                    gutter_width = gutter_width
                ));
            }
        }
    }

    out
}

/// Locates a JSON fragment in the pretty-printed lines by matching the JSON path structure.
///
/// Returns `(line_index, col_start, col_end)` of the value in the pretty output.
fn find_fragment_location(
    lines: &[&str],
    json_path: &str,
    fragment: &str,
) -> Option<(usize, usize, usize)> {
    // Extract the last key from the path (e.g., "$.user.age" → "age")
    let last_key = extract_last_key(json_path)?;

    // Search for `"key": <fragment>` pattern in the lines
    let key_pattern = format!("\"{last_key}\":");
    for (idx, line) in lines.iter().enumerate() {
        if let Some(key_pos) = line.find(&key_pattern) {
            let after_key = key_pos + key_pattern.len();
            let rest = &line[after_key..];
            let trimmed = rest.trim_start();
            let value_start = after_key + (rest.len() - trimmed.len());

            // Check if the fragment matches (strip trailing comma)
            let line_value = trimmed.trim_end_matches(',').trim();
            let frag_trimmed = fragment.trim();
            if line_value == frag_trimmed || trimmed.starts_with(frag_trimmed) {
                let value_end = value_start + trimmed.trim_end_matches(',').trim().len();
                return Some((idx, value_start, value_end));
            }

            // For complex values (objects/arrays), just point at the start
            if !fragment.is_empty() {
                let end = value_start + trimmed.trim_end_matches(',').len().min(20);
                return Some((idx, value_start, end));
            }
        }
    }

    // Fallback for array indices: "$.items[2]" → find the 3rd element
    if let Some(arr_idx) = extract_array_index(json_path) {
        // Count array elements in pretty JSON
        let mut current_idx = 0usize;
        let mut depth = 0i32;
        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') || trimmed.starts_with('{') {
                depth += 1;
            }
            if (trimmed.ends_with(']') || trimmed.ends_with("],") || trimmed.ends_with('}') || trimmed.ends_with("},")) && depth > 0 {
                depth -= 1;
            }
            if depth == 1 && !trimmed.is_empty() && !trimmed.starts_with('[') {
                if current_idx == arr_idx {
                    let col_start = line.len() - line.trim_start().len();
                    let col_end = line.trim_end_matches(',').len();
                    return Some((line_idx, col_start, col_end));
                }
                if trimmed.ends_with(',') || trimmed.ends_with("},") || trimmed.ends_with("],") {
                    current_idx += 1;
                }
            }
        }
    }

    None
}

fn extract_last_key(path: &str) -> Option<&str> {
    let path = path.strip_prefix('$').unwrap_or(path);
    // Handle paths like ".user.age" → "age"
    if let Some(last_dot) = path.rfind('.') {
        let key = &path[last_dot + 1..];
        // Strip array index if present (e.g., "items[0]" → "items")
        Some(key.split('[').next().unwrap_or(key))
    } else {
        None
    }
}

fn extract_array_index(path: &str) -> Option<usize> {
    let open = path.rfind('[')?;
    let close = path.rfind(']')?;
    if close > open {
        path[open + 1..close].parse().ok()
    } else {
        None
    }
}

/// Convert a `CgType` to a `JsonSchema` for JSON validation at source boundaries.
///
/// Function types and non-JSON-representable types map to `Any` (no validation).
pub(crate) fn cg_type_to_json_schema(ty: &CgType) -> JsonSchema {
    match ty {
        CgType::Dynamic => JsonSchema::Any,
        CgType::Int => JsonSchema::Int,
        CgType::Float => JsonSchema::Float,
        CgType::Bool => JsonSchema::Bool,
        CgType::Text => JsonSchema::Text,
        CgType::Unit => JsonSchema::Any,
        CgType::DateTime => JsonSchema::DateTime,
        CgType::Func(_, _) => JsonSchema::Any,
        CgType::ListOf(elem) => JsonSchema::List(Box::new(cg_type_to_json_schema(elem))),
        CgType::Tuple(items) => {
            JsonSchema::Tuple(items.iter().map(cg_type_to_json_schema).collect())
        }
        CgType::Record(fields) => JsonSchema::Record(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), cg_type_to_json_schema(v)))
                .collect(),
        ),
        CgType::Adt { name, constructors } => {
            // Option A → Option(schema_of_A)
            if name == "Option" && constructors.len() == 2 {
                let some_ctor = constructors.iter().find(|(n, _)| n == "Some");
                if let Some((_, args)) = some_ctor {
                    if args.len() == 1 {
                        return JsonSchema::Option(Box::new(cg_type_to_json_schema(&args[0])));
                    }
                }
            }
            // Domain / newtype: single constructor wrapping exactly one value → unwrap
            if constructors.len() == 1 {
                let (_, args) = &constructors[0];
                if args.len() == 1 {
                    return cg_type_to_json_schema(&args[0]);
                }
            }
            // All-nullary constructors → Enum (string matching constructor names)
            if !constructors.is_empty()
                && constructors.iter().all(|(_, args)| args.is_empty())
            {
                return JsonSchema::Enum(
                    constructors.iter().map(|(n, _)| n.clone()).collect(),
                );
            }
            JsonSchema::Any
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_simple_record() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"id": 123, "age": "twenty", "name": "Alice"}"#,
        )
        .unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("id".to_string(), JsonSchema::Int);
        fields.insert("age".to_string(), JsonSchema::Int);
        fields.insert("name".to_string(), JsonSchema::Text);
        let schema = JsonSchema::Record(fields);

        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "$.age");
        assert_eq!(errors[0].expected, "Int");
        assert!(errors[0].got.contains("Text"));
    }

    #[test]
    fn validate_nested_record() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"user": {"id": 123, "age": "twenty", "name": "Alice"}}"#,
        )
        .unwrap();

        let mut inner = BTreeMap::new();
        inner.insert("id".to_string(), JsonSchema::Int);
        inner.insert("age".to_string(), JsonSchema::Int);
        inner.insert("name".to_string(), JsonSchema::Text);

        let mut outer = BTreeMap::new();
        outer.insert("user".to_string(), JsonSchema::Record(inner));
        let schema = JsonSchema::Record(outer);

        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "$.user.age");
    }

    #[test]
    fn validate_missing_field() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"id": 123}"#).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("id".to_string(), JsonSchema::Int);
        fields.insert("name".to_string(), JsonSchema::Text);
        let schema = JsonSchema::Record(fields);

        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].got, "missing field");
    }

    #[test]
    fn validate_optional_allows_null() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"age": null}"#).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert(
            "age".to_string(),
            JsonSchema::Option(Box::new(JsonSchema::Int)),
        );
        let schema = JsonSchema::Record(fields);

        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_list_elements() {
        let json: serde_json::Value =
            serde_json::from_str(r#"[1, "two", 3]"#).unwrap();
        let schema = JsonSchema::List(Box::new(JsonSchema::Int));

        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "$[1]");
    }

    #[test]
    fn format_error_output() {
        let raw = r#"{"user": {"id": 123, "age": "twenty", "name": "Alice"}}"#;
        let errors = vec![JsonMismatch {
            path: "$.user.age".to_string(),
            expected: "Int".to_string(),
            got: "Text (\"twenty\")".to_string(),
            fragment: "\"twenty\"".to_string(),
        }];

        let output = format_json_validation_errors(raw, "File", &errors);
        assert!(output.contains("failed to parse source"));
        assert!(output.contains("expected Int"));
        assert!(output.contains("twenty"));
    }

    #[test]
    fn domain_type_unwraps_to_inner_schema() {
        // domain Seconds = Int → CgType::Adt { name: "Seconds", constructors: [("Seconds", [Int])] }
        let cg = CgType::Adt {
            name: "Seconds".to_string(),
            constructors: vec![("Seconds".to_string(), vec![CgType::Int])],
        };
        assert_eq!(cg_type_to_json_schema(&cg), JsonSchema::Int);
    }

    #[test]
    fn domain_type_wrapping_text() {
        let cg = CgType::Adt {
            name: "Email".to_string(),
            constructors: vec![("Email".to_string(), vec![CgType::Text])],
        };
        assert_eq!(cg_type_to_json_schema(&cg), JsonSchema::Text);
    }

    #[test]
    fn domain_type_in_record() {
        // { duration: Seconds } where Seconds wraps Int
        let seconds_cg = CgType::Adt {
            name: "Seconds".to_string(),
            constructors: vec![("Seconds".to_string(), vec![CgType::Int])],
        };
        let mut fields = BTreeMap::new();
        fields.insert("duration".to_string(), seconds_cg);
        let schema = cg_type_to_json_schema(&CgType::Record(fields));

        let mut expected_fields = BTreeMap::new();
        expected_fields.insert("duration".to_string(), JsonSchema::Int);
        assert_eq!(schema, JsonSchema::Record(expected_fields));

        // Validate: {"duration": 42} should pass
        let json: serde_json::Value = serde_json::from_str(r#"{"duration": 42}"#).unwrap();
        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);
        assert!(errors.is_empty());

        // Validate: {"duration": "slow"} should fail
        let bad_json: serde_json::Value =
            serde_json::from_str(r#"{"duration": "slow"}"#).unwrap();
        let mut errors = Vec::new();
        validate_json(&bad_json, &schema, "$", &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "$.duration");
    }

    #[test]
    fn enum_adt_produces_enum_schema() {
        // Status = Active | Inactive | Pending
        let cg = CgType::Adt {
            name: "Status".to_string(),
            constructors: vec![
                ("Active".to_string(), vec![]),
                ("Inactive".to_string(), vec![]),
                ("Pending".to_string(), vec![]),
            ],
        };
        let schema = cg_type_to_json_schema(&cg);
        assert_eq!(
            schema,
            JsonSchema::Enum(vec![
                "Active".to_string(),
                "Inactive".to_string(),
                "Pending".to_string(),
            ])
        );
    }

    #[test]
    fn validate_enum_accepts_valid_constructor() {
        let schema = JsonSchema::Enum(vec![
            "Active".to_string(),
            "Inactive".to_string(),
        ]);

        let json: serde_json::Value = serde_json::from_str(r#""Active""#).unwrap();
        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_enum_rejects_unknown_variant() {
        let schema = JsonSchema::Enum(vec![
            "Active".to_string(),
            "Inactive".to_string(),
        ]);

        let json: serde_json::Value = serde_json::from_str(r#""Deleted""#).unwrap();
        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].expected.contains("Active"));
        assert!(errors[0].expected.contains("Inactive"));
    }

    #[test]
    fn validate_enum_rejects_non_string() {
        let schema = JsonSchema::Enum(vec![
            "Active".to_string(),
            "Inactive".to_string(),
        ]);

        let json: serde_json::Value = serde_json::from_str("42").unwrap();
        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].got.contains("Int"));
    }

    #[test]
    fn enum_in_record_field() {
        // { name: Text, status: Status } where Status = Active | Inactive
        let status_cg = CgType::Adt {
            name: "Status".to_string(),
            constructors: vec![
                ("Active".to_string(), vec![]),
                ("Inactive".to_string(), vec![]),
            ],
        };
        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), CgType::Text);
        fields.insert("status".to_string(), status_cg);
        let schema = cg_type_to_json_schema(&CgType::Record(fields));

        // Valid JSON
        let json: serde_json::Value =
            serde_json::from_str(r#"{"name": "Alice", "status": "Active"}"#).unwrap();
        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);
        assert!(errors.is_empty());

        // Invalid enum value
        let bad: serde_json::Value =
            serde_json::from_str(r#"{"name": "Alice", "status": "Deleted"}"#).unwrap();
        let mut errors = Vec::new();
        validate_json(&bad, &schema, "$", &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "$.status");
    }

    #[test]
    fn nested_record_with_subtype() {
        // { user: { name: Text, address: { city: Text, zip: Int } } }
        let mut address_fields = BTreeMap::new();
        address_fields.insert("city".to_string(), CgType::Text);
        address_fields.insert("zip".to_string(), CgType::Int);

        let mut user_fields = BTreeMap::new();
        user_fields.insert("name".to_string(), CgType::Text);
        user_fields.insert("address".to_string(), CgType::Record(address_fields));

        let mut root_fields = BTreeMap::new();
        root_fields.insert("user".to_string(), CgType::Record(user_fields));

        let schema = cg_type_to_json_schema(&CgType::Record(root_fields));

        let json: serde_json::Value = serde_json::from_str(
            r#"{"user": {"name": "Alice", "address": {"city": "Berlin", "zip": 10115}}}"#,
        )
        .unwrap();
        let mut errors = Vec::new();
        validate_json(&json, &schema, "$", &mut errors);
        assert!(errors.is_empty());

        // Wrong zip type
        let bad: serde_json::Value = serde_json::from_str(
            r#"{"user": {"name": "Alice", "address": {"city": "Berlin", "zip": "abc"}}}"#,
        )
        .unwrap();
        let mut errors = Vec::new();
        validate_json(&bad, &schema, "$", &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "$.user.address.zip");
    }

    #[test]
    fn mixed_adt_stays_any() {
        // Shape = Circle Float | Point  (not all nullary, not single-constructor domain)
        let cg = CgType::Adt {
            name: "Shape".to_string(),
            constructors: vec![
                ("Circle".to_string(), vec![CgType::Float]),
                ("Point".to_string(), vec![]),
            ],
        };
        assert_eq!(cg_type_to_json_schema(&cg), JsonSchema::Any);
    }
}
