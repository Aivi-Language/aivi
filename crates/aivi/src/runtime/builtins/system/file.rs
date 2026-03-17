use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use csv::ReaderBuilder;
use image::ImageReader;
use serde_json::Value as JsonValue;

use super::super::util::{
    builtin, expect_text, json_mismatch_to_decode_error, make_decode_error,
    make_source_decode_error, make_source_io_error,
};
use super::{
    json_to_runtime_with_schema, scalar_text_to_value, source_decode_error, source_transport_error,
};
use crate::runtime::json_schema::{constructor_name_for_enum_value, JsonSchema};
use crate::runtime::{EffectValue, RuntimeError, SourceValue, Value};

pub(in crate::runtime::builtins) fn build_file_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "read".to_string(),
        builtin("file.read", 1, |mut args, _runtime| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.read expects Text path".to_string(),
                    ));
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::read_to_string(&path) {
                    Ok(text) => Ok(Value::Text(text)),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue::new(
                "File".to_string(),
                Arc::new(effect),
            ))))
        }),
    );
    fields.insert(
        "json".to_string(),
        builtin("file.json", 1, |mut args, _| {
            let path = file_source_path(args.remove(0), "file.json")?;
            let schema_slot: Arc<Mutex<Option<crate::runtime::json_schema::JsonSchema>>> =
                Arc::new(Mutex::new(None));
            let raw_text_slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let schema_ref = schema_slot.clone();
            let raw_ref = raw_text_slot.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let raw = std::fs::read_to_string(&path).map_err(|err| {
                        RuntimeError::Error(make_source_io_error(format!(
                            "file.json path={path}: {err}"
                        )))
                    })?;
                    let parsed: JsonValue = serde_json::from_str(&raw).map_err(|err| {
                        RuntimeError::Error(make_source_decode_error(vec![make_decode_error(
                            Vec::new(),
                            format!(
                                "failed to parse JSON in {path} at line {}, column {}: {err}",
                                err.line(),
                                err.column()
                            ),
                        )]))
                    })?;

                    // Validate against schema if set
                    if let Ok(guard) = schema_ref.lock() {
                        if let Some(ref schema) = *guard {
                            let mut errors = Vec::new();
                            crate::runtime::json_schema::validate_json(
                                &parsed,
                                schema,
                                "$",
                                &mut errors,
                            );
                            if !errors.is_empty() {
                                let decode_errors =
                                    errors.iter().map(json_mismatch_to_decode_error).collect();
                                return Err(RuntimeError::Error(make_source_decode_error(
                                    decode_errors,
                                )));
                            }
                        }
                    }
                    // Store raw text for potential later error rendering
                    if let Ok(mut guard) = raw_ref.lock() {
                        *guard = Some(raw.clone());
                    }

                    // Use the schema (if set) to produce proper Option wrappers.
                    let schema_opt = schema_ref.lock().ok().and_then(|g| g.clone());
                    Ok(json_to_runtime_with_schema(&parsed, schema_opt.as_ref()))
                }),
            };
            let mut source = SourceValue::new("File".to_string(), Arc::new(effect));
            source.schema = schema_slot;
            source.raw_text = raw_text_slot;
            Ok(Value::Source(Arc::new(source)))
        }),
    );
    fields.insert(
        "csv".to_string(),
        builtin("file.csv", 1, |mut args, _| {
            let path = file_source_path(args.remove(0), "file.csv")?;
            let schema_slot: Arc<Mutex<Option<JsonSchema>>> = Arc::new(Mutex::new(None));
            let raw_text_slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let schema_ref = schema_slot.clone();
            let raw_ref = raw_text_slot.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let raw = std::fs::read_to_string(&path).map_err(|err| {
                        RuntimeError::Error(make_source_io_error(format!(
                            "file.csv path={path}: {err}"
                        )))
                    })?;
                    if let Ok(mut guard) = raw_ref.lock() {
                        *guard = Some(raw.clone());
                    }
                    let schema_opt = schema_ref.lock().ok().and_then(|g| g.clone());
                    let row_schema = match schema_opt.as_ref() {
                        Some(JsonSchema::List(inner)) => Some(inner.as_ref()),
                        Some(JsonSchema::Any) | None => None,
                        Some(other) => {
                            return Err(RuntimeError::Error(make_source_decode_error(vec![
                                make_decode_error(
                                    Vec::new(),
                                    format!(
                                        "file.csv expects `List {{ ... }}` or `List Any`, got `{other}`"
                                    ),
                                ),
                            ])));
                        }
                    };
                    let mut reader = ReaderBuilder::new()
                        .has_headers(true)
                        .from_reader(raw.as_bytes());
                    let headers = reader
                        .headers()
                        .map_err(|err| {
                            RuntimeError::Error(make_source_decode_error(vec![make_decode_error(
                                Vec::new(),
                                format!("failed to parse CSV headers in {path}: {err}"),
                            )]))
                        })?
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>();
                    if let Some(JsonSchema::Record(fields)) = row_schema {
                        for (field_name, field_schema) in fields {
                            if !headers.iter().any(|header| header == field_name)
                                && !matches!(field_schema, JsonSchema::Option(_))
                            {
                                return Err(RuntimeError::Error(make_source_decode_error(vec![
                                    make_decode_error(
                                        vec![field_name.clone()],
                                        format!("missing CSV column `{field_name}` in {path}"),
                                    ),
                                ])));
                            }
                        }
                    }
                    let mut rows = Vec::new();
                    for (idx, row) in reader.records().enumerate() {
                        let row = row.map_err(|err| {
                            RuntimeError::Error(make_source_decode_error(vec![make_decode_error(
                                vec![idx.to_string()],
                                format!("failed to parse CSV row {} in {path}: {err}", idx + 1),
                            )]))
                        })?;
                        rows.push(csv_row_to_runtime(&headers, &row, idx, &path, row_schema)?);
                    }
                    Ok(Value::List(Arc::new(rows)))
                }),
            };
            let mut source = SourceValue::new("File".to_string(), Arc::new(effect));
            source.schema = schema_slot;
            source.raw_text = raw_text_slot;
            Ok(Value::Source(Arc::new(source)))
        }),
    );
    fields.insert(
        "imageMeta".to_string(),
        builtin("file.imageMeta", 1, |mut args, _| {
            let path = expect_text(args.remove(0), "file.imageMeta")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let reader = ImageReader::open(&path).map_err(|err| {
                        RuntimeError::Error(Value::Text(source_transport_error(
                            "Image",
                            &format!("path={path}"),
                            &err.to_string(),
                        )))
                    })?;
                    let reader = reader.with_guessed_format().map_err(|err| {
                        RuntimeError::Error(Value::Text(source_decode_error(
                            "Image",
                            "$",
                            "known image format",
                            "unknown image format",
                            "",
                            1,
                            1,
                            &format!("failed to detect image format in {path}: {err}"),
                        )))
                    })?;
                    let format = reader
                        .format()
                        .map(|fmt| format!("{fmt:?}"))
                        .unwrap_or_else(|| "Unknown".to_string());
                    let (width, height) = reader.into_dimensions().map_err(|err| {
                        RuntimeError::Error(Value::Text(source_decode_error(
                            "Image",
                            "$",
                            "readable image metadata",
                            "unreadable image metadata",
                            "",
                            1,
                            1,
                            &format!("failed reading image dimensions in {path}: {err}"),
                        )))
                    })?;
                    let mut meta = HashMap::new();
                    meta.insert("width".to_string(), Value::Int(width as i64));
                    meta.insert("height".to_string(), Value::Int(height as i64));
                    meta.insert("format".to_string(), Value::Text(format));
                    Ok(Value::Record(Arc::new(meta)))
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue::new(
                "Image".to_string(),
                Arc::new(effect),
            ))))
        }),
    );
    fields.insert(
        "image".to_string(),
        builtin("file.image", 1, |mut args, _| {
            let path = expect_text(args.remove(0), "file.image")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let dyn_img = image::open(&path).map_err(|err| {
                        RuntimeError::Error(Value::Text(source_transport_error(
                            "Image",
                            &format!("path={path}"),
                            &err.to_string(),
                        )))
                    })?;
                    let width = dyn_img.width() as usize;
                    let height = dyn_img.height() as usize;
                    let pixels_count = width.saturating_mul(height);
                    if pixels_count > 16_000_000 {
                        return Err(RuntimeError::Error(Value::Text(source_decode_error(
                            "Image",
                            "$.pixels",
                            "at most 16,000,000 pixels",
                            &pixels_count.to_string(),
                            "",
                            1,
                            1,
                            "image is too large to decode safely",
                        ))));
                    }
                    let rgb = dyn_img.to_rgb8();
                    let mut rows = Vec::with_capacity(height);
                    for y in 0..height {
                        let mut row = Vec::with_capacity(width);
                        for x in 0..width {
                            let p = rgb.get_pixel(x as u32, y as u32);
                            row.push(Value::Tuple(vec![
                                Value::Int(p[0] as i64),
                                Value::Int(p[1] as i64),
                                Value::Int(p[2] as i64),
                            ]));
                        }
                        rows.push(Value::List(Arc::new(row)));
                    }
                    let mut image = HashMap::new();
                    image.insert("width".to_string(), Value::Int(width as i64));
                    image.insert("height".to_string(), Value::Int(height as i64));
                    image.insert("format".to_string(), Value::Text("Rgb8".to_string()));
                    image.insert("pixels".to_string(), Value::List(Arc::new(rows)));
                    Ok(Value::Record(Arc::new(image)))
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue::new(
                "Image".to_string(),
                Arc::new(effect),
            ))))
        }),
    );
    fields.insert(
        "open".to_string(),
        builtin("file.open", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.open expects Text path".to_string(),
                    ));
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::File::open(&path) {
                    Ok(file) => Ok(Value::FileHandle(Arc::new(Mutex::new(Some(file))))),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "close".to_string(),
        builtin("file.close", 1, |mut args, _| {
            let handle = match args.remove(0) {
                Value::FileHandle(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.close expects a file handle".to_string(),
                    ));
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut file = handle.lock().map_err(|_| RuntimeError::IOError {
                        context: "file.close".to_string(),
                        cause: "file handle poisoned".to_string(),
                    })?;
                    *file = None;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "readAll".to_string(),
        builtin("file.readAll", 1, |mut args, _| {
            let handle = match args.remove(0) {
                Value::FileHandle(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.readAll expects a file handle".to_string(),
                    ));
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut handle = handle.lock().map_err(|_| RuntimeError::IOError {
                        context: "file.readAll".to_string(),
                        cause: "file handle poisoned".to_string(),
                    })?;
                    let Some(file) = handle.as_mut() else {
                        return Err(RuntimeError::Error(Value::Text(
                            "file handle is closed".to_string(),
                        )));
                    };
                    let _ = std::io::Seek::seek(file, std::io::SeekFrom::Start(0));
                    let mut buffer = String::new();
                    std::io::Read::read_to_string(file, &mut buffer)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    Ok(Value::Text(buffer))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "write_text".to_string(),
        builtin("file.write_text", 2, |mut args, _| {
            let content = match args.remove(1) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.write_text expects Text content".to_string(),
                    ));
                }
            };
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.write_text expects Text path".to_string(),
                    ));
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::write(&path, content.as_bytes()) {
                    Ok(()) => Ok(Value::Unit),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "exists".to_string(),
        builtin("file.exists", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.exists expects Text path".to_string(),
                    ));
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Ok(Value::Bool(std::path::Path::new(&path).exists()))),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "stat".to_string(),
        builtin("file.stat", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.stat expects Text path".to_string(),
                    ));
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let metadata = std::fs::metadata(&path)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    let modified = metadata
                        .modified()
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    let modified_ms = system_time_to_millis(modified)?;
                    let created_ms = metadata
                        .created()
                        .ok()
                        .and_then(|created| system_time_to_millis(created).ok())
                        .unwrap_or(modified_ms);
                    let size = i64::try_from(metadata.len()).map_err(|_| {
                        RuntimeError::Error(Value::Text("file too large".to_string()))
                    })?;
                    let created = i64::try_from(created_ms).map_err(|_| {
                        RuntimeError::Error(Value::Text("timestamp overflow".to_string()))
                    })?;
                    let modified = i64::try_from(modified_ms).map_err(|_| {
                        RuntimeError::Error(Value::Text("timestamp overflow".to_string()))
                    })?;
                    let mut stats = HashMap::new();
                    stats.insert("size".to_string(), Value::Int(size));
                    stats.insert("created".to_string(), Value::Int(created));
                    stats.insert("modified".to_string(), Value::Int(modified));
                    stats.insert("isFile".to_string(), Value::Bool(metadata.is_file()));
                    stats.insert("isDirectory".to_string(), Value::Bool(metadata.is_dir()));
                    Ok(Value::Record(Arc::new(stats)))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "delete".to_string(),
        builtin("file.delete", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.delete expects Text path".to_string(),
                    ));
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::remove_file(&path) {
                    Ok(()) => Ok(Value::Unit),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn file_source_path(arg: Value, context: &str) -> Result<String, RuntimeError> {
    match arg {
        Value::Text(path) => Ok(path),
        Value::Record(record) => match record.get("path") {
            Some(Value::Text(path)) => Ok(path.clone()),
            Some(other) => Err(RuntimeError::TypeError {
                context: context.to_string(),
                expected: "Text".to_string(),
                got: super::super::util::value_type_name(other).to_string(),
            }),
            None => Err(RuntimeError::Message(format!("{context} expects config.path"))),
        },
        other => Err(RuntimeError::TypeError {
            context: context.to_string(),
            expected: "Text or Record".to_string(),
            got: super::super::util::value_type_name(&other).to_string(),
        }),
    }
}

fn system_time_to_millis(time: SystemTime) -> Result<u128, RuntimeError> {
    time.duration_since(UNIX_EPOCH)
        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))
        .map(|duration| duration.as_millis())
}

fn csv_row_to_runtime(
    headers: &[String],
    row: &csv::StringRecord,
    row_index: usize,
    path: &str,
    row_schema: Option<&JsonSchema>,
) -> Result<Value, RuntimeError> {
    let mut rec = HashMap::new();
    let record_schema = match row_schema {
        Some(JsonSchema::Record(fields)) => Some(fields),
        None => None,
        Some(other) => {
            return Err(RuntimeError::Error(make_source_decode_error(vec![
                make_decode_error(
                    Vec::new(),
                    format!("file.csv expects row schema Record or Any, got `{other}`"),
                ),
            ])));
        }
    };

    for (col_idx, value) in row.iter().enumerate() {
        let key = headers
            .get(col_idx)
            .cloned()
            .unwrap_or_else(|| format!("col{col_idx}"));
        let field_schema = record_schema.and_then(|fields| fields.get(key.as_str()));
        let runtime_value = csv_scalar_to_runtime(value, field_schema).map_err(|message| {
            RuntimeError::Error(make_source_decode_error(vec![make_decode_error(
                vec![row_index.to_string(), key.clone()],
                format!("{message} in {path}"),
            )]))
        })?;
        rec.insert(key, runtime_value);
    }

    if let Some(fields) = record_schema {
        for (key, field_schema) in fields {
            if rec.contains_key(key.as_str()) {
                continue;
            }
            if matches!(field_schema, JsonSchema::Option(_)) {
                rec.insert(
                    key.clone(),
                    Value::Constructor {
                        name: "None".to_string(),
                        args: Vec::new(),
                    },
                );
                continue;
            }
            return Err(RuntimeError::Error(make_source_decode_error(vec![
                make_decode_error(
                    vec![row_index.to_string(), key.clone()],
                    format!("missing CSV column `{key}` in {path}"),
                ),
            ])));
        }
    }

    Ok(Value::Record(Arc::new(rec)))
}

fn csv_scalar_to_runtime(raw: &str, schema: Option<&JsonSchema>) -> Result<Value, String> {
    let Some(schema) = schema else {
        return Ok(scalar_text_to_value(raw));
    };
    let json_value = csv_scalar_to_json(raw, schema)?;
    Ok(json_to_runtime_with_schema(&json_value, Some(schema)))
}

fn csv_scalar_to_json(raw: &str, schema: &JsonSchema) -> Result<JsonValue, String> {
    match schema {
        JsonSchema::Any => Ok(JsonValue::String(raw.to_string())),
        JsonSchema::Int => raw
            .parse::<i64>()
            .map(JsonValue::from)
            .map_err(|_| format!("expected Int, got {:?}", raw)),
        JsonSchema::Float => raw
            .parse::<f64>()
            .map(JsonValue::from)
            .map_err(|_| format!("expected Float, got {:?}", raw)),
        JsonSchema::Text | JsonSchema::DateTime => Ok(JsonValue::String(raw.to_string())),
        JsonSchema::Bool => raw
            .parse::<bool>()
            .map(JsonValue::from)
            .map_err(|_| format!("expected Bool, got {:?}", raw)),
        JsonSchema::Option(inner) => csv_scalar_to_json(raw, inner),
        JsonSchema::Enum(variants) => {
            if constructor_name_for_enum_value(variants, raw).is_none() {
                let expected = variants
                    .iter()
                    .map(|variant| format!("{:?}", variant.json_value))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!("expected one of {expected}, got {:?}", raw));
            }
            Ok(JsonValue::String(raw.to_string()))
        }
        JsonSchema::List(_) | JsonSchema::Tuple(_) | JsonSchema::Record(_) => Err(format!(
            "expected scalar CSV field, got `{schema}`"
        )),
    }
}
