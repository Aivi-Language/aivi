use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use csv::ReaderBuilder;
use image::ImageReader;
use serde_json::Value as JsonValue;

use super::super::util::{builtin, expect_text};
use super::{json_to_runtime, scalar_text_to_value, source_decode_error, source_transport_error};
use crate::runtime::{EffectValue, RuntimeError, SourceValue, Value};

pub(in crate::runtime::builtins) fn build_file_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "read".to_string(),
        builtin("file.read", 1, |mut args, _| {
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.read expects Text path".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::read_to_string(&path) {
                    Ok(text) => Ok(Value::Text(text)),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue {
                kind: "File".to_string(),
                effect: Arc::new(effect),
            })))
        }),
    );
    fields.insert(
        "json".to_string(),
        builtin("file.json", 1, |mut args, _| {
            let path = expect_text(args.remove(0), "file.json")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let raw = std::fs::read_to_string(&path).map_err(|err| {
                        RuntimeError::Error(Value::Text(source_transport_error(
                            "File",
                            &format!("path={path}"),
                            &err.to_string(),
                        )))
                    })?;
                    let parsed: JsonValue = serde_json::from_str(&raw).map_err(|err| {
                        RuntimeError::Error(Value::Text(source_decode_error(
                            "File",
                            "$",
                            "valid JSON",
                            "invalid JSON text",
                            &raw,
                            err.line(),
                            err.column(),
                            &format!("failed to parse file {path}"),
                        )))
                    })?;
                    Ok(json_to_runtime(&parsed))
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue {
                kind: "File".to_string(),
                effect: Arc::new(effect),
            })))
        }),
    );
    fields.insert(
        "csv".to_string(),
        builtin("file.csv", 1, |mut args, _| {
            let path = expect_text(args.remove(0), "file.csv")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let raw = std::fs::read_to_string(&path).map_err(|err| {
                        RuntimeError::Error(Value::Text(source_transport_error(
                            "File",
                            &format!("path={path}"),
                            &err.to_string(),
                        )))
                    })?;
                    let mut reader = ReaderBuilder::new()
                        .has_headers(true)
                        .from_reader(raw.as_bytes());
                    let headers = reader
                        .headers()
                        .map_err(|err| {
                            RuntimeError::Error(Value::Text(source_decode_error(
                                "File",
                                "$",
                                "valid CSV headers",
                                "invalid CSV header row",
                                &raw,
                                1,
                                1,
                                &format!("failed to parse CSV headers in {path}: {err}"),
                            )))
                        })?
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>();
                    let mut rows = Vec::new();
                    for (idx, row) in reader.records().enumerate() {
                        let row = row.map_err(|err| {
                            RuntimeError::Error(Value::Text(source_decode_error(
                                "File",
                                &format!("$[{}]", idx),
                                "valid CSV row",
                                "invalid CSV row",
                                &raw,
                                idx + 2,
                                1,
                                &format!("failed to parse CSV row {} in {path}: {err}", idx + 1),
                            )))
                        })?;
                        let mut rec = HashMap::new();
                        for (col_idx, value) in row.iter().enumerate() {
                            let key = headers
                                .get(col_idx)
                                .cloned()
                                .unwrap_or_else(|| format!("col{col_idx}"));
                            rec.insert(key, scalar_text_to_value(value));
                        }
                        rows.push(Value::Record(Arc::new(rec)));
                    }
                    Ok(Value::List(Arc::new(rows)))
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue {
                kind: "File".to_string(),
                effect: Arc::new(effect),
            })))
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
            Ok(Value::Source(Arc::new(SourceValue {
                kind: "Image".to_string(),
                effect: Arc::new(effect),
            })))
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
            Ok(Value::Source(Arc::new(SourceValue {
                kind: "Image".to_string(),
                effect: Arc::new(effect),
            })))
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
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| match std::fs::File::open(&path) {
                    Ok(file) => Ok(Value::FileHandle(Arc::new(Mutex::new(file)))),
                    Err(err) => Err(RuntimeError::Error(Value::Text(err.to_string()))),
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "close".to_string(),
        builtin("file.close", 1, |mut args, _| {
            let _handle = match args.remove(0) {
                Value::FileHandle(handle) => handle,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.close expects a file handle".to_string(),
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Ok(Value::Unit)),
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
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut file = handle
                        .lock()
                        .map_err(|_| RuntimeError::IOError { context: "file.readAll".to_string(), cause: "file handle poisoned".to_string() })?;
                    let _ = std::io::Seek::seek(&mut *file, std::io::SeekFrom::Start(0));
                    let mut buffer = String::new();
                    std::io::Read::read_to_string(&mut *file, &mut buffer)
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
                    ))
                }
            };
            let path = match args.remove(0) {
                Value::Text(text) => text,
                _ => {
                    return Err(RuntimeError::Message(
                        "file.write_text expects Text path".to_string(),
                    ))
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
                    ))
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
                    ))
                }
            };
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let metadata = std::fs::metadata(&path)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    let created = metadata
                        .created()
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    let modified = metadata
                        .modified()
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
                    let created_ms = created
                        .duration_since(UNIX_EPOCH)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?
                        .as_millis();
                    let modified_ms = modified
                        .duration_since(UNIX_EPOCH)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?
                        .as_millis();
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
                    ))
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
