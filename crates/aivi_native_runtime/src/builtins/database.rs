use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use serde_json::Value as JsonValue;

use super::util::{
    builtin, builtin_constructor, expect_list, expect_record, expect_text, list_value,
};
use crate::{EffectValue, Runtime, RuntimeError, Value};

const META_TABLE: &str = "aivi_tables";
const EMPTY_ROWS_JSON: &str = "{\"t\":\"List\",\"v\":[]}";

type DbResp<T> = mpsc::Sender<Result<T, String>>;
type LoadTableRow = (i64, String, String);

#[derive(Clone, Copy, Debug)]
enum Driver {
    Sqlite,
    Postgresql,
    Mysql,
}

enum DbRequest {
    Configure {
        driver: Driver,
        url: String,
        resp: DbResp<()>,
    },
    EnsureSchema {
        resp: DbResp<()>,
    },
    LoadTable {
        name: String,
        resp: DbResp<Option<LoadTableRow>>,
    },
    MigrateTable {
        name: String,
        columns_json: String,
        resp: DbResp<()>,
    },
    CompareAndSwapRows {
        name: String,
        expected_rev: i64,
        columns_json: String,
        rows_json: String,
        resp: DbResp<i64>,
    },
}

#[derive(Clone)]
struct DbHandle {
    tx: mpsc::Sender<DbRequest>,
}

impl DbHandle {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel::<DbRequest>();
        std::thread::spawn(move || db_worker(rx));
        Self { tx }
    }

    fn request<T>(
        &self,
        req: impl FnOnce(mpsc::Sender<Result<T, String>>) -> DbRequest,
    ) -> Result<T, String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.tx
            .send(req(resp_tx))
            .map_err(|_| "database backend worker stopped".to_string())?;
        resp_rx
            .recv()
            .map_err(|_| "database backend worker stopped".to_string())?
    }
}

struct DatabaseState {
    configured: AtomicBool,
    handle: DbHandle,
}

impl DatabaseState {
    fn new() -> Self {
        Self {
            configured: AtomicBool::new(false),
            handle: DbHandle::new(),
        }
    }

    fn is_configured(&self) -> bool {
        self.configured.load(Ordering::SeqCst)
    }
}

fn db_worker(rx: mpsc::Receiver<DbRequest>) {
    use mysql::prelude::*;
    use rusqlite::OptionalExtension;

    enum Backend {
        Sqlite(rusqlite::Connection),
        Postgresql(Box<postgres::Client>),
        Mysql(mysql::Conn),
    }

    fn backend_err(ctx: &str, err: impl std::fmt::Display) -> String {
        format!("{ctx}: {err}")
    }

    impl Backend {
        fn ensure_schema(&mut self) -> Result<(), String> {
            match self {
                Backend::Sqlite(conn) => {
                    conn.execute(
                        &format!(
                            "CREATE TABLE IF NOT EXISTS {META_TABLE} (\
                                name TEXT PRIMARY KEY,\
                                rev INTEGER NOT NULL DEFAULT 0,\
                                columns_json TEXT NOT NULL,\
                                rows_json TEXT NOT NULL\
                            )"
                        ),
                        [],
                    )
                    .map_err(|e| backend_err("sqlite.ensure_schema", e))?;
                    Ok(())
                }
                Backend::Postgresql(client) => {
                    client
                        .execute(
                            &format!(
                                "CREATE TABLE IF NOT EXISTS {META_TABLE} (\
                                    name TEXT PRIMARY KEY,\
                                    rev BIGINT NOT NULL DEFAULT 0,\
                                    columns_json TEXT NOT NULL,\
                                    rows_json TEXT NOT NULL\
                                )"
                            ),
                            &[],
                        )
                        .map_err(|e| backend_err("postgres.ensure_schema", e))?;
                    Ok(())
                }
                Backend::Mysql(conn) => {
                    conn.query_drop(format!(
                        "CREATE TABLE IF NOT EXISTS {META_TABLE} (\
                            name VARCHAR(255) PRIMARY KEY,\
                            rev BIGINT NOT NULL DEFAULT 0,\
                            columns_json LONGTEXT NOT NULL,\
                            rows_json LONGTEXT NOT NULL\
                        )"
                    ))
                    .map_err(|e| backend_err("mysql.ensure_schema", e))?;
                    Ok(())
                }
            }
        }

        fn load_table(&mut self, name: &str) -> Result<Option<(i64, String, String)>, String> {
            match self {
                Backend::Sqlite(conn) => {
                    let mut stmt = conn
                        .prepare(&format!(
                            "SELECT rev, columns_json, rows_json FROM {META_TABLE} WHERE name = ?1"
                        ))
                        .map_err(|e| backend_err("sqlite.load_table.prepare", e))?;
                    let row = stmt
                        .query_row([name], |row| {
                            let rev: i64 = row.get(0)?;
                            let columns_json: String = row.get(1)?;
                            let rows_json: String = row.get(2)?;
                            Ok((rev, columns_json, rows_json))
                        })
                        .optional()
                        .map_err(|e| backend_err("sqlite.load_table.query_row", e))?;
                    Ok(row)
                }
                Backend::Postgresql(client) => {
                    let row = client
                        .query_opt(
                            &format!(
                                "SELECT rev, columns_json, rows_json FROM {META_TABLE} WHERE name = $1"
                            ),
                            &[&name],
                        )
                        .map_err(|e| backend_err("postgres.load_table.query_opt", e))?;
                    Ok(row.map(|row| {
                        let rev: i64 = row.get::<usize, i64>(0);
                        let columns_json: String = row.get::<usize, String>(1);
                        let rows_json: String = row.get::<usize, String>(2);
                        (rev, columns_json, rows_json)
                    }))
                }
                Backend::Mysql(conn) => {
                    let row: Option<(i64, String, String)> = conn
                        .exec_first(
                            format!(
                                "SELECT rev, columns_json, rows_json FROM {META_TABLE} WHERE name = ?"
                            ),
                            (name,),
                        )
                        .map_err(|e| backend_err("mysql.load_table.exec_first", e))?;
                    Ok(row)
                }
            }
        }

        fn migrate_table(&mut self, name: &str, columns_json: &str) -> Result<(), String> {
            match self {
                Backend::Sqlite(conn) => {
                    conn.execute(
                        &format!(
                            "INSERT INTO {META_TABLE} (name, rev, columns_json, rows_json) VALUES (?1, 0, ?2, '{EMPTY_ROWS_JSON}') \
                             ON CONFLICT(name) DO UPDATE SET columns_json = excluded.columns_json"
                        ),
                        [name, columns_json],
                    )
                    .map_err(|e| backend_err("sqlite.migrate_table", e))?;
                    Ok(())
                }
                Backend::Postgresql(client) => {
                    client
                        .execute(
                            &format!(
                                "INSERT INTO {META_TABLE} (name, rev, columns_json, rows_json) VALUES ($1, 0, $2, '{EMPTY_ROWS_JSON}') \
                                 ON CONFLICT (name) DO UPDATE SET columns_json = EXCLUDED.columns_json"
                            ),
                            &[&name, &columns_json],
                        )
                        .map_err(|e| backend_err("postgres.migrate_table", e))?;
                    Ok(())
                }
                Backend::Mysql(conn) => {
                    conn.exec_drop(
                        format!(
                            "INSERT INTO {META_TABLE} (name, rev, columns_json, rows_json) VALUES (?, 0, ?, '{EMPTY_ROWS_JSON}') \
                             ON DUPLICATE KEY UPDATE columns_json = VALUES(columns_json)"
                        ),
                        (name, columns_json),
                    )
                    .map_err(|e| backend_err("mysql.migrate_table", e))?;
                    Ok(())
                }
            }
        }

        fn compare_and_swap_rows(
            &mut self,
            name: &str,
            expected_rev: i64,
            columns_json: &str,
            rows_json: &str,
        ) -> Result<i64, String> {
            match self {
                Backend::Sqlite(conn) => {
                    let changed = conn
                        .execute(
                            &format!(
                                "UPDATE {META_TABLE} SET columns_json = ?1, rows_json = ?2, rev = rev + 1 \
                                 WHERE name = ?3 AND rev = ?4"
                            ),
                            rusqlite::params![columns_json, rows_json, name, expected_rev],
                        )
                        .map_err(|e| backend_err("sqlite.cas_rows", e))?;
                    if changed == 0 {
                        return Err("concurrent write detected; retry".to_string());
                    }
                    let new_rev: i64 = conn
                        .query_row(
                            &format!("SELECT rev FROM {META_TABLE} WHERE name = ?1"),
                            [name],
                            |row| row.get(0),
                        )
                        .map_err(|e| backend_err("sqlite.cas_rows.read_rev", e))?;
                    Ok(new_rev)
                }
                Backend::Postgresql(client) => {
                    let changed = client
                        .execute(
                            &format!(
                                "UPDATE {META_TABLE} SET columns_json = $1, rows_json = $2, rev = rev + 1 \
                                 WHERE name = $3 AND rev = $4"
                            ),
                            &[&columns_json, &rows_json, &name, &expected_rev],
                        )
                        .map_err(|e| backend_err("postgres.cas_rows", e))?;
                    if changed == 0 {
                        return Err("concurrent write detected; retry".to_string());
                    }
                    let row = client
                        .query_one(
                            &format!("SELECT rev FROM {META_TABLE} WHERE name = $1"),
                            &[&name],
                        )
                        .map_err(|e| backend_err("postgres.cas_rows.read_rev", e))?;
                    Ok(row.get::<usize, i64>(0))
                }
                Backend::Mysql(conn) => {
                    conn.exec_drop(
                        format!(
                            "UPDATE {META_TABLE} SET columns_json = ?, rows_json = ?, rev = rev + 1 \
                             WHERE name = ? AND rev = ?"
                        ),
                        (columns_json, rows_json, name, expected_rev),
                    )
                    .map_err(|e| backend_err("mysql.cas_rows", e))?;
                    let changed = conn.affected_rows().try_into().unwrap_or(0usize);
                    if changed == 0 {
                        return Err("concurrent write detected; retry".to_string());
                    }
                    let row: Option<i64> = conn
                        .exec_first(
                            format!("SELECT rev FROM {META_TABLE} WHERE name = ?"),
                            (name,),
                        )
                        .map_err(|e| backend_err("mysql.cas_rows.read_rev", e))?;
                    row.ok_or_else(|| "missing table after update".to_string())
                }
            }
        }
    }

    let mut backend: Option<Backend> = None;

    for req in rx {
        match req {
            DbRequest::Configure { driver, url, resp } => {
                let result = (|| -> Result<(), String> {
                    backend = Some(match driver {
                        Driver::Sqlite => {
                            let conn = rusqlite::Connection::open(url)
                                .map_err(|e| backend_err("sqlite.open", e))?;
                            Backend::Sqlite(conn)
                        }
                        Driver::Postgresql => {
                            let client = postgres::Client::connect(&url, postgres::NoTls)
                                .map_err(|e| backend_err("postgres.connect", e))?;
                            Backend::Postgresql(Box::new(client))
                        }
                        Driver::Mysql => {
                            let opts = mysql::Opts::from_url(&url)
                                .map_err(|e| backend_err("mysql.parse_url", e))?;
                            let conn = mysql::Conn::new(opts)
                                .map_err(|e| backend_err("mysql.connect", e))?;
                            Backend::Mysql(conn)
                        }
                    });
                    if let Some(backend) = backend.as_mut() {
                        backend.ensure_schema()?;
                    }
                    Ok(())
                })();
                let _ = resp.send(result);
            }
            DbRequest::EnsureSchema { resp } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.ensure_schema(),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
            DbRequest::LoadTable { name, resp } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.load_table(&name),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
            DbRequest::MigrateTable {
                name,
                columns_json,
                resp,
            } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.migrate_table(&name, &columns_json),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
            DbRequest::CompareAndSwapRows {
                name,
                expected_rev,
                columns_json,
                rows_json,
                resp,
            } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.compare_and_swap_rows(
                        &name,
                        expected_rev,
                        &columns_json,
                        &rows_json,
                    ),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
        }
    }
}

fn table_parts(value: Value, ctx: &str) -> Result<(String, Value, Arc<Vec<Value>>), RuntimeError> {
    let fields = expect_record(value, ctx)?;
    let name = fields
        .get("name")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Table.name")))?;
    let name = expect_text(name.clone(), ctx)?;
    let columns = fields
        .get("columns")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Table.columns")))?;
    let rows = fields
        .get("rows")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Table.rows")))?;
    let rows = expect_list(rows.clone(), ctx)?;
    Ok((name, columns.clone(), rows))
}

fn make_table(name: String, columns: Value, rows: Vec<Value>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), Value::Text(name));
    fields.insert("columns".to_string(), columns);
    fields.insert("rows".to_string(), list_value(rows));
    Value::Record(Arc::new(fields))
}

fn encode_value(value: &Value) -> Result<JsonValue, RuntimeError> {
    Ok(match value {
        Value::Unit => serde_json::json!({ "t": "Unit" }),
        Value::Bool(v) => serde_json::json!({ "t": "Bool", "v": v }),
        Value::Int(v) => serde_json::json!({ "t": "Int", "v": v }),
        Value::Float(v) => serde_json::json!({ "t": "Float", "v": v }),
        Value::Text(v) => serde_json::json!({ "t": "Text", "v": v }),
        Value::DateTime(v) => serde_json::json!({ "t": "DateTime", "v": v }),
        Value::BigInt(v) => serde_json::json!({ "t": "BigInt", "v": v.to_string() }),
        Value::Rational(v) => serde_json::json!({ "t": "Rational", "v": v.to_string() }),
        Value::Decimal(v) => serde_json::json!({ "t": "Decimal", "v": v.to_string() }),
        Value::Bytes(bytes) => {
            let arr: Vec<JsonValue> = bytes.iter().copied().map(JsonValue::from).collect();
            serde_json::json!({ "t": "Bytes", "v": arr })
        }
        Value::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items.iter() {
                out.push(encode_value(item)?);
            }
            serde_json::json!({ "t": "List", "v": out })
        }
        Value::Tuple(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items.iter() {
                out.push(encode_value(item)?);
            }
            serde_json::json!({ "t": "Tuple", "v": out })
        }
        Value::Record(fields) => {
            let mut map = serde_json::Map::new();
            for (k, v) in fields.iter() {
                map.insert(k.clone(), encode_value(v)?);
            }
            serde_json::json!({ "t": "Record", "v": JsonValue::Object(map) })
        }
        Value::Constructor { name, args } => {
            let mut out = Vec::with_capacity(args.len());
            for arg in args.iter() {
                out.push(encode_value(arg)?);
            }
            serde_json::json!({ "t": "Constructor", "name": name, "args": out })
        }
        other => {
            return Err(RuntimeError::Message(format!(
                "database: cannot persist value {}",
                crate::format_value(other)
            )))
        }
    })
}

fn decode_value(value: &JsonValue) -> Result<Value, RuntimeError> {
    let obj = value.as_object().ok_or_else(|| {
        RuntimeError::Message("database: invalid persisted value (expected object)".to_string())
    })?;
    let tag = obj.get("t").and_then(|v| v.as_str()).ok_or_else(|| {
        RuntimeError::Message("database: missing persisted value tag".to_string())
    })?;
    match tag {
        "Unit" => Ok(Value::Unit),
        "Bool" => Ok(Value::Bool(
            obj.get("v")
                .and_then(|v| v.as_bool())
                .ok_or_else(|| RuntimeError::Message("database: invalid Bool".to_string()))?,
        )),
        "Int" => Ok(Value::Int(
            obj.get("v")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| RuntimeError::Message("database: invalid Int".to_string()))?,
        )),
        "Float" => Ok(Value::Float(
            obj.get("v")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| RuntimeError::Message("database: invalid Float".to_string()))?,
        )),
        "Text" => Ok(Value::Text(
            obj.get("v")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid Text".to_string()))?
                .to_string(),
        )),
        "DateTime" => Ok(Value::DateTime(
            obj.get("v")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid DateTime".to_string()))?
                .to_string(),
        )),
        "BigInt" => {
            let s = obj
                .get("v")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid BigInt".to_string()))?;
            let parsed = s
                .parse::<num_bigint::BigInt>()
                .map_err(|_| RuntimeError::Message("database: invalid BigInt".to_string()))?;
            Ok(Value::BigInt(Arc::new(parsed)))
        }
        "Rational" => {
            let s = obj
                .get("v")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid Rational".to_string()))?;
            let parsed = s
                .parse::<num_rational::BigRational>()
                .map_err(|_| RuntimeError::Message("database: invalid Rational".to_string()))?;
            Ok(Value::Rational(Arc::new(parsed)))
        }
        "Decimal" => {
            let s = obj
                .get("v")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid Decimal".to_string()))?;
            let parsed = s
                .parse::<rust_decimal::Decimal>()
                .map_err(|_| RuntimeError::Message("database: invalid Decimal".to_string()))?;
            Ok(Value::Decimal(parsed))
        }
        "Bytes" => {
            let arr = obj
                .get("v")
                .and_then(|v| v.as_array())
                .ok_or_else(|| RuntimeError::Message("database: invalid Bytes".to_string()))?;
            let mut out = Vec::with_capacity(arr.len());
            for item in arr.iter() {
                let b = item
                    .as_u64()
                    .and_then(|b| u8::try_from(b).ok())
                    .ok_or_else(|| RuntimeError::Message("database: invalid Bytes".to_string()))?;
                out.push(b);
            }
            Ok(Value::Bytes(Arc::new(out)))
        }
        "List" => {
            let arr = obj
                .get("v")
                .and_then(|v| v.as_array())
                .ok_or_else(|| RuntimeError::Message("database: invalid List".to_string()))?;
            let mut out = Vec::with_capacity(arr.len());
            for item in arr.iter() {
                out.push(decode_value(item)?);
            }
            Ok(list_value(out))
        }
        "Tuple" => {
            let arr = obj
                .get("v")
                .and_then(|v| v.as_array())
                .ok_or_else(|| RuntimeError::Message("database: invalid Tuple".to_string()))?;
            let mut out = Vec::with_capacity(arr.len());
            for item in arr.iter() {
                out.push(decode_value(item)?);
            }
            Ok(Value::Tuple(out))
        }
        "Record" => {
            let map = obj
                .get("v")
                .and_then(|v| v.as_object())
                .ok_or_else(|| RuntimeError::Message("database: invalid Record".to_string()))?;
            let mut out = HashMap::new();
            for (k, v) in map.iter() {
                out.insert(k.clone(), decode_value(v)?);
            }
            Ok(Value::Record(Arc::new(out)))
        }
        "Constructor" => {
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RuntimeError::Message("database: invalid Constructor".to_string()))?
                .to_string();
            let args_arr = obj.get("args").and_then(|v| v.as_array()).ok_or_else(|| {
                RuntimeError::Message("database: invalid Constructor".to_string())
            })?;
            let mut args = Vec::with_capacity(args_arr.len());
            for item in args_arr.iter() {
                args.push(decode_value(item)?);
            }
            Ok(Value::Constructor { name, args })
        }
        _ => Err(RuntimeError::Message(format!(
            "database: unknown persisted value tag {tag}"
        ))),
    }
}

fn encode_json(value: &Value) -> Result<String, RuntimeError> {
    let json = encode_value(value)?;
    serde_json::to_string(&json)
        .map_err(|e| RuntimeError::Message(format!("database: json encode error: {e}")))
}

fn decode_json(text: &str) -> Result<Value, RuntimeError> {
    let json: JsonValue = serde_json::from_str(text)
        .map_err(|e| RuntimeError::Message(format!("database: json decode error: {e}")))?;
    decode_value(&json)
}

fn apply_delta_rows(
    rows: &[Value],
    delta: Value,
    runtime: &mut Runtime,
) -> Result<Vec<Value>, RuntimeError> {
    let mut out = Vec::with_capacity(rows.len());
    match delta {
        Value::Constructor { name: tag, args } => match tag.as_str() {
            "Insert" => {
                if args.len() != 1 {
                    return Err(RuntimeError::Message(
                        "database.applyDelta expects Insert value".to_string(),
                    ));
                }
                out.extend(rows.iter().cloned());
                out.push(args[0].clone());
            }
            "Update" => {
                if args.len() != 2 {
                    return Err(RuntimeError::Message(
                        "database.applyDelta expects Update predicate and patch".to_string(),
                    ));
                }
                let pred = args[0].clone();
                let patch = args[1].clone();
                for row in rows.iter() {
                    let keep = runtime.apply(pred.clone(), row.clone())?;
                    let keep = match keep {
                        Value::Bool(value) => value,
                        other => {
                            return Err(RuntimeError::Message(format!(
                                "database.applyDelta Update predicate expects Bool, got {}",
                                crate::format_value(&other)
                            )))
                        }
                    };
                    if keep {
                        let updated = runtime.apply(patch.clone(), row.clone())?;
                        out.push(updated);
                    } else {
                        out.push(row.clone());
                    }
                }
            }
            "Delete" => {
                if args.len() != 1 {
                    return Err(RuntimeError::Message(
                        "database.applyDelta expects Delete predicate".to_string(),
                    ));
                }
                let pred = args[0].clone();
                for row in rows.iter() {
                    let matches = runtime.apply(pred.clone(), row.clone())?;
                    let matches = match matches {
                        Value::Bool(value) => value,
                        other => {
                            return Err(RuntimeError::Message(format!(
                                "database.applyDelta Delete predicate expects Bool, got {}",
                                crate::format_value(&other)
                            )))
                        }
                    };
                    if !matches {
                        out.push(row.clone());
                    }
                }
            }
            _ => {
                return Err(RuntimeError::Message(
                    "database.applyDelta expects Delta".to_string(),
                ))
            }
        },
        _ => {
            return Err(RuntimeError::Message(
                "database.applyDelta expects Delta".to_string(),
            ))
        }
    }
    Ok(out)
}

fn apply_delta(table: Value, delta: Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
    let (name, columns, rows) = table_parts(table, "database.applyDelta")?;
    let out = apply_delta_rows(rows.as_ref(), delta, runtime)?;
    Ok(make_table(name, columns, out))
}

fn parse_driver(value: Value) -> Result<Driver, RuntimeError> {
    match value {
        Value::Constructor { name, args } if args.is_empty() => match name.as_str() {
            "Sqlite" => Ok(Driver::Sqlite),
            "Postgresql" => Ok(Driver::Postgresql),
            "Mysql" => Ok(Driver::Mysql),
            _ => Err(RuntimeError::Message(format!(
                "database.configure expects Driver (Sqlite|Postgresql|Mysql), got {name}"
            ))),
        },
        other => Err(RuntimeError::Message(format!(
            "database.configure expects Driver, got {}",
            crate::format_value(&other)
        ))),
    }
}

pub(super) fn build_database_record() -> Value {
    let state = Arc::new(DatabaseState::new());

    let mut fields = HashMap::new();

    fields.insert(
        "table".to_string(),
        builtin("database.table", 2, |mut args, _| {
            let columns = args.pop().unwrap();
            let name = expect_text(args.pop().unwrap(), "database.table")?;
            Ok(make_table(name, columns, Vec::new()))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "configure".to_string(),
            builtin("database.configure", 1, move |mut args, _| {
                let config = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            let config_fields =
                                expect_record(config.clone(), "database.configure")?;
                            let driver = config_fields
                                .get("driver")
                                .ok_or_else(|| {
                                    RuntimeError::Message(
                                        "database.configure expects DbConfig.driver".to_string(),
                                    )
                                })?
                                .clone();
                            let url = config_fields
                                .get("url")
                                .ok_or_else(|| {
                                    RuntimeError::Message(
                                        "database.configure expects DbConfig.url".to_string(),
                                    )
                                })?
                                .clone();

                            let driver = parse_driver(driver)?;
                            let url = expect_text(url, "database.configure")?;

                            state
                                .handle
                                .request(|resp| DbRequest::Configure { driver, url, resp })
                                .map_err(RuntimeError::Message)?;
                            state.configured.store(true, Ordering::SeqCst);
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "load".to_string(),
            builtin("database.load", 1, move |mut args, _| {
                let table = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            if !state.is_configured() {
                                let (_, _, rows) = table_parts(table.clone(), "database.load")?;
                                return Ok(list_value(rows.iter().cloned().collect()));
                            }

                            let (name, _columns, _rows) =
                                table_parts(table.clone(), "database.load")?;
                            let entry = state
                                .handle
                                .request(|resp| DbRequest::LoadTable {
                                    name: name.clone(),
                                    resp,
                                })
                                .map_err(RuntimeError::Message)?;
                            let rows_json = match entry {
                                Some((_rev, _cols, rows_json)) => rows_json,
                                None => EMPTY_ROWS_JSON.to_string(),
                            };
                            let rows_value = decode_json(&rows_json)?;
                            let Value::List(rows) = rows_value else {
                                return Err(RuntimeError::Message(
                                    "database: invalid persisted rows (expected List)".to_string(),
                                ));
                            };
                            Ok(Value::List(rows))
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "applyDelta".to_string(),
            builtin("database.applyDelta", 2, move |mut args, _| {
                let delta = args.pop().unwrap();
                let table = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| {
                            if !state.is_configured() {
                                return apply_delta(table.clone(), delta.clone(), runtime);
                            }

                            let (name, columns, _rows) =
                                table_parts(table.clone(), "database.applyDelta")?;
                            let columns_json = encode_json(&columns)?;

                            for _attempt in 0..3 {
                                let entry = state
                                    .handle
                                    .request(|resp| DbRequest::LoadTable {
                                        name: name.clone(),
                                        resp,
                                    })
                                    .map_err(RuntimeError::Message)?;

                                if entry.is_none() {
                                    state
                                        .handle
                                        .request(|resp| DbRequest::MigrateTable {
                                            name: name.clone(),
                                            columns_json: columns_json.clone(),
                                            resp,
                                        })
                                        .map_err(RuntimeError::Message)?;
                                    continue;
                                }

                                let (rev, _stored_cols, rows_json) = entry.unwrap();
                                let rows_value = decode_json(&rows_json)?;
                                let Value::List(rows_list) = rows_value else {
                                    return Err(RuntimeError::Message(
                                        "database: invalid persisted rows (expected List)"
                                            .to_string(),
                                    ));
                                };
                                let current_rows: Vec<Value> = rows_list.iter().cloned().collect();

                                let new_rows =
                                    apply_delta_rows(&current_rows, delta.clone(), runtime)?;
                                let rows_json = encode_json(&list_value(new_rows.clone()))?;

                                let saved =
                                    state.handle.request(|resp| DbRequest::CompareAndSwapRows {
                                        name: name.clone(),
                                        expected_rev: rev,
                                        columns_json: columns_json.clone(),
                                        rows_json,
                                        resp,
                                    });
                                match saved {
                                    Ok(_new_rev) => {
                                        return Ok(make_table(
                                            name.clone(),
                                            columns.clone(),
                                            new_rows,
                                        ))
                                    }
                                    Err(err) => {
                                        if err.contains("retry") {
                                            continue;
                                        }
                                        return Err(RuntimeError::Message(err));
                                    }
                                }
                            }

                            Err(RuntimeError::Message(
                                "database.applyDelta failed due to concurrent writes; retry"
                                    .to_string(),
                            ))
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "runMigrations".to_string(),
            builtin("database.runMigrations", 1, move |mut args, _| {
                let tables = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            if !state.is_configured() {
                                return Ok(Value::Unit);
                            }
                            let tables = expect_list(tables.clone(), "database.runMigrations")?;

                            state
                                .handle
                                .request(|resp| DbRequest::EnsureSchema { resp })
                                .map_err(RuntimeError::Message)?;

                            for table in tables.iter() {
                                let (name, columns, _rows) =
                                    table_parts(table.clone(), "database.runMigrations")?;
                                let columns_json = encode_json(&columns)?;
                                state
                                    .handle
                                    .request(|resp| DbRequest::MigrateTable {
                                        name,
                                        columns_json,
                                        resp,
                                    })
                                    .map_err(RuntimeError::Message)?;
                            }
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert("ins".to_string(), builtin_constructor("Insert", 1));
    fields.insert("upd".to_string(), builtin_constructor("Update", 2));
    fields.insert("del".to_string(), builtin_constructor("Delete", 1));
    Value::Record(Arc::new(fields))
}
