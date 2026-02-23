use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use serde_json::Value as JsonValue;

use super::util::{
    builtin, builtin_constructor, expect_list, expect_record, expect_text, list_value,
};
use crate::runtime::{EffectValue, Runtime, RuntimeError, Value};

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
    SqliteConfigure {
        wal: bool,
        busy_timeout_ms: i64,
        resp: DbResp<()>,
    },
    BeginTransaction {
        resp: DbResp<()>,
    },
    CommitTransaction {
        resp: DbResp<()>,
    },
    RollbackTransaction {
        resp: DbResp<()>,
    },
    Savepoint {
        name: String,
        resp: DbResp<()>,
    },
    ReleaseSavepoint {
        name: String,
        resp: DbResp<()>,
    },
    RollbackToSavepoint {
        name: String,
        resp: DbResp<()>,
    },
    RunMigrationSql {
        statements: Vec<String>,
        resp: DbResp<()>,
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

        fn sqlite_configure(&mut self, wal: bool, busy_timeout_ms: i64) -> Result<(), String> {
            match self {
                Backend::Sqlite(conn) => {
                    let journal_mode = if wal { "WAL" } else { "DELETE" };
                    conn.pragma_update(None, "journal_mode", journal_mode)
                        .map_err(|e| backend_err("sqlite.configure.journal_mode", e))?;
                    conn.busy_timeout(std::time::Duration::from_millis(
                        busy_timeout_ms.max(0) as u64,
                    ))
                    .map_err(|e| backend_err("sqlite.configure.busy_timeout", e))?;
                    Ok(())
                }
                _ => Ok(()),
            }
        }

        fn execute_statement(&mut self, statement: &str) -> Result<(), String> {
            match self {
                Backend::Sqlite(conn) => conn
                    .execute_batch(statement)
                    .map_err(|e| backend_err("sqlite.execute", e)),
                Backend::Postgresql(client) => client
                    .batch_execute(statement)
                    .map_err(|e| backend_err("postgres.execute", e)),
                Backend::Mysql(conn) => conn
                    .query_drop(statement)
                    .map_err(|e| backend_err("mysql.execute", e)),
            }
        }

        fn begin_transaction(&mut self) -> Result<(), String> {
            self.execute_statement("BEGIN TRANSACTION")
        }

        fn commit_transaction(&mut self) -> Result<(), String> {
            self.execute_statement("COMMIT")
        }

        fn rollback_transaction(&mut self) -> Result<(), String> {
            self.execute_statement("ROLLBACK")
        }

        fn savepoint(&mut self, name: &str) -> Result<(), String> {
            self.execute_statement(&format!("SAVEPOINT {name}"))
        }

        fn release_savepoint(&mut self, name: &str) -> Result<(), String> {
            self.execute_statement(&format!("RELEASE SAVEPOINT {name}"))
        }

        fn rollback_to_savepoint(&mut self, name: &str) -> Result<(), String> {
            self.execute_statement(&format!("ROLLBACK TO SAVEPOINT {name}"))
        }

        fn run_migration_sql(&mut self, statements: &[String]) -> Result<(), String> {
            for statement in statements {
                let trimmed = statement.trim();
                if trimmed.is_empty() {
                    continue;
                }
                self.execute_statement(trimmed)?;
            }
            Ok(())
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
            DbRequest::SqliteConfigure {
                wal,
                busy_timeout_ms,
                resp,
            } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.sqlite_configure(wal, busy_timeout_ms),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
            DbRequest::BeginTransaction { resp } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.begin_transaction(),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
            DbRequest::CommitTransaction { resp } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.commit_transaction(),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
            DbRequest::RollbackTransaction { resp } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.rollback_transaction(),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
            DbRequest::Savepoint { name, resp } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.savepoint(&name),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
            DbRequest::ReleaseSavepoint { name, resp } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.release_savepoint(&name),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
            DbRequest::RollbackToSavepoint { name, resp } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.rollback_to_savepoint(&name),
                    None => Err("database backend is not configured".to_string()),
                };
                let _ = resp.send(result);
            }
            DbRequest::RunMigrationSql { statements, resp } => {
                let result = match backend.as_mut() {
                    Some(backend) => backend.run_migration_sql(&statements),
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
                crate::runtime::format_value(other)
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
