#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs, io,
    path::{Path, PathBuf},
};

use aivi_backend::RuntimeValue;
use aivi_base::{Diagnostic, SourceDatabase, SourceFile, SourceSpan};
use aivi_hir::{
    DecodeProgramStep, DecodeProgramStepId, SourceDecodeProgram, SourceDecodeProgramOutcome,
    ValidationMode, generate_source_decode_programs, lower_module, validate_module,
};
use aivi_runtime::{
    ExternalSourceValue, SourceDecodeError, SourceDecodeProgramSupportError, decode_external,
    encode_runtime_json, parse_json_text, validate_supported_program,
};
use aivi_syntax::{Formatter, Item as SyntaxItem, ParsedModule, parse_module};
use aivi_typing::PrimitiveType;
use arbitrary::{Arbitrary, Unstructured};

pub const PARSER_TARGET: &str = "parser_lossless";
pub const DECODER_TARGET: &str = "decoder_paths";
const PAYLOAD_SENTINEL: &[u8] = b"\0AIVI_FUZZ_PAYLOAD\0";
const MAX_ARBITRARY_DEPTH: usize = 5;
const MAX_ARBITRARY_WIDTH: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
struct FixtureSeed {
    relative_path: PathBuf,
    bytes: Vec<u8>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BuildState {
    Visiting,
    Done,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BuildAction {
    Visit(DecodeProgramStepId),
    Assemble(DecodeProgramStepId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DecodeErrorFacts {
    record_fields: BTreeSet<String>,
    variant_names: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefreshSummary {
    pub parser_seed_count: usize,
    pub decoder_seed_count: usize,
}

pub fn parser_target(data: &[u8]) {
    let source_text = String::from_utf8_lossy(data).into_owned();
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file("parser_fuzz_input.aivi", source_text);
    let file = &sources[file_id];
    let parsed = parse_module(file);

    assert_parser_invariants(file, &parsed);

    if formatter_safe(&parsed) {
        let formatted = Formatter.format(&parsed.module);
        let mut reformatted_sources = SourceDatabase::new();
        let reformatted_id =
            reformatted_sources.add_file("formatted_fuzz_input.aivi", formatted.clone());
        let reparsed = parse_module(&reformatted_sources[reformatted_id]);
        assert_parser_invariants(&reformatted_sources[reformatted_id], &reparsed);
        assert!(
            formatter_safe(&reparsed),
            "formatter output must stay parseable without error items: {:?}",
            reparsed.all_diagnostics().collect::<Vec<_>>()
        );
        assert_eq!(
            Formatter.format(&reparsed.module),
            formatted,
            "formatter output must stay idempotent after reparsing"
        );
    }
}

pub fn decoder_target(data: &[u8]) {
    let (source_bytes, payload_bytes) = split_decoder_input(data);
    let source_text = String::from_utf8_lossy(source_bytes).into_owned();
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file("decoder_fuzz_input.aivi", source_text);
    let file = &sources[file_id];
    let parsed = parse_module(file);

    assert_parser_invariants(file, &parsed);

    let lowered = lower_module(&parsed.module);
    assert_diagnostics(
        file,
        lowered.diagnostics().iter(),
        "HIR lowering diagnostics",
    );

    let validation = validate_module(lowered.module(), ValidationMode::RequireResolvedNames);
    assert_diagnostics(
        file,
        validation.diagnostics().iter(),
        "HIR validation diagnostics",
    );

    let report = generate_source_decode_programs(lowered.module());
    for node in report.nodes() {
        assert_source_span(file, node.source_span, "source decode program node span");
        if let SourceDecodeProgramOutcome::Planned(program) = &node.outcome {
            exercise_decode_program(program, payload_bytes.unwrap_or(data));
        }
    }
}

pub fn refresh_corpus_from_fixtures() -> io::Result<RefreshSummary> {
    let parser_seeds = collect_parser_fixture_seeds()?;
    let decoder_seeds = collect_decoder_fixture_seeds()?;
    sync_corpus_target(PARSER_TARGET, &parser_seeds)?;
    sync_corpus_target(DECODER_TARGET, &decoder_seeds)?;
    Ok(RefreshSummary {
        parser_seed_count: parser_seeds.len(),
        decoder_seed_count: decoder_seeds.len(),
    })
}

fn assert_parser_invariants(file: &SourceFile, parsed: &ParsedModule) {
    assert_eq!(
        parsed.lexed.replay(file),
        file.text(),
        "lexer replay must stay lossless"
    );
    assert_eq!(
        parsed.module.token_count,
        parsed.lexed.tokens().len(),
        "parsed module token count must match lexed token buffer"
    );

    let mut cursor = 0;
    for token in parsed.lexed.tokens() {
        let span = token.span();
        let start = span.start().as_usize();
        let end = span.end().as_usize();
        assert_eq!(
            start, cursor,
            "token spans must replay the source contiguously"
        );
        assert!(end <= file.len(), "token span must stay within the source");
        assert!(
            file.text().is_char_boundary(start) && file.text().is_char_boundary(end),
            "token spans must stay on UTF-8 boundaries"
        );
        cursor = end;
    }
    assert_eq!(
        cursor,
        file.len(),
        "token buffer must cover the whole source"
    );

    let mut previous_token_end = 0;
    for item in parsed.module.items() {
        let token_range = item.token_range();
        assert!(
            token_range.start() <= token_range.end(),
            "item token ranges must stay ordered"
        );
        assert!(
            token_range.end() <= parsed.module.token_count,
            "item token ranges must stay within the token buffer"
        );
        assert!(
            previous_token_end <= token_range.start(),
            "top-level item token ranges must stay monotonic"
        );
        previous_token_end = token_range.end();
        assert_source_span(file, item.span(), "top-level parser item span");
    }

    assert_diagnostics(file, parsed.all_diagnostics(), "parser diagnostics");
}

fn formatter_safe(parsed: &ParsedModule) -> bool {
    !parsed.has_errors()
        && !parsed
            .module
            .items()
            .iter()
            .any(|item| matches!(item, SyntaxItem::Error(_)))
}

fn exercise_decode_program(program: &SourceDecodeProgram, payload_seed: &[u8]) {
    let support = validate_supported_program(program);

    let valid_value = build_valid_external_value(program)
        .expect("decode program DAG should admit a deterministic sample value");
    match &support {
        Ok(()) => {
            let decoded = decode_external(program, &valid_value)
                .expect("supported decode program should accept its deterministic sample");
            assert_runtime_roundtrip(program, &decoded);
        }
        Err(expected) => {
            let error = decode_external(program, &valid_value)
                .expect_err("unsupported decode program must fail before runtime decoding");
            assert_eq!(
                error,
                SourceDecodeError::UnsupportedProgram(expected.clone()),
                "unsupported decode programs must preserve their support error"
            );
        }
    }

    let invalid_value = build_invalid_root_sample(program);
    assert_decode_result(program, decode_external(program, &invalid_value), &support);

    let payload_text = String::from_utf8_lossy(payload_seed).into_owned();
    match parse_json_text(&payload_text) {
        Ok(value) => assert_decode_result(program, decode_external(program, &value), &support),
        Err(SourceDecodeError::InvalidJson { detail }) => {
            assert!(
                !detail.is_empty(),
                "invalid JSON errors should explain why parsing failed"
            );
        }
        Err(SourceDecodeError::UnsupportedNumber { value }) => {
            assert!(
                !value.is_empty(),
                "unsupported JSON numbers should preserve the original number text"
            );
        }
        Err(other) => panic!("JSON parsing should fail only before decode: {other:?}"),
    }

    if let Some(external) = arbitrary_external_value(payload_seed) {
        assert_decode_result(program, decode_external(program, &external), &support);
    }
}

fn assert_decode_result(
    program: &SourceDecodeProgram,
    result: Result<RuntimeValue, SourceDecodeError>,
    support: &Result<(), SourceDecodeProgramSupportError>,
) {
    match result {
        Ok(decoded) => {
            assert!(
                support.is_ok(),
                "unsupported decode programs must not decode successfully"
            );
            assert_runtime_roundtrip(program, &decoded);
        }
        Err(SourceDecodeError::UnsupportedProgram(error)) => {
            let expected = support
                .as_ref()
                .expect_err("supported decode programs must not return UnsupportedProgram");
            assert_eq!(
                &error, expected,
                "unsupported decode errors must match validate_supported_program"
            );
        }
        Err(error) => {
            assert!(
                support.is_ok(),
                "unsupported decode programs must fail with UnsupportedProgram, got {error:?}"
            );
            assert_decode_error_consistent(program, &error);
        }
    }
}

fn assert_runtime_roundtrip(program: &SourceDecodeProgram, decoded: &RuntimeValue) {
    let encoded = encode_runtime_json(decoded).expect(
        "decoded runtime values should remain JSON-encodable for the current decoder slice",
    );
    let reparsed = parse_json_text(&encoded)
        .expect("runtime JSON re-encoding should stay valid source-decoder JSON");
    let redecoded = decode_external(program, &reparsed)
        .expect("runtime JSON re-encoding should decode through the same program");
    assert_eq!(
        redecoded, *decoded,
        "runtime JSON roundtrips must preserve decoded runtime values"
    );
}

fn assert_decode_error_consistent(program: &SourceDecodeProgram, error: &SourceDecodeError) {
    let facts = collect_decode_error_facts(program);
    match error {
        SourceDecodeError::TypeMismatch { found, .. } => {
            assert!(
                matches!(
                    *found,
                    "unit" | "bool" | "integer" | "text" | "list" | "record" | "variant"
                ),
                "type-mismatch reports must use known runtime source kinds"
            );
        }
        SourceDecodeError::InvalidTupleLength { expected, found } => {
            assert_ne!(
                expected, found,
                "tuple length errors should report a real mismatch"
            );
        }
        SourceDecodeError::MissingField { field } => {
            assert!(
                facts.record_fields.contains(field.as_ref()),
                "missing-field errors must reference a schema field"
            );
        }
        SourceDecodeError::UnexpectedFields { fields } => {
            assert!(
                !fields.is_empty(),
                "unexpected-field errors should preserve at least one extra field"
            );
            let unique = fields.iter().collect::<BTreeSet<_>>();
            assert_eq!(
                unique.len(),
                fields.len(),
                "unexpected-field errors should not duplicate field names"
            );
        }
        SourceDecodeError::UnknownVariant { found, expected } => {
            assert!(
                !expected.is_empty(),
                "unknown-variant errors must preserve the expected variants"
            );
            for variant in expected.iter() {
                assert!(
                    facts.variant_names.contains(variant.as_ref()),
                    "unknown-variant errors must reference schema variants"
                );
            }
            assert!(
                !expected
                    .iter()
                    .any(|variant| variant.as_ref() == found.as_ref()),
                "unknown-variant errors should not report the found variant as expected"
            );
        }
        SourceDecodeError::MissingVariantPayload { variant }
        | SourceDecodeError::UnexpectedVariantPayload { variant } => {
            assert!(
                facts.variant_names.contains(variant.as_ref()),
                "variant-payload errors must reference a known variant"
            );
        }
        SourceDecodeError::InvalidJson { .. }
        | SourceDecodeError::UnsupportedNumber { .. }
        | SourceDecodeError::UnsupportedProgram(_) => {}
    }
}

fn collect_decode_error_facts(program: &SourceDecodeProgram) -> DecodeErrorFacts {
    let mut facts = DecodeErrorFacts::default();
    for step in program.steps() {
        match step {
            DecodeProgramStep::Record { fields, .. } => {
                facts
                    .record_fields
                    .extend(fields.iter().map(|field| field.name.as_str().to_owned()));
            }
            DecodeProgramStep::Sum { variants, .. } => {
                facts.variant_names.extend(
                    variants
                        .iter()
                        .map(|variant| variant.name.as_str().to_owned()),
                );
            }
            DecodeProgramStep::Option { .. } => {
                facts.variant_names.insert("None".to_owned());
                facts.variant_names.insert("Some".to_owned());
            }
            DecodeProgramStep::Result { .. } => {
                facts.variant_names.insert("Ok".to_owned());
                facts.variant_names.insert("Err".to_owned());
            }
            DecodeProgramStep::Validation { .. } => {
                facts.variant_names.insert("Valid".to_owned());
                facts.variant_names.insert("Invalid".to_owned());
            }
            DecodeProgramStep::Scalar { .. }
            | DecodeProgramStep::Tuple { .. }
            | DecodeProgramStep::Domain { .. }
            | DecodeProgramStep::List { .. } => {}
        }
    }
    facts
}

fn build_valid_external_value(program: &SourceDecodeProgram) -> Option<ExternalSourceValue> {
    let mut states = HashMap::new();
    let mut values = HashMap::new();
    let mut stack = vec![BuildAction::Visit(program.root())];

    while let Some(action) = stack.pop() {
        match action {
            BuildAction::Visit(id) => match states.get(&id).copied() {
                Some(BuildState::Done) => continue,
                Some(BuildState::Visiting) => return None,
                None => {
                    states.insert(id, BuildState::Visiting);
                    stack.push(BuildAction::Assemble(id));
                    let mut children = step_children(program.step(id));
                    children.reverse();
                    for child in children {
                        stack.push(BuildAction::Visit(child));
                    }
                }
            },
            BuildAction::Assemble(id) => {
                let value = assemble_valid_value(program, id, &values)?;
                values.insert(id, value);
                states.insert(id, BuildState::Done);
            }
        }
    }

    values.remove(&program.root())
}

fn assemble_valid_value(
    program: &SourceDecodeProgram,
    id: DecodeProgramStepId,
    values: &HashMap<DecodeProgramStepId, ExternalSourceValue>,
) -> Option<ExternalSourceValue> {
    let value = match program.step(id) {
        DecodeProgramStep::Scalar { scalar } => match scalar {
            PrimitiveType::Unit => ExternalSourceValue::Unit,
            PrimitiveType::Bool => ExternalSourceValue::Bool(false),
            PrimitiveType::Int => ExternalSourceValue::Int(0),
            PrimitiveType::Text => ExternalSourceValue::Text("seed".into()),
            PrimitiveType::Float
            | PrimitiveType::Decimal
            | PrimitiveType::BigInt
            | PrimitiveType::Bytes => ExternalSourceValue::Int(0),
        },
        DecodeProgramStep::Tuple { elements } => ExternalSourceValue::List(
            elements
                .iter()
                .map(|element| values.get(element).cloned())
                .collect::<Option<Vec<_>>>()?,
        ),
        DecodeProgramStep::Record { fields, .. } => {
            ExternalSourceValue::Record(BTreeMap::from_iter(
                fields
                    .iter()
                    .map(|field| {
                        Some((
                            field.name.as_str().into(),
                            values.get(&field.step).cloned()?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?,
            ))
        }
        DecodeProgramStep::Sum { variants, .. } => {
            let variant = variants.first()?;
            match variant.payload {
                None => ExternalSourceValue::variant(variant.name.as_str()),
                Some(payload) => ExternalSourceValue::variant_with_payload(
                    variant.name.as_str(),
                    values.get(&payload).cloned()?,
                ),
            }
        }
        DecodeProgramStep::Domain { carrier, .. } => values.get(carrier).cloned()?,
        DecodeProgramStep::List { element } => {
            ExternalSourceValue::List(vec![values.get(element).cloned()?])
        }
        DecodeProgramStep::Option { element } => {
            ExternalSourceValue::variant_with_payload("Some", values.get(element).cloned()?)
        }
        DecodeProgramStep::Result { value, .. } => {
            ExternalSourceValue::variant_with_payload("Ok", values.get(value).cloned()?)
        }
        DecodeProgramStep::Validation { value, .. } => {
            ExternalSourceValue::variant_with_payload("Valid", values.get(value).cloned()?)
        }
    };
    Some(value)
}

fn step_children(step: &DecodeProgramStep) -> Vec<DecodeProgramStepId> {
    match step {
        DecodeProgramStep::Scalar { .. } => Vec::new(),
        DecodeProgramStep::Tuple { elements } => elements.clone(),
        DecodeProgramStep::Record { fields, .. } => fields.iter().map(|field| field.step).collect(),
        DecodeProgramStep::Sum { variants, .. } => variants
            .iter()
            .filter_map(|variant| variant.payload)
            .collect(),
        DecodeProgramStep::Domain { carrier, .. } => vec![*carrier],
        DecodeProgramStep::List { element } | DecodeProgramStep::Option { element } => {
            vec![*element]
        }
        DecodeProgramStep::Result { error, value }
        | DecodeProgramStep::Validation { error, value } => vec![*error, *value],
    }
}

fn build_invalid_root_sample(program: &SourceDecodeProgram) -> ExternalSourceValue {
    match program.root_step() {
        DecodeProgramStep::Scalar { scalar } => match scalar {
            PrimitiveType::Unit => ExternalSourceValue::Bool(true),
            PrimitiveType::Bool => ExternalSourceValue::Int(0),
            PrimitiveType::Int => ExternalSourceValue::Text("oops".into()),
            PrimitiveType::Text => ExternalSourceValue::Int(0),
            PrimitiveType::Float
            | PrimitiveType::Decimal
            | PrimitiveType::BigInt
            | PrimitiveType::Bytes => ExternalSourceValue::Bool(true),
        },
        DecodeProgramStep::Tuple { elements } => {
            if elements.is_empty() {
                ExternalSourceValue::Text("oops".into())
            } else {
                ExternalSourceValue::List(Vec::new())
            }
        }
        DecodeProgramStep::Record { fields, .. } => {
            if fields.is_empty() {
                ExternalSourceValue::Text("oops".into())
            } else {
                ExternalSourceValue::Record(BTreeMap::new())
            }
        }
        DecodeProgramStep::Sum { variants, .. } => ExternalSourceValue::variant(
            unique_unknown_variant(variants.iter().map(|variant| variant.name.as_str())),
        ),
        DecodeProgramStep::Domain { .. } => ExternalSourceValue::Text("oops".into()),
        DecodeProgramStep::List { .. } => ExternalSourceValue::Text("oops".into()),
        DecodeProgramStep::Option { .. } => ExternalSourceValue::variant("Some"),
        DecodeProgramStep::Result { .. } => ExternalSourceValue::variant("Ok"),
        DecodeProgramStep::Validation { .. } => ExternalSourceValue::variant("Valid"),
    }
}

fn unique_unknown_variant<'a>(known: impl IntoIterator<Item = &'a str>) -> Box<str> {
    let known = known
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut candidate = "__aivi_unknown_variant__".to_owned();
    while known.contains(&candidate) {
        candidate.push('_');
    }
    candidate.into_boxed_str()
}

fn arbitrary_external_value(bytes: &[u8]) -> Option<ExternalSourceValue> {
    let mut input = Unstructured::new(bytes);
    arbitrary_external_value_at_depth(&mut input, 0).ok()
}

fn arbitrary_external_value_at_depth(
    input: &mut Unstructured<'_>,
    depth: usize,
) -> arbitrary::Result<ExternalSourceValue> {
    if depth >= MAX_ARBITRARY_DEPTH {
        return arbitrary_leaf_value(input);
    }

    match input.int_in_range(0..=6u8)? {
        0 => arbitrary_leaf_value(input),
        1 => {
            let len = input.int_in_range(0..=MAX_ARBITRARY_WIDTH)?;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(arbitrary_external_value_at_depth(input, depth + 1)?);
            }
            Ok(ExternalSourceValue::List(values))
        }
        2 => {
            let len = input.int_in_range(0..=MAX_ARBITRARY_WIDTH)?;
            let mut fields = BTreeMap::new();
            for _ in 0..len {
                fields.insert(
                    arbitrary_name(input)?,
                    arbitrary_external_value_at_depth(input, depth + 1)?,
                );
            }
            Ok(ExternalSourceValue::Record(fields))
        }
        3 => Ok(ExternalSourceValue::variant(arbitrary_name_string(input)?)),
        4 => Ok(ExternalSourceValue::variant_with_payload(
            arbitrary_name_string(input)?,
            arbitrary_external_value_at_depth(input, depth + 1)?,
        )),
        5 => Ok(ExternalSourceValue::variant("None")),
        6 => Ok(ExternalSourceValue::variant_with_payload(
            "Some",
            arbitrary_external_value_at_depth(input, depth + 1)?,
        )),
        _ => unreachable!("bounded arbitrary choice should stay within range"),
    }
}

fn arbitrary_leaf_value(input: &mut Unstructured<'_>) -> arbitrary::Result<ExternalSourceValue> {
    match input.int_in_range(0..=3u8)? {
        0 => Ok(ExternalSourceValue::Unit),
        1 => Ok(ExternalSourceValue::Bool(bool::arbitrary(input)?)),
        2 => Ok(ExternalSourceValue::Int(i64::arbitrary(input)?)),
        3 => Ok(ExternalSourceValue::Text(arbitrary_name(input)?)),
        _ => unreachable!("bounded arbitrary leaf choice should stay within range"),
    }
}

fn arbitrary_name(input: &mut Unstructured<'_>) -> arbitrary::Result<Box<str>> {
    Ok(arbitrary_name_string(input)?.into_boxed_str())
}

fn arbitrary_name_string(input: &mut Unstructured<'_>) -> arbitrary::Result<String> {
    let raw = Vec::<u8>::arbitrary(input)?;
    let mut text = String::from_utf8_lossy(&raw).into_owned();
    text.retain(|character| !character.is_control());
    if text.is_empty() {
        text.push('x');
    }
    Ok(text)
}

fn assert_source_span(file: &SourceFile, span: SourceSpan, context: &str) {
    assert_eq!(
        span.file(),
        file.id(),
        "{context} must stay attached to the source file being fuzzed"
    );
    let raw = span.span();
    let start = raw.start().as_usize();
    let end = raw.end().as_usize();
    assert!(start <= end, "{context} must stay ordered");
    assert!(
        end <= file.len(),
        "{context} must stay within the source text"
    );
    assert!(
        file.text().is_char_boundary(start) && file.text().is_char_boundary(end),
        "{context} must stay on UTF-8 character boundaries"
    );
}

fn assert_diagnostics<'a>(
    file: &SourceFile,
    diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
    context: &str,
) {
    for diagnostic in diagnostics {
        let primary_count = diagnostic
            .labels
            .iter()
            .filter(|label| matches!(label.style, aivi_base::LabelStyle::Primary))
            .count();
        assert!(
            primary_count <= 1,
            "{context} should emit at most one primary label per diagnostic"
        );
        for label in &diagnostic.labels {
            assert_source_span(file, label.span, context);
        }
    }
}

fn split_decoder_input(data: &[u8]) -> (&[u8], Option<&[u8]>) {
    let Some(position) = data
        .windows(PAYLOAD_SENTINEL.len())
        .position(|window| window == PAYLOAD_SENTINEL)
    else {
        return (data, None);
    };
    let payload_start = position + PAYLOAD_SENTINEL.len();
    (&data[..position], Some(&data[payload_start..]))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("fuzz package should live directly under the repository root")
        .to_path_buf()
}

fn fixture_root() -> PathBuf {
    workspace_root().join("fixtures/frontend")
}

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus")
}

fn corpus_target_dir(target: &str) -> PathBuf {
    corpus_root().join(target)
}

fn collect_parser_fixture_seeds() -> io::Result<Vec<FixtureSeed>> {
    collect_fixture_seeds(|_, _| true)
}

fn collect_decoder_fixture_seeds() -> io::Result<Vec<FixtureSeed>> {
    collect_fixture_seeds(|_, text| text.contains("@source"))
}

fn collect_fixture_seeds(
    mut include: impl FnMut(&Path, &str) -> bool,
) -> io::Result<Vec<FixtureSeed>> {
    let root = fixture_root();
    let mut stack = vec![root.clone()];
    let mut files = Vec::new();

    while let Some(path) = stack.pop() {
        let mut entries = fs::read_dir(&path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("aivi") {
                files.push(path);
            }
        }
    }

    files.sort();
    let mut seeds = Vec::new();
    for path in files {
        let bytes = fs::read(&path)?;
        let relative_path = path
            .strip_prefix(workspace_root())
            .expect("fixture path should stay under the workspace root")
            .to_path_buf();
        let text = String::from_utf8_lossy(&bytes);
        if include(&relative_path, &text) {
            seeds.push(FixtureSeed {
                relative_path,
                bytes,
            });
        }
    }
    Ok(seeds)
}

fn sync_corpus_target(target: &str, seeds: &[FixtureSeed]) -> io::Result<()> {
    let directory = corpus_target_dir(target);
    fs::create_dir_all(&directory)?;

    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }

    for seed in seeds {
        fs::write(
            directory.join(corpus_file_name(&seed.relative_path)),
            &seed.bytes,
        )?;
    }

    Ok(())
}

fn corpus_file_name(relative_path: &Path) -> String {
    relative_path
        .components()
        .map(|component| sanitize_component(&component.as_os_str().to_string_lossy()))
        .collect::<Vec<_>>()
        .join("__")
}

fn sanitize_component(component: &str) -> String {
    component
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => character,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
fn read_corpus_entries(target: &str) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let directory = corpus_target_dir(target);
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            entries.insert(
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path())?,
            );
        }
    }
    Ok(entries)
}

#[cfg(test)]
fn expected_corpus_entries(seeds: &[FixtureSeed]) -> BTreeMap<String, Vec<u8>> {
    seeds
        .iter()
        .map(|seed| (corpus_file_name(&seed.relative_path), seed.bytes.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_corpus_matches(target: &str, seeds: &[FixtureSeed]) {
        let actual = read_corpus_entries(target).expect("corpus directory should be readable");
        let expected = expected_corpus_entries(seeds);
        assert_eq!(
            actual, expected,
            "fuzz corpus `{target}` drifted from fixture seeds; run `cargo run --manifest-path fuzz/Cargo.toml --bin refresh_corpus`"
        );
    }

    fn replay_corpus(target: &str, harness: fn(&[u8])) {
        let entries = read_corpus_entries(target).expect("corpus directory should be readable");
        assert!(
            !entries.is_empty(),
            "fuzz corpus `{target}` should contain at least one seed"
        );
        for (name, bytes) in entries {
            harness(&bytes);
            assert!(
                !name.is_empty(),
                "corpus file names should stay stable and non-empty"
            );
        }
    }

    #[test]
    fn parser_corpus_matches_fixture_selection() {
        let seeds = collect_parser_fixture_seeds().expect("parser fixture seeds should load");
        assert_corpus_matches(PARSER_TARGET, &seeds);
    }

    #[test]
    fn decoder_corpus_matches_fixture_selection() {
        let seeds = collect_decoder_fixture_seeds().expect("decoder fixture seeds should load");
        assert_corpus_matches(DECODER_TARGET, &seeds);
    }

    #[test]
    fn parser_corpus_replays() {
        replay_corpus(PARSER_TARGET, parser_target);
    }

    #[test]
    fn decoder_corpus_replays() {
        replay_corpus(DECODER_TARGET, decoder_target);
    }
}
